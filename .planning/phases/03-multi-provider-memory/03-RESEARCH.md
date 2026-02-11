# Phase 3: Multi-Provider + Memory - Research

**Researched:** 2026-02-12 (deep dive update)
**Domain:** Multi-LLM provider abstraction, fallback chains, vector memory (LanceDB), embeddings, shared memory with trust, per-bot file storage (Rust)
**Confidence:** HIGH (standard stack, provider integration), MEDIUM-HIGH (LanceDB + fastembed), LOW (Claude.ai subscription proxy -- ToS violation confirmed)

## Summary

Phase 3 extends the Phase 2 single-provider LLM abstraction to support six additional providers (OpenAI, Google Gemini, Mistral, AWS Bedrock, Claude.ai subscription, GLM 4.7), implements automatic failover with configurable fallback chains, adds long-term vector memory via LanceDB with local embeddings via fastembed, builds a shared memory layer with trust-level partitioning and provenance tracking, and adds per-bot persistent file storage.

The critical architectural insight is that **four of the six new providers (OpenAI, Gemini, Mistral, GLM 4.7) use OpenAI-compatible API formats**. This means we can build a single `OpenAiCompatibleProvider` implementation with configurable base URLs and model mappings, then specialize only where providers diverge (auth headers, streaming quirks). AWS Bedrock uses its own SDK with a completely different API. The Claude.ai subscription proxy uses OpenAI format but runs through a local Node.js proxy (claude-max-api-proxy) that wraps the Claude Code CLI -- **however, Anthropic actively enforces against this usage as of January 2026, making it a ToS-violating, unreliable path**.

For vector memory, LanceDB v0.26.2 provides a mature embedded Rust SDK that stores data in Lance columnar format. Combined with fastembed v5.9.0 for local ONNX-based embeddings (BGESmallENV15, 384 dimensions), this gives us a fully local, zero-external-dependency vector memory system. The LanceDB Rust API includes `drop_table()`, `table_names()`, and `open_table()` for full lifecycle management, and supports SQL-like filter expressions with pre-filtering for trust-partitioned queries. Concurrent reads scale well; concurrent writes to the shared memory table should be serialized through a write queue.

For fallback chains, we implement a custom `FallbackChain` that wraps `BoxLlmProvider` instances with per-provider health state. Rather than using tower-circuitbreaker as a Tower Service middleware (our providers are not Tower Services), we implement a lightweight circuit breaker state machine inspired by the `failsafe` crate's policy pattern: consecutive failure counting with configurable thresholds, exponential backoff in open state, and half-open probe requests.

**Primary recommendation:** Build a unified `OpenAiCompatibleProvider` for OpenAI/Gemini/Mistral/GLM, a dedicated `BedrockProvider` using the AWS SDK, and a `ClaudeSubscriptionProvider` marked as experimental/unsupported. Wrap all providers in a `FallbackChain` with per-provider circuit breaker state. Use LanceDB + fastembed for fully local vector memory. Use LanceDB metadata columns with pre-filter expressions for trust-level partitioning in shared memory.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `async-openai` | 0.32.4 | OpenAI-compatible API client (OpenAI, Gemini, Mistral, GLM 4.7) | De facto Rust OpenAI client. 2.6M+ downloads. Configurable base URLs via `OpenAIConfig::with_api_base()`, built-in SSE streaming via `create_stream()`, type-safe request/response. Published 2026-01-25. |
| `aws-sdk-bedrockruntime` | 1.124.0 | AWS Bedrock model invocation | Official AWS SDK. ConverseStream API provides unified streaming across Claude, Llama, Mistral, Nova, Titan models on Bedrock. |
| `aws-config` | 1.x | AWS SDK configuration and credential loading | Required by aws-sdk-bedrockruntime. Handles env vars, profiles, IAM roles, instance metadata, SSO. |
| `lancedb` | 0.26.2 | Embedded vector database for memory embeddings | Embedded (no server), Lance columnar format, IVF-PQ indexing, SQL-like filter expressions with pre/post-filtering, multi-version concurrency control. Published 2026-02-09. Handles 200M+ vectors in production. |
| `fastembed` | 5.9.0 | Local embedding model inference (ONNX runtime) | 44+ text models, local ONNX inference, no API keys. BGESmallENV15 default (384 dims). Configurable cache via `FASTEMBED_CACHE_PATH` env var or InitOptions. |
| `arrow-schema` | (match lancedb transitive) | Arrow schema definitions for LanceDB tables | Required for defining LanceDB table schemas with FixedSizeList vector columns. Version MUST match lancedb's transitive dependency. |
| `arrow-array` | (match lancedb transitive) | Arrow array types for LanceDB data ingestion | Required for creating RecordBatch data. Version MUST match lancedb's transitive dependency. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `reqwest` | 0.12.x | HTTP client (already in workspace) | Claude.ai subscription proxy health checks |
| `reqwest-eventsource` | 0.6.0 | SSE streaming (already in workspace) | Only if async-openai streaming insufficient for a provider quirk |
| `serde` / `serde_json` | 1.x (already in workspace) | Serialization | All API request/response types |
| `tokio` | 1.x (already in workspace) | Async runtime | Everything async |
| `futures-util` | 0.3.x (already in workspace) | Stream combinators | Processing async-openai ChatCompletionResponseStream |
| `secrecy` | 0.10.x (already in workspace) | API key wrapping | All provider API keys |
| `tracing` | 0.1.x (already in workspace) | Structured logging | Provider health events, failover tracing |
| `chrono` | 0.4.x (already in workspace) | Timestamps | Memory provenance, health check timestamps |
| `uuid` | 1.20.x (already in workspace) | Unique IDs | Memory entries, file storage entries |
| `sha2` | 0.10.x (already in workspace) | SHA-256 hashing | Shared memory write validation hash |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `async-openai` for all OpenAI-compat providers | Individual provider crates (gemini-rust, mistralai-client) | More maintenance, no shared code. async-openai's configurable base URL handles all OpenAI-compat providers. |
| `async-openai` for all OpenAI-compat providers | Raw `reqwest` + custom types | Full control but rebuilds SSE streaming, type safety, retry. async-openai handles all this. |
| `fastembed` (local) | OpenAI Embeddings API | Requires API key, network, costs money. Local is free, offline-capable, consistent across providers. |
| `lancedb` | `qdrant` (embedded mode) | Qdrant requires running a separate server process. LanceDB is truly embedded (in-process), no server. |
| Custom circuit breaker | `tower-circuitbreaker` as Tower middleware | Our LLM providers are NOT Tower Services, making tower-circuitbreaker awkward to integrate. A custom state machine is simpler and more direct. |
| Custom circuit breaker | `failsafe` crate | failsafe is more mature but has fewer recent updates. Our needs are simple: consecutive failure + threshold + backoff. |
| `vec-embed-store` wrapper | Direct LanceDB + fastembed | vec-embed-store abstracts Arrow types but limits control. We need custom schema for trust metadata columns. |

**Installation (workspace Cargo.toml additions):**
```toml
# New workspace dependencies for Phase 3
async-openai = "0.32"
aws-sdk-bedrockruntime = "1.124"
aws-config = "1"
lancedb = "0.26"
fastembed = "5"

# NOTE: Do NOT specify arrow-schema/arrow-array versions directly.
# Instead, after adding lancedb, run: cargo tree -p lancedb | grep arrow
# to find the exact transitive arrow version, then pin to that.
# Using mismatched arrow versions causes type incompatibilities.
```

## Architecture Patterns

### Recommended Module Structure (Phase 3 additions)
```
crates/
  boternity-types/
    src/
      llm.rs                    # Extended: add ProviderConfig, FallbackChainConfig
      memory.rs                 # Extended: add VectorMemoryEntry, SharedMemoryEntry, TrustLevel
      storage.rs                # NEW: per-bot file storage types

  boternity-core/
    src/
      llm/
        provider.rs             # Existing: LlmProvider trait (unchanged)
        box_provider.rs         # Existing: BoxLlmProvider (unchanged)
        fallback.rs             # NEW: FallbackChain wrapping multiple BoxLlmProviders
        health.rs               # NEW: ProviderHealth, CircuitState per provider
        registry.rs             # NEW: ProviderRegistry for provider lookup by name
      memory/
        store.rs                # Existing: MemoryRepository (unchanged)
        vector.rs               # NEW: VectorMemoryRepository trait
        shared.rs               # NEW: SharedMemoryRepository trait with trust partitioning
        embedder.rs             # NEW: Embedder trait (abstracts fastembed vs API embeddings)
      storage/
        mod.rs                  # NEW: re-exports
        repository.rs           # NEW: BotStorageRepository trait

  boternity-infra/
    src/
      llm/
        anthropic/              # Existing: AnthropicProvider (unchanged from Phase 2)
        openai_compat/
          mod.rs                # NEW: OpenAiCompatibleProvider
          config.rs             # NEW: provider-specific configs (Gemini, Mistral, GLM, etc.)
          streaming.rs          # NEW: OpenAI SSE stream adapter -> StreamEvent
        bedrock/
          mod.rs                # NEW: BedrockProvider
          streaming.rs          # NEW: Bedrock ConverseStream adapter -> StreamEvent
        claude_sub/
          mod.rs                # NEW: ClaudeSubscriptionProvider (experimental, ToS risk)
      vector/
        mod.rs                  # NEW: re-exports
        lance.rs                # NEW: LanceDB vector store implementation
        embedder.rs             # NEW: FastEmbedEmbedder implementing Embedder trait
        schema.rs               # NEW: Arrow schema definitions for memory tables
      storage/
        mod.rs                  # NEW: re-exports
        filesystem.rs           # NEW: Per-bot file storage on local filesystem
        metadata.rs             # NEW: SQLite metadata for file storage
```

