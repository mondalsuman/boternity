//! BedrockProvider -- concrete [`LlmProvider`] implementation for AWS Bedrock.
//!
//! Sends requests to the AWS Bedrock Runtime API using Bearer token
//! authentication. Supports both non-streaming (`invoke`) and streaming
//! (`invoke-with-response-stream`) modes.
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

use super::super::anthropic::types::{
    AnthropicContentBlock, AnthropicMessage, AnthropicNonStreamResponse,
};
use super::streaming::create_bedrock_stream;
use super::types::BedrockRequest;

/// AWS Bedrock Claude LLM provider.
///
/// Implements [`LlmProvider`] for the AWS Bedrock Runtime API.
///
/// # API Key Security
///
/// The API key is stored as a [`SecretString`] and is only exposed when
/// constructing HTTP request headers. It never appears in Debug output.
pub struct BedrockProvider {
    client: reqwest::Client,
    api_key: SecretString,
    region: String,
    model_id: String,
    #[allow(dead_code)]
    model: String,
    capabilities: ProviderCapabilities,
}

impl BedrockProvider {
    /// The Anthropic API version for Bedrock.
    const API_VERSION: &'static str = "bedrock-2023-05-31";

    /// Prefix used to identify Bedrock API keys.
    const KEY_PREFIX: &'static str = "bedrock-api-key-";

    /// Create a new Bedrock provider.
    ///
    /// # Arguments
    ///
    /// * `api_key` - AWS Bedrock bearer token wrapped in SecretString.
    ///   If the key starts with `bedrock-api-key-`, the prefix is stripped
    ///   and the remainder is used as the Bearer token. The token is a
    ///   base64-encoded presigned URL containing SigV4 params.
    /// * `model` - Model identifier (e.g., "claude-sonnet-4-20250514")
    /// * `region` - AWS region (e.g., "us-east-1"). If the token's embedded
    ///   credential scope specifies a different region, that region is used
    ///   instead (with a warning).
    pub fn new(api_key: SecretString, model: String, region: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .expect("failed to create reqwest client");

        // Strip the bedrock-api-key- prefix so only the base64 token is used
        // as the Bearer token in HTTP requests.
        let raw_key = api_key.expose_secret().to_string();
        let token_part = raw_key.strip_prefix(Self::KEY_PREFIX).unwrap_or(&raw_key);
        let effective_region = Self::detect_region_from_token(token_part)
            .unwrap_or(region);

        let bearer_token = SecretString::from(token_part.to_string());

        let model_id = Self::to_bedrock_model_id(&model, &effective_region);
        let capabilities = Self::capabilities_for_model(&model);

        Self {
            client,
            api_key: bearer_token,
            region: effective_region,
            model_id,
            model,
            capabilities,
        }
    }

    /// Try to extract the AWS region from a base64-encoded presigned URL token.
    ///
    /// The token decodes to a URL like:
    /// `bedrock.amazonaws.com/?...&X-Amz-Credential=AKIA.../20260212/us-east-1/bedrock/aws4_request&...`
    ///
    /// Returns `Some(region)` if found, `None` otherwise.
    fn detect_region_from_token(token: &str) -> Option<String> {
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(token)
            .ok()?;
        let text = String::from_utf8(decoded).ok()?;

        // Look for X-Amz-Credential=.../<date>/<region>/bedrock/aws4_request
        let cred_start = text.find("X-Amz-Credential=")?;
        let cred_value = &text[cred_start + "X-Amz-Credential=".len()..];
        // Format: <access-key>/<date>/<region>/<service>/aws4_request
        let parts: Vec<&str> = cred_value.split('/').collect();
        if parts.len() >= 3 {
            let region = parts[2].split('&').next().unwrap_or(parts[2]);
            tracing::info!(region = %region, "Detected region from Bedrock bearer token");
            Some(region.to_string())
        } else {
            None
        }
    }

