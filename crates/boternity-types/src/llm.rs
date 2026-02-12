//! LLM request/response types for Boternity.
//!
//! These types model the data shapes for LLM provider interactions:
//! completion requests, streaming events, usage tracking, and error handling.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Role of a message in an LLM conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
        }
    }
}

impl FromStr for MessageRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "system" => Ok(MessageRole::System),
            "user" => Ok(MessageRole::User),
            "assistant" => Ok(MessageRole::Assistant),
            other => Err(format!("invalid message role: '{other}'")),
        }
    }
}

/// A single message in an LLM conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// Request to an LLM provider for a completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

/// Response from an LLM provider for a non-streaming completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub content: String,
    pub model: String,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

/// Reason why the LLM stopped generating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
    PauseTurn,
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StopReason::EndTurn => write!(f, "end_turn"),
            StopReason::ToolUse => write!(f, "tool_use"),
            StopReason::MaxTokens => write!(f, "max_tokens"),
            StopReason::StopSequence => write!(f, "stop_sequence"),
            StopReason::PauseTurn => write!(f, "pause_turn"),
        }
    }
}

impl FromStr for StopReason {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "end_turn" => Ok(StopReason::EndTurn),
            "tool_use" => Ok(StopReason::ToolUse),
            "max_tokens" => Ok(StopReason::MaxTokens),
            "stop_sequence" => Ok(StopReason::StopSequence),
            "pause_turn" => Ok(StopReason::PauseTurn),
            other => Err(format!("invalid stop reason: '{other}'")),
        }
    }
}

/// Token usage for a completion request/response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
}

/// Token count for a request (used by count_tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCount {
    pub input_tokens: u32,
}

/// Events emitted during a streaming LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Connection established with the provider.
    Connected,

    /// A new content block has started.
    ContentBlockStart {
        index: u32,
        content_type: String,
    },

    /// A delta of text content within a content block.
    TextDelta {
        index: u32,
        text: String,
    },

    /// A delta of thinking/reasoning content within a content block.
    ThinkingDelta {
        index: u32,
        thinking: String,
    },

    /// A tool use block has been fully received.
    ToolUseComplete {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// A content block has stopped.
    ContentBlockStop {
        index: u32,
    },

    /// The message is finishing with a stop reason.
    MessageDelta {
        stop_reason: StopReason,
    },

    /// Token usage information.
    Usage(Usage),

    /// The stream has completed.
    Done,
}

/// Errors from LLM provider operations.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("provider error: {message}")]
    Provider { message: String },

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("stream error: {0}")]
    Stream(String),

    #[error("rate limited (retry after {retry_after_ms:?}ms)")]
    RateLimited { retry_after_ms: Option<u64> },

    #[error("provider overloaded: {0}")]
    Overloaded(String),

    #[error("authentication failed")]
    AuthenticationFailed,

    #[error("context length exceeded: max {max}, requested {requested}")]
    ContextLengthExceeded { max: u32, requested: u32 },

    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

/// Capabilities of an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tool_calling: bool,
    pub vision: bool,
    pub extended_thinking: bool,
    pub max_context_tokens: u32,
    pub max_output_tokens: u32,
}

/// Type of LLM provider backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Anthropic,
    Bedrock,
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible,
    ClaudeSubscription,
}

impl fmt::Display for ProviderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderType::Anthropic => write!(f, "anthropic"),
            ProviderType::Bedrock => write!(f, "bedrock"),
            ProviderType::OpenAiCompatible => write!(f, "openai_compatible"),
            ProviderType::ClaudeSubscription => write!(f, "claude_subscription"),
        }
    }
}

impl FromStr for ProviderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "anthropic" => Ok(ProviderType::Anthropic),
            "bedrock" => Ok(ProviderType::Bedrock),
            "openai_compatible" => Ok(ProviderType::OpenAiCompatible),
            "claude_subscription" => Ok(ProviderType::ClaudeSubscription),
            other => Err(format!("invalid provider type: '{other}'")),
        }
    }
}

