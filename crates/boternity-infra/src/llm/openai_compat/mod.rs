//! OpenAI-compatible LLM provider implementation.
//!
//! A single [`OpenAiCompatibleProvider`] serves OpenAI, Google Gemini,
//! Mistral, GLM 4.7, and Claude.ai subscription proxy -- four+ providers
//! from one codebase via configurable base URLs and factory functions.
//!
//! Uses [`async_openai`] for type-safe request/response handling and
//! built-in SSE streaming.

pub mod config;
pub mod streaming;

use std::pin::Pin;

use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestAssistantMessageContent,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, ChatCompletionStreamOptions,
    CreateChatCompletionRequest, FinishReason, StopConfiguration,
};
use async_openai::Client;
use futures_util::Stream;

use boternity_core::llm::provider::LlmProvider;
use boternity_types::llm::{
    CompletionRequest, CompletionResponse, LlmError, MessageRole, ProviderCapabilities,
    StopReason, StreamEvent, TokenCount, Usage,
};

use self::config::OpenAiCompatConfig;
use self::streaming::map_openai_stream;

/// Unified provider for any OpenAI-compatible API.
///
/// Supports: OpenAI, Google Gemini, Mistral, GLM 4.7, Claude.ai subscription proxy.
///
/// # API Key Security
///
/// Does NOT derive Debug to prevent accidental exposure of the API key
/// stored inside the `async_openai::Client`. Same defense-in-depth pattern
/// as [`super::anthropic::client::AnthropicProvider`].
pub struct OpenAiCompatibleProvider {
    client: Client<OpenAIConfig>,
    provider_name: String,
    model: String,
    capabilities: ProviderCapabilities,
}

impl OpenAiCompatibleProvider {
    /// Create a new OpenAI-compatible provider from a configuration.
    pub fn new(config: OpenAiCompatConfig) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_key(&config.api_key)
            .with_api_base(&config.base_url);

        Self {
            client: Client::with_config(openai_config),
            provider_name: config.provider_name,
            model: config.model,
            capabilities: config.capabilities,
        }
    }

    /// Create an OpenAI provider.
    ///
    /// Uses `https://api.openai.com/v1` as the base URL.
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::new(config::openai_defaults(api_key, model))
    }

    /// Create a Google Gemini provider (OpenAI-compatible beta endpoint).
    ///
    /// Uses `https://generativelanguage.googleapis.com/v1beta/openai` as the base URL.
    pub fn gemini(api_key: &str, model: &str) -> Self {
        Self::new(config::gemini_defaults(api_key, model))
    }

    /// Create a Mistral AI provider.
    ///
    /// Uses `https://api.mistral.ai/v1` as the base URL.
    pub fn mistral(api_key: &str, model: &str) -> Self {
        Self::new(config::mistral_defaults(api_key, model))
    }

    /// Create a GLM 4.7 (z.ai) provider.
    ///
    /// Uses `https://api.z.ai/api/paas/v4` as the base URL.
    pub fn glm(api_key: &str, model: &str) -> Self {
        Self::new(config::glm_defaults(api_key, model))
    }

    /// Create a Claude.ai subscription provider via local proxy.
    ///
    /// **EXPERIMENTAL:** Requires `claude-max-api-proxy` running at `localhost:3456`.
    ///
    /// **WARNING:** Anthropic actively enforces against this usage as of January 2026.
    /// This is a ToS-violating, unreliable path. Use the official Anthropic API as
    /// your primary provider and only use this for experimentation.
    pub fn claude_subscription(model: &str) -> Self {
        Self::new(config::claude_subscription_defaults(model))
    }

    /// Build a [`CreateChatCompletionRequest`] from a generic [`CompletionRequest`].
    fn build_request(
        &self,
        request: &CompletionRequest,
        stream: bool,
    ) -> Result<CreateChatCompletionRequest, LlmError> {
        let mut messages: Vec<ChatCompletionRequestMessage> = Vec::new();

        // System message
        if let Some(ref system) = request.system {
            messages.push(ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessage {
                    content: ChatCompletionRequestSystemMessageContent::Text(system.clone()),
                    name: None,
                },
            ));
        }

        // Conversation messages
        for msg in &request.messages {
            let oai_msg = match msg.role {
                MessageRole::System => ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessage {
                        content: ChatCompletionRequestSystemMessageContent::Text(
                            msg.content.clone(),
                        ),
                        name: None,
                    },
                ),
                MessageRole::User => ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessage {
                        content: ChatCompletionRequestUserMessageContent::Text(
                            msg.content.clone(),
                        ),
                        name: None,
                    },
                ),
                MessageRole::Assistant => {
                    #[allow(deprecated)]
                    ChatCompletionRequestMessage::Assistant(
                        ChatCompletionRequestAssistantMessage {
                            content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                                msg.content.clone(),
                            )),
                            refusal: None,
                            name: None,
                            audio: None,
                            tool_calls: None,
                            function_call: None,
                        },
                    )
                }
            };
            messages.push(oai_msg);
        }

        // Use the model from the request if set, otherwise fall back to config default
        let model = if request.model.is_empty() {
            self.model.clone()
        } else {
            request.model.clone()
        };

        let mut req = CreateChatCompletionRequest {
            model,
            messages,
            max_completion_tokens: Some(request.max_tokens),
            temperature: request.temperature.map(|t| t as f32),
            ..Default::default()
        };

        // Stop sequences
        if let Some(ref stops) = request.stop_sequences {
            if !stops.is_empty() {
                req.stop = Some(StopConfiguration::StringArray(stops.clone()));
            }
        }

        // Streaming configuration
        if stream {
            req.stream = Some(true);
            req.stream_options = Some(ChatCompletionStreamOptions {
                include_usage: Some(true),
                include_obfuscation: None,
            });
        }

        Ok(req)
    }
}

