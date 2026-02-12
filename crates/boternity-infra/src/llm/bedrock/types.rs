//! AWS Bedrock request/response types.
//!
//! Bedrock uses the same Claude Messages API JSON format as the direct
//! Anthropic API, but with two differences:
//! - The `model` field is omitted from the request body (it goes in the URL path).
//! - An `anthropic_version` field is required in the request body.
//!
//! Response types (`AnthropicNonStreamResponse`, SSE event payloads) are
//! identical and reused from `super::anthropic::types`.

use serde::{Deserialize, Serialize};

use super::super::anthropic::types::AnthropicMessage;

/// Request body for AWS Bedrock Claude invoke / invoke-with-response-stream.
///
/// Unlike [`AnthropicRequest`], this omits `model` (passed in the URL) and
/// includes `anthropic_version`.
#[derive(Debug, Clone, Serialize)]
pub struct BedrockRequest {
    pub anthropic_version: String,
    pub max_tokens: u32,
    pub messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

/// A single chunk in the Bedrock event stream.
///
/// Bedrock wraps each SSE-equivalent event inside `{"bytes":"<base64>"}`.
/// The base64-decoded payload is the same JSON as Anthropic SSE `data:` lines.
#[derive(Debug, Clone, Deserialize)]
pub struct BedrockStreamChunk {
    pub bytes: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bedrock_request_serialization_no_model() {
        let req = BedrockRequest {
            anthropic_version: "bedrock-2023-05-31".to_string(),
            max_tokens: 1024,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            system: Some("Be helpful.".to_string()),
            temperature: Some(0.7),
            stop_sequences: None,
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["anthropic_version"], "bedrock-2023-05-31");
        assert_eq!(json["max_tokens"], 1024);
        // model must NOT be present
        assert!(json.get("model").is_none());
        // stop_sequences skipped when None
        assert!(json.get("stop_sequences").is_none());
    }

    #[test]
    fn test_bedrock_stream_chunk_deserialization() {
        let json = r#"{"bytes":"eyJ0eXBlIjoiY29udGVudF9ibG9ja19kZWx0YSJ9"}"#;
        let chunk: BedrockStreamChunk = serde_json::from_str(json).unwrap();
        assert!(!chunk.bytes.is_empty());

        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&chunk.bytes)
            .unwrap();
        let text = String::from_utf8(decoded).unwrap();
        assert!(text.contains("content_block_delta"));
    }
}
