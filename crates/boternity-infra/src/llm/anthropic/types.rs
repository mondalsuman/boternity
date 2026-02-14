//! Anthropic Messages API types.
//!
//! These are Anthropic-specific request/response structures used for HTTP
//! communication with the Anthropic Messages API. They are NOT the generic
//! LLM types from boternity-types -- those are provider-agnostic.

use serde::{Deserialize, Serialize};

use boternity_types::llm::OutputConfig;

/// Request body for the Anthropic Messages API.
#[derive(Debug, Clone, Serialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Structured output configuration. When present, constrains the LLM's
    /// response to match the given JSON schema. Skipped when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,
}

/// A single message in an Anthropic conversation.
#[derive(Debug, Clone, Serialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: String,
}

// ---------------------------------------------------------------------------
// SSE event payload structs
//
// The Anthropic SSE stream uses the `event:` field to name the event type
// (e.g., "message_start", "content_block_delta") and the `data:` field
// contains JSON. We deserialize each payload into a specific struct based
// on the event type string -- NOT via serde tag on an outer enum.
// ---------------------------------------------------------------------------

/// Payload for `event: message_start`.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageStartPayload {
    pub message: AnthropicMessageObj,
}

/// The message object inside a `message_start` event.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicMessageObj {
    pub id: String,
    pub model: String,
    pub usage: Option<AnthropicUsage>,
}

/// Payload for `event: content_block_start`.
#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlockStartPayload {
    pub index: u32,
    pub content_block: AnthropicContentBlock,
}

/// A content block in an Anthropic response.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

impl AnthropicContentBlock {
    /// Returns the type name string for this content block.
    pub fn type_name(&self) -> &str {
        match self {
            AnthropicContentBlock::Text { .. } => "text",
            AnthropicContentBlock::ToolUse { .. } => "tool_use",
        }
    }
}

/// Payload for `event: content_block_delta`.
#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlockDeltaPayload {
    pub index: u32,
    pub delta: AnthropicDelta,
}

/// Delta types within a content block.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
    #[serde(rename = "signature_delta")]
    SignatureDelta { signature: String },
}

/// Payload for `event: content_block_stop`.
#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlockStopPayload {
    pub index: u32,
}

/// Payload for `event: message_delta`.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeltaPayload {
    pub delta: MessageDeltaObj,
    pub usage: AnthropicUsage,
}

/// The delta object inside a `message_delta` event.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeltaObj {
    pub stop_reason: Option<String>,
}

/// Token usage from Anthropic.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AnthropicUsage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
    pub cache_creation_input_tokens: Option<u32>,
    pub cache_read_input_tokens: Option<u32>,
}

/// Payload for `event: error`.
#[derive(Debug, Clone, Deserialize)]
pub struct ErrorPayload {
    pub error: AnthropicError,
}

/// An error from the Anthropic API.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// Non-streaming response from the Anthropic Messages API.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicNonStreamResponse {
    pub id: String,
    pub content: Vec<AnthropicContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: AnthropicUsage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_request_serialization() {
        let req = AnthropicRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 1024,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            system: Some("You are helpful.".to_string()),
            stream: false,
            temperature: Some(0.7),
            stop_sequences: None,
            output_config: None,
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["model"], "claude-sonnet-4-20250514");
        assert_eq!(json["max_tokens"], 1024);
        assert_eq!(json["stream"], false);
        assert!(json.get("stop_sequences").is_none());
        // output_config should not appear when None
        assert!(json.get("output_config").is_none());
    }

    #[test]
    fn test_anthropic_request_with_output_config() {
        use boternity_types::llm::{OutputFormat, OutputJsonSchema};

        let req = AnthropicRequest {
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 2048,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            system: None,
            stream: false,
            temperature: Some(0.7),
            stop_sequences: None,
            output_config: Some(OutputConfig {
                format: OutputFormat {
                    type_field: "json_schema".to_string(),
                    json_schema: OutputJsonSchema {
                        name: "BuilderTurn".to_string(),
                        schema: serde_json::json!({"type": "object"}),
                        strict: Some(true),
                    },
                },
            }),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("output_config").is_some());
        assert_eq!(json["output_config"]["format"]["type"], "json_schema");
        assert_eq!(json["output_config"]["format"]["json_schema"]["name"], "BuilderTurn");
        assert_eq!(json["output_config"]["format"]["json_schema"]["strict"], true);
    }

    #[test]
    fn test_content_block_text_deserialization() {
        let json = r#"{"type": "text", "text": "Hello world"}"#;
        let block: AnthropicContentBlock = serde_json::from_str(json).unwrap();
        match block {
            AnthropicContentBlock::Text { text } => assert_eq!(text, "Hello world"),
            _ => panic!("expected Text variant"),
        }
    }

    #[test]
    fn test_content_block_tool_use_deserialization() {
        let json = r#"{"type": "tool_use", "id": "tool_1", "name": "calculator", "input": {"x": 1}}"#;
        let block: AnthropicContentBlock = serde_json::from_str(json).unwrap();
        match block {
            AnthropicContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tool_1");
                assert_eq!(name, "calculator");
                assert_eq!(input["x"], 1);
            }
            _ => panic!("expected ToolUse variant"),
        }
    }

    #[test]
    fn test_delta_text_deserialization() {
        let json = r#"{"type": "text_delta", "text": "Hi"}"#;
        let delta: AnthropicDelta = serde_json::from_str(json).unwrap();
        match delta {
            AnthropicDelta::TextDelta { text } => assert_eq!(text, "Hi"),
            _ => panic!("expected TextDelta variant"),
        }
    }

    #[test]
    fn test_delta_input_json_deserialization() {
        let json = r#"{"type": "input_json_delta", "partial_json": "{\"x\":"}"#;
        let delta: AnthropicDelta = serde_json::from_str(json).unwrap();
        match delta {
            AnthropicDelta::InputJsonDelta { partial_json } => {
                assert_eq!(partial_json, "{\"x\":");
            }
            _ => panic!("expected InputJsonDelta variant"),
        }
    }

    #[test]
    fn test_anthropic_error_deserialization() {
        let json = r#"{"type": "overloaded_error", "message": "Server busy"}"#;
        let err: AnthropicError = serde_json::from_str(json).unwrap();
        assert_eq!(err.error_type, "overloaded_error");
        assert_eq!(err.message, "Server busy");
    }

    #[test]
    fn test_message_start_payload_deserialization() {
        let json = r#"{
            "type": "message_start",
            "message": {
                "id": "msg_123",
                "model": "claude-sonnet-4-20250514",
                "usage": {"input_tokens": 100, "output_tokens": 0}
            }
        }"#;
        let payload: MessageStartPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.message.id, "msg_123");
        assert_eq!(payload.message.usage.as_ref().unwrap().input_tokens, 100);
    }

    #[test]
    fn test_non_stream_response_deserialization() {
        let json = r#"{
            "id": "msg_456",
            "content": [{"type": "text", "text": "Hello!"}],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 50, "output_tokens": 20}
        }"#;
        let resp: AnthropicNonStreamResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "msg_456");
        assert_eq!(resp.content.len(), 1);
        assert_eq!(resp.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(resp.usage.input_tokens, 50);
    }

    #[test]
    fn test_content_block_type_name() {
        let text = AnthropicContentBlock::Text {
            text: "hi".to_string(),
        };
        assert_eq!(text.type_name(), "text");

        let tool = AnthropicContentBlock::ToolUse {
            id: "t1".to_string(),
            name: "calc".to_string(),
            input: serde_json::Value::Null,
        };
        assert_eq!(tool.type_name(), "tool_use");
    }
}
