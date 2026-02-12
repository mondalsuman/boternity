//! AWS Bedrock event stream parser and async stream adapter.
//!
//! Bedrock streaming uses the AWS event stream binary protocol (not SSE).
//! Each frame has the layout:
//!
//! ```text
//! [total_len:4][headers_len:4][prelude_crc:4][headers...][payload...][msg_crc:4]
//! ```
//!
//! For `chunk` events the payload is `{"bytes":"<base64>"}` where the
//! base64-decoded content is an Anthropic SSE-style JSON event (e.g.
//! `{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}`).
//!
//! This module provides a minimal parser that extracts events without pulling
//! in the full AWS SDK.

use std::collections::HashMap;
use std::pin::Pin;

use base64::Engine;
use futures_util::{Stream, StreamExt};

use boternity_types::llm::{LlmError, StopReason, StreamEvent, Usage};

use super::super::anthropic::types::{
    AnthropicContentBlock, AnthropicDelta, ContentBlockDeltaPayload, ContentBlockStartPayload,
    ContentBlockStopPayload, ErrorPayload, MessageDeltaPayload, MessageStartPayload,
};

/// Accumulates partial JSON fragments for tool use input within a content block.
struct ToolUseAccumulator {
    id: String,
    name: String,
    json_buffer: String,
}

/// Internal state for the event stream state machine.
struct StreamState {
    tool_input_buffers: HashMap<u32, ToolUseAccumulator>,
    #[allow(dead_code)]
    message_id: Option<String>,
    #[allow(dead_code)]
    model: Option<String>,
}

/// Parsed header from a binary event stream frame.
#[derive(Debug)]
struct EventHeader {
    name: String,
    value: String,
}

/// Parse binary headers from an AWS event stream frame.
///
/// Header format: `[name_len:1][name:N][type:1][value_len:2][value:M]`
/// We only handle type 7 (string) which is what Bedrock uses.
fn parse_headers(mut buf: &[u8]) -> Vec<EventHeader> {
    let mut headers = Vec::new();
    while !buf.is_empty() {
        if buf.is_empty() {
            break;
        }
        let name_len = buf[0] as usize;
        buf = &buf[1..];
        if buf.len() < name_len {
            break;
        }
        let name = String::from_utf8_lossy(&buf[..name_len]).to_string();
        buf = &buf[name_len..];

        if buf.is_empty() {
            break;
        }
        let header_type = buf[0];
        buf = &buf[1..];

        if header_type == 7 {
            // String type: [value_len:2][value:M]
            if buf.len() < 2 {
                break;
            }
            let value_len = u16::from_be_bytes([buf[0], buf[1]]) as usize;
            buf = &buf[2..];
            if buf.len() < value_len {
                break;
            }
            let value = String::from_utf8_lossy(&buf[..value_len]).to_string();
            buf = &buf[value_len..];
            headers.push(EventHeader { name, value });
        } else {
            // Skip unknown header types -- we can't know the length, so bail
            break;
        }
    }
    headers
}

/// Parse one binary event stream frame from the buffer.
///
/// Returns `Some((event_type, payload_bytes, bytes_consumed))` on success,
/// or `None` if the buffer doesn't contain a complete frame yet.
fn parse_event_stream_frame(buf: &[u8]) -> Option<(String, Vec<u8>, usize)> {
    if buf.len() < 12 {
        return None; // Need at least the prelude
    }

    let total_len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let headers_len = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    // bytes 8..12 = prelude CRC (skip)

    if buf.len() < total_len {
        return None; // Incomplete frame
    }

    let headers_start = 12;
    let headers_end = headers_start + headers_len;
    let payload_end = total_len - 4; // last 4 bytes = message CRC

    if headers_end > payload_end || payload_end > buf.len() {
        return None;
    }

    let headers = parse_headers(&buf[headers_start..headers_end]);
    let payload = buf[headers_end..payload_end].to_vec();

    let event_type = headers
        .iter()
        .find(|h| h.name == ":event-type" || h.name == ":exception-type")
        .map(|h| h.value.clone())
        .unwrap_or_default();

    Some((event_type, payload, total_len))
}