    /// Convert a standard Claude model name to a Bedrock inference profile ID.
    ///
    /// Bedrock cross-region inference profiles use a region shorthand prefix
    /// (e.g., `eu.`, `us.`) before the model ID. The `region` parameter is
    /// the full AWS region (e.g., `eu-west-1`); the shorthand is extracted
    /// from the first segment before the dash.
    ///
    /// If the model already contains a `.` (e.g., `eu.anthropic.claude-...`
    /// or `anthropic.claude-...`), it is returned as-is.
    ///
    /// # Examples
    ///
    /// ```text
    /// ("claude-sonnet-4-5-20250929", "eu-west-1") → "eu.anthropic.claude-sonnet-4-5-20250929-v1:0"
    /// ("claude-sonnet-4-5-20250929", "us-east-1") → "us.anthropic.claude-sonnet-4-5-20250929-v1:0"
    /// ("eu.anthropic.claude-sonnet-4-5-20250929-v1:0", _) → unchanged
    /// ("anthropic.claude-sonnet-4-5-20250929-v1:0", _) → unchanged
    /// ```
    pub fn to_bedrock_model_id(model: &str, region: &str) -> String {
        if model.contains('.') {
            // Already fully qualified (e.g., "eu.anthropic.claude-..." or "anthropic.claude-...")
            model.to_string()
        } else {
            // Extract region shorthand: "eu-west-1" → "eu", "us-east-1" → "us"
            let region_prefix = region.split('-').next().unwrap_or("us");
            format!("{region_prefix}.anthropic.{model}-v1:0")
        }
    }