### Pattern 1: OpenAI-Compatible Provider with Configurable Base URL
**What:** A single `OpenAiCompatibleProvider` struct that uses `async-openai` with different `OpenAIConfig` base URLs to support OpenAI, Gemini, Mistral, and GLM 4.7 -- all of which expose OpenAI-compatible `/v1/chat/completions` endpoints.
**When to use:** For any provider that speaks the OpenAI chat completions protocol.
**Why:** Avoids maintaining separate implementations for each provider. Auth header and base URL are the only differences.

```rust
// In boternity-infra/src/llm/openai_compat/mod.rs

use async_openai::{Client, config::OpenAIConfig};
use async_openai::types::{
    CreateChatCompletionRequestArgs,
    CreateChatCompletionStreamResponse,
    ChatCompletionRequestUserMessageArgs,
    ChatCompletionRequestSystemMessageArgs,
    ChatChoiceStream,
};
use boternity_core::llm::provider::LlmProvider;

/// Unified provider for any OpenAI-compatible API.
/// Supports: OpenAI, Google Gemini, Mistral, GLM 4.7, Claude.ai subscription proxy.
pub struct OpenAiCompatibleProvider {
    client: Client<OpenAIConfig>,
    provider_name: String,
    model: String,
    capabilities: ProviderCapabilities,
}

impl OpenAiCompatibleProvider {
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
}

/// Provider-specific factory functions
impl OpenAiCompatibleProvider {
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::new(OpenAiCompatConfig {
            provider_name: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: api_key.into(),
            model: model.into(),
            capabilities: ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: false,
                max_context_tokens: 128_000, // GPT-4o
                max_output_tokens: 16_384,
            },
        })
    }

    pub fn gemini(api_key: &str, model: &str) -> Self {
        // Verified: https://ai.google.dev/gemini-api/docs/openai
        Self::new(OpenAiCompatConfig {
            provider_name: "gemini".into(),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".into(),
            api_key: api_key.into(),
            model: model.into(),
            capabilities: ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: false, // Gemini has thinking but via extra_body, not standard
                max_context_tokens: 1_000_000, // Gemini 2.5 Pro
                max_output_tokens: 65_536,
            },
        })
    }

    pub fn mistral(api_key: &str, model: &str) -> Self {
        // Verified: https://docs.mistral.ai/api
        Self::new(OpenAiCompatConfig {
            provider_name: "mistral".into(),
            base_url: "https://api.mistral.ai/v1".into(),
            api_key: api_key.into(),
            model: model.into(),
            capabilities: ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,  // Pixtral models
                extended_thinking: false,
                max_context_tokens: 128_000,
                max_output_tokens: 32_768,
            },
        })
    }

    pub fn glm(api_key: &str, model: &str) -> Self {
        // Verified: https://docs.z.ai/guides/develop/http/introduction
        // Z.ai uses Bearer token auth, standard OpenAI request format
        Self::new(OpenAiCompatConfig {
            provider_name: "glm".into(),
            base_url: "https://api.z.ai/api/paas/v4".into(),
            api_key: api_key.into(),
            model: model.into(),
            capabilities: ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: false, // GLM-4.7 text-only
                extended_thinking: false,
                max_context_tokens: 200_000, // GLM-4.7
                max_output_tokens: 128_000,
            },
        })
    }

    /// EXPERIMENTAL: Claude.ai subscription via local proxy.
    /// Requires claude-max-api-proxy running at localhost:3456.
    /// WARNING: Anthropic actively enforces against this as of Jan 2026.
    pub fn claude_subscription(model: &str) -> Self {
        Self::new(OpenAiCompatConfig {
            provider_name: "claude_subscription".into(),
            base_url: "http://localhost:3456/v1".into(),
            api_key: "dummy-key".into(), // Proxy doesn't need real key
            model: model.into(),
            capabilities: ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: true,
                max_context_tokens: 200_000,
                max_output_tokens: 128_000,
            },
        })
    }
}
```

### Pattern 2: async-openai Streaming to StreamEvent Adapter (RESOLVED)
**What:** Map `async-openai`'s `CreateChatCompletionStreamResponse` to the Phase 2 `StreamEvent` enum. This is the critical bridging layer.
**When to use:** Inside `OpenAiCompatibleProvider::stream()` implementation.
**Why:** Phase 2 established `StreamEvent` as the unified streaming type. All providers must produce `StreamEvent` instances.

The OpenAI streaming chunk format (which async-openai mirrors):
```json
{"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1234,"model":"gpt-4o",
 "choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}
{"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1234,"model":"gpt-4o",
 "choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}
{"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1234,"model":"gpt-4o",
 "choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_xxx","type":"function",
  "function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}
{"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1234,"model":"gpt-4o",
 "choices":[{"index":0,"delta":{"tool_calls":[{"index":0,
  "function":{"arguments":"{\"loc"}}]},"finish_reason":null}]}
{"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1234,"model":"gpt-4o",
 "choices":[{"index":0,"delta":{},"finish_reason":"stop"}],
 "usage":{"prompt_tokens":10,"completion_tokens":20,"total_tokens":30}}
```

async-openai types mapping:
- `CreateChatCompletionStreamResponse.choices[i]` -> `ChatChoiceStream`
- `ChatChoiceStream.delta` -> has fields: `role`, `content`, `tool_calls`, `function_call` (deprecated)
- `ChatChoiceStream.finish_reason` -> `Option<FinishReason>` enum: `Stop`, `Length`, `ToolCalls`, `ContentFilter`, `FunctionCall`
- `CreateChatCompletionStreamResponse.usage` -> `Option<CompletionUsage>` (only when `stream_options.include_usage = true`)

```rust
// In boternity-infra/src/llm/openai_compat/streaming.rs

use async_openai::types::{
    CreateChatCompletionStreamResponse, ChatChoiceStream, FinishReason,
};
use std::collections::HashMap;

/// State for accumulating tool call arguments across streaming chunks.
/// Tool calls in OpenAI format arrive as partial JSON fragments across
/// multiple chunks, similar to Anthropic's input_json_delta.
struct ToolCallAccumulator {
    id: String,
    name: String,
    arguments_buffer: String,
}

/// Convert an async-openai streaming chunk to zero or more StreamEvents.
/// A single chunk can produce multiple events (e.g., content + usage).
pub fn openai_chunk_to_stream_events(
    chunk: &CreateChatCompletionStreamResponse,
    tool_accumulators: &mut HashMap<i32, ToolCallAccumulator>,
) -> Vec<Result<StreamEvent, LlmError>> {
    let mut events = Vec::new();

    for choice in &chunk.choices {
        let index = choice.index as u32;

        // Text content delta
        if let Some(ref content) = choice.delta.content {
            if !content.is_empty() {
                events.push(Ok(StreamEvent::TextDelta {
                    index,
                    text: content.clone(),
                }));
            }
        }

        // Tool call deltas (partial JSON arguments)
        if let Some(ref tool_calls) = choice.delta.tool_calls {
            for tc in tool_calls {
                let tc_index = tc.index.unwrap_or(0);

                // First chunk for this tool call: has id and function name
                if let Some(ref id) = tc.id {
                    let name = tc.function.as_ref()
                        .and_then(|f| f.name.clone())
                        .unwrap_or_default();
                    tool_accumulators.insert(tc_index, ToolCallAccumulator {
                        id: id.clone(),
                        name,
                        arguments_buffer: String::new(),
                    });
                }

                // Accumulate argument fragments
                if let Some(ref func) = tc.function {
                    if let Some(ref args) = func.arguments {
                        if let Some(acc) = tool_accumulators.get_mut(&tc_index) {
                            acc.arguments_buffer.push_str(args);
                        }
                    }
                }
            }
        }

        // Finish reason -> emit tool completions and/or done
        if let Some(ref finish_reason) = choice.finish_reason {
            // Flush all accumulated tool calls
            if matches!(finish_reason, FinishReason::ToolCalls) {
                for (_, acc) in tool_accumulators.drain() {
                    let input: serde_json::Value = if acc.arguments_buffer.is_empty() {
                        serde_json::Value::Object(Default::default())
                    } else {
                        serde_json::from_str(&acc.arguments_buffer)
                            .unwrap_or(serde_json::Value::String(acc.arguments_buffer))
                    };
                    events.push(Ok(StreamEvent::ToolUseComplete {
                        id: acc.id,
                        name: acc.name,
                        input,
                    }));
                }
            }

            let stop_reason = match finish_reason {
                FinishReason::Stop => StopReason::EndTurn,
                FinishReason::Length => StopReason::MaxTokens,
                FinishReason::ToolCalls => StopReason::ToolUse,
                FinishReason::ContentFilter => StopReason::EndTurn,
                FinishReason::FunctionCall => StopReason::ToolUse, // deprecated path
            };
            events.push(Ok(StreamEvent::MessageDelta { stop_reason }));
            events.push(Ok(StreamEvent::Done));
        }
    }

    // Token usage (appears in final chunk when stream_options.include_usage = true)
    if let Some(ref usage) = chunk.usage {
        events.push(Ok(StreamEvent::Usage(Usage {
            input_tokens: usage.prompt_tokens.unwrap_or(0) as u32,
            output_tokens: usage.completion_tokens.unwrap_or(0) as u32,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        })));
    }

    events
}
```

