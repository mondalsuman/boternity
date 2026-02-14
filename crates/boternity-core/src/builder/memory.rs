//! Builder memory store trait.
//!
//! Defines `BuilderMemoryStore` for recording choices made during past
//! builder sessions. This powers Forge's suggestion engine: "Last time
//! you made a coding bot, you chose formal tone -- same here?"
//!
//! Builder memory entries are recorded when a builder session completes
//! (bot is assembled) and recalled by purpose category or recency.

use std::future::Future;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use boternity_types::builder::PurposeCategory;
use boternity_types::error::RepositoryError;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// A record of choices made during a completed builder session.
///
/// Captures the high-level decisions (tone, model, skills) so that Forge
/// can suggest similar choices when the user starts a new builder session
/// with a similar purpose category.
#[derive(Debug, Clone)]
pub struct BuilderMemoryEntry {
    /// Unique identifier (UUID v7).
    pub id: Uuid,
    /// Purpose category of the bot that was built (serialized as string
    /// for flexible querying -- avoids needing to deserialize the enum
    /// for SQL WHERE clauses).
    pub purpose_category: String,
    /// The initial description the user provided.
    pub initial_description: String,
    /// The tone that was chosen (e.g., "formal", "casual").
    pub chosen_tone: Option<String>,
    /// The model that was chosen (e.g., "claude-sonnet-4-20250514").
    pub chosen_model: Option<String>,
    /// The skills that were attached.
    pub chosen_skills: Vec<String>,
    /// The slug of the bot that was created (if assembly succeeded).
    pub bot_slug: Option<String>,
    /// When the session completed.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Persistence interface for builder memory (past session recall).
///
/// Uses RPITIT (return position `impl Trait` in traits) consistent with
/// all async traits in this project.
pub trait BuilderMemoryStore: Send + Sync {
    /// Record a completed builder session's choices.
    fn record_session(
        &self,
        memory: BuilderMemoryEntry,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Recall past sessions that used the given purpose category.
    ///
    /// Returns up to `limit` entries ordered by `created_at` descending
    /// (most recent first).
    fn recall_by_category(
        &self,
        category: &PurposeCategory,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<BuilderMemoryEntry>, RepositoryError>> + Send;

    /// Recall the most recent builder sessions regardless of category.
    ///
    /// Returns up to `limit` entries ordered by `created_at` descending.
    fn recall_recent(
        &self,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<BuilderMemoryEntry>, RepositoryError>> + Send;
}