    /// Determine capabilities based on model name (same logic as AnthropicProvider).
    fn capabilities_for_model(model: &str) -> ProviderCapabilities {
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

    /// Build the full Bedrock Runtime URL for a given action.
    fn url(&self, action: &str) -> String {
        format!(
            "https://bedrock-runtime.{}.amazonaws.com/model/{}/{}",
            self.region, self.model_id, action
        )
    }

    /// Convert a generic [`CompletionRequest`] into a [`BedrockRequest`].
    fn to_bedrock_request(&self, request: &CompletionRequest) -> BedrockRequest {
        let messages = request
            .messages
            .iter()
            .map(|m| AnthropicMessage {
                role: m.role.to_string(),
                content: m.content.clone(),
            })
            .collect();

        BedrockRequest {
            anthropic_version: Self::API_VERSION.to_string(),
            max_tokens: request.max_tokens,
            messages,
            system: request.system.clone(),
            temperature: request.temperature,
            stop_sequences: request.stop_sequences.clone(),
        }
    }
}

// BedrockProvider intentionally does NOT derive Debug to prevent
// accidental exposure of internal state (same pattern as AnthropicProvider).

impl LlmProvider for BedrockProvider {
    fn name(&self) -> &str {
        "bedrock"
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let body = self.to_bedrock_request(request);
        let url = self.url("invoke");

        tracing::debug!(url = %url, model_id = %self.model_id, region = %self.region, "Bedrock invoke request");

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Provider {
                message: format!("HTTP request failed: {e}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            tracing::warn!(status = %status, body = %error_body, url = %url, "Bedrock API error response");
            return Err(match status.as_u16() {
                401 | 403 => LlmError::Provider {
                    message: format!("Bedrock authentication failed (HTTP {status}): {error_body}"),
                },
                429 => LlmError::RateLimited {
                    retry_after_ms: None,
                },
                529 => LlmError::Overloaded(error_body),
                s if s >= 500 => LlmError::Provider {
                    message: format!("Bedrock server error HTTP {status}: {error_body}"),
                },
                _ => LlmError::Provider {
                    message: format!("HTTP {status}: {error_body}"),
                },
            });
        }

        let bedrock_resp: AnthropicNonStreamResponse =
            response.json().await.map_err(|e| {
                LlmError::Deserialization(format!("failed to parse response: {e}"))
            })?;

        let content = bedrock_resp
            .content
            .iter()
            .filter_map(|block| match block {
                AnthropicContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        let stop_reason = match bedrock_resp.stop_reason.as_deref() {
            Some("end_turn") => StopReason::EndTurn,
            Some("tool_use") => StopReason::ToolUse,
            Some("max_tokens") => StopReason::MaxTokens,
            Some("stop_sequence") => StopReason::StopSequence,
            Some("pause_turn") => StopReason::PauseTurn,
            _ => StopReason::EndTurn,
        };

        Ok(CompletionResponse {
            id: bedrock_resp.id,
            content,
            model: bedrock_resp.model,
            stop_reason,
            usage: Usage {
                input_tokens: bedrock_resp.usage.input_tokens,
                output_tokens: bedrock_resp.usage.output_tokens,
                cache_creation_input_tokens: bedrock_resp.usage.cache_creation_input_tokens,
                cache_read_input_tokens: bedrock_resp.usage.cache_read_input_tokens,
            },
        })
    }

    fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
        let body = self.to_bedrock_request(&request);
        let url = self.url("invoke-with-response-stream");

        create_bedrock_stream(&self.client, &url, body, &self.api_key)
    }

    async fn count_tokens(&self, request: &CompletionRequest) -> Result<TokenCount, LlmError> {
        // Same estimation as AnthropicProvider: ~4 chars per token
        let mut total_chars: usize = 0;

        if let Some(system) = &request.system {
            total_chars += system.len();
        }

        for msg in &request.messages {
            total_chars += msg.content.len();
            total_chars += 10; // overhead for role and message structure
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

    fn make_provider() -> BedrockProvider {
        BedrockProvider::new(
            SecretString::from("bedrock-api-key-test-not-real"),
            "claude-sonnet-4-20250514".to_string(),
            "us-east-1".to_string(),
        )
    }

    #[test]
    fn test_provider_name() {
        let provider = make_provider();
        assert_eq!(provider.name(), "bedrock");
    }

    #[test]
    fn test_model_id_mapping_eu_region() {
        assert_eq!(
            BedrockProvider::to_bedrock_model_id("claude-sonnet-4-5-20250929", "eu-west-1"),
            "eu.anthropic.claude-sonnet-4-5-20250929-v1:0"
        );
    }

    #[test]
    fn test_model_id_mapping_us_region() {
        assert_eq!(
            BedrockProvider::to_bedrock_model_id("claude-sonnet-4-5-20250929", "us-east-1"),
            "us.anthropic.claude-sonnet-4-5-20250929-v1:0"
        );
    }

    #[test]
    fn test_model_id_mapping_already_prefixed() {
        let id = "eu.anthropic.claude-sonnet-4-5-20250929-v1:0";
        assert_eq!(BedrockProvider::to_bedrock_model_id(id, "us-east-1"), id);
    }

    #[test]
    fn test_model_id_mapping_opus() {
        assert_eq!(
            BedrockProvider::to_bedrock_model_id("claude-opus-4-20250514", "us-west-2"),
            "us.anthropic.claude-opus-4-20250514-v1:0"
        );
    }

    #[test]
    fn test_url_construction() {
        let provider = make_provider();
        assert_eq!(
            provider.url("invoke"),
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/us.anthropic.claude-sonnet-4-20250514-v1:0/invoke"
        );
        assert_eq!(
            provider.url("invoke-with-response-stream"),
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/us.anthropic.claude-sonnet-4-20250514-v1:0/invoke-with-response-stream"
        );
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
        let provider = BedrockProvider::new(
            SecretString::from("bedrock-api-key-test"),
            "claude-opus-4-20250514".to_string(),
            "us-west-2".to_string(),
        );
        let caps = provider.capabilities();
        assert_eq!(caps.max_output_tokens, 32_000);
        assert!(caps.extended_thinking);
    }

    #[test]
    fn test_to_bedrock_request() {
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

        let bedrock_req = provider.to_bedrock_request(&request);
        assert_eq!(bedrock_req.anthropic_version, "bedrock-2023-05-31");
        assert_eq!(bedrock_req.max_tokens, 1024);
        assert_eq!(bedrock_req.messages.len(), 1);
        assert_eq!(bedrock_req.messages[0].role, "user");
        assert_eq!(bedrock_req.system.as_deref(), Some("Be helpful"));
    }

    #[test]
    fn test_bedrock_request_no_model_field() {
        let provider = make_provider();
        let request = CompletionRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            messages: vec![],
            system: None,
            max_tokens: 1024,
            temperature: None,
            stream: false,
            stop_sequences: None,
        };

        let bedrock_req = provider.to_bedrock_request(&request);
        let json = serde_json::to_value(&bedrock_req).unwrap();
        // model must NOT be in the request body (it's in the URL path)
        assert!(json.get("model").is_none());
        // anthropic_version must be present
        assert_eq!(json["anthropic_version"], "bedrock-2023-05-31");
    }

    #[test]
    fn test_custom_region() {
        let provider = BedrockProvider::new(
            SecretString::from("bedrock-api-key-test"),
            "claude-sonnet-4-20250514".to_string(),
            "eu-west-1".to_string(),
        );
        let url = provider.url("invoke");
        assert!(url.contains("eu-west-1"));
        assert!(url.contains("eu.anthropic."));
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
        assert!(count.input_tokens > 0);
        assert!(count.input_tokens < 100);
    }
}