**Key detail for token usage in streaming:** Set `stream_options: StreamOptions { include_usage: Some(true) }` in the `CreateChatCompletionRequest`. The usage appears in the final chunk (where `choices` may be empty) as a `CompletionUsage` object with `prompt_tokens`, `completion_tokens`, `total_tokens`. Without this option, usage is NOT reported during streaming.

### Pattern 3: AWS Bedrock Provider with ConverseStream
**What:** A dedicated `BedrockProvider` using the official AWS SDK since Bedrock has its own API format (not OpenAI-compatible).
**When to use:** When the bot is configured to use an AWS Bedrock model.
**Why:** Bedrock uses a completely different request/response format with its own streaming protocol.

**Bedrock ConverseStream event types (verified against AWS docs):**
- `ContentBlockStart` -- new content block beginning
- `ContentBlockDelta` -- text delta within a block, access via `.delta().as_text()`
- `ContentBlockStop` -- content block complete
- `MessageStart` -- message metadata (role)
- `MessageStop` -- message complete, includes `stop_reason`
- `Metadata` -- token usage (`usage.input_tokens`, `usage.output_tokens`)

**Models supported via Converse/ConverseStream API (verified 2026-02-12):**
| Provider | Models | Tool Use | Vision | Streaming Tool Use |
|----------|--------|----------|--------|-------------------|
| Anthropic | Claude 3.x, 3.5, 3.7, Sonnet 4, Opus 4, Sonnet 4.5, Haiku 4.5, Opus 4.1, Opus 4.5 | Yes | Yes | Yes |
| Amazon | Nova Premier/Pro/Lite/Micro | Yes | Yes (not Micro) | Yes |
| Meta | Llama 3.1, 3.2, 4 | Yes | Yes (11b/90b/4) | Varies |
| Mistral | Large, Large 2, Small, Pixtral Large | Yes | No | No |
| Cohere | Command R, Command R+ | Yes | No | Yes |
| DeepSeek | R1 | No | No | No |
| AI21 | Jamba 1.5 Large/Mini | Yes | No | Yes |

```rust
// In boternity-infra/src/llm/bedrock/mod.rs

use aws_sdk_bedrockruntime::Client as BedrockClient;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, Message as BedrockMessage,
    ConverseStreamOutput,
};

pub struct BedrockProvider {
    client: BedrockClient,
    model_id: String,
    region: String,
    capabilities: ProviderCapabilities,
}

impl BedrockProvider {
    pub async fn new(model_id: &str, region: Option<&str>) -> Result<Self, LlmError> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = BedrockClient::new(&config);

        // Capabilities vary by model; this is a reasonable default for Claude on Bedrock
        Ok(Self {
            client,
            model_id: model_id.to_string(),
            region: region.unwrap_or("us-east-1").to_string(),
            capabilities: Self::capabilities_for_model(model_id),
        })
    }

    fn capabilities_for_model(model_id: &str) -> ProviderCapabilities {
        // Model IDs follow pattern: provider.model-version
        if model_id.starts_with("anthropic.claude") {
            ProviderCapabilities {
                streaming: true, tool_calling: true, vision: true,
                extended_thinking: false, max_context_tokens: 200_000,
                max_output_tokens: 8_192,
            }
        } else if model_id.starts_with("amazon.nova") {
            ProviderCapabilities {
                streaming: true, tool_calling: true,
                vision: !model_id.contains("micro"),
                extended_thinking: false, max_context_tokens: 300_000,
                max_output_tokens: 5_120,
            }
        } else if model_id.starts_with("meta.llama") {
            ProviderCapabilities {
                streaming: true, tool_calling: true, vision: false,
                extended_thinking: false, max_context_tokens: 128_000,
                max_output_tokens: 4_096,
            }
        } else {
            // Conservative defaults for unknown models
            ProviderCapabilities {
                streaming: true, tool_calling: false, vision: false,
                extended_thinking: false, max_context_tokens: 32_000,
                max_output_tokens: 4_096,
            }
        }
    }
}

impl LlmProvider for BedrockProvider {
    fn name(&self) -> &str { "bedrock" }
    fn capabilities(&self) -> &ProviderCapabilities { &self.capabilities }

    fn stream(&self, request: CompletionRequest)
        -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>>
    {
        let client = self.client.clone();
        let model_id = self.model_id.clone();

        Box::pin(async_stream::try_stream! {
            let messages = convert_to_bedrock_messages(&request.messages);

            let mut builder = client.converse_stream()
                .model_id(&model_id)
                .set_messages(Some(messages));

            // Set system prompt if present
            if let Some(ref system) = request.system {
                builder = builder.system(
                    aws_sdk_bedrockruntime::types::SystemContentBlock::Text(system.clone())
                );
            }

            let response = builder.send().await
                .map_err(|e| LlmError::Provider { message: e.to_string() })?;

            let mut stream = response.stream;
            let mut block_index: u32 = 0;

            loop {
                match stream.recv().await {
                    Ok(Some(event)) => {
                        match event {
                            ConverseStreamOutput::ContentBlockStart(start) => {
                                block_index = start.content_block_index() as u32;
                                yield StreamEvent::ContentBlockStart {
                                    index: block_index,
                                    content_type: "text".to_string(),
                                };
                            }
                            ConverseStreamOutput::ContentBlockDelta(delta) => {
                                if let Some(d) = delta.delta() {
                                    if let Ok(text) = d.as_text() {
                                        yield StreamEvent::TextDelta {
                                            index: block_index,
                                            text: text.to_string(),
                                        };
                                    }
                                }
                            }
                            ConverseStreamOutput::ContentBlockStop(_) => {
                                yield StreamEvent::ContentBlockStop { index: block_index };
                            }
                            ConverseStreamOutput::MessageStop(stop) => {
                                let stop_reason = match stop.stop_reason() {
                                    Some(r) => match r.as_str() {
                                        "end_turn" => StopReason::EndTurn,
                                        "tool_use" => StopReason::ToolUse,
                                        "max_tokens" => StopReason::MaxTokens,
                                        "stop_sequence" => StopReason::StopSequence,
                                        _ => StopReason::EndTurn,
                                    },
                                    None => StopReason::EndTurn,
                                };
                                yield StreamEvent::MessageDelta { stop_reason };
                                yield StreamEvent::Done;
                            }
                            ConverseStreamOutput::Metadata(meta) => {
                                if let Some(usage) = meta.usage() {
                                    yield StreamEvent::Usage(Usage {
                                        input_tokens: usage.input_tokens() as u32,
                                        output_tokens: usage.output_tokens() as u32,
                                        cache_creation_input_tokens: None,
                                        cache_read_input_tokens: None,
                                    });
                                }
                            }
                            _ => {} // Unknown event types: skip gracefully
                        }
                    }
                    Ok(None) => break,
                    Err(e) => Err(LlmError::Stream(e.to_string()))?,
                }
            }
        })
    }

    fn complete(&self, request: &CompletionRequest)
        -> impl Future<Output = Result<CompletionResponse, LlmError>> + Send
    {
        async move {
            let messages = convert_to_bedrock_messages(&request.messages);
            let mut builder = self.client.converse()
                .model_id(&self.model_id)
                .set_messages(Some(messages));

            if let Some(ref system) = request.system {
                builder = builder.system(
                    aws_sdk_bedrockruntime::types::SystemContentBlock::Text(system.clone())
                );
            }

            let response = builder.send().await
                .map_err(|e| LlmError::Provider { message: e.to_string() })?;
            convert_bedrock_response(response)
        }
    }

    fn count_tokens(&self, _request: &CompletionRequest)
        -> impl Future<Output = Result<TokenCount, LlmError>> + Send
    {
        // Bedrock does not have a separate token counting API.
        // Token usage is reported in the Metadata event during streaming
        // and in the response for non-streaming calls.
        async move {
            Err(LlmError::InvalidRequest(
                "Bedrock reports tokens in response metadata, not via separate API".into()
            ))
        }
    }
}
```

### Pattern 4: Fallback Chain with Custom Circuit Breaker
**What:** A `FallbackChain` struct that wraps multiple `BoxLlmProvider` instances with per-provider circuit breaker state. Routes requests to the first healthy provider, automatically failing over.
**When to use:** When the bot has a configured fallback provider chain.
**Why:** Automatic failover requires health tracking per provider with circuit breaker state to prevent hammering a dead provider.

**Design decision:** We do NOT use `tower-circuitbreaker` as a Tower middleware because our LLM providers implement the `LlmProvider` trait, not the Tower `Service` trait. Wrapping them would add unnecessary complexity. Instead, we implement a simple 3-state circuit breaker state machine directly, inspired by `failsafe`'s approach.

