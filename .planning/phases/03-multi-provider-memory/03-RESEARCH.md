# Phase 3: Multi-Provider + Memory - Research

**Researched:** 2026-02-11
**Domain:** Multi-LLM provider abstraction, fallback chains, vector memory (LanceDB), embeddings, shared memory with trust, per-bot file storage (Rust)
**Confidence:** HIGH (standard stack), MEDIUM (LanceDB Rust SDK specifics), LOW (Claude.ai subscription proxy)

## Summary

Phase 3 extends the Phase 2 single-provider LLM abstraction to support six additional providers (OpenAI, Google Gemini, Mistral, AWS Bedrock, Claude.ai subscription, GLM 4.7), implements automatic failover with configurable fallback chains, adds long-term vector memory via LanceDB with local embeddings via fastembed, builds a shared memory layer with trust-level partitioning and provenance tracking, and adds per-bot persistent file storage.

The critical architectural insight is that **four of the six new providers (OpenAI, Gemini, Mistral, GLM 4.7) use OpenAI-compatible API formats**. This means we can build a single `OpenAiCompatibleProvider` implementation with configurable base URLs and model mappings, then specialize only where providers diverge (auth headers, streaming quirks). AWS Bedrock uses its own SDK with a completely different API. The Claude.ai subscription proxy uses OpenAI format but runs through a local Node.js proxy (claude-max-api-proxy) that wraps the Claude Code CLI.

For vector memory, LanceDB v0.26.2 provides a mature embedded Rust SDK that stores data in Lance columnar format. Combined with fastembed v5 for local ONNX-based embeddings (BGESmallENV15, 384 dimensions), this gives us a fully local, zero-external-dependency vector memory system. The key challenge is bridging fastembed's embedding output with LanceDB's Arrow RecordBatch input format.

For fallback chains, the tower ecosystem provides circuit breaker middleware (`tower-circuitbreaker` v0.2.0) that integrates cleanly with the existing async Rust stack. However, since our LLM providers are not Tower Services, we implement a lightweight `FallbackChain` that wraps `BoxLlmProvider` instances with per-provider health state and circuit breaker logic, using tower-circuitbreaker's state machine internally.

**Primary recommendation:** Build a unified `OpenAiCompatibleProvider` for OpenAI/Gemini/Mistral/GLM, a dedicated `BedrockProvider` using the AWS SDK, and a `ClaudeSubscriptionProvider` that calls the local proxy. Wrap all providers in a `FallbackChain` that manages health state and automatic failover. Use LanceDB + fastembed for fully local vector memory. Use LanceDB metadata columns for trust-level partitioning in shared memory.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `async-openai` | 0.32.4 | OpenAI-compatible API client (OpenAI, Gemini, Mistral, GLM 4.7) | De facto Rust OpenAI client. 2.6M+ downloads. Configurable base URLs, streaming SSE, retry logic. Works with any OpenAI-compatible endpoint. |
| `aws-sdk-bedrockruntime` | 1.124.0 | AWS Bedrock model invocation (Claude, Llama, etc.) | Official AWS SDK. ConverseStream API for streaming. Native auth via AWS credentials. |
| `aws-config` | 1.x | AWS SDK configuration and credential loading | Required by aws-sdk-bedrockruntime for auth |
| `lancedb` | 0.26.2 | Embedded vector database for memory embeddings | Embedded (no server), Lance columnar format, IVF-PQ indexing, metadata filtering. Published 2026-02-09. |
| `fastembed` | 5.x | Local embedding model inference (ONNX runtime) | 44+ models, local ONNX inference, no API keys needed. BGESmallENV15 default (384 dims). |
| `tower-circuitbreaker` | 0.2.0 | Circuit breaker state machine for provider health | Tower-compatible, configurable failure thresholds, sliding window tracking. Published 2025-10-08. |
| `arrow-schema` | latest (transitive via lancedb) | Arrow schema definitions for LanceDB tables | Required for defining LanceDB table schemas with vector columns |
| `arrow-array` | latest (transitive via lancedb) | Arrow array types for LanceDB data ingestion | Required for creating RecordBatch data for LanceDB |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `reqwest` | 0.12.x | HTTP client (already in workspace) | Claude.ai subscription proxy calls, health checks |
| `reqwest-eventsource` | 0.6.0 | SSE streaming (already in workspace) | Claude.ai subscription proxy streaming |
| `serde` / `serde_json` | 1.x (already in workspace) | Serialization | All API request/response types |
| `tokio` | 1.x (already in workspace) | Async runtime | Everything async |
| `futures-util` | 0.3.x (already in workspace) | Stream combinators | Processing streaming responses |
| `secrecy` | 0.10.x (already in workspace) | API key wrapping | All provider API keys |
| `tracing` | 0.1.x (already in workspace) | Structured logging | Provider health events, failover tracing |
| `chrono` | 0.4.x (already in workspace) | Timestamps | Memory provenance, health check timestamps |
| `uuid` | 1.20.x (already in workspace) | Unique IDs | Memory entries, file storage entries |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `async-openai` for all OpenAI-compat providers | Individual provider crates (gemini-rust, mistralai-client) | More maintenance, no shared code. async-openai's configurable base URL handles all OpenAI-compat providers. |
| `async-openai` for all OpenAI-compat providers | Raw `reqwest` + custom types | Full control but rebuilds streaming, retry, type safety. async-openai handles SSE parsing, exponential backoff. |
| `fastembed` (local) | OpenAI Embeddings API | Requires API key, network, costs money. Local is free, offline-capable, consistent. |
| `lancedb` | `qdrant` (embedded mode) | Qdrant requires running a server. LanceDB is truly embedded, no server process. |
| `tower-circuitbreaker` | Custom circuit breaker | Well-tested state machine with sliding windows. Don't hand-roll. |
| `vec-embed-store` wrapper | Direct LanceDB + fastembed | vec-embed-store abstracts Arrow types but limits control. We need custom schema for trust metadata. |

