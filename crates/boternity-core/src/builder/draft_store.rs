//! Builder draft persistence trait.
//!
//! Defines `BuilderDraftStore` for saving and restoring builder session
//! progress. When a user interrupts a builder session (closes terminal,
//! loses connection), the draft is auto-saved and can be resumed later.
//!
//! Uses dedicated SQLite tables (NOT bot-scoped KvStore) because builder
//! drafts exist before any bot is created.

use std::future::Future;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use boternity_types::error::RepositoryError;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// A saved builder draft containing the full serialized state.
///
/// `state_json` holds the serialized `BuilderState`. The `schema_version`
/// field enables forward-compatible deserialization: if the `BuilderState`
/// shape changes in a future release, migration logic can inspect the
/// version and transform the JSON before deserializing.
#[derive(Debug, Clone)]
pub struct BuilderDraft {
    /// Session ID (matches `BuilderState.session_id`).
    pub session_id: Uuid,
    /// Serialized `BuilderState` as JSON.
    pub state_json: String,
    /// Schema version for forward-compatible deserialization.
    pub schema_version: u32,
    /// When the draft was first created.
    pub created_at: DateTime<Utc>,
    /// When the draft was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Lightweight summary of a builder draft for listing.
///
/// Avoids deserializing the full `state_json` when only metadata is needed
/// (e.g., "Resume session: 'A coding bot' -- Personality phase, 2 min ago").
#[derive(Debug, Clone)]
pub struct BuilderDraftSummary {
    /// Session ID.
    pub session_id: Uuid,
    /// The initial description the user provided when starting the session.
    pub initial_description: String,
    /// Current phase name (e.g., "personality", "model").
    pub phase: String,
    /// When the draft was last updated.
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Persistence interface for builder drafts.
///
/// Uses RPITIT (return position `impl Trait` in traits) consistent with
/// all async traits in this project.
pub trait BuilderDraftStore: Send + Sync {
    /// Save or update a builder draft (upsert on session_id).
    fn save_draft(
        &self,
        draft: BuilderDraft,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Load a builder draft by session ID.
    ///
    /// Returns `None` if no draft exists for the given session.
    fn load_draft(
        &self,
        session_id: &Uuid,
    ) -> impl Future<Output = Result<Option<BuilderDraft>, RepositoryError>> + Send;

    /// List all saved drafts as lightweight summaries.
    ///
    /// Ordered by `updated_at` descending (most recently touched first).
    fn list_drafts(
        &self,
    ) -> impl Future<Output = Result<Vec<BuilderDraftSummary>, RepositoryError>> + Send;

    /// Delete a builder draft by session ID.
    ///
    /// No-op if the draft does not exist.
    fn delete_draft(
        &self,
        session_id: &Uuid,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}
