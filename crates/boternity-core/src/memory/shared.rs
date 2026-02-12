//! Shared memory store trait.
//!
//! Defines the interface for the cross-bot shared memory pool.
//! Bots can share memories with each other subject to trust levels.

use boternity_types::error::RepositoryError;
use boternity_types::memory::{RankedMemory, SharedMemoryEntry, TrustLevel};
use uuid::Uuid;

/// Trait for cross-bot shared memory storage with trust-level filtering.
///
/// Uses RPITIT (native async fn in traits, Rust 2024 edition).
/// Implementations live in boternity-infra.
pub trait SharedMemoryStore: Send + Sync {
    /// Add a shared memory entry with its embedding vector.
    fn add(
        &self,
        entry: &SharedMemoryEntry,
        embedding: &[f32],
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Search shared memories visible to the reading bot.
    ///
    /// Filters by trust level: includes Public memories and Trusted memories
    /// from bots in `trusted_bot_ids`.
    fn search(
        &self,
        reading_bot_id: &Uuid,
        trusted_bot_ids: &[Uuid],
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> impl std::future::Future<Output = Result<Vec<RankedMemory>, RepositoryError>> + Send;

    /// Delete a shared memory. Only the author bot can delete.
    fn delete(
        &self,
        memory_id: &Uuid,
        author_bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Update the trust level of a shared memory (e.g., Private -> Public).
    fn share(
        &self,
        memory_id: &Uuid,
        trust_level: TrustLevel,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Revoke sharing: sets the memory back to Private. Only the author can revoke.
    fn revoke(
        &self,
        memory_id: &Uuid,
        author_bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Count shared memories authored by a specific bot (for per-bot cap enforcement).
    fn count_by_author(
        &self,
        author_bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<u64, RepositoryError>> + Send;

    /// Verify the SHA-256 integrity hash of a shared memory entry.
    fn verify_integrity(
        &self,
        memory_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<bool, RepositoryError>> + Send;
}