**Installation (workspace Cargo.toml additions):**
```toml
# New workspace dependencies for Phase 3
async-openai = "0.32"
aws-sdk-bedrockruntime = "1.124"
aws-config = "1"
lancedb = "0.26"
fastembed = "5"
tower-circuitbreaker = "0.2"
arrow-schema = "54"
arrow-array = "54"
```

Note: `arrow-schema` and `arrow-array` versions should match the version transitively pulled by `lancedb`. Check `cargo tree -p lancedb | grep arrow` to confirm the correct version.

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
        health.rs               # NEW: ProviderHealth, CircuitBreakerState per provider
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
          mod.rs                # NEW: ClaudeSubscriptionProvider (proxy to claude-max-api-proxy)
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
**Why:** Avoids maintaining separate implementations for each provider. auth and base URL are the only differences.

```rust
// In boternity-infra/src/llm/openai_compat/mod.rs

use async_openai::{Client, config::OpenAIConfig};
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
        Self::new(OpenAiCompatConfig {
            provider_name: "gemini".into(),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".into(),
            api_key: api_key.into(),
            model: model.into(),
            capabilities: ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: false,
                max_context_tokens: 1_000_000, // Gemini 2.5 Pro
                max_output_tokens: 65_536,
            },
        })
    }

    pub fn mistral(api_key: &str, model: &str) -> Self {
        Self::new(OpenAiCompatConfig {
            provider_name: "mistral".into(),
            base_url: "https://api.mistral.ai/v1".into(),
            api_key: api_key.into(),
            model: model.into(),
            capabilities: ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: false,
                max_context_tokens: 128_000,
                max_output_tokens: 32_768,
            },
        })
    }

    pub fn glm(api_key: &str, model: &str) -> Self {
        Self::new(OpenAiCompatConfig {
            provider_name: "glm".into(),
            base_url: "https://api.z.ai/api/paas/v4".into(),
            api_key: api_key.into(),
            model: model.into(),
            capabilities: ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: false,
                extended_thinking: false,
                max_context_tokens: 200_000, // GLM-4.7
                max_output_tokens: 128_000,
            },
        })
    }

    pub fn claude_subscription(model: &str) -> Self {
        // Proxy runs locally, no API key needed (dummy key satisfies async-openai)
        Self::new(OpenAiCompatConfig {
            provider_name: "claude_subscription".into(),
            base_url: "http://localhost:3456/v1".into(),
            api_key: "dummy-key".into(),
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

### Pattern 2: AWS Bedrock Provider with ConverseStream
**What:** A dedicated `BedrockProvider` using the official AWS SDK since Bedrock has its own API format (not OpenAI-compatible).
**When to use:** When the bot is configured to use an AWS Bedrock model.
**Why:** Bedrock uses a completely different request/response format with its own streaming protocol.

```rust
// In boternity-infra/src/llm/bedrock/mod.rs

use aws_sdk_bedrockruntime::Client as BedrockClient;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, Message as BedrockMessage,
    ConverseStreamOutputType,
};
use boternity_core::llm::provider::LlmProvider;

pub struct BedrockProvider {
    client: BedrockClient,
    model_id: String,
    capabilities: ProviderCapabilities,
}

impl BedrockProvider {
    pub async fn new(model_id: &str) -> Result<Self, LlmError> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = BedrockClient::new(&config);

        Ok(Self {
            client,
            model_id: model_id.to_string(),
            capabilities: ProviderCapabilities {
                streaming: true,
                tool_calling: true,
                vision: true,
                extended_thinking: false,
                max_context_tokens: 200_000,
                max_output_tokens: 4_096,
            },
        })
    }
}

// The stream() implementation converts Bedrock's ConverseStreamOutputType events
// into the existing StreamEvent enum from Phase 2.
impl LlmProvider for BedrockProvider {
    fn name(&self) -> &str { "bedrock" }
    fn capabilities(&self) -> &ProviderCapabilities { &self.capabilities }

    fn complete(&self, request: &CompletionRequest)
        -> impl Future<Output = Result<CompletionResponse, LlmError>> + Send
    {
        async move {
            let messages = convert_to_bedrock_messages(&request.messages);
            let response = self.client.converse()
                .model_id(&self.model_id)
                .set_messages(Some(messages))
                .send()
                .await
                .map_err(|e| LlmError::Provider { message: e.to_string() })?;
            convert_bedrock_response(response)
        }
    }

