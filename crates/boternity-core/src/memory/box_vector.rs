//! BoxVectorMemoryStore -- object-safe dynamic dispatch wrapper for VectorMemoryStore.
//!
//! Follows the same blanket-impl pattern as BoxLlmProvider (02-01):
//! 1. Define an object-safe `VectorMemoryStoreDyn` trait with boxed futures
//! 2. Blanket-impl `VectorMemoryStoreDyn` for all `T: VectorMemoryStore`
//! 3. `BoxVectorMemoryStore` wraps `Box<dyn VectorMemoryStoreDyn>` and delegates

use std::future::Future;
use std::pin::Pin;

use boternity_types::error::RepositoryError;
use boternity_types::memory::{RankedMemory, VectorMemoryEntry};
use uuid::Uuid;

use super::vector::VectorMemoryStore;

/// Object-safe version of [`VectorMemoryStore`] with boxed futures.
///
/// This trait exists solely to enable dynamic dispatch (`dyn VectorMemoryStoreDyn`).
/// A blanket implementation is provided for all types implementing `VectorMemoryStore`.
pub trait VectorMemoryStoreDyn: Send + Sync {
    fn search_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
        query_embedding: &'a [f32],
        limit: usize,
        min_similarity: f32,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<RankedMemory>, RepositoryError>> + Send + 'a>>;

    fn add_boxed<'a>(
        &'a self,
        entry: &'a VectorMemoryEntry,
        embedding: &'a [f32],
    ) -> Pin<Box<dyn Future<Output = Result<(), RepositoryError>> + Send + 'a>>;

    fn delete_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
        memory_id: &'a Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<(), RepositoryError>> + Send + 'a>>;

    fn delete_all_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<u64, RepositoryError>> + Send + 'a>>;

    fn count_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<u64, RepositoryError>> + Send + 'a>>;

    fn check_duplicate_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
        embedding: &'a [f32],
        threshold: f32,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<VectorMemoryEntry>, RepositoryError>> + Send + 'a,
        >,
    >;

    fn get_all_for_reembedding_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
        current_model: &'a str,
    ) -> Pin<
        Box<dyn Future<Output = Result<Vec<VectorMemoryEntry>, RepositoryError>> + Send + 'a>,
    >;

    fn update_embedding_boxed<'a>(
        &'a self,
        memory_id: &'a Uuid,
        new_embedding: &'a [f32],
        model_name: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), RepositoryError>> + Send + 'a>>;
}

/// Blanket implementation: any `VectorMemoryStore` automatically implements `VectorMemoryStoreDyn`.
impl<T: VectorMemoryStore> VectorMemoryStoreDyn for T {
    fn search_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
        query_embedding: &'a [f32],
        limit: usize,
        min_similarity: f32,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<RankedMemory>, RepositoryError>> + Send + 'a>>
    {
        Box::pin(self.search(bot_id, query_embedding, limit, min_similarity))
    }

    fn add_boxed<'a>(
        &'a self,
        entry: &'a VectorMemoryEntry,
        embedding: &'a [f32],
    ) -> Pin<Box<dyn Future<Output = Result<(), RepositoryError>> + Send + 'a>> {
        Box::pin(self.add(entry, embedding))
    }

    fn delete_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
        memory_id: &'a Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<(), RepositoryError>> + Send + 'a>> {
        Box::pin(self.delete(bot_id, memory_id))
    }

    fn delete_all_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<u64, RepositoryError>> + Send + 'a>> {
        Box::pin(self.delete_all(bot_id))
    }

    fn count_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<u64, RepositoryError>> + Send + 'a>> {
        Box::pin(self.count(bot_id))
    }

    fn check_duplicate_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
        embedding: &'a [f32],
        threshold: f32,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Option<VectorMemoryEntry>, RepositoryError>> + Send + 'a,
        >,
    > {
        Box::pin(self.check_duplicate(bot_id, embedding, threshold))
    }

    fn get_all_for_reembedding_boxed<'a>(
        &'a self,
        bot_id: &'a Uuid,
        current_model: &'a str,
    ) -> Pin<
        Box<dyn Future<Output = Result<Vec<VectorMemoryEntry>, RepositoryError>> + Send + 'a>,
    > {
        Box::pin(self.get_all_for_reembedding(bot_id, current_model))
    }

    fn update_embedding_boxed<'a>(
        &'a self,
        memory_id: &'a Uuid,
        new_embedding: &'a [f32],
        model_name: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), RepositoryError>> + Send + 'a>> {
        Box::pin(self.update_embedding(memory_id, new_embedding, model_name))
    }
}

