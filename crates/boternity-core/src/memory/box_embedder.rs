//! BoxEmbedder -- object-safe dynamic dispatch wrapper for Embedder.
//!
//! Follows the same blanket-impl pattern as BoxLlmProvider (02-01):
//! 1. Define an object-safe `EmbedderDyn` trait with boxed futures
//! 2. Blanket-impl `EmbedderDyn` for all `T: Embedder`
//! 3. `BoxEmbedder` wraps `Box<dyn EmbedderDyn>` and delegates

use std::future::Future;
use std::pin::Pin;

use boternity_types::error::RepositoryError;

use super::embedder::Embedder;

/// Object-safe version of [`Embedder`] with boxed futures.
///
/// This trait exists solely to enable dynamic dispatch (`dyn EmbedderDyn`).
/// A blanket implementation is provided for all types implementing `Embedder`.
pub trait EmbedderDyn: Send + Sync {
    fn embed_boxed<'a>(
        &'a self,
        texts: &'a [String],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Vec<f32>>, RepositoryError>> + Send + 'a>>;

    fn model_name_dyn(&self) -> &str;

    fn dimension_dyn(&self) -> usize;
}

/// Blanket implementation: any `Embedder` automatically implements `EmbedderDyn`.
impl<T: Embedder> EmbedderDyn for T {
    fn embed_boxed<'a>(
        &'a self,
        texts: &'a [String],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Vec<f32>>, RepositoryError>> + Send + 'a>> {
        Box::pin(self.embed(texts))
    }

    fn model_name_dyn(&self) -> &str {
        self.model_name()
    }

    fn dimension_dyn(&self) -> usize {
        self.dimension()
    }
}

/// Type-erased embedder for runtime selection.
///
/// Wraps any `Embedder` implementation behind dynamic dispatch,
/// enabling runtime selection of embedding models (e.g., fastembed, OpenAI).
///
/// Since `Embedder` uses RPITIT, it cannot be used as a trait object directly.
/// `BoxEmbedder` provides equivalent methods that delegate to the inner
/// `EmbedderDyn` trait object.
pub struct BoxEmbedder {
    inner: Box<dyn EmbedderDyn + Send + Sync>,
}

impl BoxEmbedder {
    /// Wrap a concrete `Embedder` in a type-erased box.
    pub fn new<T: Embedder + 'static>(embedder: T) -> Self {
        Self {
            inner: Box::new(embedder),
        }
    }

    /// Embed one or more texts into vectors.
    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, RepositoryError> {
        self.inner.embed_boxed(texts).await
    }

    /// The model name used for embeddings.
    pub fn model_name(&self) -> &str {
        self.inner.model_name_dyn()
    }

    /// The dimensionality of the output vectors.
    pub fn dimension(&self) -> usize {
        self.inner.dimension_dyn()
    }
}