```rust
// In boternity-core/src/llm/health.rs

use std::time::{Duration, Instant};

/// Three-state circuit breaker following standard pattern.
#[derive(Debug)]
pub enum CircuitState {
    /// Normal operation: requests pass through.
    Closed {
        consecutive_failures: u32,
    },
    /// Failing fast: all requests rejected without calling provider.
    Open {
        opened_at: Instant,
        wait_duration: Duration,
    },
    /// Testing: allowing one probe request to check recovery.
    HalfOpen,
}

#[derive(Debug)]
pub struct ProviderHealth {
    pub name: String,
    pub state: CircuitState,
    pub last_error: Option<String>,
    pub last_success: Option<Instant>,
    pub total_calls: u64,
    pub total_failures: u64,
    // Configuration
    pub failure_threshold: u32,     // consecutive failures before opening (default: 3)
    pub success_threshold: u32,     // consecutive successes in half-open before closing (default: 1)
    pub open_duration: Duration,    // time to wait before half-open (default: 30s)
    half_open_successes: u32,
}

impl ProviderHealth {
    pub fn new(name: String, config: &CircuitBreakerConfig) -> Self {
        Self {
            name,
            state: CircuitState::Closed { consecutive_failures: 0 },
            last_error: None,
            last_success: None,
            total_calls: 0,
            total_failures: 0,
            failure_threshold: config.failure_threshold,
            success_threshold: config.success_threshold,
            open_duration: config.open_duration,
            half_open_successes: 0,
        }
    }

    /// Can this provider accept a request right now?
    pub fn is_available(&mut self) -> bool {
        match &self.state {
            CircuitState::Closed { .. } => true,
            CircuitState::Open { opened_at, wait_duration } => {
                if opened_at.elapsed() >= *wait_duration {
                    // Transition to half-open: allow one probe
                    tracing::info!(provider = %self.name, "Circuit transitioning to half-open");
                    self.state = CircuitState::HalfOpen;
                    self.half_open_successes = 0;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true, // Allow probe requests
        }
    }

    pub fn record_success(&mut self) {
        self.total_calls += 1;
        self.last_success = Some(Instant::now());

        match &self.state {
            CircuitState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= self.success_threshold {
                    tracing::info!(provider = %self.name, "Circuit closing (recovered)");
                    self.state = CircuitState::Closed { consecutive_failures: 0 };
                }
            }
            _ => {
                self.state = CircuitState::Closed { consecutive_failures: 0 };
            }
        }
    }

    pub fn record_failure(&mut self, error: &LlmError) {
        self.total_calls += 1;
        self.total_failures += 1;
        self.last_error = Some(error.to_string());

        match &mut self.state {
            CircuitState::Closed { consecutive_failures } => {
                *consecutive_failures += 1;
                if *consecutive_failures >= self.failure_threshold {
                    tracing::warn!(
                        provider = %self.name,
                        failures = *consecutive_failures,
                        "Circuit opening (threshold reached)"
                    );
                    self.state = CircuitState::Open {
                        opened_at: Instant::now(),
                        wait_duration: self.open_duration,
                    };
                }
            }
            CircuitState::HalfOpen => {
                // Probe failed: back to open with longer wait
                tracing::warn!(provider = %self.name, "Half-open probe failed, reopening");
                self.state = CircuitState::Open {
                    opened_at: Instant::now(),
                    wait_duration: self.open_duration * 2, // Exponential backoff
                };
            }
            CircuitState::Open { .. } => {} // Already open
        }
    }

    /// Should we fail over based on this error type?
    /// Not all errors warrant failover (e.g., auth errors won't succeed on retry).
    pub fn is_failover_error(error: &LlmError) -> bool {
        matches!(error,
            LlmError::Provider { .. } |
            LlmError::Stream(_) |
            LlmError::RateLimited { .. } |
            LlmError::Overloaded(_)
        )
    }
}
```

```rust
// In boternity-core/src/llm/fallback.rs

pub struct FallbackChain {
    providers: Vec<(ProviderHealth, BoxLlmProvider)>,
}

impl FallbackChain {
    pub fn new(
        providers: Vec<BoxLlmProvider>,
        config: FallbackConfig,
    ) -> Self {
        let providers = providers.into_iter().map(|p| {
            let health = ProviderHealth::new(
                p.name().to_string(),
                &config.circuit_breaker,
            );
            (health, p)
        }).collect();
        Self { providers }
    }

    /// Returns (response, provider_name_used).
    /// Tries providers in order, skipping those with open circuits.
    pub async fn complete(&mut self, request: &CompletionRequest)
        -> Result<(CompletionResponse, String), LlmError>
    {
        let mut last_error = None;

        for (health, provider) in &mut self.providers {
            if !health.is_available() {
                tracing::debug!(provider = %health.name, "Skipping (circuit open)");
                continue;
            }

            match provider.complete(request).await {
                Ok(response) => {
                    health.record_success();
                    return Ok((response, health.name.clone()));
                }
                Err(e) => {
                    if ProviderHealth::is_failover_error(&e) {
                        tracing::warn!(
                            provider = %health.name,
                            error = %e,
                            "Provider failed, trying next in chain"
                        );
                        health.record_failure(&e);
                        last_error = Some(e);
                    } else {
                        // Non-failover errors (auth, invalid request) propagate immediately
                        return Err(e);
                    }
                }
            }
        }

        Err(last_error.unwrap_or(LlmError::Provider {
            message: "All providers in fallback chain are unavailable".into(),
        }))
    }

    /// Stream from first available provider.
    /// Returns (stream, provider_name_used).
    pub async fn stream(&mut self, request: CompletionRequest)
        -> Result<(Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>, String), LlmError>
    {
        // For streaming, we can only detect failure at connection time.
        // Once the stream starts producing events, we cannot fail over mid-stream.
        // The circuit breaker state is updated based on whether the stream
        // connects successfully (first event) or fails to connect.
        let mut last_error = None;

        for (health, provider) in &mut self.providers {
            if !health.is_available() {
                continue;
            }

            // Try to initiate the stream
            let stream = provider.stream(request.clone());
            // We wrap the stream to detect early failures and update health
            health.record_success(); // Optimistic; stream wrapper handles failures
            return Ok((stream, health.name.clone()));
        }

        Err(last_error.unwrap_or(LlmError::Provider {
            message: "All providers in fallback chain are unavailable".into(),
        }))
    }

    /// Get health status of all providers (for monitoring/display).
    pub fn health_status(&self) -> Vec<(&str, bool)> {
        self.providers.iter().map(|(h, _)| {
            (h.name.as_str(), matches!(h.state, CircuitState::Closed { .. }))
        }).collect()
    }
}
```

### Pattern 5: LanceDB Vector Memory with fastembed (Deep Dive)
**What:** Use LanceDB as an embedded vector store with fastembed for local embedding generation. Full lifecycle: create, query, delete, compact.
**When to use:** For all long-term vector memory operations (MEMO-02, MEMO-03).

**LanceDB Rust API (verified against docs.rs and official docs):**
- `lancedb::connect(path).execute().await` -- connect to local database
- `connection.create_table(name, data).execute().await` -- create new table
- `connection.open_table(name).execute().await` -- open existing table
- `connection.drop_table(name).execute().await` -- PERMANENTLY delete table (irreversible)
- `connection.table_names().execute().await` -- list all table names
- `table.query().nearest_to(vector).limit(n).execute().await` -- vector search
- `table.query().nearest_to(vector).filter("sql expression").limit(n).execute().await` -- filtered vector search
- `table.add(data).execute().await` -- add records to table
- `table.create_index(&["vector"], Index::Auto).execute().await` -- create vector index

**Distance metrics:** L2 (default), Cosine (best for text embeddings), Dot, Hamming
**Pre-filtering vs post-filtering:** Pre-filter (default) narrows search space first, better for metadata-heavy queries. Post-filter searches all vectors first, better for pure similarity.
**Index threshold:** Not needed for < 100K records. IVF-PQ with ~50 probes + refine_factor=50 achieves > 0.95 recall at < 10ms.

**fastembed cache (verified):**
- Default cache: `fastembed_cache` in system temp directory
- Override: Set `FASTEMBED_CACHE_PATH` env var, or use `InitOptions { cache_dir: PathBuf::from("..."), .. }`
- BGESmallENV15 model size: ~23MB ONNX file (downloads once, works offline after)
- Quantized variant BGESmallENV15Q: even smaller, slightly less accurate

