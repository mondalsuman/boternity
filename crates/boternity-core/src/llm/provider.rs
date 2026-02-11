//! LlmProvider trait definition.
//!
//! This is the core abstraction that all LLM providers implement.
//! Uses RPITIT for `complete` and `count_tokens`, and `Pin<Box<dyn Stream>>`
//! for `stream` (streams need to be object-safe for the BoxLlmProvider wrapper).

use std::pin::Pin;

use futures_util::Stream;

use boternity_types::llm::{
    CompletionRequest, CompletionResponse, LlmError, ProviderCapabilities, StreamEvent, TokenCount,
};

/// Trait for LLM provider backends (Anthropic, OpenAI, etc.).
///
/// Uses native async fn in traits (RPITIT, Rust 2024 edition) for
/// `complete` and `count_tokens`. The `stream` method returns a boxed
/// stream because streams need to be object-safe for `BoxLlmProvider`.
///
/// Implementations live in boternity-infra (e.g., `AnthropicProvider`).
pub trait LlmProvider: Send + Sync {
    /// Human-readable provider name (e.g., "anthropic", "openai").
    fn name(&self) -> &str;

    /// What this provider supports (streaming, tool calling, etc.).
    fn capabilities(&self) -> &ProviderCapabilities;

    /// Send a completion request and receive the full response.
    fn complete(
        &self,
        request: &CompletionRequest,
    ) -> impl std::future::Future<Output = Result<CompletionResponse, LlmError>> + Send;

    /// Send a streaming completion request. Returns a stream of events.
    ///
    /// Returns a boxed stream (not RPITIT) because streams need to be
    /// object-safe for the `BoxLlmProvider` wrapper.
    fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>>;

    /// Count the tokens in a request without sending it to the LLM.
    fn count_tokens(
        &self,
        request: &CompletionRequest,
    ) -> impl std::future::Future<Output = Result<TokenCount, LlmError>> + Send;
}
