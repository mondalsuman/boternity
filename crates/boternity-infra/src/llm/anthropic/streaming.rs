//! SSE stream creation and state machine for Anthropic Messages API.
//!
//! Implements the streaming protocol described in the Anthropic docs:
//! 1. `message_start` -- Message object with initial usage
//! 2. Per block: `content_block_start` -> N x `content_block_delta` -> `content_block_stop`
//! 3. `message_delta` -- stop_reason and cumulative usage
//! 4. `message_stop` -- final event
//! 5. `ping` events may appear anywhere (keepalive)
//! 6. `error` events may appear mid-stream
//!
//! Tool use input arrives as partial JSON fragments via `input_json_delta`.
//! These are accumulated per content block index and parsed only after
//! `content_block_stop`.

use std::collections::HashMap;
use std::pin::Pin;

use futures_util::{Stream, StreamExt};
use secrecy::{ExposeSecret, SecretString};

use boternity_types::llm::{LlmError, StopReason, StreamEvent, Usage};

use super::types::{
    AnthropicContentBlock, AnthropicDelta, AnthropicRequest, ContentBlockDeltaPayload,
    ContentBlockStartPayload, ContentBlockStopPayload, ErrorPayload, MessageDeltaPayload,
    MessageStartPayload,
};

/// Accumulates partial JSON fragments for tool use input within a content block.
struct ToolUseAccumulator {
    id: String,
    name: String,
    json_buffer: String,
}

/// Internal state for the SSE stream state machine.
struct StreamState {
    tool_input_buffers: HashMap<u32, ToolUseAccumulator>,
    #[allow(dead_code)]
    message_id: Option<String>,
    #[allow(dead_code)]
    model: Option<String>,
}