```rust
// In boternity-infra/src/vector/lance.rs

use lancedb::{connect, Connection, Table as LanceTable};
use arrow_schema::{Schema, Field, DataType};
use arrow_array::{
    RecordBatch, RecordBatchIterator, StringArray,
    Float32Array, FixedSizeListArray, Int32Array,
};
use std::sync::Arc;

const EMBEDDING_DIM: i32 = 384; // BGESmallENV15
const EMBEDDING_MODEL_NAME: &str = "BGESmallENV15";

pub struct LanceVectorStore {
    connection: Connection,
}

impl LanceVectorStore {
    pub async fn new(db_path: &str) -> Result<Self, VectorStoreError> {
        let connection = connect(db_path).execute().await?;
        Ok(Self { connection })
    }

    /// Create or open per-bot memory table.
    pub async fn ensure_bot_table(&self, bot_id: &str) -> Result<LanceTable, VectorStoreError> {
        let table_name = format!("bot_memory_{}", bot_id);

        match self.connection.open_table(&table_name).execute().await {
            Ok(table) => Ok(table),
            Err(_) => {
                let schema = Arc::new(self.bot_memory_schema());
                // Create with one dummy record (LanceDB requires at least one record at creation)
                // Then delete it. Alternatively, check if create_empty_table exists.
                let batch = self.create_empty_batch(&schema)?;
                let batches = RecordBatchIterator::new(
                    vec![Ok(batch)],
                    schema.clone(),
                );
                self.connection
                    .create_table(&table_name, Box::new(batches))
                    .execute()
                    .await
                    .map_err(VectorStoreError::from)
            }
        }
    }

    /// Delete a bot's entire memory table (on bot deletion).
    pub async fn delete_bot_table(&self, bot_id: &str) -> Result<(), VectorStoreError> {
        let table_name = format!("bot_memory_{}", bot_id);
        self.connection.drop_table(&table_name).execute().await?;
        Ok(())
    }

    /// List all bot memory tables.
    pub async fn list_bot_tables(&self) -> Result<Vec<String>, VectorStoreError> {
        let names = self.connection.table_names().execute().await?;
        Ok(names.into_iter()
            .filter(|n| n.starts_with("bot_memory_"))
            .collect())
    }

    /// Semantic search with optional metadata filter.
    pub async fn search(
        &self,
        table: &LanceTable,
        query_vector: &[f32],
        limit: usize,
        filter: Option<&str>,
    ) -> Result<Vec<RecordBatch>, VectorStoreError> {
        let mut query = table.query()
            .nearest_to(query_vector)?
            .distance_type(lancedb::DistanceType::Cosine) // Best for text embeddings
            .limit(limit);

        if let Some(filter_expr) = filter {
            query = query.filter(filter_expr); // SQL-like: "importance >= 3"
        }

        let results = query.execute().await?;
        // results is a stream of RecordBatch
        let batches: Vec<RecordBatch> = results.try_collect().await?;
        Ok(batches)
    }

    /// Create IVF-PQ index when table exceeds threshold.
    pub async fn create_index_if_needed(
        &self,
        table: &LanceTable,
        row_count_threshold: usize,
    ) -> Result<(), VectorStoreError> {
        let count = table.count_rows(None).await?;
        if count >= row_count_threshold {
            table.create_index(&["vector"], lancedb::index::Index::Auto)
                .execute()
                .await?;
        }
        Ok(())
    }

    fn bot_memory_schema(&self) -> Schema {
        Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("bot_id", DataType::Utf8, false),
            Field::new("fact", DataType::Utf8, false),
            Field::new("category", DataType::Utf8, false),
            Field::new("importance", DataType::Int32, false),
            Field::new("session_id", DataType::Utf8, true),
            Field::new("created_at", DataType::Utf8, false),
            Field::new("embedding_model", DataType::Utf8, false), // Track which model produced the embedding
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    EMBEDDING_DIM,
                ),
                false,
            ),
        ])
    }
}
```

### Pattern 6: Shared Memory with Trust-Level Partitioning
**What:** A single shared memory LanceDB table with metadata columns for trust level, author bot ID, and write validation. Uses LanceDB's SQL-like filter expressions with pre-filtering to enforce partitioning at query time.
**When to use:** For MEMO-03 (shared memory) and MEMO-04 (write validation).

```rust
// Shared memory schema with trust metadata
fn shared_memory_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("fact", DataType::Utf8, false),
        Field::new("category", DataType::Utf8, false),
        Field::new("importance", DataType::Int32, false),
        Field::new("author_bot_id", DataType::Utf8, false),    // Provenance: who wrote this
        Field::new("author_bot_name", DataType::Utf8, false),   // For display
        Field::new("trust_level", DataType::Utf8, false),        // "public", "trusted", "private"
        Field::new("created_at", DataType::Utf8, false),
        Field::new("write_hash", DataType::Utf8, false),         // SHA-256 for tamper detection
        Field::new("embedding_model", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                384,
            ),
            false,
        ),
    ])
}

// Trust-filtered query: Bot B reads only public memories + its own
// Pre-filter (default) applies trust filter BEFORE vector search = faster
let bot_b_id = "uuid-of-bot-b";
let filter = format!(
    "trust_level = 'public' OR author_bot_id = '{}'",
    bot_b_id
);

let results = shared_table
    .query()
    .nearest_to(&query_vector)?
    .distance_type(lancedb::DistanceType::Cosine)
    .filter(&filter) // Pre-filter: narrows search space first
    .limit(10)
    .execute()
    .await?;

// Write validation on shared memory
fn validate_and_store_shared_memory(
    entry: &SharedMemoryEntry,
    table: &LanceTable,
) -> Result<(), SharedMemoryError> {
    // 1. Compute write hash for tamper detection
    let hash = compute_write_hash(&entry.fact, &entry.author_bot_id, &entry.created_at);

    // 2. Check for duplicate facts from same author (dedup)
    let existing = table.query()
        .filter(&format!(
            "author_bot_id = '{}' AND fact = '{}'",
            entry.author_bot_id,
            entry.fact.replace('\'', "''") // SQL escape
        ))
        .execute().await?;

    if !existing.is_empty() {
        return Err(SharedMemoryError::DuplicateEntry);
    }

    // 3. Store with provenance
    store_with_embedding(table, entry, &hash).await
}
```

### Pattern 7: Provider Registry for Runtime Selection
**What:** A registry that maps provider names to factory functions, enabling runtime provider construction from bot config.
**When to use:** When creating providers from bot configuration at startup or config change.

```rust
// In boternity-core/src/llm/registry.rs

pub struct ProviderRegistry {
    factories: HashMap<String, Box<dyn ProviderFactory>>,
}

pub trait ProviderFactory: Send + Sync {
    fn create(&self, config: &ProviderConfig) -> Result<BoxLlmProvider, LlmError>;
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { factories: HashMap::new() }
    }

    pub fn register(&mut self, name: &str, factory: Box<dyn ProviderFactory>) {
        self.factories.insert(name.to_string(), factory);
    }

    pub fn create_provider(&self, config: &ProviderConfig) -> Result<BoxLlmProvider, LlmError> {
        let factory = self.factories.get(&config.provider_name)
            .ok_or_else(|| LlmError::InvalidRequest(
                format!("Unknown provider: {}", config.provider_name)
            ))?;
        factory.create(config)
    }

    pub fn create_fallback_chain(
        &self,
        configs: &[ProviderConfig],
        fallback_config: FallbackConfig,
    ) -> Result<FallbackChain, LlmError> {
        let providers: Vec<BoxLlmProvider> = configs.iter()
            .map(|c| self.create_provider(c))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(FallbackChain::new(providers, fallback_config))
    }

    /// List all registered provider names.
    pub fn available_providers(&self) -> Vec<&str> {
        self.factories.keys().map(|s| s.as_str()).collect()
    }
}
```

### Anti-Patterns to Avoid
- **Separate implementations for each OpenAI-compatible provider:** OpenAI, Gemini, Mistral, and GLM all speak OpenAI chat completions. Use one `OpenAiCompatibleProvider` with different base URLs.
- **Using raw reqwest for OpenAI-compatible providers:** `async-openai` already handles SSE parsing, retry, type safety. Don't rebuild it.
- **Running fastembed on the Tokio runtime thread:** Embedding generation is CPU-intensive ONNX inference. Always use `tokio::task::spawn_blocking` to avoid blocking the async runtime.
- **Storing embeddings in SQLite:** SQLite is not designed for vector similarity search. Use LanceDB for vectors, SQLite for relational metadata.
- **Creating one global LanceDB table for all bots:** Use per-bot tables for isolation and performance. Shared memory gets its own table with trust columns.
- **Hard-coding provider URLs:** Base URLs should come from configuration, not constants. Providers change endpoints.
- **Ignoring `stream_options.include_usage`:** Without setting `include_usage: true`, OpenAI-compatible streaming responses do NOT report token usage. Always set this.
- **Using tower-circuitbreaker as Tower middleware for LLM providers:** Our providers implement LlmProvider, not Tower Service. A simple custom state machine is cleaner than wrapping.
- **Trusting all bots equally in shared memory:** Every shared memory write must include provenance (author_bot_id) and validation (write_hash). Never allow anonymous writes.
- **Relying on claude-max-api-proxy for production use:** Anthropic actively blocks third-party programmatic access to subscriptions as of January 2026. This can break at any time.
- **Ignoring LanceDB's `embedding_model` column:** Always store which model produced the embedding. Mismatch between ingest and query models produces garbage results.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| OpenAI-compatible API client | Custom reqwest + SSE parser | `async-openai` 0.32.4 | Handles streaming (ChatCompletionResponseStream), retry, types, auth, stream_options. 2.6M+ downloads. |
| AWS Bedrock integration | Custom SigV4 signing + HTTP | `aws-sdk-bedrockruntime` 1.124.0 | Official SDK handles auth, region, ConverseStream protocol. |
| Vector similarity search | Custom cosine similarity over SQLite | `lancedb` 0.26.2 | IVF-PQ indexing, ANN search, 200M+ vectors tested. SQL-like filters with pre/post-filtering. |
| Embedding generation | Custom model loading + inference | `fastembed` 5.9.0 | ONNX runtime, 44+ models, quantized variants, configurable cache. |
| SSE stream parsing for OpenAI format | Manual HTTP chunked parsing | `async-openai` (built-in) | Handles `data: [DONE]`, reconnection, partial chunks, tool_calls accumulation. |
| Arrow RecordBatch creation | Manual byte buffer management | `arrow-array` / `arrow-schema` | Type-safe batch creation, schema validation, FixedSizeList for vectors. |
| Provider-specific auth (AWS) | Manual credential chain | `aws-config` | Handles env vars, profiles, instance metadata, SSO, credential refresh. |
| Circuit breaker state machine | No library needed | Custom 3-state machine (50 lines) | Simple enough to own. failsafe/tower-circuitbreaker are overkill for our use case. |
| Content hash for tamper detection | Custom algorithm | `sha2` (already in workspace) | SHA-256 is standard, already used in Phase 1 for SOUL.md integrity. |

