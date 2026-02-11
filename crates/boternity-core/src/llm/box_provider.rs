//! BoxLlmProvider -- object-safe dynamic dispatch wrapper for LlmProvider.
//!
//! Follows the same blanket-impl pattern as BoxSecretProvider (01-04):
//! 1. Define an object-safe `LlmProviderDyn` trait with boxed futures
//! 2. Blanket-impl `LlmProviderDyn` for all `T: LlmProvider`
//! 3. `BoxLlmProvider` wraps `Box<dyn LlmProviderDyn>` and delegates

use std::future::Future;
use std::pin::Pin;

use futures_util::Stream;

use boternity_types::llm::{
    CompletionRequest, CompletionResponse, LlmError, ProviderCapabilities, StreamEvent, TokenCount,
};

use super::provider::LlmProvider;

/// Object-safe version of [`LlmProvider`] with boxed futures.
///
/// This trait exists solely to enable dynamic dispatch (`dyn LlmProviderDyn`).
/// A blanket implementation is provided for all types implementing `LlmProvider`.
pub trait LlmProviderDyn: Send + Sync {
    fn name(&self) -> &str;

    fn capabilities(&self) -> &ProviderCapabilities;

    fn complete_boxed<'a>(
        &'a self,
        request: &'a CompletionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<CompletionResponse, LlmError>> + Send + 'a>>;

    fn stream_boxed(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>>;

    fn count_tokens_boxed<'a>(
        &'a self,
        request: &'a CompletionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<TokenCount, LlmError>> + Send + 'a>>;
}

/// Blanket implementation: any `LlmProvider` automatically implements `LlmProviderDyn`.
impl<T: LlmProvider> LlmProviderDyn for T {
    fn name(&self) -> &str {
        LlmProvider::name(self)
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        LlmProvider::capabilities(self)
    }

    fn complete_boxed<'a>(
        &'a self,
        request: &'a CompletionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<CompletionResponse, LlmError>> + Send + 'a>> {
        Box::pin(self.complete(request))
    }

    fn stream_boxed(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
        self.stream(request)
    }

    fn count_tokens_boxed<'a>(
        &'a self,
        request: &'a CompletionRequest,
    ) -> Pin<Box<dyn Future<Output = Result<TokenCount, LlmError>> + Send + 'a>> {
        Box::pin(self.count_tokens(request))
    }
}

/// Type-erased LLM provider for runtime provider selection.
///
/// Wraps any `LlmProvider` implementation behind dynamic dispatch,
/// enabling runtime selection of providers (e.g., Anthropic vs OpenAI).
///
/// Since `LlmProvider` uses RPITIT, it cannot be used as a trait object directly.
/// `BoxLlmProvider` provides equivalent methods that delegate to the inner
/// `LlmProviderDyn` trait object.
pub struct BoxLlmProvider {
    inner: Box<dyn LlmProviderDyn + Send + Sync>,
}

impl BoxLlmProvider {
    /// Wrap a concrete `LlmProvider` in a type-erased box.
    pub fn new<T: LlmProvider + 'static>(provider: T) -> Self {
        Self {
            inner: Box::new(provider),
        }
    }

    /// Human-readable provider name.
    pub fn name(&self) -> &str {
        self.inner.name()
    }

    /// What this provider supports.
    pub fn capabilities(&self) -> &ProviderCapabilities {
        self.inner.capabilities()
    }

    /// Send a completion request and receive the full response.
    pub async fn complete(
        &self,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse, LlmError> {
        self.inner.complete_boxed(request).await
    }

    /// Send a streaming completion request. Returns a stream of events.
    pub fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
        self.inner.stream_boxed(request)
    }

    /// Count the tokens in a request without sending it to the LLM.
    pub async fn count_tokens(
        &self,
        request: &CompletionRequest,
    ) -> Result<TokenCount, LlmError> {
        self.inner.count_tokens_boxed(request).await
    }
}