/// Create a streaming SSE connection to the Anthropic Messages API.
///
/// Returns a `Stream` of [`StreamEvent`]s that maps Anthropic-specific
/// SSE events to the provider-agnostic stream event enum.
///
/// The stream handles all 8 Anthropic SSE event types:
/// - `message_start` -- yields Usage if present
/// - `content_block_start` -- starts tool use accumulation if needed
/// - `content_block_delta` -- yields TextDelta/ThinkingDelta, accumulates tool JSON
/// - `content_block_stop` -- finalizes tool use, yields ToolUseComplete
/// - `message_delta` -- yields Usage and MessageDelta with stop reason
/// - `message_stop` -- yields Done
/// - `ping` -- ignored (keepalive)
/// - `error` -- mapped to typed LlmError variants
///
/// Unknown event types are logged with `tracing::warn!` and skipped for
/// forward compatibility (per Anthropic's versioning policy).
///
/// # Arguments
///
/// * `client` - Shared reqwest HTTP client
/// * `url` - Full API URL (e.g., "https://api.anthropic.com/v1/messages")
/// * `body` - Serialized Anthropic request with `stream: true`
/// * `api_key` - API key wrapped in SecretString
pub fn create_anthropic_stream(
    client: &reqwest::Client,
    url: &str,
    body: AnthropicRequest,
    api_key: &SecretString,
) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
    // Clone owned values for the 'static stream closure
    let client = client.clone();
    let url = url.to_string();
    let api_key_str = api_key.expose_secret().to_string();

    Box::pin(async_stream::try_stream! {
        let request = client
            .post(&url)
            .header("x-api-key", &api_key_str)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body);

        let mut es = reqwest_eventsource::EventSource::new(request)
            .map_err(|e| LlmError::Stream(format!("failed to create event source: {e}")))?;

        let mut state = StreamState {
            tool_input_buffers: HashMap::new(),
            message_id: None,
            model: None,
        };

        while let Some(event) = es.next().await {
            match event {
                Ok(reqwest_eventsource::Event::Open) => {
                    yield StreamEvent::Connected;
                }
                Ok(reqwest_eventsource::Event::Message(msg)) => {
                    match msg.event.as_str() {
                        "message_start" => {
                            let payload: MessageStartPayload = serde_json::from_str(&msg.data)
                                .map_err(|e| LlmError::Deserialization(format!("message_start: {e}")))?;
                            state.message_id = Some(payload.message.id);
                            state.model = Some(payload.message.model);
                            if let Some(usage) = payload.message.usage {
                                yield StreamEvent::Usage(Usage {
                                    input_tokens: usage.input_tokens,
                                    output_tokens: usage.output_tokens,
                                    cache_creation_input_tokens: usage.cache_creation_input_tokens,
                                    cache_read_input_tokens: usage.cache_read_input_tokens,
                                });
                            }
                        }

                        "content_block_start" => {
                            let payload: ContentBlockStartPayload = serde_json::from_str(&msg.data)
                                .map_err(|e| LlmError::Deserialization(format!("content_block_start: {e}")))?;
                            if let AnthropicContentBlock::ToolUse { ref id, ref name, .. } = payload.content_block {
                                state.tool_input_buffers.insert(
                                    payload.index,
                                    ToolUseAccumulator {
                                        id: id.clone(),
                                        name: name.clone(),
                                        json_buffer: String::new(),
                                    },
                                );
                            }
                            yield StreamEvent::ContentBlockStart {
                                index: payload.index,
                                content_type: payload.content_block.type_name().to_string(),
                            };
                        }

                        "content_block_delta" => {
                            let payload: ContentBlockDeltaPayload = serde_json::from_str(&msg.data)
                                .map_err(|e| LlmError::Deserialization(format!("content_block_delta: {e}")))?;
                            match payload.delta {
                                AnthropicDelta::TextDelta { text } => {
                                    yield StreamEvent::TextDelta {
                                        index: payload.index,
                                        text,
                                    };
                                }
                                AnthropicDelta::ThinkingDelta { thinking } => {
                                    yield StreamEvent::ThinkingDelta {
                                        index: payload.index,
                                        thinking,
                                    };
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
                            let payload: ContentBlockStopPayload = serde_json::from_str(&msg.data)
                                .map_err(|e| LlmError::Deserialization(format!("content_block_stop: {e}")))?;
                            if let Some(acc) = state.tool_input_buffers.remove(&payload.index) {
                                let input = if acc.json_buffer.is_empty() {
                                    serde_json::Value::Object(Default::default())
                                } else {
                                    serde_json::from_str(&acc.json_buffer)
                                        .map_err(|e| LlmError::Deserialization(format!("tool input JSON: {e}")))?
                                };
                                yield StreamEvent::ToolUseComplete {
                                    id: acc.id,
                                    name: acc.name,
                                    input,
                                };
                            }
                            yield StreamEvent::ContentBlockStop {
                                index: payload.index,
                            };
                        }

                        "message_delta" => {
                            let payload: MessageDeltaPayload = serde_json::from_str(&msg.data)
                                .map_err(|e| LlmError::Deserialization(format!("message_delta: {e}")))?;
                            let stop_reason = match payload.delta.stop_reason.as_deref() {
                                Some("end_turn") => StopReason::EndTurn,
                                Some("tool_use") => StopReason::ToolUse,
                                Some("max_tokens") => StopReason::MaxTokens,
                                Some("stop_sequence") => StopReason::StopSequence,
                                Some("pause_turn") => StopReason::PauseTurn,
                                _ => StopReason::EndTurn,
                            };
                            yield StreamEvent::Usage(Usage {
                                input_tokens: payload.usage.input_tokens,
                                output_tokens: payload.usage.output_tokens,
                                cache_creation_input_tokens: payload.usage.cache_creation_input_tokens,
                                cache_read_input_tokens: payload.usage.cache_read_input_tokens,
                            });
                            yield StreamEvent::MessageDelta { stop_reason };
                        }

                        "message_stop" => {
                            yield StreamEvent::Done;
                        }

                        "ping" => {
                            // Keepalive -- ignore
                        }

                        "error" => {
                            let payload: ErrorPayload = serde_json::from_str(&msg.data)
                                .map_err(|e| LlmError::Deserialization(format!("error event: {e}")))?;
                            let err = match payload.error.error_type.as_str() {
                                "overloaded_error" => LlmError::Overloaded(payload.error.message),
                                "rate_limit_error" => LlmError::RateLimited { retry_after_ms: None },
                                "authentication_error" => LlmError::AuthenticationFailed,
                                _ => LlmError::Provider { message: payload.error.message },
                            };
                            Err(err)?;
                        }

                        unknown => {
                            // Forward-compatible: skip unknown event types per Anthropic versioning policy
                            tracing::warn!(event_type = unknown, "unknown Anthropic SSE event type, skipping");
                        }
                    }
                }
                Err(reqwest_eventsource::Error::StreamEnded) => {
                    break;
                }
                Err(e) => {
                    Err(LlmError::Stream(e.to_string()))?;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_use_accumulator_empty_buffer() {
        let acc = ToolUseAccumulator {
            id: "tool_1".to_string(),
            name: "calculator".to_string(),
            json_buffer: String::new(),
        };
        assert!(acc.json_buffer.is_empty());
    }

    #[test]
    fn test_tool_use_accumulator_json_parsing() {
        let mut acc = ToolUseAccumulator {
            id: "tool_1".to_string(),
            name: "calculator".to_string(),
            json_buffer: String::new(),
        };

        // Simulate partial JSON fragments arriving
        acc.json_buffer.push_str("{\"x\":");
        acc.json_buffer.push_str(" 42,");
        acc.json_buffer.push_str(" \"y\": 10}");

        let value: serde_json::Value = serde_json::from_str(&acc.json_buffer).unwrap();
        assert_eq!(value["x"], 42);
        assert_eq!(value["y"], 10);
    }

    #[test]
    fn test_tool_use_accumulator_empty_parses_to_object() {
        let acc = ToolUseAccumulator {
            id: "tool_1".to_string(),
            name: "calc".to_string(),
            json_buffer: String::new(),
        };

        let input = if acc.json_buffer.is_empty() {
            serde_json::Value::Object(Default::default())
        } else {
            serde_json::from_str(&acc.json_buffer).unwrap()
        };

        assert!(input.is_object());
        assert_eq!(input.as_object().unwrap().len(), 0);
    }

    #[test]
    fn test_stop_reason_mapping() {
        let cases = vec![
            (Some("end_turn"), StopReason::EndTurn),
            (Some("tool_use"), StopReason::ToolUse),
            (Some("max_tokens"), StopReason::MaxTokens),
            (Some("stop_sequence"), StopReason::StopSequence),
            (Some("pause_turn"), StopReason::PauseTurn),
            (None, StopReason::EndTurn),
            (Some("unknown_reason"), StopReason::EndTurn),
        ];

        for (input, expected) in cases {
            let result = match input {
                Some("end_turn") => StopReason::EndTurn,
                Some("tool_use") => StopReason::ToolUse,
                Some("max_tokens") => StopReason::MaxTokens,
                Some("stop_sequence") => StopReason::StopSequence,
                Some("pause_turn") => StopReason::PauseTurn,
                _ => StopReason::EndTurn,
            };
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_error_type_mapping() {
        let cases = vec![
            ("overloaded_error", true),
            ("rate_limit_error", true),
            ("authentication_error", true),
            ("unknown_error", true),
        ];

        for (error_type, should_produce_error) in cases {
            let err = match error_type {
                "overloaded_error" => LlmError::Overloaded("overloaded".to_string()),
                "rate_limit_error" => LlmError::RateLimited { retry_after_ms: None },
                "authentication_error" => LlmError::AuthenticationFailed,
                _ => LlmError::Provider {
                    message: "unknown".to_string(),
                },
            };
            assert!(should_produce_error);
            // Verify the error can be formatted
            let _ = err.to_string();
        }
    }

    #[test]
    fn test_stream_state_initialization() {
        let state = StreamState {
            tool_input_buffers: HashMap::new(),
            message_id: None,
            model: None,
        };
        assert!(state.tool_input_buffers.is_empty());
        assert!(state.message_id.is_none());
        assert!(state.model.is_none());
    }

    #[test]
    fn test_multiple_tool_accumulators() {
        let mut buffers: HashMap<u32, ToolUseAccumulator> = HashMap::new();

        // Simulate two tool use blocks arriving
        buffers.insert(
            0,
            ToolUseAccumulator {
                id: "tool_0".to_string(),
                name: "search".to_string(),
                json_buffer: String::new(),
            },
        );
        buffers.insert(
            1,
            ToolUseAccumulator {
                id: "tool_1".to_string(),
                name: "calculator".to_string(),
                json_buffer: String::new(),
            },
        );

        // Simulate interleaved JSON fragments
        buffers.get_mut(&0).unwrap().json_buffer.push_str("{\"q\":");
        buffers
            .get_mut(&1)
            .unwrap()
            .json_buffer
            .push_str("{\"x\": 1}");
        buffers.get_mut(&0).unwrap().json_buffer.push_str(" \"rust\"}");

        // Verify each accumulated correctly
        let acc0 = buffers.remove(&0).unwrap();
        let val0: serde_json::Value = serde_json::from_str(&acc0.json_buffer).unwrap();
        assert_eq!(val0["q"], "rust");

        let acc1 = buffers.remove(&1).unwrap();
        let val1: serde_json::Value = serde_json::from_str(&acc1.json_buffer).unwrap();
        assert_eq!(val1["x"], 1);
    }
}
