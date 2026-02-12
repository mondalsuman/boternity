//! FastEmbed-based local embedding generator.
//!
//! Implements the `Embedder` trait from `boternity-core` using fastembed's
//! BGESmallENV15 model (384 dimensions) with ONNX runtime inference.
//!
//! CRITICAL: Embedding generation is CPU-intensive ONNX inference.
//! All embed calls use `tokio::task::spawn_blocking` to avoid blocking
//! the async runtime (RESEARCH.md Pitfall 1).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use boternity_core::memory::embedder::Embedder;
use boternity_types::error::RepositoryError;
use fastembed::{EmbeddingModel, TextEmbedding};

use super::schema::EMBEDDING_DIMENSION;

/// Local embedding generator using fastembed's BGESmallENV15 model.
///
/// Wraps `TextEmbedding` in `Arc<Mutex<_>>` because `embed()` requires `&mut self`.
/// The mutex is only held inside `spawn_blocking`, so it does not block the
/// async runtime.
pub struct FastEmbedEmbedder {
    model: Arc<Mutex<TextEmbedding>>,
    model_name: String,
    dimension: usize,
}

impl FastEmbedEmbedder {
    /// Create a new FastEmbedEmbedder with the BGESmallENV15 model.
    ///
    /// Downloads the model on first use to `{data_dir}/boternity/models`.
    /// Subsequent calls use the cached model files.
    pub fn new() -> Result<Self, RepositoryError> {
        let cache_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("boternity")
            .join("models");

        Self::with_cache_dir(cache_dir)
    }

    /// Create with a custom cache directory (useful for testing).
    pub fn with_cache_dir(cache_dir: PathBuf) -> Result<Self, RepositoryError> {
        let model = TextEmbedding::try_new(
            fastembed::TextInitOptions::new(EmbeddingModel::BGESmallENV15)
                .with_cache_dir(cache_dir)
                .with_show_download_progress(true),
        )
        .map_err(|e| RepositoryError::Query(format!("Failed to load embedding model: {e}")))?;

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            model_name: "bge-small-en-v1.5".to_string(),
            dimension: EMBEDDING_DIMENSION as usize,
        })
    }
}

impl Embedder for FastEmbedEmbedder {
    /// Embed texts into 384-dimensional vectors using BGESmallENV15.
    ///
    /// Uses `tokio::task::spawn_blocking` to avoid blocking the async runtime
    /// during CPU-intensive ONNX inference (RESEARCH.md Pitfall 1).
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, RepositoryError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        // Clone texts and Arc for the blocking task
        let texts_owned: Vec<String> = texts.to_vec();
        let model = Arc::clone(&self.model);

        let embeddings: Result<Vec<Vec<f32>>, RepositoryError> =
            tokio::task::spawn_blocking(move || {
                let mut model = model.lock().map_err(|e| {
                    RepositoryError::Query(format!("Embedding model lock poisoned: {e}"))
                })?;
                model.embed(texts_owned, None).map_err(|e| {
                    RepositoryError::Query(format!("Embedding generation failed: {e}"))
                })
            })
            .await
            .map_err(|e| RepositoryError::Query(format!("Embedding task panicked: {e}")))?;

        embeddings
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_dimension() {
        // Verify the dimension constant matches BGESmallENV15
        assert_eq!(EMBEDDING_DIMENSION, 384);
    }

    // Integration test that actually loads the model and generates embeddings.
    // This downloads the model on first run (~23MB).
    #[tokio::test]
    async fn test_embed_generates_384_dim_vectors() {
        let cache_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("boternity")
            .join("models");

        let embedder =
            FastEmbedEmbedder::with_cache_dir(cache_dir).expect("Failed to create embedder");

        assert_eq!(embedder.model_name(), "bge-small-en-v1.5");
        assert_eq!(embedder.dimension(), 384);

        let texts = vec![
            "User prefers concise responses".to_string(),
            "The weather is nice today".to_string(),
        ];

        let embeddings = embedder.embed(&texts).await.expect("Embedding failed");

        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 384);
        assert_eq!(embeddings[1].len(), 384);

        // Vectors should be non-zero
        assert!(embeddings[0].iter().any(|&v| v != 0.0));
        assert!(embeddings[1].iter().any(|&v| v != 0.0));
    }

    #[tokio::test]
    async fn test_embed_empty_input() {
        let cache_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("boternity")
            .join("models");

        let embedder =
            FastEmbedEmbedder::with_cache_dir(cache_dir).expect("Failed to create embedder");

        let result = embedder.embed(&[]).await.expect("Empty embed failed");
        assert!(result.is_empty());
    }
}