    fn stream(&self, request: CompletionRequest)
        -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>>
    {
        let client = self.client.clone();
        let model_id = self.model_id.clone();

        Box::pin(async_stream::try_stream! {
            let messages = convert_to_bedrock_messages(&request.messages);
            let response = client.converse_stream()
                .model_id(&model_id)
                .set_messages(Some(messages))
                .send()
                .await
                .map_err(|e| LlmError::Provider { message: e.to_string() })?;

            let mut stream = response.stream;
            loop {
                match stream.recv().await {
                    Ok(Some(event)) => {
                        match event {
                            ConverseStreamOutputType::ContentBlockDelta(delta) => {
                                if let Some(d) = delta.delta() {
                                    if let Ok(text) = d.as_text() {
                                        yield StreamEvent::TextDelta { index: 0, text: text.clone() };
                                    }
                                }
                            }
                            ConverseStreamOutputType::MessageStop(_) => {
                                yield StreamEvent::Done;
                            }
                            _ => {}
                        }
                    }
                    Ok(None) => break,
                    Err(e) => Err(LlmError::Stream(e.to_string()))?,
                }
            }
        })
    }

    fn count_tokens(&self, _request: &CompletionRequest)
        -> impl Future<Output = Result<TokenCount, LlmError>> + Send
    {
        // Bedrock does not have a separate token counting API
        // Use rough estimation: ~4 chars per token
        async move {
            Err(LlmError::InvalidRequest("Bedrock does not support token counting".into()))
        }
    }
}
```

### Pattern 3: Fallback Chain with Circuit Breaker
**What:** A `FallbackChain` struct that wraps multiple `BoxLlmProvider` instances and routes requests to the first healthy provider, failing over to the next when a provider is down or rate-limited.
**When to use:** When the bot has a configured fallback provider chain.
**Why:** Automatic failover requires health tracking per provider with circuit breaker state.

```rust
// In boternity-core/src/llm/fallback.rs

use tower_circuitbreaker::CircuitBreakerConfig;

/// Per-provider health state
pub struct ProviderHealth {
    pub name: String,
    pub provider: BoxLlmProvider,
    pub circuit_state: CircuitState,
    pub last_error: Option<String>,
    pub last_success: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub failure_rate_threshold: f64,   // default 0.5
    pub min_calls_before_open: u32,    // default 5
    pub open_duration: Duration,       // default 30s
}

pub enum CircuitState {
    Closed,       // Normal operation
    Open {        // Rejecting requests
        opened_at: Instant,
        duration: Duration,
    },
    HalfOpen,     // Testing recovery
}

pub struct FallbackChain {
    providers: Vec<ProviderHealth>,
}

impl FallbackChain {
    pub fn new(providers: Vec<BoxLlmProvider>, config: FallbackConfig) -> Self {
        let providers = providers.into_iter().map(|p| {
            ProviderHealth {
                name: p.name().to_string(),
                provider: p,
                circuit_state: CircuitState::Closed,
                last_error: None,
                last_success: None,
                consecutive_failures: 0,
                failure_rate_threshold: config.failure_rate_threshold,
                min_calls_before_open: config.min_calls_before_open,
                open_duration: config.open_duration,
            }
        }).collect();

        Self { providers }
    }

    /// Try providers in order, failing over on error.
    /// Returns the response from the first healthy provider.
    pub async fn complete(&mut self, request: &CompletionRequest)
        -> Result<(CompletionResponse, &str /* provider_name */), LlmError>
    {
        let mut last_error = None;

        for health in &mut self.providers {
            if !health.is_available() {
                tracing::debug!(provider = %health.name, "Skipping provider (circuit open)");
                continue;
            }

            match health.provider.complete(request).await {
                Ok(response) => {
                    health.record_success();
                    return Ok((response, &health.name));
                }
                Err(e) => {
                    tracing::warn!(
                        provider = %health.name,
                        error = %e,
                        "Provider failed, trying next in chain"
                    );
                    health.record_failure(&e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or(LlmError::Provider {
            message: "All providers in fallback chain are unavailable".into(),
        }))
    }

    // Similar method for stream() -- try providers in order
}
```

### Pattern 4: LanceDB Vector Memory with fastembed
**What:** Use LanceDB as an embedded vector store with fastembed for local embedding generation. Each bot has its own table, shared memory has a separate table with trust metadata columns.
**When to use:** For all long-term vector memory operations (MEMO-02, MEMO-03).

```rust
// In boternity-infra/src/vector/lance.rs

use lancedb::{connect, Connection, Table as LanceTable};
use arrow_schema::{Schema, Field, DataType};
use arrow_array::{
    RecordBatch, RecordBatchIterator, StringArray,
    Float32Array, FixedSizeListArray, Int32Array,
};
use fastembed::{TextEmbedding, EmbeddingModel, InitOptions};

const EMBEDDING_DIM: i32 = 384; // BGESmallENV15

pub struct LanceVectorStore {
    connection: Connection,
    embedder: TextEmbedding,
}

impl LanceVectorStore {
    pub async fn new(db_path: &str) -> Result<Self, VectorStoreError> {
        let connection = connect(db_path).execute().await?;
        let embedder = TextEmbedding::try_new(InitOptions {
            model_name: EmbeddingModel::BGESmallENV15,
            show_download_progress: false,
            ..Default::default()
        })?;

        Ok(Self { connection, embedder })
    }

    /// Create a per-bot memory table if it doesn't exist.
    pub async fn ensure_bot_table(&self, bot_id: &str) -> Result<LanceTable, VectorStoreError> {
        let table_name = format!("bot_memory_{}", bot_id);

        let schema = Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("bot_id", DataType::Utf8, false),
            Field::new("fact", DataType::Utf8, false),
            Field::new("category", DataType::Utf8, false),
            Field::new("importance", DataType::Int32, false),
            Field::new("session_id", DataType::Utf8, true),
            Field::new("created_at", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Box::new(Field::new("item", DataType::Float32, true)).into(),
                    EMBEDDING_DIM,
                ),
                false,
            ),
        ]);

        // Try to open existing table, create if not found
        match self.connection.open_table(&table_name).execute().await {
            Ok(table) => Ok(table),
            Err(_) => {
                // Create empty table with schema
                let batch = RecordBatch::new_empty(schema.into());
                let batches = RecordBatchIterator::new(
                    vec![Ok(batch)],
                    schema.into(),
                );
                self.connection
                    .create_table(&table_name, Box::new(batches))
                    .execute()
                    .await
                    .map_err(VectorStoreError::from)
            }
        }
    }