/// Type-erased vector memory store for runtime selection.
///
/// Wraps any `VectorMemoryStore` implementation behind dynamic dispatch,
/// enabling runtime selection of vector backends (e.g., LanceDB, sqlite-vec).
///
/// Since `VectorMemoryStore` uses RPITIT, it cannot be used as a trait object
/// directly. `BoxVectorMemoryStore` provides equivalent methods that delegate
/// to the inner `VectorMemoryStoreDyn` trait object.
pub struct BoxVectorMemoryStore {
    inner: Box<dyn VectorMemoryStoreDyn + Send + Sync>,
}

impl BoxVectorMemoryStore {
    /// Wrap a concrete `VectorMemoryStore` in a type-erased box.
    pub fn new<T: VectorMemoryStore + 'static>(store: T) -> Self {
        Self {
            inner: Box::new(store),
        }
    }

    /// Search for memories semantically similar to the query embedding.
    pub async fn search(
        &self,
        bot_id: &Uuid,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<RankedMemory>, RepositoryError> {
        self.inner
            .search_boxed(bot_id, query_embedding, limit, min_similarity)
            .await
    }

    /// Add a memory entry with its embedding vector.
    pub async fn add(
        &self,
        entry: &VectorMemoryEntry,
        embedding: &[f32],
    ) -> Result<(), RepositoryError> {
        self.inner.add_boxed(entry, embedding).await
    }

    /// Delete a specific memory by ID.
    pub async fn delete(
        &self,
        bot_id: &Uuid,
        memory_id: &Uuid,
    ) -> Result<(), RepositoryError> {
        self.inner.delete_boxed(bot_id, memory_id).await
    }

    /// Delete all memories for a bot. Returns the count of deleted entries.
    pub async fn delete_all(&self, bot_id: &Uuid) -> Result<u64, RepositoryError> {
        self.inner.delete_all_boxed(bot_id).await
    }

    /// Count total memories for a bot.
    pub async fn count(&self, bot_id: &Uuid) -> Result<u64, RepositoryError> {
        self.inner.count_boxed(bot_id).await
    }

    /// Check if a near-duplicate memory exists (by embedding similarity threshold).
    pub async fn check_duplicate(
        &self,
        bot_id: &Uuid,
        embedding: &[f32],
        threshold: f32,
    ) -> Result<Option<VectorMemoryEntry>, RepositoryError> {
        self.inner
            .check_duplicate_boxed(bot_id, embedding, threshold)
            .await
    }

    /// Get all entries whose `embedding_model` differs from `current_model`.
    pub async fn get_all_for_reembedding(
        &self,
        bot_id: &Uuid,
        current_model: &str,
    ) -> Result<Vec<VectorMemoryEntry>, RepositoryError> {
        self.inner
            .get_all_for_reembedding_boxed(bot_id, current_model)
            .await
    }

    /// Update a memory's embedding vector and model name.
    pub async fn update_embedding(
        &self,
        memory_id: &Uuid,
        new_embedding: &[f32],
        model_name: &str,
    ) -> Result<(), RepositoryError> {
        self.inner
            .update_embedding_boxed(memory_id, new_embedding, model_name)
            .await
    }
}