// OpenAiCompatibleProvider intentionally does NOT derive Debug to prevent
// accidental exposure of internal state including the API key inside the
// async-openai Client. Same defense-in-depth pattern as AnthropicProvider.

impl LlmProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }

    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let oai_request = self.build_request(request, false)?;

        let response = self
            .client
            .chat()
            .create(oai_request)
            .await
            .map_err(map_openai_error)?;

        // Extract content from the first choice
        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        // Map finish reason
        let stop_reason = response
            .choices
            .first()
            .and_then(|c| c.finish_reason.as_ref())
            .map(|fr| match fr {
                FinishReason::Stop => StopReason::EndTurn,
                FinishReason::Length => StopReason::MaxTokens,
                FinishReason::ToolCalls => StopReason::ToolUse,
                FinishReason::ContentFilter => StopReason::EndTurn,
                FinishReason::FunctionCall => StopReason::ToolUse,
            })
            .unwrap_or(StopReason::EndTurn);

        // Extract usage
        let usage = response
            .usage
            .map(|u| Usage {
                input_tokens: u.prompt_tokens,
                output_tokens: u.completion_tokens,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            })
            .unwrap_or_default();

        Ok(CompletionResponse {
            id: response.id,
            content,
            model: response.model,
            stop_reason,
            usage,
        })
    }

    fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
        // Build the request. If it fails, return a stream that immediately errors.
        let oai_request = match self.build_request(&request, true) {
            Ok(req) => req,
            Err(e) => {
                return Box::pin(futures_util::stream::once(async move { Err(e) }));
            }
        };

        // Clone the client for the 'static stream closure
        let client = self.client.clone();

        Box::pin(async_stream::try_stream! {
            let oai_stream = client
                .chat()
                .create_stream(oai_request)
                .await
                .map_err(map_openai_error)?;

            let mut inner = map_openai_stream(oai_stream);

            use futures_util::StreamExt;
            while let Some(event) = inner.next().await {
                match event {
                    Ok(ev) => yield ev,
                    Err(e) => Err(e)?,
                }
            }
        })
    }

    async fn count_tokens(&self, request: &CompletionRequest) -> Result<TokenCount, LlmError> {
        // Character-based estimation: ~4 chars per token (per project pattern from 02-05).
        let mut total_chars: usize = 0;

        if let Some(ref system) = request.system {
            total_chars += system.len();
        }

        for msg in &request.messages {
            total_chars += msg.content.len();
            // Overhead for role and message structure
            total_chars += 10;
        }

        let estimated_tokens = (total_chars as f64 / 4.0).ceil() as u32;

        Ok(TokenCount {
            input_tokens: estimated_tokens,
        })
    }
}