    /// Store a memory with its embedding.
    pub async fn store_memory(
        &self,
        table: &LanceTable,
        entry: &VectorMemoryEntry,
    ) -> Result<(), VectorStoreError> {
        // Generate embedding from the fact text
        let embeddings = self.embedder.embed(vec![&entry.fact], None)?;
        let embedding = &embeddings[0]; // 384-dim vector

        // Create RecordBatch with the memory data + embedding
        let batch = create_memory_record_batch(entry, embedding)?;
        let batches = RecordBatchIterator::new(
            vec![Ok(batch)],
            table.schema().await?,
        );
        table.add(Box::new(batches)).execute().await?;
        Ok(())
    }

    /// Semantic search: find memories similar to a query.
    pub async fn search_memories(
        &self,
        table: &LanceTable,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorMemoryEntry>, VectorStoreError> {
        // Generate query embedding
        let embeddings = self.embedder.embed(vec![query], None)?;
        let query_vector: &[f32] = &embeddings[0];

        let results = table
            .query()
            .nearest_to(query_vector)?
            .limit(limit)
            .execute()
            .await?;

        // Convert Arrow RecordBatch results back to VectorMemoryEntry
        convert_results_to_entries(results).await
    }
}
```

### Pattern 5: Shared Memory with Trust-Level Partitioning
**What:** A shared memory LanceDB table with additional metadata columns for trust level, author bot ID, and write validation. Uses LanceDB's filter expressions to enforce partitioning at query time.
**When to use:** For MEMO-03 (shared memory) and MEMO-04 (write validation).

```rust
// Shared memory schema with trust metadata
let shared_schema = Schema::new(vec![
    Field::new("id", DataType::Utf8, false),
    Field::new("fact", DataType::Utf8, false),
    Field::new("category", DataType::Utf8, false),
    Field::new("importance", DataType::Int32, false),
    Field::new("author_bot_id", DataType::Utf8, false),    // Who wrote this
    Field::new("author_bot_name", DataType::Utf8, false),   // For display
    Field::new("trust_level", DataType::Utf8, false),        // "public", "trusted", "private"
    Field::new("created_at", DataType::Utf8, false),
    Field::new("write_hash", DataType::Utf8, false),         // SHA-256 of content for tamper detection
    Field::new(
        "vector",
        DataType::FixedSizeList(
            Box::new(Field::new("item", DataType::Float32, true)).into(),
            384,
        ),
        false,
    ),
]);

// Query with trust partitioning: Bot B reads only public + its own
// LanceDB supports SQL-like filter expressions
let results = shared_table
    .query()
    .nearest_to(query_vector)?
    .filter("trust_level = 'public' OR author_bot_id = '{bot_b_id}'")
    .limit(10)
    .execute()
    .await?;
```

### Pattern 6: Provider Registry for Runtime Selection
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
        let mut registry = Self { factories: HashMap::new() };
        // Register all known providers
        // Factories are registered by boternity-api at startup, injecting infra implementations
        registry
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
}
```

### Anti-Patterns to Avoid
- **Separate implementations for each OpenAI-compatible provider:** OpenAI, Gemini, Mistral, and GLM all speak OpenAI chat completions. Use one `OpenAiCompatibleProvider` with different base URLs.
- **Using raw reqwest for OpenAI-compatible providers:** `async-openai` already handles SSE parsing, retry, type safety. Don't rebuild it.
- **Running fastembed on the Tokio runtime thread:** Embedding generation is CPU-intensive ONNX inference. Always use `tokio::task::spawn_blocking` to avoid blocking the async runtime.
- **Storing embeddings in SQLite:** SQLite is not designed for vector similarity search. Use LanceDB for vectors, SQLite for relational metadata.
- **Creating one global LanceDB table for all bots:** Use per-bot tables for isolation and performance. Shared memory gets its own table with trust columns.
- **Hard-coding provider URLs:** Base URLs should come from configuration, not constants. Providers change endpoints.
- **Ignoring provider-specific streaming quirks:** Gemini and Mistral's OpenAI-compat mode may have subtle differences in SSE event format. Test each provider's streaming.
- **Trusting all bots equally in shared memory:** Every shared memory write must include provenance (author_bot_id) and validation (write_hash). Never allow anonymous writes.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| OpenAI-compatible API client | Custom reqwest + SSE parser | `async-openai` 0.32.4 | Handles streaming, retry, types, auth, rate limiting. 2.6M+ downloads. |
| AWS Bedrock integration | Custom SigV4 signing + HTTP | `aws-sdk-bedrockruntime` 1.124.0 | Official SDK handles auth, region, streaming protocol. |
| Vector similarity search | Custom cosine similarity over SQLite | `lancedb` 0.26.2 | IVF-PQ indexing, ANN search, billions of vectors. Arrow columnar format. |
| Embedding generation | Custom model loading + inference | `fastembed` 5.x | ONNX runtime, 44+ models, quantized variants, 384-dim BGESmallENV15. |
| Circuit breaker state machine | Custom health tracking | `tower-circuitbreaker` 0.2.0 | Sliding window, failure rate thresholds, half-open recovery. |
| SSE stream parsing for OpenAI format | Manual HTTP chunked parsing | `async-openai` (built-in) | Handles `data: [DONE]`, reconnection, partial chunks. |
| Arrow RecordBatch creation | Manual byte buffer management | `arrow-array` / `arrow-schema` | Type-safe batch creation, schema validation, FixedSizeList for vectors. |
| Provider-specific auth (AWS) | Manual credential chain | `aws-config` | Handles env vars, profiles, instance metadata, SSO. |

**Key insight:** The biggest "don't hand-roll" win is the OpenAI-compatible provider pattern. By using `async-openai` with configurable base URLs, we get four providers (OpenAI, Gemini, Mistral, GLM) from a single implementation. This is orders of magnitude less code and maintenance than building separate clients.

## Common Pitfalls

### Pitfall 1: Blocking Tokio Runtime with fastembed Embedding Generation
**What goes wrong:** Calling `fastembed::TextEmbedding::embed()` on the Tokio runtime thread blocks all async tasks, causing streaming to freeze and health checks to time out.
**Why it happens:** fastembed uses ONNX Runtime internally, which is CPU-intensive synchronous computation. Even for small models like BGESmallENV15, embedding generation can take 10-50ms per batch.
**How to avoid:** Always wrap embedding calls in `tokio::task::spawn_blocking`. Create the `TextEmbedding` model once and share it via `Arc`.
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
**What goes wrong:** Memories are stored with one embedding model's vectors but queried with a different model's vectors. Results are garbage.
**Why it happens:** Changing the embedding model without re-embedding existing data. Or using different models for different bots sharing memory.
**How to avoid:** Store the embedding model name and dimension in the LanceDB table metadata. Validate model consistency on every query. If the model changes, re-embed all existing data.
**Warning signs:** Semantic search returns irrelevant results. Vector dimensions mismatch errors.

### Pitfall 3: OpenAI-Compatible Providers with Subtle API Differences
**What goes wrong:** A provider claims OpenAI compatibility but has quirks -- different error formats, missing fields in streaming responses, different rate limit headers.
**Why it happens:** "OpenAI-compatible" is a loose claim. Gemini's compatibility mode was released recently and may not cover all edge cases. GLM 4.7's endpoint structure differs slightly.
**How to avoid:** Write provider-specific integration tests that exercise streaming, error handling, and edge cases. Create an adapter layer in `openai_compat/config.rs` that handles per-provider quirks.
**Warning signs:** Parsing errors only with specific providers. Missing fields in streaming responses.

### Pitfall 4: Fallback Chain Masking Persistent Provider Issues
**What goes wrong:** The primary provider is down but the fallback chain silently routes to a secondary provider with different capabilities (smaller context window, no tool calling). Users don't realize they're on a fallback.
**Why it happens:** Fallback is transparent by default. Users don't know their expensive Claude requests are being served by a cheaper model.
**How to avoid:** Log every failover event with tracing::warn. Include the active provider name in the stats footer (already shown in chat from Phase 2). Emit a user-visible notification when failover occurs. Track time-on-fallback metrics.
**Warning signs:** Users reporting degraded bot quality without clear errors.

### Pitfall 5: Shared Memory Trust Bypass via Direct LanceDB Access
**What goes wrong:** A bug or misconfiguration allows a bot to read another bot's private shared memories by querying the LanceDB table directly without trust filters.
**Why it happens:** Trust partitioning is enforced at the application layer (filter expressions), not at the storage layer. If code bypasses the trust-aware query method, all data is accessible.
**How to avoid:** Make the trust-aware query the ONLY way to access shared memory. The raw LanceDB table handle should never be exposed outside the SharedMemoryRepository. Encapsulate all access through a trait that enforces the trust filter.
**Warning signs:** Bots referencing information they shouldn't have access to.

### Pitfall 6: LanceDB Concurrent Write Conflicts
**What goes wrong:** Multiple bots writing to the shared memory table simultaneously cause commit failures.
**Why it happens:** LanceDB handles concurrent writes but excessive concurrent writers may cause commit failures due to retry limitations (per official FAQ).
**How to avoid:** Serialize writes to shared memory through a channel or mutex. Memory writes are not latency-critical, so a write queue is acceptable. Per-bot tables don't have this issue since only one bot writes to its own table.
**Warning signs:** Intermittent "commit failed" errors during high-activity periods with multiple bots.

### Pitfall 7: Claude.ai Subscription Proxy Reliability
**What goes wrong:** The claude-max-api-proxy (Node.js) crashes, hangs, or becomes unavailable, making the Claude subscription provider unreliable.
**Why it happens:** The proxy is a community tool, not officially supported. It wraps the Claude Code CLI as a subprocess. CLI updates can break it. Rate limiting from Anthropic can affect it unpredictably.
**How to avoid:** Treat the Claude subscription provider as inherently less reliable. Always place it behind a fallback chain with the official Anthropic API as fallback. Health-check the proxy's `/health` endpoint. Set short timeouts.
**Warning signs:** Intermittent failures only on the Claude subscription provider. Proxy process consuming high memory.

## Code Examples

### async-openai with Custom Base URL for Gemini
```rust
// Source: async-openai docs + Google Gemini OpenAI compat docs
use async_openai::{Client, config::OpenAIConfig};
use async_openai::types::{
    CreateChatCompletionRequestArgs, ChatCompletionRequestUserMessageArgs,
};

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
    .build()?;

// Non-streaming
let response = client.chat().create(request).await?;

// Streaming
let mut stream = client.chat().create_stream(request).await?;
while let Some(result) = stream.next().await {
    match result {
        Ok(response) => {
            for choice in &response.choices {
                if let Some(ref content) = choice.delta.content {
                    print!("{}", content);
                }
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### AWS Bedrock ConverseStream
```rust
// Source: AWS SDK for Rust docs
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, Message,
    ConverseStreamOutputType,
};