**Key insight:** The biggest "don't hand-roll" win is the OpenAI-compatible provider pattern. By using `async-openai` with configurable base URLs, we get four providers (OpenAI, Gemini, Mistral, GLM) from a single implementation. The second biggest win is LanceDB -- embedding vector similarity search correctly with indexing, filtering, and concurrent access is a deep problem.

## Common Pitfalls

### Pitfall 1: Blocking Tokio Runtime with fastembed Embedding Generation
**What goes wrong:** Calling `fastembed::TextEmbedding::embed()` on the Tokio runtime thread blocks all async tasks, causing streaming to freeze and health checks to time out.
**Why it happens:** fastembed uses ONNX Runtime internally, which is CPU-intensive synchronous computation. Even for BGESmallENV15, embedding generation takes 10-50ms per batch.
**How to avoid:** Always wrap embedding calls in `tokio::task::spawn_blocking`. Create the `TextEmbedding` model once at startup and share it via `Arc<TextEmbedding>`.
**Warning signs:** Streaming responses stutter during memory storage operations. Health check timeouts when embeddings are being generated.

```rust
// WRONG: Blocks Tokio
let embeddings = model.embed(vec!["text"], None)?;

// RIGHT: Offload to blocking thread pool
let model = self.model.clone(); // Arc<TextEmbedding>
let text = text.to_string();
let embeddings = tokio::task::spawn_blocking(move || {
    model.embed(vec![&text], None)
}).await??;
```

### Pitfall 2: Embedding Model Mismatch Between Ingest and Query
**What goes wrong:** Memories stored with one model's vectors queried with a different model's vectors. Search returns garbage.
**Why it happens:** Changing the embedding model without re-embedding existing data. Different dimensions or semantic spaces.
**How to avoid:** Store `embedding_model` name in every LanceDB row. On query, validate the model matches. If the model changes, re-embed ALL existing data (migration). The `embedding_model` column in the schema is mandatory, not optional.
**Warning signs:** Semantic search returns irrelevant results after an upgrade. Vector dimension mismatch errors.

### Pitfall 3: OpenAI-Compatible Providers with Subtle API Differences
**What goes wrong:** A provider claims OpenAI compatibility but has quirks.
**Why it happens:** "OpenAI-compatible" is a loose standard.
**Specific known quirks (verified 2026-02-12):**
- **Gemini:** Still in beta for OpenAI compat. Cannot disable reasoning on Gemini 3.x models. `reasoning_effort` parameter maps differently. Safety filters may block requests unexpectedly. Batch file upload/download requires separate `genai` SDK.
- **Mistral:** Supports parallel function calling. Standard OpenAI format otherwise.
- **GLM (z.ai):** Uses `/api/paas/v4` path (not `/v1`). Supports JWT auth in addition to Bearer token. Also has dedicated `/api/coding/paas/v4` endpoint for the coding plan.
- **Claude subscription proxy:** Depends on Claude Code CLI being authenticated. Maps only Opus 4, Sonnet 4, Haiku 4 models.
**How to avoid:** Write provider-specific integration tests. Store per-provider quirk config in `OpenAiCompatConfig`. Test streaming with each provider individually.
**Warning signs:** Parsing errors only with specific providers. Missing fields in streaming responses. Unexpected content filter rejections (Gemini).

### Pitfall 4: Fallback Chain Masking Persistent Provider Issues
**What goes wrong:** Primary provider is down but fallback silently routes to a secondary provider with different capabilities. Users don't realize they're on a cheaper/weaker model.
**Why it happens:** Fallback is transparent by default.
**How to avoid:** (1) Log every failover event with `tracing::warn`. (2) Include the active provider name in the stats footer. (3) Emit a visible "[FAILOVER] Using {provider}" notice in chat. (4) Track time-on-fallback as a metric. (5) Non-failover errors (auth, invalid request) should NOT trigger failover -- propagate immediately.
**Warning signs:** Users reporting degraded bot quality without clear errors. Unexpectedly high usage on secondary providers.

### Pitfall 5: Shared Memory Trust Bypass via Direct LanceDB Access
**What goes wrong:** A bug allows a bot to read another bot's private shared memories.
**Why it happens:** Trust partitioning is enforced via filter expressions at the application layer, not at the storage layer. If code bypasses the trust-aware query method, all data is accessible.
**How to avoid:** The `SharedMemoryRepository` trait MUST be the ONLY way to access the shared memory table. The raw `LanceTable` handle must never be exposed outside the implementation. Encapsulate ALL access behind the trait.
**Warning signs:** Bots referencing information they shouldn't have access to. Tests passing without filter assertions.

### Pitfall 6: LanceDB Concurrent Write Conflicts on Shared Memory Table
**What goes wrong:** Multiple bots writing to the shared memory table simultaneously cause commit failures.
**Why it happens:** LanceDB supports concurrent writes but "too many concurrent writers can lead to failing writes as there is a limited number of times a writer retries a commit" (official FAQ).
**How to avoid:** Serialize writes to the shared memory table through a `tokio::sync::mpsc` channel. A single writer task processes the queue. Per-bot tables do NOT have this issue (only one bot writes to its own table). Memory writes are not latency-critical.
**Warning signs:** Intermittent "commit failed" errors. Lost shared memories during high-activity periods.

### Pitfall 7: LanceDB Single-Record Inserts Creating Disk Fragments
**What goes wrong:** Storing one memory at a time creates many small Lance fragments, degrading read performance over time.
**Why it happens:** Each `table.add()` call creates a new data fragment on disk. LanceDB documentation explicitly warns that "single-record inserts create inefficient disk fragments."
**How to avoid:** Batch memory writes. Collect memories in a buffer and flush every N entries or every T seconds. Use LanceDB's compaction to consolidate fragments periodically. For initial implementation, batching every 10 memories or every 60 seconds is reasonable.
**Warning signs:** Gradually increasing query latency over time. Many small files in the Lance data directory.

### Pitfall 8: Claude.ai Subscription Provider -- ToS Violation (CRITICAL)
**What goes wrong:** Anthropic detects and blocks the subscription proxy. The provider stops working entirely.
**Why it happens:** As of January 2026, Anthropic explicitly prohibits "accessing the Services through automated or non-human means, whether through a bot, script, or otherwise, except when you are accessing our Services via an Anthropic API Key." They implemented technical safeguards on January 9, 2026 that actively block third-party harnesses (OpenCode, Moltbot, and similar tools were affected).
**How to avoid:** (1) Mark this provider as EXPERIMENTAL in all documentation and config. (2) NEVER make it a default or recommended provider. (3) Always require the official Anthropic API as a fallback. (4) Add clear user-facing warning when this provider is configured. (5) Be prepared to remove this provider entirely if Anthropic's enforcement tightens.
**Warning signs:** Proxy returning 403/429 errors. Claude Code CLI failing authentication. Anthropic account warnings.

### Pitfall 9: Missing stream_options for Token Usage in Streaming
**What goes wrong:** Token usage is never reported for OpenAI-compatible streaming calls.
**Why it happens:** By default, OpenAI streaming does NOT include usage information. You must explicitly set `stream_options: { include_usage: true }` in the request.
**How to avoid:** Always set `stream_options` when creating streaming requests via async-openai. The usage appears in the final chunk as a `CompletionUsage` object with `prompt_tokens`, `completion_tokens`, `total_tokens`. The `choices` array may be empty in this final usage-only chunk.
**Warning signs:** Token counts always showing 0 for streaming responses. Stats footer not showing token usage.

## Code Examples

### async-openai Streaming with Token Usage
```rust
// Source: async-openai docs + OpenAI streaming documentation
use async_openai::{Client, config::OpenAIConfig};
use async_openai::types::{
    CreateChatCompletionRequestArgs,
    ChatCompletionRequestUserMessageArgs,
    ChatCompletionStreamOptions,
};
use futures_util::StreamExt;

let config = OpenAIConfig::new()
    .with_api_key("GEMINI_API_KEY")
    .with_api_base("https://generativelanguage.googleapis.com/v1beta/openai");

let client = Client::with_config(config);

let request = CreateChatCompletionRequestArgs::default()
    .model("gemini-2.5-pro")
    .messages(vec![
        ChatCompletionRequestUserMessageArgs::default()
            .content("Hello!")
            .build()?
            .into(),
    ])
    // CRITICAL: Enable usage reporting in streaming
    .stream_options(ChatCompletionStreamOptions {
        include_usage: Some(true),
    })
    .build()?;

// Streaming -- returns ChatCompletionResponseStream (impl Stream)
let mut stream = client.chat().create_stream(request).await?;
let mut tool_accumulators = HashMap::new();

while let Some(result) = stream.next().await {
    match result {
        Ok(chunk) => {
            // chunk: CreateChatCompletionStreamResponse
            // chunk.choices: Vec<ChatChoiceStream>
            //   .delta.content: Option<String>       -- text content
            //   .delta.role: Option<Role>             -- "assistant" (first chunk only)
            //   .delta.tool_calls: Option<Vec<...>>   -- tool call deltas
            //   .finish_reason: Option<FinishReason>  -- Stop|Length|ToolCalls|ContentFilter
            // chunk.usage: Option<CompletionUsage>    -- only in final chunk
            //   .prompt_tokens, .completion_tokens, .total_tokens

            let events = openai_chunk_to_stream_events(&chunk, &mut tool_accumulators);
            for event in events {
                // Process StreamEvent...
            }
        }
        Err(e) => eprintln!("Stream error: {}", e),
    }
}
```