/// Process a decoded Anthropic JSON event into zero or more `StreamEvent`s.
///
/// This mirrors the SSE handler in `anthropic::streaming` but operates on
/// already-parsed JSON payloads from the Bedrock binary stream.
fn process_anthropic_event(
    event_type: &str,
    json_data: &str,
    state: &mut StreamState,
) -> Result<Vec<StreamEvent>, LlmError> {
    let mut events = Vec::new();

    match event_type {
        "message_start" => {
            let payload: MessageStartPayload = serde_json::from_str(json_data)
                .map_err(|e| LlmError::Deserialization(format!("message_start: {e}")))?;
            state.message_id = Some(payload.message.id);
            state.model = Some(payload.message.model);
            if let Some(usage) = payload.message.usage {
                events.push(StreamEvent::Usage(Usage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_creation_input_tokens: usage.cache_creation_input_tokens,
                    cache_read_input_tokens: usage.cache_read_input_tokens,
                }));
            }
        }

        "content_block_start" => {
            let payload: ContentBlockStartPayload = serde_json::from_str(json_data)
                .map_err(|e| LlmError::Deserialization(format!("content_block_start: {e}")))?;
            if let AnthropicContentBlock::ToolUse {
                ref id, ref name, ..
            } = payload.content_block
            {
                state.tool_input_buffers.insert(
                    payload.index,
                    ToolUseAccumulator {
                        id: id.clone(),
                        name: name.clone(),
                        json_buffer: String::new(),
                    },
                );
            }
            events.push(StreamEvent::ContentBlockStart {
                index: payload.index,
                content_type: payload.content_block.type_name().to_string(),
            });
        }

        "content_block_delta" => {
            let payload: ContentBlockDeltaPayload = serde_json::from_str(json_data)
                .map_err(|e| LlmError::Deserialization(format!("content_block_delta: {e}")))?;
            match payload.delta {
                AnthropicDelta::TextDelta { text } => {
                    events.push(StreamEvent::TextDelta {
                        index: payload.index,
                        text,
                    });
                }
                AnthropicDelta::ThinkingDelta { thinking } => {
                    events.push(StreamEvent::ThinkingDelta {
                        index: payload.index,
                        thinking,
                    });
                }
                AnthropicDelta::InputJsonDelta { partial_json } => {
                    if let Some(acc) = state.tool_input_buffers.get_mut(&payload.index) {
                        acc.json_buffer.push_str(&partial_json);
                    }
                }
                AnthropicDelta::SignatureDelta { .. } => {
                    // Signature for thinking block verification -- skip for now
                }
            }
        }

        "content_block_stop" => {
            let payload: ContentBlockStopPayload = serde_json::from_str(json_data)
                .map_err(|e| LlmError::Deserialization(format!("content_block_stop: {e}")))?;
            if let Some(acc) = state.tool_input_buffers.remove(&payload.index) {
                let input = if acc.json_buffer.is_empty() {
                    serde_json::Value::Object(Default::default())
                } else {
                    serde_json::from_str(&acc.json_buffer)
                        .map_err(|e| LlmError::Deserialization(format!("tool input JSON: {e}")))?
                };
                events.push(StreamEvent::ToolUseComplete {
                    id: acc.id,
                    name: acc.name,
                    input,
                });
            }
            events.push(StreamEvent::ContentBlockStop {
                index: payload.index,
            });
        }

        "message_delta" => {
            let payload: MessageDeltaPayload = serde_json::from_str(json_data)
                .map_err(|e| LlmError::Deserialization(format!("message_delta: {e}")))?;
            let stop_reason = match payload.delta.stop_reason.as_deref() {
                Some("end_turn") => StopReason::EndTurn,
                Some("tool_use") => StopReason::ToolUse,
                Some("max_tokens") => StopReason::MaxTokens,
                Some("stop_sequence") => StopReason::StopSequence,
                Some("pause_turn") => StopReason::PauseTurn,
                _ => StopReason::EndTurn,
            };
            events.push(StreamEvent::Usage(Usage {
                input_tokens: payload.usage.input_tokens,
                output_tokens: payload.usage.output_tokens,
                cache_creation_input_tokens: payload.usage.cache_creation_input_tokens,
                cache_read_input_tokens: payload.usage.cache_read_input_tokens,
            }));
            events.push(StreamEvent::MessageDelta { stop_reason });
        }

        "message_stop" => {
            events.push(StreamEvent::Done);
        }

        "ping" => {
            // Keepalive -- ignore
        }

        "error" => {
            let payload: ErrorPayload = serde_json::from_str(json_data)
                .map_err(|e| LlmError::Deserialization(format!("error event: {e}")))?;
            let err = match payload.error.error_type.as_str() {
                "overloaded_error" => LlmError::Overloaded(payload.error.message),
                "rate_limit_error" => LlmError::RateLimited {
                    retry_after_ms: None,
                },
                "authentication_error" => LlmError::AuthenticationFailed,
                _ => LlmError::Provider {
                    message: payload.error.message,
                },
            };
            return Err(err);
        }

        unknown => {
            tracing::warn!(
                event_type = unknown,
                "unknown Bedrock/Anthropic event type, skipping"
            );
        }
    }

    Ok(events)
}