let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
let client = Client::new(&config);

let response = client
    .converse_stream()
    .model_id("anthropic.claude-3-5-sonnet-20241022-v2:0")
    .messages(
        Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text("Hello".to_string()))
            .build()
            .unwrap()
    )
    .send()
    .await?;

let mut stream = response.stream;
loop {
    match stream.recv().await {
        Ok(Some(event)) => match event {
            ConverseStreamOutputType::ContentBlockDelta(delta) => {
                if let Some(d) = delta.delta() {
                    if let Ok(text) = d.as_text() {
                        print!("{}", text);
                    }
                }
            }
            ConverseStreamOutputType::MessageStop(_) => break,
            _ => {}
        },
        Ok(None) => break,
        Err(e) => return Err(e.into()),
    }
}
```

### LanceDB Table Creation and Vector Search in Rust
```rust
// Source: LanceDB Rust docs (docs.rs/lancedb)
use lancedb::connect;
use arrow_schema::{Schema, Field, DataType};
use arrow_array::{
    RecordBatch, RecordBatchIterator,
    StringArray, Float32Array, FixedSizeListArray, Int32Array,
};
use std::sync::Arc;

// Connect to embedded LanceDB
let db = connect("data/vector_store").execute().await?;

// Define schema with 384-dim vector column
let schema = Arc::new(Schema::new(vec![
    Field::new("id", DataType::Utf8, false),
    Field::new("text", DataType::Utf8, false),
    Field::new("category", DataType::Utf8, false),
    Field::new(
        "vector",
        DataType::FixedSizeList(
            Arc::new(Field::new("item", DataType::Float32, true)),
            384, // BGESmallENV15 dimension
        ),
        false,
    ),
]));