/// Configuration for a single LLM provider in a fallback chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Human-readable name (e.g., "openai", "gemini", "bedrock").
    pub name: String,
    /// Backend type for this provider.
    pub provider_type: ProviderType,
    /// Reference to a secret name in the vault for the API key.
    pub api_key_secret_name: Option<String>,
    /// Override the default base URL for the provider.
    pub base_url: Option<String>,
    /// Model identifier to use.
    pub model: String,
    /// Priority for fallback ordering; lower = higher priority.
    pub priority: u32,
    /// Whether this provider is enabled.
    pub enabled: bool,
    /// What this provider supports.
    pub capabilities: ProviderCapabilities,
}

/// Configuration for the multi-provider fallback chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackChainConfig {
    /// Ordered list of provider configurations.
    pub providers: Vec<ProviderConfig>,
    /// Maximum time (ms) to wait in rate-limit queue before failing over.
    #[serde(default = "default_rate_limit_queue_timeout_ms")]
    pub rate_limit_queue_timeout_ms: u64,
    /// Warn if fallback provider costs more than this multiplier of the primary.
    #[serde(default = "default_cost_warning_multiplier")]
    pub cost_warning_multiplier: f64,
}

fn default_rate_limit_queue_timeout_ms() -> u64 {
    5000
}

fn default_cost_warning_multiplier() -> f64 {
    3.0
}

/// Cost information for a specific provider/model combination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCostInfo {
    pub provider_name: String,
    pub model: String,
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
}

/// Status information for a provider in the fallback chain (for CLI display).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatusInfo {
    pub name: String,
    /// One of "closed", "open", "half_open".
    pub circuit_state: String,
    pub last_error: Option<String>,
    /// Human-readable time since last success (e.g., "2m ago").
    pub last_success_ago: Option<String>,
    pub total_calls: u64,
    pub total_failures: u64,
    /// ISO 8601 timestamp of when the provider started being available.
    pub uptime_since: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_role_roundtrip() {
        for role in [MessageRole::System, MessageRole::User, MessageRole::Assistant] {
            let s = role.to_string();
            let parsed: MessageRole = s.parse().unwrap();
            assert_eq!(role, parsed);
        }
    }

    #[test]
    fn test_stop_reason_roundtrip() {
        for reason in [
            StopReason::EndTurn,
            StopReason::ToolUse,
            StopReason::MaxTokens,
            StopReason::StopSequence,
            StopReason::PauseTurn,
        ] {
            let s = reason.to_string();
            let parsed: StopReason = s.parse().unwrap();
            assert_eq!(reason, parsed);
        }
    }

    #[test]
    fn test_usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert!(usage.cache_creation_input_tokens.is_none());
        assert!(usage.cache_read_input_tokens.is_none());
    }

    #[test]
    fn test_message_role_serde() {
        let role = MessageRole::Assistant;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"assistant\"");
        let parsed: MessageRole = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, MessageRole::Assistant);
    }

    #[test]
    fn test_stop_reason_serde() {
        let reason = StopReason::EndTurn;
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, "\"end_turn\"");
        let parsed: StopReason = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, StopReason::EndTurn);
    }

    #[test]
    fn test_llm_error_display() {
        let err = LlmError::ContextLengthExceeded {
            max: 100_000,
            requested: 120_000,
        };
        assert!(err.to_string().contains("100000"));
        assert!(err.to_string().contains("120000"));
    }

    #[test]
    fn test_provider_type_roundtrip() {
        for pt in [
            ProviderType::Anthropic,
            ProviderType::Bedrock,
            ProviderType::OpenAiCompatible,
            ProviderType::ClaudeSubscription,
        ] {
            let s = pt.to_string();
            let parsed: ProviderType = s.parse().unwrap();
            assert_eq!(pt, parsed);
        }
    }

    #[test]
    fn test_provider_type_serde() {
        let pt = ProviderType::OpenAiCompatible;
        let json = serde_json::to_string(&pt).unwrap();
        assert_eq!(json, "\"openai_compatible\"");
        let parsed: ProviderType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ProviderType::OpenAiCompatible);
    }

    #[test]
    fn test_fallback_chain_config_defaults() {
        let json = r#"{"providers":[]}"#;
        let config: FallbackChainConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.rate_limit_queue_timeout_ms, 5000);
        assert!((config.cost_warning_multiplier - 3.0).abs() < f64::EPSILON);
    }
}