/// Create a streaming connection to the AWS Bedrock Runtime API.
///
/// Sends the HTTP request, checks the response status, then reads the
/// binary event stream body. Each `chunk` frame's payload is base64-decoded
/// to reveal the inner Anthropic JSON event, which is then processed
/// identically to the Anthropic SSE handler.
///
/// # Arguments
///
/// * `client` - Shared reqwest HTTP client
/// * `url` - Full Bedrock Runtime URL (e.g., `.../invoke-with-response-stream`)
/// * `body` - Serialized Bedrock request
/// * `api_key` - Bearer token wrapped in SecretString
pub fn create_bedrock_stream(
    client: &reqwest::Client,
    url: &str,
    body: super::types::BedrockRequest,
    api_key: &secrecy::SecretString,
) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
    let client = client.clone();
    let url = url.to_string();
    let api_key_str = secrecy::ExposeSecret::expose_secret(api_key).to_string();

    Box::pin(async_stream::try_stream! {
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key_str))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Provider {
                message: format!("HTTP request failed: {e}"),
            })?;

        let status = response.status();
        let response = if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            tracing::warn!(status = %status, body = %error_body, "Bedrock stream API error response");
            let err = match status.as_u16() {
                401 | 403 => LlmError::Provider {
                    message: format!("Bedrock authentication failed (HTTP {status}): {error_body}"),
                },
                429 => LlmError::RateLimited { retry_after_ms: None },
                529 => LlmError::Overloaded(error_body),
                s if s >= 500 => LlmError::Provider {
                    message: format!("Bedrock server error HTTP {status}: {error_body}"),
                },
                _ => LlmError::Provider {
                    message: format!("HTTP {status}: {error_body}"),
                },
            };
            Err(err)?;
            unreachable!()
        } else {
            response
        };

        yield StreamEvent::Connected;

        let mut byte_stream = response.bytes_stream();
        let mut buffer = Vec::new();

        let mut state = StreamState {
            tool_input_buffers: HashMap::new(),
            message_id: None,
            model: None,
        };

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = chunk_result.map_err(|e| LlmError::Stream(format!("response body read: {e}")))?;
            buffer.extend_from_slice(&chunk);

            // Parse as many complete frames as possible from the buffer
            loop {
                match parse_event_stream_frame(&buffer) {
                    Some((event_type, payload, consumed)) => {
                        buffer.drain(..consumed);

                        if event_type == "chunk" {
                            // Payload is JSON: {"bytes":"<base64>"}
                            let stream_chunk: super::types::BedrockStreamChunk =
                                serde_json::from_slice(&payload)
                                    .map_err(|e| LlmError::Deserialization(format!("bedrock chunk wrapper: {e}")))?;

                            let decoded = base64::engine::general_purpose::STANDARD
                                .decode(&stream_chunk.bytes)
                                .map_err(|e| LlmError::Deserialization(format!("base64 decode: {e}")))?;

                            let json_str = String::from_utf8(decoded)
                                .map_err(|e| LlmError::Deserialization(format!("utf8 decode: {e}")))?;

                            // The decoded JSON has a "type" field indicating the Anthropic event type
                            let event_json: serde_json::Value = serde_json::from_str(&json_str)
                                .map_err(|e| LlmError::Deserialization(format!("inner json: {e}")))?;

                            let inner_type = event_json
                                .get("type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();

                            let stream_events = process_anthropic_event(&inner_type, &json_str, &mut state)?;
                            for ev in stream_events {
                                yield ev;
                            }
                        } else if event_type.is_empty() {
                            // Possible keep-alive or unknown frame, skip
                        } else {
                            tracing::debug!(event_type = %event_type, "non-chunk bedrock frame, skipping");
                        }
                    }
                    None => break, // Need more data
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_headers_single_string() {
        // Build a header: name_len=12, name=":event-type", type=7, value_len=5, value="chunk"
        let mut buf = Vec::new();
        let name = b":event-type";
        buf.push(name.len() as u8);
        buf.extend_from_slice(name);
        buf.push(7); // string type
        let value = b"chunk";
        buf.extend_from_slice(&(value.len() as u16).to_be_bytes());
        buf.extend_from_slice(value);

        let headers = parse_headers(&buf);
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].name, ":event-type");
        assert_eq!(headers[0].value, "chunk");
    }

    #[test]
    fn test_parse_event_stream_frame() {
        // Build a minimal frame with a "chunk" event type
        let mut frame = Vec::new();

        // Build headers section
        let mut headers_buf = Vec::new();
        let name = b":event-type";
        headers_buf.push(name.len() as u8);
        headers_buf.extend_from_slice(name);
        headers_buf.push(7); // string type
        let value = b"chunk";
        headers_buf.extend_from_slice(&(value.len() as u16).to_be_bytes());
        headers_buf.extend_from_slice(value);

        let payload = b"{\"bytes\":\"dGVzdA==\"}";
        let headers_len = headers_buf.len() as u32;
        let total_len = 12 + headers_buf.len() + payload.len() + 4; // prelude + headers + payload + msg_crc

        frame.extend_from_slice(&(total_len as u32).to_be_bytes()); // total length
        frame.extend_from_slice(&headers_len.to_be_bytes()); // headers length
        frame.extend_from_slice(&[0u8; 4]); // prelude CRC (dummy)
        frame.extend_from_slice(&headers_buf); // headers
        frame.extend_from_slice(payload); // payload
        frame.extend_from_slice(&[0u8; 4]); // message CRC (dummy)

        let result = parse_event_stream_frame(&frame);
        assert!(result.is_some());

        let (event_type, payload_bytes, consumed) = result.unwrap();
        assert_eq!(event_type, "chunk");
        assert_eq!(consumed, total_len);
        assert_eq!(payload_bytes, payload);
    }

    #[test]
    fn test_parse_event_stream_frame_incomplete() {
        let buf = vec![0u8; 8]; // Too short for even the prelude
        assert!(parse_event_stream_frame(&buf).is_none());
    }

    #[test]
    fn test_process_message_start() {
        let json = r#"{"type":"message_start","message":{"id":"msg_123","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"output_tokens":0}}}"#;
        let mut state = StreamState {
            tool_input_buffers: HashMap::new(),
            message_id: None,
            model: None,
        };
        let events = process_anthropic_event("message_start", json, &mut state).unwrap();
        assert_eq!(events.len(), 1);
        matches!(&events[0], StreamEvent::Usage(_));
        assert_eq!(state.message_id.as_deref(), Some("msg_123"));
    }

    #[test]
    fn test_process_text_delta() {
        let json =
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#;
        let mut state = StreamState {
            tool_input_buffers: HashMap::new(),
            message_id: None,
            model: None,
        };
        let events = process_anthropic_event("content_block_delta", json, &mut state).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::TextDelta { text, index } => {
                assert_eq!(text, "Hi");
                assert_eq!(*index, 0);
            }
            other => panic!("expected TextDelta, got {other:?}"),
        }
    }

    #[test]
    fn test_process_message_stop() {
        let json = r#"{"type":"message_stop"}"#;
        let mut state = StreamState {
            tool_input_buffers: HashMap::new(),
            message_id: None,
            model: None,
        };
        let events = process_anthropic_event("message_stop", json, &mut state).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::Done));
    }

    #[test]
    fn test_process_error_auth() {
        let json = r#"{"error":{"type":"authentication_error","message":"Invalid API key"}}"#;
        let mut state = StreamState {
            tool_input_buffers: HashMap::new(),
            message_id: None,
            model: None,
        };
        let result = process_anthropic_event("error", json, &mut state);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LlmError::AuthenticationFailed));
    }
}