// Create table with initial data
let ids = StringArray::from(vec!["mem_001"]);
let texts = StringArray::from(vec!["User prefers concise responses"]);
let categories = StringArray::from(vec!["preference"]);

// Flatten embedding into a Float32Array, then wrap in FixedSizeList
let embedding_values = Float32Array::from(vec![0.1_f32; 384]);
let vectors = FixedSizeListArray::try_new_from_values(embedding_values, 384)?;

let batch = RecordBatch::try_new(schema.clone(), vec![
    Arc::new(ids),
    Arc::new(texts),
    Arc::new(categories),
    Arc::new(vectors),
])?;

let batches = RecordBatchIterator::new(vec![Ok(batch)], schema.clone());
let table = db.create_table("bot_memory_abc123", Box::new(batches))
    .execute()
    .await?;

// Vector search: find similar memories
let query_vec: Vec<f32> = vec![0.1; 384]; // From fastembed
let results = table
    .query()
    .nearest_to(&query_vec)?
    .limit(5)
    .execute()
    .await?;
```

### fastembed Embedding Generation
```rust
// Source: fastembed-rs README
use fastembed::{TextEmbedding, EmbeddingModel, InitOptions};

// Initialize embedding model (downloads on first use)
let model = TextEmbedding::try_new(InitOptions {
    model_name: EmbeddingModel::BGESmallENV15, // 384 dimensions
    show_download_progress: true,
    ..Default::default()
})?;

// Generate embeddings for texts
let documents = vec![
    "User prefers concise code examples",
    "User works as a Rust developer",
];

// IMPORTANT: Run in spawn_blocking to avoid blocking Tokio
let embeddings = tokio::task::spawn_blocking(move || {
    model.embed(documents, None)
}).await??;

// embeddings[0] is Vec<f32> with 384 elements
assert_eq!(embeddings[0].len(), 384);
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

/// Trust levels for shared memory
#[derive(Debug, Clone, PartialEq)]
pub enum TrustLevel {
    /// Readable by all bots
    Public,
    /// Readable by bots in the same trust group
    Trusted,
    /// Only readable by the author bot
    Private,
}