### LanceDB Full Lifecycle (Create, Insert, Search, Delete)
```rust
// Source: LanceDB Rust docs (docs.rs/lancedb) + official docs
use lancedb::connect;
use arrow_schema::{Schema, Field, DataType};
use arrow_array::{
    RecordBatch, RecordBatchIterator,
    StringArray, Float32Array, FixedSizeListArray,
};
use std::sync::Arc;
use futures_util::TryStreamExt; // for try_collect()

// 1. Connect to embedded LanceDB
let db = connect("data/vector_store").execute().await?;

// 2. List existing tables
let tables = db.table_names().execute().await?;
println!("Existing tables: {:?}", tables);

// 3. Define schema with 384-dim vector column
let schema = Arc::new(Schema::new(vec![
    Field::new("id", DataType::Utf8, false),
    Field::new("text", DataType::Utf8, false),
    Field::new("category", DataType::Utf8, false),
    Field::new("importance", DataType::Int32, false),
    Field::new("embedding_model", DataType::Utf8, false),
    Field::new(
        "vector",
        DataType::FixedSizeList(
            Arc::new(Field::new("item", DataType::Float32, true)),
            384,
        ),
        false,
    ),
]));

// 4. Create RecordBatch with data
let ids = StringArray::from(vec!["mem_001"]);
let texts = StringArray::from(vec!["User prefers concise responses"]);
let categories = StringArray::from(vec!["preference"]);
let importances = Int32Array::from(vec![4]);
let models = StringArray::from(vec!["BGESmallENV15"]);

// Embedding: flatten into Float32Array, wrap in FixedSizeList
let embedding_values = Float32Array::from(vec![0.1_f32; 384]);
let vectors = FixedSizeListArray::try_new_from_values(embedding_values, 384)?;

let batch = RecordBatch::try_new(schema.clone(), vec![
    Arc::new(ids),
    Arc::new(texts),
    Arc::new(categories),
    Arc::new(importances),
    Arc::new(models),
    Arc::new(vectors),
])?;

// 5. Create table
let batches = RecordBatchIterator::new(vec![Ok(batch)], schema.clone());
let table = db.create_table("bot_memory_abc123", Box::new(batches))
    .execute()
    .await?;

// 6. Vector search with metadata filter
let query_vec: Vec<f32> = vec![0.1; 384]; // From fastembed
let results = table
    .query()
    .nearest_to(&query_vec)?
    .distance_type(lancedb::DistanceType::Cosine)
    .filter("importance >= 3")          // SQL-like pre-filter
    .limit(5)
    .execute()
    .await?;

let batches: Vec<RecordBatch> = results.try_collect().await?;

// 7. Create index when table grows
table.create_index(&["vector"], lancedb::index::Index::Auto)
    .execute()
    .await?;

// 8. Drop table (IRREVERSIBLE)
db.drop_table("bot_memory_abc123").execute().await?;
```

### fastembed with Configurable Cache Directory
```rust
// Source: fastembed-rs README + docs.rs
use fastembed::{TextEmbedding, EmbeddingModel, InitOptions};
use std::path::PathBuf;

// Configure cache to Boternity data directory
let cache_dir = dirs::data_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("boternity")
    .join("models");

let model = TextEmbedding::try_new(InitOptions {
    model_name: EmbeddingModel::BGESmallENV15, // 384 dimensions, ~23MB
    show_download_progress: true,
    cache_dir,  // Store in boternity's data dir
    ..Default::default()
})?;

// Share model across async tasks via Arc
let model = Arc::new(model);

// Generate embeddings (MUST use spawn_blocking)
let model_clone = model.clone();
let texts = vec!["User prefers concise responses".to_string()];
let embeddings = tokio::task::spawn_blocking(move || {
    let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
    model_clone.embed(text_refs, None) // None = default batch size (256)
}).await??;

// embeddings[0]: Vec<f32> with exactly 384 elements
assert_eq!(embeddings[0].len(), 384);

// Quantized variant for smaller size (slightly less accurate):
// EmbeddingModel::BGESmallENV15Q
```

### Provider Configuration Types
```rust
// In boternity-types/src/llm.rs (additions for Phase 3)

/// Configuration for a single LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_name: String,      // "anthropic", "openai", "gemini", "mistral", "bedrock", "glm", "claude_subscription"
    pub model: String,              // "claude-sonnet-4-20250514", "gpt-4o", "gemini-2.5-pro", etc.
    pub api_key_secret_name: Option<String>, // Reference to secret in vault
    pub base_url: Option<String>,   // Override for OpenAI-compat providers
    pub region: Option<String>,     // For AWS Bedrock (default: "us-east-1")
    pub extra: HashMap<String, String>, // Provider-specific options
}

/// Configuration for a fallback chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackChainConfig {
    pub providers: Vec<ProviderConfig>,
    pub circuit_breaker: CircuitBreakerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,     // Consecutive failures before opening (default: 3)
    pub success_threshold: u32,     // Successes in half-open before closing (default: 1)
    pub open_duration_secs: u64,    // Seconds to wait before half-open (default: 30)
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            success_threshold: 1,
            open_duration_secs: 30,
        }
    }
}
```

### Shared Memory Provenance Tracking
```rust
// In boternity-core/src/memory/shared.rs

use sha2::{Sha256, Digest};

/// Write validation: hash content for tamper detection
fn compute_write_hash(fact: &str, author_bot_id: &str, timestamp: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(fact.as_bytes());
    hasher.update(author_bot_id.as_bytes());
    hasher.update(timestamp.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Verify a shared memory entry hasn't been tampered with
fn verify_write_hash(entry: &SharedMemoryEntry) -> bool {
    let expected = compute_write_hash(
        &entry.fact,
        &entry.author_bot_id.to_string(),
        &entry.created_at.to_rfc3339(),
    );
    entry.write_hash == expected
}
```

## Provider-Specific Reference

### Provider Endpoint Summary (verified 2026-02-12)

| Provider | Type | Base URL | Auth | Streaming | Tool Calling |
|----------|------|----------|------|-----------|-------------|
| OpenAI | OpenAI-compat | `https://api.openai.com/v1` | Bearer token | SSE via `create_stream()` | Yes |
| Gemini | OpenAI-compat | `https://generativelanguage.googleapis.com/v1beta/openai` | API key as Bearer | SSE (beta) | Yes |
| Mistral | OpenAI-compat | `https://api.mistral.ai/v1` | Bearer token | SSE | Yes (parallel) |
| GLM (z.ai) | OpenAI-compat | `https://api.z.ai/api/paas/v4` | Bearer token (or JWT) | SSE | Yes |
| Bedrock | AWS SDK | N/A (regional) | AWS credentials | ConverseStream recv() | Yes (model-dependent) |
| Anthropic | Custom (Phase 2) | `https://api.anthropic.com/v1` | x-api-key header | SSE via reqwest-eventsource | Yes |
| Claude.ai sub | OpenAI-compat (proxy) | `http://localhost:3456/v1` | Dummy key | SSE | Yes |

### Per-Provider Token Usage in Streaming

| Provider | How to Get Tokens | Method |
|----------|-------------------|--------|
| OpenAI | Set `stream_options.include_usage = true` | Final chunk has `usage` field |
| Gemini | Set `stream_options.include_usage = true` | Same as OpenAI (compat mode) |
| Mistral | Set `stream_options.include_usage = true` | Same as OpenAI (compat mode) |
| GLM | Included by default in response | Check `usage` in last chunk |
| Bedrock | `Metadata` event in ConverseStream | `meta.usage().input_tokens()` / `.output_tokens()` |
| Anthropic | `message_start` and `message_delta` events | `usage.input_tokens` / `usage.output_tokens` (Phase 2) |
| Claude.ai sub | Via proxy, same as OpenAI | `stream_options.include_usage = true` |

### Rate Limit Detection per Provider

| Provider | Rate Limit Signal | How to Detect |
|----------|-------------------|---------------|
| OpenAI | HTTP 429 + `Retry-After` header | Check response status |
| Gemini | HTTP 429 | Check response status |
| Mistral | HTTP 429 + `Retry-After` header | Check response status |
| GLM | HTTP 429 | Check response status |
| Bedrock | `ThrottlingException` | Match AWS SDK error type |
| Anthropic | `rate_limit_error` in SSE stream | Match `LlmError::RateLimited` (Phase 2) |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate client libraries per LLM provider | OpenAI-compatible endpoints with configurable base URL | 2025-2026 (Gemini, Mistral adopted OpenAI compat) | One client (async-openai) serves 4+ providers |
| External vector DB server (Pinecone, Weaviate) | Embedded vector DB (LanceDB) | 2025-2026 | Zero infrastructure, in-process, local-first |
| API-based embeddings (OpenAI embeddings) | Local ONNX-based embeddings (fastembed) | 2025-2026 | Free, offline, consistent, no API dependency |
| Tower middleware for circuit breaking | Custom lightweight state machine | Current | LLM providers are not Tower Services; custom is simpler |
| AWS Bedrock InvokeModel (raw bytes) | AWS Bedrock Converse/ConverseStream API | 2024-2025 | Unified conversation API across all Bedrock models |
| Qdrant/Milvus for embedded vector search | LanceDB on Lance columnar format | 2025-2026 | True embedded (no server), Apache Arrow native, Rust-first |
| Claude.ai subscription as viable API path | Official API only (ToS enforcement) | January 2026 | Subscription proxies actively blocked by Anthropic |

