//! MemoryRepository trait definition.
//!
//! Provides CRUD operations for bot memories (extracted facts, preferences)
//! and pending extraction jobs. Follows the same RPITIT pattern as BotRepository.

use boternity_types::error::RepositoryError;
use boternity_types::memory::{MemoryEntry, PendingExtraction};
use uuid::Uuid;

/// Repository trait for bot long-term memory persistence.
///
/// Implementations live in boternity-infra (e.g., `SqliteMemoryRepository`).
/// Uses native async fn in traits (RPITIT, Rust 2024 edition).
pub trait MemoryRepository: Send + Sync {
    /// Save a new memory entry.
    fn save_memory(
        &self,
        entry: &MemoryEntry,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Get memories for a bot, ordered by importance DESC, created_at DESC.
    fn get_memories(
        &self,
        bot_id: &Uuid,
        limit: Option<i64>,
    ) -> impl std::future::Future<Output = Result<Vec<MemoryEntry>, RepositoryError>> + Send;

    /// Delete a single memory entry by ID.
    fn delete_memory(
        &self,
        memory_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Delete all memories for a bot. Returns the count of deleted entries.
    fn delete_all_memories(
        &self,
        bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<u64, RepositoryError>> + Send;

    /// Get all memories extracted from a specific session.
    fn get_memories_by_session(
        &self,
        session_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<MemoryEntry>, RepositoryError>> + Send;

    /// Save a new pending extraction job.
    fn save_pending_extraction(
        &self,
        pending: &PendingExtraction,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Get all pending extraction jobs for a bot.
    fn get_pending_extractions(
        &self,
        bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<PendingExtraction>, RepositoryError>> + Send;

    /// Delete a pending extraction job (after successful extraction).
    fn delete_pending_extraction(
        &self,
        id: &Uuid,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Update a pending extraction job (e.g., increment attempt count).
    fn update_pending_extraction(
        &self,
        pending: &PendingExtraction,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;
}