/// Shared memory entry with provenance
pub struct SharedMemoryEntry {
    pub id: Uuid,
    pub fact: String,
    pub category: MemoryCategory,
    pub importance: u8,
    pub author_bot_id: Uuid,
    pub author_bot_name: String,
    pub trust_level: TrustLevel,
    pub created_at: DateTime<Utc>,
    pub write_hash: String,  // SHA-256 for tamper detection
    pub vector: Vec<f32>,     // Embedding
}
```

### Provider Configuration Types
```rust
// In boternity-types/src/llm.rs (additions)

/// Configuration for a single LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_name: String,      // "openai", "gemini", "mistral", "bedrock", etc.
    pub model: String,              // "gpt-4o", "gemini-2.5-pro", etc.
    pub api_key_secret_name: Option<String>, // Reference to secret in vault
    pub base_url: Option<String>,   // Override for OpenAI-compat providers
    pub region: Option<String>,     // For AWS Bedrock
    pub extra: HashMap<String, String>, // Provider-specific options
}

/// Configuration for a fallback chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackChainConfig {
    pub providers: Vec<ProviderConfig>,
    pub failure_rate_threshold: f64,    // default 0.5
    pub min_calls_before_open: u32,     // default 5
    pub open_duration_secs: u64,        // default 30
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate client libraries per LLM provider | OpenAI-compatible endpoints with configurable base URL | 2025-2026 (Gemini, Mistral adopted OpenAI compat) | One client (async-openai) serves 4+ providers |
| External vector DB server (Pinecone, Weaviate) | Embedded vector DB (LanceDB) | 2025-2026 | Zero infrastructure, in-process, local-first |
| API-based embeddings (OpenAI embeddings) | Local ONNX-based embeddings (fastembed) | 2025-2026 | Free, offline, consistent, no API dependency |
| Custom retry logic per provider | Tower middleware ecosystem (circuit breaker, retry) | 2025-2026 (tower-resilience) | Composable, tested, standard patterns |
| AWS Bedrock InvokeModel (raw bytes) | AWS Bedrock Converse/ConverseStream API | 2024-2025 | Unified conversation API across all Bedrock models |
| Qdrant/Milvus for embedded vector search | LanceDB on Lance columnar format | 2025-2026 | True embedded (no server), Apache Arrow native, Rust-first |
| Separate LLM implementations per provider | Unified OpenAI-compat abstraction | 2025-2026 | 80% less provider code, shared streaming logic |

**Deprecated/outdated:**
- `vectordb` crate: Renamed to `lancedb`. Old crate name deprecated.
- AWS Bedrock `invoke_model` for chat: Use `converse`/`converse_stream` instead (unified across models).
- tiktoken for non-OpenAI token counting: Each provider has different tokenizers. Use provider APIs or estimate.
- Separate Gemini SDK crates: Google now provides OpenAI-compatible endpoint, eliminating need for Gemini-specific clients.

## Open Questions

1. **async-openai streaming adapter to Phase 2 StreamEvent**
   - What we know: async-openai returns `CreateChatCompletionStreamResponse` objects. Phase 2 established the `StreamEvent` enum. We need to map between them.
   - What's unclear: The exact field mapping, especially for tool use streaming which differs between OpenAI and Anthropic.
   - Recommendation: Build an `openai_to_stream_event()` adapter function. Map `ChatCompletionStreamResponseDelta` -> `StreamEvent::TextDelta`. Handle `finish_reason` -> `StreamEvent::MessageDelta`. Test with tool use responses.
   - Confidence: MEDIUM

2. **fastembed model download and caching location**
   - What we know: fastembed downloads ONNX models on first use. Default cache is in the user's home directory.
   - What's unclear: Whether we should configure a Boternity-specific cache directory, how large the BGESmallENV15 model is, whether the download works offline after first use.
   - Recommendation: Configure fastembed to use `{boternity_data_dir}/models/` as cache. Test offline behavior. BGESmallENV15 is a small model (~23MB based on BAAI spec). The quantized variant (BGESmallENV15Q) is even smaller.
   - Confidence: MEDIUM

3. **LanceDB table management for bot deletion**
   - What we know: Each bot gets its own LanceDB table. When a bot is deleted, the table should be cleaned up.
   - What's unclear: Whether LanceDB supports table deletion in the Rust SDK. How to handle shared memory references to a deleted bot.
   - Recommendation: Drop the bot's LanceDB table on bot deletion. For shared memory, mark entries from deleted bots as "orphaned" but don't delete them (they may still be valuable to other bots). Verify `connection.drop_table("name")` exists in the Rust SDK.
   - Confidence: MEDIUM

4. **Claude.ai Subscription Provider Viability**
   - What we know: claude-max-api-proxy wraps Claude Code CLI as OpenAI-compatible API. It's a community tool, not officially supported. Anthropic has been enforcing ToS against using subscription tokens in third-party tools.
   - What's unclear: Whether this approach is reliable enough for production use. Whether Anthropic will shut it down. Whether it's a ToS violation.
   - Recommendation: Implement the provider but document it as "experimental" and "community-supported". Always recommend the official Anthropic API as primary. Place claude subscription behind a fallback chain with the official API. Consider it may stop working at any time.
   - Confidence: LOW

