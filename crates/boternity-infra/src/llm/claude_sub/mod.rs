//! Claude.ai subscription provider (EXPERIMENTAL).
//!
//! **WARNING: This provider violates Anthropic's Terms of Service as of January 2026.**
//!
//! Anthropic actively enforces against using Claude.ai subscription access
//! programmatically via local proxies. This provider is unreliable and may
//! stop working at any time. It is included only for experimentation and
//! MUST NOT be relied upon for production use.
//!
//! The provider works by proxying requests through a local `claude-max-api-proxy`
//! instance running at `localhost:3456`, which wraps the Claude Code CLI to expose
//! an OpenAI-compatible API endpoint.
//!
//! **Requirements:**
//! - `claude-max-api-proxy` running at `http://localhost:3456`
//! - Active Claude.ai subscription (Pro/Team/Enterprise)
//! - Claude Code CLI authenticated
//!
//! **Recommendations:**
//! - Always configure an official API provider (Anthropic, OpenAI, etc.) as primary
//! - Only use this as a secondary/experimental provider
//! - Expect intermittent failures

use std::pin::Pin;

use futures_util::Stream;

use boternity_core::llm::provider::LlmProvider;
use boternity_types::llm::{
    CompletionRequest, CompletionResponse, LlmError, ProviderCapabilities, StreamEvent, TokenCount,
};

use super::openai_compat::OpenAiCompatibleProvider;

/// EXPERIMENTAL: Claude.ai subscription provider via local proxy.
///
/// Wraps [`OpenAiCompatibleProvider`] configured for the `claude-max-api-proxy`
/// running at `localhost:3456`.
///
/// # Terms of Service Warning
///
/// **Anthropic actively enforces against this usage as of January 2026.**
/// This is a ToS-violating, unreliable path. Use the official Anthropic API
/// as your primary provider. This provider is hidden behind an experimental
/// flag and will warn users about ToS implications when configured.
///
/// # API Key Security
///
/// Does NOT derive Debug to prevent accidental exposure of internal state,
/// consistent with AnthropicProvider and OpenAiCompatibleProvider.
pub struct ClaudeSubscriptionProvider {
    inner: OpenAiCompatibleProvider,
}

impl ClaudeSubscriptionProvider {
    /// Create a new Claude subscription provider.
    ///
    /// **EXPERIMENTAL:** Requires `claude-max-api-proxy` running at `localhost:3456`.
    ///
    /// # Arguments
    ///
    /// * `model` - Model identifier (e.g., "claude-opus-4-20250514", "claude-sonnet-4-20250514")
    ///
    /// # ToS Warning
    ///
    /// Anthropic actively enforces against this usage. This provider is
    /// unreliable and may stop working at any time.
    pub fn new(model: &str) -> Self {
        Self {
            inner: OpenAiCompatibleProvider::claude_subscription(model),
        }
    }

    /// Print a warning to stderr about the experimental/ToS-violating nature
    /// of this provider. Called when the provider is first configured.
    pub fn print_experimental_warning() {
        eprintln!();
        eprintln!("  WARNING: Claude subscription provider is EXPERIMENTAL");
        eprintln!("  -------------------------------------------------------");
        eprintln!("  Anthropic actively enforces against programmatic access");
        eprintln!("  to Claude.ai subscriptions as of January 2026.");
        eprintln!();
        eprintln!("  This provider may stop working at any time.");
        eprintln!("  Always configure an official API as your primary provider.");
        eprintln!();
    }
}

impl LlmProvider for ClaudeSubscriptionProvider {
    fn name(&self) -> &str {
        "claude_subscription"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        self.inner.capabilities()
    }

    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, LlmError> {
        self.inner.complete(request).await
    }

    fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
        self.inner.stream(request)
    }

    async fn count_tokens(&self, request: &CompletionRequest) -> Result<TokenCount, LlmError> {
        self.inner.count_tokens(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_subscription_provider_name() {
        let provider = ClaudeSubscriptionProvider::new("claude-sonnet-4-20250514");
        assert_eq!(provider.name(), "claude_subscription");
    }

    #[test]
    fn test_claude_subscription_capabilities() {
        let provider = ClaudeSubscriptionProvider::new("claude-opus-4-20250514");
        let caps = provider.capabilities();
        assert!(caps.streaming);
        assert!(caps.tool_calling);
        assert!(caps.vision);
        assert!(caps.extended_thinking);
        assert_eq!(caps.max_context_tokens, 200_000);
        assert_eq!(caps.max_output_tokens, 128_000);
    }
}
