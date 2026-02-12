//! OpenAI SSE stream to [`StreamEvent`] adapter.
//!
//! Maps `async-openai`'s [`ChatCompletionResponseStream`] events to the
//! provider-agnostic [`StreamEvent`] enum defined in `boternity-types`.
//!
//! Tool call arguments arrive as partial JSON fragments across multiple
//! streaming chunks (keyed by tool call index). These are accumulated and
//! emitted as [`StreamEvent::ToolUseComplete`] when the stream finishes
//! or a finish_reason is received.

use std::collections::HashMap;
use std::pin::Pin;

use futures_util::{Stream, StreamExt};

use async_openai::types::chat::{ChatCompletionResponseStream, FinishReason};

use boternity_types::llm::{LlmError, StopReason, StreamEvent, Usage};

/// Accumulates partial JSON fragments for a tool call during streaming.
struct ToolCallAccumulator {
    id: String,
    name: String,
    json_buffer: String,
}

/// Map an async-openai [`ChatCompletionResponseStream`] to a stream of [`StreamEvent`]s.
///
/// The returned stream emits events in this order:
/// 1. `Connected` -- immediately on entry
/// 2. `TextDelta` -- for each text content chunk
/// 3. `ToolUseComplete` -- when tool call JSON is fully assembled
/// 4. `MessageDelta` -- with the stop reason when finish_reason appears
/// 5. `Usage` -- token usage (requires `stream_options.include_usage = true` on request)
/// 6. `Done` -- at the end of the stream
pub fn map_openai_stream(
    stream: ChatCompletionResponseStream,
) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
    Box::pin(async_stream::try_stream! {
        yield StreamEvent::Connected;

        let mut tool_accumulators: HashMap<u32, ToolCallAccumulator> = HashMap::new();
        let mut stream = stream;

        while let Some(result) = stream.next().await {
            let chunk = result.map_err(|e| LlmError::Stream(e.to_string()))?;

            // Process usage if present (from stream_options.include_usage = true).
            // The final chunk contains usage data with an empty choices array.
            if chunk.usage.is_some() {
                let usage = chunk.usage.as_ref().unwrap();
                yield StreamEvent::Usage(Usage {
                    input_tokens: usage.prompt_tokens,
                    output_tokens: usage.completion_tokens,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                });
            }

            // Process each choice in the chunk (typically just one).
            let choices_len = chunk.choices.len();
            for i in 0..choices_len {
                let choice = &chunk.choices[i];

                // Text content delta
                let has_content = choice.delta.content.is_some();
                if has_content {
                    let text: String = choice.delta.content.clone().unwrap();
                    if !text.is_empty() {
                        yield StreamEvent::TextDelta {
                            index: 0,
                            text,
                        };
                    }
                }

                // Tool call deltas -- accumulate fragments
                let has_tool_calls = choice.delta.tool_calls.is_some();
                if has_tool_calls {
                    let tool_calls = choice.delta.tool_calls.clone().unwrap();
                    for tc in &tool_calls {
                        let tc_index: u32 = tc.index;
                        let tc_id: String = tc.id.clone().unwrap_or_default();
                        let tc_name: String = tc
                            .function
                            .as_ref()
                            .and_then(|f| f.name.clone())
                            .unwrap_or_default();

                        let acc = tool_accumulators
                            .entry(tc_index)
                            .or_insert_with(|| ToolCallAccumulator {
                                id: tc_id.clone(),
                                name: tc_name.clone(),
                                json_buffer: String::new(),
                            });

                        // Update id/name if provided in this chunk (first chunk has them)
                        if !tc_id.is_empty() {
                            acc.id = tc_id;
                        }
                        if !tc_name.is_empty() {
                            acc.name = tc_name;
                        }
                        let func_args: String = tc
                            .function
                            .as_ref()
                            .and_then(|f| f.arguments.clone())
                            .unwrap_or_default();
                        acc.json_buffer.push_str(&func_args);
                    }
                }

                // Finish reason -- emit tool completions and message delta
                let finish_reason_opt: Option<FinishReason> = choice.finish_reason.clone();
                if let Some(finish_reason) = finish_reason_opt {
                    // If finishing with tool calls, emit accumulated tool use events
                    if matches!(finish_reason, FinishReason::ToolCalls) {
                        let mut indices: Vec<u32> = tool_accumulators.keys().copied().collect();
                        indices.sort();
                        for idx in indices {
                            if let Some(acc) = tool_accumulators.remove(&idx) {
                                let input: serde_json::Value = if acc.json_buffer.is_empty() {
                                    serde_json::Value::Object(Default::default())
                                } else {
                                    serde_json::from_str(&acc.json_buffer).map_err(|e| {
                                        LlmError::Deserialization(format!(
                                            "tool call JSON for '{}': {e}",
                                            acc.name
                                        ))
                                    })?
                                };
                                yield StreamEvent::ToolUseComplete {
                                    id: acc.id,
                                    name: acc.name,
                                    input,
                                };
                            }
                        }
                    }

                    let stop_reason: StopReason = match finish_reason {
                        FinishReason::Stop => StopReason::EndTurn,
                        FinishReason::Length => StopReason::MaxTokens,
                        FinishReason::ToolCalls => StopReason::ToolUse,
                        FinishReason::ContentFilter => StopReason::EndTurn,
                        FinishReason::FunctionCall => StopReason::ToolUse,
                    };
                    yield StreamEvent::MessageDelta { stop_reason };
                }
            }
        }

        yield StreamEvent::Done;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_call_accumulator_empty_buffer() {
        let acc = ToolCallAccumulator {
            id: "call_abc".to_string(),
            name: "search".to_string(),
            json_buffer: String::new(),
        };
        assert!(acc.json_buffer.is_empty());
    }

    #[test]
    fn test_tool_call_accumulator_json_parsing() {
        let mut acc = ToolCallAccumulator {
            id: "call_abc".to_string(),
            name: "search".to_string(),
            json_buffer: String::new(),
        };

        acc.json_buffer.push_str("{\"query\":");
        acc.json_buffer.push_str(" \"rust async\"}");

        let value: serde_json::Value = serde_json::from_str(&acc.json_buffer).unwrap();
        assert_eq!(value["query"], "rust async");
    }

    #[test]
    fn test_tool_call_accumulator_empty_parses_to_object() {
        let acc = ToolCallAccumulator {
            id: "call_abc".to_string(),
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
    fn test_finish_reason_to_stop_reason_mapping() {
        let cases = vec![
            (FinishReason::Stop, StopReason::EndTurn),
            (FinishReason::Length, StopReason::MaxTokens),
            (FinishReason::ToolCalls, StopReason::ToolUse),
            (FinishReason::ContentFilter, StopReason::EndTurn),
            (FinishReason::FunctionCall, StopReason::ToolUse),
        ];

        for (finish, expected_stop) in cases {
            let result = match finish {
                FinishReason::Stop => StopReason::EndTurn,
                FinishReason::Length => StopReason::MaxTokens,
                FinishReason::ToolCalls => StopReason::ToolUse,
                FinishReason::ContentFilter => StopReason::EndTurn,
                FinishReason::FunctionCall => StopReason::ToolUse,
            };
            assert_eq!(result, expected_stop);
        }
    }

    #[test]
    fn test_multiple_tool_accumulators() {
        let mut accumulators: HashMap<u32, ToolCallAccumulator> = HashMap::new();

        accumulators.insert(
            0,
            ToolCallAccumulator {
                id: "call_0".to_string(),
                name: "search".to_string(),
                json_buffer: String::new(),
            },
        );
        accumulators.insert(
            1,
            ToolCallAccumulator {
                id: "call_1".to_string(),
                name: "calculator".to_string(),
                json_buffer: String::new(),
            },
        );

        accumulators.get_mut(&0).unwrap().json_buffer.push_str("{\"q\":");
        accumulators.get_mut(&1).unwrap().json_buffer.push_str("{\"x\": 1}");
        accumulators.get_mut(&0).unwrap().json_buffer.push_str(" \"rust\"}");

        let acc0 = accumulators.remove(&0).unwrap();
        let val0: serde_json::Value = serde_json::from_str(&acc0.json_buffer).unwrap();
        assert_eq!(val0["q"], "rust");

        let acc1 = accumulators.remove(&1).unwrap();
        let val1: serde_json::Value = serde_json::from_str(&acc1.json_buffer).unwrap();
        assert_eq!(val1["x"], 1);
    }
}