5. **Per-Provider Token Counting**
   - What we know: Anthropic has a free /count_tokens API (from Phase 2). OpenAI uses tiktoken. Other providers may not have counting APIs.
   - What's unclear: How to provide consistent token counting across all providers for the TokenBudget system.
   - Recommendation: Implement per-provider token counting where APIs exist (Anthropic, OpenAI). For providers without counting APIs (Gemini, Mistral, Bedrock, GLM), use a rough estimation (~4 chars per token). Mark estimation-based counts in the Usage struct.
   - Confidence: MEDIUM

6. **Arrow version compatibility between lancedb and arrow crates**
   - What we know: lancedb v0.26.2 depends on specific arrow-rs versions. Directly adding arrow-schema and arrow-array must match.
   - What's unclear: The exact arrow version that lancedb 0.26.2 requires.
   - Recommendation: Don't specify arrow crate versions directly in workspace Cargo.toml. Instead, re-export arrow types from lancedb if possible, or check `cargo tree -p lancedb` to determine the correct versions. Use `lancedb` feature flags to pull in the right arrow versions.
   - Confidence: MEDIUM

## Sources

### Primary (HIGH confidence)
- [async-openai v0.32.4](https://github.com/64bit/async-openai) -- GitHub README, configurable base URLs, OpenAI-compatible provider support. Published 2026-01-25.
- [aws-sdk-bedrockruntime](https://docs.aws.amazon.com/sdk-for-rust/latest/dg/rust_bedrock-runtime_code_examples.html) -- Official AWS examples with Converse/ConverseStream API. v1.124.0.
- [LanceDB v0.26.2](https://docs.rs/lancedb/latest/lancedb/) -- Rust API docs: Connection, Table, vector search, embedding modules. Published 2026-02-09.
- [fastembed v5](https://github.com/Anush008/fastembed-rs) -- 44+ embedding models, ONNX runtime, BGESmallENV15 default. README verified.
- [Google Gemini OpenAI Compatibility](https://ai.google.dev/gemini-api/docs/openai) -- Base URL: `https://generativelanguage.googleapis.com/v1beta/openai/`. Chat completions + embeddings.
- [Mistral AI API](https://docs.mistral.ai/api/endpoint/chat) -- OpenAI-compatible at `https://api.mistral.ai/v1/chat/completions`.
- [Z.ai (Zhipu) GLM API](https://docs.z.ai/guides/overview/quick-start) -- OpenAI-compatible at `https://api.z.ai/api/paas/v4/chat/completions`. GLM-4.7 model.
- [tower-circuitbreaker v0.2.0](https://docs.rs/tower-circuitbreaker/latest/tower_circuitbreaker/) -- Circuit breaker pattern with configurable failure thresholds.
- [LanceDB FAQ](https://docs.lancedb.com/faq/faq-oss) -- Concurrent access, storage format, embedded architecture.
- Phase 2 RESEARCH.md -- LlmProvider trait, BoxLlmProvider pattern, StreamEvent types, established architecture.

### Secondary (MEDIUM confidence)
- [claude-max-api-proxy](https://docs.openclaw.ai/providers/claude-max-api-proxy) -- OpenClaw docs for Claude subscription proxy. Community tool.
- [GLM-4.7 via AIML API](https://docs.aimlapi.com/api-references/text-models-llm/zhipu/glm-4.7) -- OpenAI-compatible API format, parameters, streaming.
- [Embedding Model Dimensions](https://app.ailog.fr/en/blog/guides/choosing-embedding-models) -- BGESmallENV15: 384 dims, MiniLM-L6-v2: 384 dims.
- [vec-embed-store](https://crates.io/crates/vec-embed-store) -- Wrapper crate demonstrating LanceDB + fastembed integration pattern.
- [Qdrant Multitenancy Guide](https://qdrant.tech/documentation/guides/multitenancy/) -- Payload-based partitioning pattern applicable to shared memory trust levels.

### Tertiary (LOW confidence)
- [claude-max-api-proxy GitHub](https://github.com/atalovesyou/claude-max-api-proxy) -- Community tool for Claude subscription access. ToS concerns.
- GLM 4.7 Rust SDK availability -- No dedicated Rust crate found. OpenAI-compatible endpoint is the only path.

## Metadata

**Confidence breakdown:**
- Standard stack (OpenAI/Gemini/Mistral via async-openai): HIGH -- async-openai v0.32.4 verified, all providers confirmed OpenAI-compatible
- Standard stack (Bedrock): HIGH -- Official AWS SDK with ConverseStream, extensive examples
- Standard stack (LanceDB + fastembed): MEDIUM-HIGH -- Both libraries verified, but Rust SDK integration patterns (Arrow types) need prototyping
- Architecture (OpenAI-compat provider): HIGH -- Pattern verified against async-openai docs and multiple provider compat pages
- Architecture (fallback chain): MEDIUM -- tower-circuitbreaker verified, custom FallbackChain design is original but based on established patterns
- Architecture (shared memory trust): MEDIUM -- Trust partitioning via LanceDB filter expressions is a straightforward application of LanceDB's query capabilities, but needs testing
- Claude subscription provider: LOW -- Community tool, ToS concerns, proxy reliability unknown
- GLM 4.7: MEDIUM -- API format confirmed OpenAI-compatible, no Rust-specific crate but async-openai handles it
- Pitfalls: HIGH -- All pitfalls based on verified library behavior and documented limitations

**Research date:** 2026-02-11
**Valid until:** 2026-03-11 (30 days; LanceDB and async-openai update frequently but core patterns are stable)