**Deprecated/outdated:**
- `vectordb` crate: Renamed to `lancedb`. Old crate name deprecated.
- AWS Bedrock `invoke_model` for chat: Use `converse`/`converse_stream` instead.
- tiktoken for non-OpenAI token counting: Each provider has different tokenizers.
- Separate Gemini SDK crates: Google now provides OpenAI-compatible endpoint.
- Claude.ai subscription proxies for production use: Anthropic actively enforces against this (January 2026).
- `tower-circuitbreaker` for LLM fallback: Our providers are not Tower Services; custom state machine is simpler.

## Open Questions (Updated)

1. **Arrow version compatibility between lancedb and arrow crates** (MEDIUM confidence)
   - What we know: lancedb v0.26.2 depends on specific arrow-rs versions. Mismatched versions cause type incompatibilities at compile time.
   - What's unclear: The exact arrow version pinned by lancedb 0.26.2.
   - Recommendation: After adding `lancedb = "0.26"` to Cargo.toml, run `cargo tree -p lancedb | grep arrow` to discover the exact version. Then pin arrow-schema and arrow-array to that version. Alternatively, use lancedb's re-exported arrow types if available. This is a first-task investigation item.
   - Confidence: MEDIUM

2. **LanceDB empty table creation** (MEDIUM confidence)
   - What we know: `create_table` requires initial data (a RecordBatchIterator). There is no `create_empty_table` method documented.
   - What's unclear: Whether you can pass an empty RecordBatch or if you need at least one row.
   - Recommendation: Try creating with `RecordBatch::new_empty(schema)`. If that fails, create with one sentinel row and immediately delete it. Test during implementation.
   - Confidence: MEDIUM

3. **fastembed thread safety with Arc** (MEDIUM-HIGH confidence)
   - What we know: TextEmbedding should be `Send + Sync` for Arc sharing. ONNX Runtime supports concurrent inference.
   - What's unclear: Whether fastembed's TextEmbedding implements `Send + Sync` in practice.
   - Recommendation: Verify at compile time by wrapping in `Arc<TextEmbedding>`. If it doesn't impl Send+Sync, create a pool of models or use a dedicated embedding thread.
   - Confidence: MEDIUM-HIGH

4. **Gemini OpenAI-compat beta stability** (MEDIUM confidence)
   - What we know: Google's OpenAI-compatible endpoint is "still in beta." It supports chat completions, streaming, function calling, embeddings.
   - What's unclear: How stable the beta is. Whether breaking changes are expected. Whether all OpenAI features work correctly.
   - Recommendation: Implement with the understanding that Gemini may require provider-specific error handling. Add integration tests that run against the real API. Be prepared to handle unexpected responses gracefully.
   - Confidence: MEDIUM

## Sources

### Primary (HIGH confidence)
- [async-openai v0.32.4](https://github.com/64bit/async-openai) -- GitHub README, configurable base URLs, streaming API with ChatCompletionResponseStream. Published 2026-01-25.
- [async-openai types](https://docs.rs/async-openai/0.32.4/async_openai/types/) -- CreateChatCompletionStreamResponse, ChatChoiceStream, FinishReason types.
- [OpenAI Streaming API](https://platform.openai.com/docs/api-reference/chat-streaming) -- Chunk format, delta fields, tool_calls streaming, stream_options.include_usage for token reporting.
- [aws-sdk-bedrockruntime examples](https://docs.aws.amazon.com/sdk-for-rust/latest/dg/rust_bedrock-runtime_code_examples.html) -- Converse/ConverseStream with Rust. v1.124.0.
- [AWS Bedrock supported models](https://docs.aws.amazon.com/bedrock/latest/userguide/conversation-inference-supported-models-features.html) -- Complete model list with Converse API feature matrix. Verified 2026-02-12.
- [LanceDB v0.26.2 Rust API](https://docs.rs/lancedb/latest/lancedb/) -- Connection, Table, query, create_table, open_table, drop_table, table_names. Published 2026-02-09.
- [LanceDB FAQ](https://docs.lancedb.com/faq/faq-oss) -- Concurrent access (reads scale, writes limited), 200M+ vector scale, batch insert recommendation, auto-versioning.
- [LanceDB Vector Search](https://docs.lancedb.com/search/vector-search) -- Distance metrics (L2, Cosine, Dot, Hamming), pre/post-filtering, IVF-PQ tuning, distance_range.
- [fastembed v5.9.0](https://github.com/Anush008/fastembed-rs) -- 44+ models, BGESmallENV15 (384d), cache via FASTEMBED_CACHE_PATH, InitOptions.cache_dir. README verified.
- [Google Gemini OpenAI Compatibility](https://ai.google.dev/gemini-api/docs/openai) -- Base URL verified, supported endpoints (chat, embedding, vision, audio), beta status, known limitations, thinking parameter mapping.
- [Mistral AI API](https://docs.mistral.ai/api) -- OpenAI-compatible at `https://api.mistral.ai/v1`. Parallel function calling. Streaming SSE.
- [Z.ai (Zhipu) GLM HTTP API](https://docs.z.ai/guides/develop/http/introduction) -- Base URL: `https://api.z.ai/api/paas/v4`. Bearer token + JWT auth. GLM-5 latest model. Streaming SSE.
- [Anthropic ToS Enforcement](https://www.techbuddies.io/2026/01/12/anthropic-tightens-control-over-claude-code-access-disrupting-third-party-harnesses-and-rival-labs/) -- January 2026 crackdown on third-party subscription access. Technical safeguards implemented.
- Phase 2 RESEARCH.md -- LlmProvider trait, BoxLlmProvider pattern, StreamEvent types, established architecture.

### Secondary (MEDIUM confidence)
- [OpenAI stream_options.include_usage](https://community.openai.com/t/usage-stats-now-available-when-using-streaming-with-the-chat-completions-api-or-completions-api/738156) -- Community announcement + documentation for streaming token usage.
- [failsafe-rs](https://github.com/dmexe/failsafe-rs) -- Circuit breaker patterns: consecutive_failures, success_rate_over_time_window, backoff strategies. Pattern reference for custom implementation.
- [claude-max-api-proxy](https://docs.openclaw.ai/providers/claude-max-api-proxy) -- Community tool docs. Endpoints: /health, /v1/models, /v1/chat/completions.

### Tertiary (LOW confidence)
- [claude-max-api-proxy GitHub](https://github.com/atalovesyou/claude-max-api-proxy) -- Community tool. ToS-violating. May stop working at any time.
- GLM 4.7 Rust SDK availability -- No dedicated Rust crate found. OpenAI-compatible endpoint is the only path.

## Metadata

**Confidence breakdown:**
- Standard stack (OpenAI/Gemini/Mistral via async-openai): HIGH -- async-openai v0.32.4 verified, all providers confirmed OpenAI-compatible with specific base URLs and quirks documented
- Standard stack (Bedrock): HIGH -- Official AWS SDK with ConverseStream, model feature matrix verified
- Standard stack (LanceDB + fastembed): MEDIUM-HIGH -- Both libraries verified, table lifecycle (create/open/drop/list) confirmed, Arrow bridging pattern documented but needs compile-time validation
- Architecture (OpenAI-compat provider): HIGH -- Pattern verified against async-openai docs, streaming adapter mapped field-by-field
- Architecture (fallback chain + circuit breaker): HIGH (upgraded from MEDIUM) -- Custom 3-state machine design complete, error classification for failover vs propagation defined
- Architecture (shared memory trust): MEDIUM-HIGH -- LanceDB filter expressions with pre-filtering verified for trust partitioning, write serialization pattern defined
- Claude subscription provider: LOW -- Anthropic ToS violation confirmed, active enforcement since January 2026
- GLM 4.7: MEDIUM-HIGH -- API format confirmed OpenAI-compatible, base URL and auth verified against official docs
- Pitfalls: HIGH -- All pitfalls verified against library docs, official FAQs, and Anthropic's public enforcement actions
- Token usage in streaming: HIGH -- stream_options.include_usage mechanism verified for OpenAI-compat, Bedrock Metadata event verified

**Research date:** 2026-02-12 (deep dive)
**Previous research:** 2026-02-11 (initial)
**Valid until:** 2026-03-12 (30 days; LanceDB and async-openai update frequently but core patterns are stable)

---
*Phase 3 research for: Boternity -- Multi-Provider + Memory*
*Deep dive: all 6 open questions investigated, low-confidence areas resolved, LanceDB+fastembed deep dive, provider integration deep dive*
*Key updates from deep dive: streaming adapter mapped, Claude.ai ToS violation confirmed, circuit breaker redesigned as custom (not tower), LanceDB lifecycle API verified, fastembed cache configuration documented, provider quirks catalogued per-provider*
