//! Vector memory store trait.
//!
//! Defines the interface for semantic vector search over bot memories.
//! Implementations (e.g., LanceDB, sqlite-vec) live in boternity-infra.

use boternity_types::error::RepositoryError;
use boternity_types::memory::{RankedMemory, VectorMemoryEntry};
use uuid::Uuid;

/// Trait for vector-indexed memory storage with semantic search.
///
/// Uses RPITIT (native async fn in traits, Rust 2024 edition).
/// Implementations live in boternity-infra.
pub trait VectorMemoryStore: Send + Sync {
    /// Search for memories semantically similar to the query embedding.
    ///
    /// Returns results ranked by relevance, filtered by `min_similarity`.
    fn search(
        &self,
        bot_id: &Uuid,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> impl std::future::Future<Output = Result<Vec<RankedMemory>, RepositoryError>> + Send;

    /// Add a memory entry with its embedding vector.
    fn add(
        &self,
        entry: &VectorMemoryEntry,
        embedding: &[f32],
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Delete a specific memory by ID.
    fn delete(
        &self,
        bot_id: &Uuid,
        memory_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Delete all memories for a bot. Returns the count of deleted entries.
    fn delete_all(
        &self,
        bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<u64, RepositoryError>> + Send;

    /// Count total memories for a bot.
    fn count(
        &self,
        bot_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<u64, RepositoryError>> + Send;

    /// Check if a near-duplicate memory exists (by embedding similarity threshold).
    fn check_duplicate(
        &self,
        bot_id: &Uuid,
        embedding: &[f32],
        threshold: f32,
    ) -> impl std::future::Future<Output = Result<Option<VectorMemoryEntry>, RepositoryError>> + Send;

    /// Get all entries whose `embedding_model` differs from `current_model`.
    ///
    /// Used to find memories that need re-embedding after a model upgrade.
    fn get_all_for_reembedding(
        &self,
        bot_id: &Uuid,
        current_model: &str,
    ) -> impl std::future::Future<Output = Result<Vec<VectorMemoryEntry>, RepositoryError>> + Send;

    /// Update a memory's embedding vector and model name.
    fn update_embedding(
        &self,
        memory_id: &Uuid,
        new_embedding: &[f32],
        model_name: &str,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;
}
