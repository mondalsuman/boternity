//! Embedder trait for text-to-vector conversion.
//!
//! Defines the interface for embedding text into vectors for semantic search.
//! Implementations (e.g., OpenAI embeddings, local models) live in boternity-infra.

use boternity_types::error::RepositoryError;

/// Trait for converting text into embedding vectors.
///
/// Uses RPITIT (native async fn in traits, Rust 2024 edition).
/// Implementations live in boternity-infra.
pub trait Embedder: Send + Sync {
    /// Embed one or more texts into vectors.
    ///
    /// Returns one vector per input text. Batch embedding is supported
    /// for efficiency when multiple texts need embedding together.
    fn embed(
        &self,
        texts: &[String],
    ) -> impl std::future::Future<Output = Result<Vec<Vec<f32>>, RepositoryError>> + Send;

    /// The model name used for embeddings (e.g., "text-embedding-3-small").
    fn model_name(&self) -> &str;

    /// The dimensionality of the output vectors.
    fn dimension(&self) -> usize;
}
