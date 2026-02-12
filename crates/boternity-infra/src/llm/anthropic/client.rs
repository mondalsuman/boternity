//! AnthropicProvider -- concrete [`LlmProvider`] implementation for Anthropic Claude.
//!
//! Sends requests to the Anthropic Messages API (`/v1/messages`) with
//! proper authentication headers. Supports both non-streaming (`complete`)
//! and streaming (`stream`) modes.
//!
//! The API key is wrapped in [`secrecy::SecretString`] and is never logged
//! or included in `Debug` output.

use std::pin::Pin;
use std::time::Duration;

use futures_util::Stream;
use secrecy::{ExposeSecret, SecretString};

use boternity_core::llm::provider::LlmProvider;
use boternity_types::llm::{
    CompletionRequest, CompletionResponse, LlmError, ProviderCapabilities, StopReason, StreamEvent,
    TokenCount, Usage,
};

use super::streaming::create_anthropic_stream;
use super::types::{AnthropicContentBlock, AnthropicMessage, AnthropicNonStreamResponse, AnthropicRequest};

/// Anthropic Claude LLM provider.
///
/// Implements [`LlmProvider`] for the Anthropic Messages API.
///
/// # API Key Security
///
/// The API key is stored as a [`SecretString`] and is only exposed when
/// constructing HTTP request headers. It never appears in Debug output,
/// Display output, or tracing logs.
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: SecretString,
    base_url: String,
    model: String,
    capabilities: ProviderCapabilities,
}

impl AnthropicProvider {
    /// The Anthropic API version header value.
    const API_VERSION: &'static str = "2023-06-01";

    /// Create a new Anthropic provider.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key wrapped in SecretString
    /// * `model` - Model identifier (e.g., "claude-sonnet-4-20250514")
    pub fn new(api_key: SecretString, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // 5 min timeout for long generations
            .build()
            .expect("failed to create reqwest client");

        let capabilities = Self::capabilities_for_model(&model);

        Self {
            client,
            api_key,
            base_url: "https://api.anthropic.com".to_string(),
            model,
            capabilities,
        }
    }

    /// The default model for this provider.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Override the base URL (useful for testing or proxies).
    #[allow(dead_code)]
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// Determine capabilities based on model name.
    fn capabilities_for_model(model: &str) -> ProviderCapabilities {
        // Default capabilities for Claude Sonnet
        if model.contains("sonnet") {
            ProviderCapabilities {
                max_context_tokens: 200_000,
                max_output_tokens: 8_192,
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: false,
            }
        } else if model.contains("opus") {
            ProviderCapabilities {
                max_context_tokens: 200_000,
                max_output_tokens: 32_000,
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: true,
            }
        } else if model.contains("haiku") {
            ProviderCapabilities {
                max_context_tokens: 200_000,
                max_output_tokens: 8_192,
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: false,
            }
        } else {
            // Conservative defaults for unknown models
            ProviderCapabilities {
                max_context_tokens: 200_000,
                max_output_tokens: 4_096,
                streaming: true,
                tool_calling: true,
                vision: false,
                extended_thinking: false,
            }
        }
    }

    /// Build the full API URL for a given path.
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Convert a generic [`CompletionRequest`] into an [`AnthropicRequest`].
    fn to_anthropic_request(&self, request: &CompletionRequest, stream: bool) -> AnthropicRequest {
        let messages = request
            .messages
            .iter()
            .map(|m| AnthropicMessage {
                role: m.role.to_string(),
                content: m.content.clone(),
            })
            .collect();

        AnthropicRequest {
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            messages,
            system: request.system.clone(),
            stream,
            temperature: request.temperature,
            stop_sequences: request.stop_sequences.clone(),
        }
    }
}

// AnthropicProvider intentionally does NOT derive Debug to prevent
// accidental exposure of internal state. The SecretString field ensures
// the API key is never printed, but we also omit Debug entirely for defense-in-depth.

impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let body = self.to_anthropic_request(request, false);
        let url = self.url("/v1/messages");

        let response = self
            .client
            .post(&url)
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", Self::API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Provider {
                message: format!("HTTP request failed: {e}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(match status.as_u16() {
                401 => LlmError::AuthenticationFailed,
                429 => LlmError::RateLimited {
                    retry_after_ms: None,
                },
                529 => LlmError::Overloaded(error_body),
                _ => LlmError::Provider {
                    message: format!("HTTP {status}: {error_body}"),
                },
            });
        }

        let anthropic_resp: AnthropicNonStreamResponse =
            response.json().await.map_err(|e| {
                LlmError::Deserialization(format!("failed to parse response: {e}"))
            })?;

        // Extract text content from the response
        let content = anthropic_resp
            .content
            .iter()
            .filter_map(|block| match block {
                AnthropicContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        let stop_reason = match anthropic_resp.stop_reason.as_deref() {
            Some("end_turn") => StopReason::EndTurn,
            Some("tool_use") => StopReason::ToolUse,
            Some("max_tokens") => StopReason::MaxTokens,
            Some("stop_sequence") => StopReason::StopSequence,
            Some("pause_turn") => StopReason::PauseTurn,
            _ => StopReason::EndTurn,
        };

        Ok(CompletionResponse {
            id: anthropic_resp.id,
            content,
            model: anthropic_resp.model,
            stop_reason,
            usage: Usage {
                input_tokens: anthropic_resp.usage.input_tokens,
                output_tokens: anthropic_resp.usage.output_tokens,
                cache_creation_input_tokens: anthropic_resp.usage.cache_creation_input_tokens,
                cache_read_input_tokens: anthropic_resp.usage.cache_read_input_tokens,
            },
        })
    }

    fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
        let body = self.to_anthropic_request(&request, true);
        let url = self.url("/v1/messages");

        create_anthropic_stream(&self.client, &url, body, &self.api_key)
    }

    async fn count_tokens(&self, request: &CompletionRequest) -> Result<TokenCount, LlmError> {
        // TODO: Use Anthropic's /v1/messages/count_tokens API endpoint for exact counts.
        // For now, use a simple estimation: ~4 chars per token (rough average for English text).
        let mut total_chars: usize = 0;

        // Count system prompt
        if let Some(system) = &request.system {
            total_chars += system.len();
        }

        // Count messages
        for msg in &request.messages {
            total_chars += msg.content.len();
            // Add overhead for role and message structure
            total_chars += 10;
        }

        let estimated_tokens = (total_chars as f64 / 4.0).ceil() as u32;

        Ok(TokenCount {
            input_tokens: estimated_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider() -> AnthropicProvider {
        AnthropicProvider::new(
            SecretString::from("test-key-not-real"),
            "claude-sonnet-4-20250514".to_string(),
        )
    }

    #[test]
    fn test_provider_name() {
        let provider = make_provider();
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_sonnet_capabilities() {
        let provider = make_provider();
        let caps = provider.capabilities();
        assert_eq!(caps.max_context_tokens, 200_000);
        assert_eq!(caps.max_output_tokens, 8_192);
        assert!(caps.streaming);
        assert!(caps.tool_calling);
        assert!(caps.vision);
        assert!(!caps.extended_thinking);
    }

    #[test]
    fn test_opus_capabilities() {
        let provider = AnthropicProvider::new(
            SecretString::from("test-key"),
            "claude-opus-4-20250514".to_string(),
        );
        let caps = provider.capabilities();
        assert_eq!(caps.max_output_tokens, 32_000);
        assert!(caps.extended_thinking);
    }

    #[test]
    fn test_haiku_capabilities() {
        let provider = AnthropicProvider::new(
            SecretString::from("test-key"),
            "claude-haiku-3-5-20250514".to_string(),
        );
        let caps = provider.capabilities();
        assert_eq!(caps.max_output_tokens, 8_192);
        assert!(!caps.extended_thinking);
    }

    #[test]
    fn test_to_anthropic_request() {
        let provider = make_provider();
        let request = CompletionRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![boternity_types::llm::Message {
                role: boternity_types::llm::MessageRole::User,
                content: "Hello".to_string(),
            }],
            system: Some("Be helpful".to_string()),
            max_tokens: 1024,
            temperature: Some(0.7),
            stream: false,
            stop_sequences: None,
        };

        let anthropic_req = provider.to_anthropic_request(&request, true);
        assert_eq!(anthropic_req.model, "claude-sonnet-4-20250514");
        assert!(anthropic_req.stream);
        assert_eq!(anthropic_req.messages.len(), 1);
        assert_eq!(anthropic_req.messages[0].role, "user");
        assert_eq!(anthropic_req.system.as_deref(), Some("Be helpful"));
    }

    #[test]
    fn test_base_url_override() {
        let provider = make_provider().with_base_url("http://localhost:8080".to_string());
        assert_eq!(provider.url("/v1/messages"), "http://localhost:8080/v1/messages");
    }

    #[tokio::test]
    async fn test_count_tokens_estimation() {
        let provider = make_provider();
        let request = CompletionRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![boternity_types::llm::Message {
                role: boternity_types::llm::MessageRole::User,
                content: "Hello world, how are you doing today?".to_string(),
            }],
            system: Some("You are helpful.".to_string()),
            max_tokens: 1024,
            temperature: None,
            stream: false,
            stop_sequences: None,
        };

        let count = provider.count_tokens(&request).await.unwrap();
        // "You are helpful." = 16 chars + "Hello world, how are you doing today?" = 37 chars + 10 overhead = 63 chars
        // 63 / 4 = 15.75 -> ceil = 16
        assert!(count.input_tokens > 0);
        assert!(count.input_tokens < 100);
    }
}