/// Map an `async_openai::error::OpenAIError` to an [`LlmError`].
fn map_openai_error(err: async_openai::error::OpenAIError) -> LlmError {
    use async_openai::error::OpenAIError;

    match &err {
        OpenAIError::ApiError(api_err) => {
            // Check for known error types by code or type field
            let code = api_err.code.as_deref().unwrap_or("");
            let error_type = api_err.r#type.as_deref().unwrap_or("");

            if code == "authentication_error"
                || error_type == "authentication_error"
                || api_err.message.contains("Incorrect API key")
                || api_err.message.contains("Invalid API key")
            {
                LlmError::AuthenticationFailed
            } else if code == "rate_limit_exceeded" || error_type == "rate_limit_error" {
                LlmError::RateLimited {
                    retry_after_ms: None,
                }
            } else if code == "context_length_exceeded"
                || api_err.message.contains("maximum context length")
            {
                LlmError::ContextLengthExceeded {
                    max: 0,
                    requested: 0,
                }
            } else if code == "server_error" || error_type == "overloaded_error" {
                LlmError::Overloaded(api_err.message.clone())
            } else {
                LlmError::Provider {
                    message: err.to_string(),
                }
            }
        }
        OpenAIError::Reqwest(reqwest_err) => {
            if let Some(status) = reqwest_err.status() {
                match status.as_u16() {
                    401 => LlmError::AuthenticationFailed,
                    429 => LlmError::RateLimited {
                        retry_after_ms: None,
                    },
                    529 => LlmError::Overloaded(err.to_string()),
                    _ => LlmError::Provider {
                        message: err.to_string(),
                    },
                }
            } else {
                LlmError::Provider {
                    message: err.to_string(),
                }
            }
        }
        OpenAIError::JSONDeserialize(_, content) => {
            LlmError::Deserialization(format!("failed to parse response: {content}"))
        }
        OpenAIError::StreamError(stream_err) => LlmError::Stream(stream_err.to_string()),
        OpenAIError::InvalidArgument(msg) => LlmError::InvalidRequest(msg.clone()),
        _ => LlmError::Provider {
            message: err.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_factory() {
        let provider = OpenAiCompatibleProvider::openai("sk-test", "gpt-4o");
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.model, "gpt-4o");
        assert!(provider.capabilities().streaming);
        assert!(provider.capabilities().tool_calling);
        assert!(provider.capabilities().vision);
        assert!(!provider.capabilities().extended_thinking);
        assert_eq!(provider.capabilities().max_context_tokens, 128_000);
        assert_eq!(provider.capabilities().max_output_tokens, 16_384);
    }

    #[test]
    fn test_gemini_factory() {
        let provider = OpenAiCompatibleProvider::gemini("gemini-key", "gemini-2.5-pro");
        assert_eq!(provider.name(), "gemini");
        assert_eq!(provider.model, "gemini-2.5-pro");
        assert_eq!(provider.capabilities().max_context_tokens, 1_000_000);
        assert_eq!(provider.capabilities().max_output_tokens, 65_536);
    }

    #[test]
    fn test_mistral_factory() {
        let provider = OpenAiCompatibleProvider::mistral("mistral-key", "mistral-large-latest");
        assert_eq!(provider.name(), "mistral");
        assert_eq!(provider.model, "mistral-large-latest");
        assert_eq!(provider.capabilities().max_context_tokens, 128_000);
        assert_eq!(provider.capabilities().max_output_tokens, 32_768);
    }

    #[test]
    fn test_glm_factory() {
        let provider = OpenAiCompatibleProvider::glm("glm-key", "glm-4.7");
        assert_eq!(provider.name(), "glm");
        assert_eq!(provider.model, "glm-4.7");
        assert!(!provider.capabilities().vision);
        assert_eq!(provider.capabilities().max_context_tokens, 200_000);
        assert_eq!(provider.capabilities().max_output_tokens, 128_000);
    }

    #[test]
    fn test_claude_subscription_factory() {
        let provider = OpenAiCompatibleProvider::claude_subscription("claude-opus-4-20250514");
        assert_eq!(provider.name(), "claude_subscription");
        assert_eq!(provider.model, "claude-opus-4-20250514");
        assert!(provider.capabilities().extended_thinking);
        assert_eq!(provider.capabilities().max_context_tokens, 200_000);
        assert_eq!(provider.capabilities().max_output_tokens, 128_000);
    }

    #[test]
    fn test_build_request_messages() {
        let provider = OpenAiCompatibleProvider::openai("sk-test", "gpt-4o");
        let request = CompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![
                boternity_types::llm::Message {
                    role: MessageRole::User,
                    content: "Hello".to_string(),
                },
                boternity_types::llm::Message {
                    role: MessageRole::Assistant,
                    content: "Hi there!".to_string(),
                },
            ],
            system: Some("Be helpful".to_string()),
            max_tokens: 1024,
            temperature: Some(0.7),
            stream: false,
            stop_sequences: None,
            output_config: None,
        };

        let oai_req = provider.build_request(&request, false).unwrap();
        assert_eq!(oai_req.model, "gpt-4o");
        // 1 system + 2 conversation = 3 messages
        assert_eq!(oai_req.messages.len(), 3);
        assert_eq!(oai_req.max_completion_tokens, Some(1024));
        assert!(oai_req.stream.is_none());
        assert!(oai_req.stream_options.is_none());
    }

    #[test]
    fn test_build_request_streaming() {
        let provider = OpenAiCompatibleProvider::openai("sk-test", "gpt-4o");
        let request = CompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![boternity_types::llm::Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
            }],
            system: None,
            max_tokens: 512,
            temperature: None,
            stream: true,
            stop_sequences: None,
            output_config: None,
        };

        let oai_req = provider.build_request(&request, true).unwrap();
        assert_eq!(oai_req.stream, Some(true));
        assert!(oai_req.stream_options.is_some());
        let opts = oai_req.stream_options.unwrap();
        assert_eq!(opts.include_usage, Some(true));
    }

    #[test]
    fn test_build_request_empty_model_uses_default() {
        let provider = OpenAiCompatibleProvider::openai("sk-test", "gpt-4o");
        let request = CompletionRequest {
            model: String::new(),
            messages: vec![],
            system: None,
            max_tokens: 1024,
            temperature: None,
            stream: false,
            stop_sequences: None,
            output_config: None,
        };

        let oai_req = provider.build_request(&request, false).unwrap();
        assert_eq!(oai_req.model, "gpt-4o");
    }

    #[test]
    fn test_build_request_stop_sequences() {
        let provider = OpenAiCompatibleProvider::openai("sk-test", "gpt-4o");
        let request = CompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![],
            system: None,
            max_tokens: 1024,
            temperature: None,
            stream: false,
            stop_sequences: Some(vec!["STOP".to_string(), "END".to_string()]),
            output_config: None,
        };

        let oai_req = provider.build_request(&request, false).unwrap();
        assert!(oai_req.stop.is_some());
    }

    #[tokio::test]
    async fn test_count_tokens_estimation() {
        let provider = OpenAiCompatibleProvider::openai("sk-test", "gpt-4o");
        let request = CompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![boternity_types::llm::Message {
                role: MessageRole::User,
                content: "Hello world, how are you doing today?".to_string(),
            }],
            system: Some("You are helpful.".to_string()),
            max_tokens: 1024,
            temperature: None,
            stream: false,
            stop_sequences: None,
            output_config: None,
        };

        let count = provider.count_tokens(&request).await.unwrap();
        // "You are helpful." = 16 chars + "Hello world..." = 37 chars + 10 overhead = 63
        // 63 / 4 = 15.75 -> ceil = 16
        assert!(count.input_tokens > 0);
        assert!(count.input_tokens < 100);
    }

    #[test]
    fn test_map_openai_error_api_auth() {
        use async_openai::error::{ApiError, OpenAIError};
        let api_err = ApiError {
            message: "Incorrect API key provided".to_string(),
            r#type: Some("authentication_error".to_string()),
            param: None,
            code: None,
        };
        let err = map_openai_error(OpenAIError::ApiError(api_err));
        assert!(matches!(err, LlmError::AuthenticationFailed));
    }

    #[test]
    fn test_map_openai_error_rate_limit() {
        use async_openai::error::{ApiError, OpenAIError};
        let api_err = ApiError {
            message: "Rate limit exceeded".to_string(),
            r#type: Some("rate_limit_error".to_string()),
            param: None,
            code: None,
        };
        let err = map_openai_error(OpenAIError::ApiError(api_err));
        assert!(matches!(err, LlmError::RateLimited { .. }));
    }

    #[test]
    fn test_map_openai_error_invalid_argument() {
        use async_openai::error::OpenAIError;
        let err = map_openai_error(OpenAIError::InvalidArgument("bad arg".to_string()));
        assert!(matches!(err, LlmError::InvalidRequest(_)));
    }
}
