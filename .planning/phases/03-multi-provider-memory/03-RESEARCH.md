# Phase 3: Multi-Provider + Memory - Research

**Researched:** 2026-02-12 (post-discussion comprehensive update)
**Domain:** Multi-LLM provider abstraction, fallback chains, vector memory (LanceDB), embeddings, shared memory with trust, per-bot file storage, KV store (Rust)
**Confidence:** HIGH (standard stack, provider integration), MEDIUM-HIGH (LanceDB + fastembed), LOW (Claude.ai subscription proxy -- ToS violation confirmed)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Failover experience:**
- Warn about limitations when fallback provider is significantly weaker (e.g., "Running on a smaller model -- responses may be less detailed")
- Auto-switch back to primary provider when it recovers (on next message)
- Global default fallback chain with per-bot overrides
- Claude.ai subscription provider included as experimental, clearly marked unsupported, hidden behind a flag
- Dedicated CLI command for provider health status (`bnity provider status`) showing circuit breaker state, last error, uptime
- Always test connection when a new provider is configured (send small request to verify API key and endpoint)
- Failover events visible in CLI output (print to stderr during chat)
- Provider priority numbers for fallback chain ordering; ties broken by latency or cost
- Track provider cost differences and warn if fallback is significantly more expensive
- Queue requests briefly when rate-limited (wait up to N seconds), then fail over to next provider
- Clear error message when ALL providers in chain are down, suggesting `bnity provider status`

**Memory recall in chat:**
- Search long-term vector memory on every user message
- Blend recalled memories naturally into responses (no explicit citation)
- Retrieve up to 10 memories with relevance threshold (minimum similarity score filter)
- Full CRUD CLI: `bnity memory list/search/delete/add` for complete management
- Memory search results silently injected into system prompt (invisible to user)
- Natural language "forget" in chat AND CLI delete command -- both paths for memory deletion
- Auto re-embed all existing memories when embedding model changes
- Time decay on memory importance -- older memories get lower retrieval priority unless reinforced
- Auto-categorized memories (LLM assigns category during extraction); user can filter by category in search
- No cap on total memories per bot -- LanceDB handles unbounded growth
- Audit log for memory additions and deletions (who, when, what)
- Verbose mode (`bnity chat --verbose`) shows which memories were injected into system prompt
- Semantic dedup using vector similarity to detect and merge near-duplicate memories
- JSON export via `bnity memory export`
- Memory search CLI shows similarity scores alongside results

**Shared memory trust model:**
- Three trust levels: Public (all bots can read), Trusted (explicitly approved bots), Private (author only)
- Explicit trust list per bot (`trusted_bots` list in config) -- bot A trusts [bot B, bot C]
- Provenance always shown -- memory includes "Written by BotX" in context injected to reading bot
- Memories are private by default; sharing is an explicit action
- Sharing via both CLI (`bnity memory share <id> --level public/trusted`) and in-chat instruction
- Tamper detection via SHA-256 hash on writes; no content-level conflict detection
- Dedicated `bnity shared-memory` CLI subcommand with list, search, and details
- Author can revoke previously shared memories
- Merged query results -- a single query returns both private and shared memories, ranked by relevance
- Configurable cap on shared memory contributions per bot (e.g., max 500) to prevent domination

**Per-bot file storage:**
- Any file type accepted (text, images, PDFs, code, binaries)
- Per-file size limit (e.g., 50MB) but no total cap per bot
- Auto-context: text files automatically indexed and searchable via vector embeddings
- Read-write access: bot can create new files and modify existing ones (notes, summaries, generated content)
- Upload via both CLI (`bnity storage upload`) and in-chat file path pasting
- Auto-index text files: chunk and embed for semantic search (personal knowledge base)
- File version history (similar to soul versioning from Phase 1)
- Files + key-value store: separate KV store alongside files for structured data (settings, state, counters)
- KV store values support arbitrary JSON (objects, arrays, nested structures)
- Full CRUD CLI: `bnity storage list/upload/download/delete/info`
- Semantic chunking for large text files (split at paragraph/section boundaries)
- Files shareable between bots with same trust levels as shared memory

### Claude's Discretion
- Failover notification method (inline chat notice vs stats footer vs both)
- Memory layer architecture (whether vector memory replaces or layers on top of Phase 2 session memory)
- Exact per-file size limit default
- Rate limit queue timeout duration
- Cost estimation data source and warning thresholds
- Embedding model migration background job scheduling
- File chunking parameters (chunk size, overlap, boundary detection heuristics)
- KV store implementation (SQLite table vs embedded store)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Summary

Phase 3 extends the Phase 2 single-provider LLM abstraction to support six additional providers (OpenAI, Google Gemini, Mistral, AWS Bedrock, Claude.ai subscription, GLM 4.7), implements automatic failover with configurable fallback chains, adds long-term vector memory via LanceDB with local embeddings via fastembed, builds a shared memory layer with trust-level partitioning and provenance tracking, and adds per-bot persistent file storage with a KV store.

The critical architectural insight is that **four of the six new providers (OpenAI, Gemini, Mistral, GLM 4.7) use OpenAI-compatible API formats**. This means we can build a single `OpenAiCompatibleProvider` implementation with configurable base URLs and model mappings, then specialize only where providers diverge (auth headers, streaming quirks). AWS Bedrock already has an implementation in Phase 2 using `reqwest` with Bearer token auth and the AWS event stream binary protocol. The Claude.ai subscription proxy uses OpenAI format but runs through a local Node.js proxy (claude-max-api-proxy) that wraps the Claude Code CLI -- **however, Anthropic actively enforces against this usage as of January 2026, making it a ToS-violating, unreliable path**.

For vector memory, LanceDB v0.26.2 provides a mature embedded Rust SDK that stores data in Lance columnar format. Combined with fastembed for local ONNX-based embeddings (BGESmallENV15, 384 dimensions), this gives us a fully local, zero-external-dependency vector memory system. The existing Phase 2 session memory in SQLite is preserved; vector memory layers on top of it -- SQLite stores the relational metadata (MemoryEntry), while LanceDB stores the embeddings and handles similarity search.

For the KV store, use SQLite (already in the workspace via sqlx). A new `bot_kv_store` table with `bot_id`, `key`, `value` (JSON text) is the simplest, most reliable approach and avoids introducing another embedded database.

**Primary recommendation:** Build a unified `OpenAiCompatibleProvider` for OpenAI/Gemini/Mistral/GLM using `async-openai`, extend the existing `BedrockProvider` for the Bedrock path (already implemented in Phase 2), and add a `ClaudeSubscriptionProvider` marked experimental/unsupported. Wrap all providers in a `FallbackChain` with per-provider circuit breaker state. Use LanceDB + fastembed for fully local vector memory. Use SQLite for the KV store, file metadata, audit logs, and provider health state.

## Discretion Recommendations

For areas marked as "Claude's Discretion":

| Area | Recommendation | Rationale |
|------|---------------|-----------|
| Failover notification | Both: inline stderr notice at failover moment + provider name in stats footer | Inline catches attention; footer provides persistent visibility |
| Memory layer architecture | Vector memory **layers on top of** Phase 2 session memory | Session memory (SQLite) handles CRUD/metadata; LanceDB handles embeddings + similarity search. No replacement. |
| Per-file size limit default | 50MB | Generous for text/code, accommodates most images. Easy to raise later. |
| Rate limit queue timeout | 5 seconds | Short enough to not block UX, long enough for transient 429s |
| Cost estimation data source | Hard-coded table of per-1K-token costs per provider/model | APIs change pricing rarely; a static table is simpler than querying. Update on version bumps. |
| Cost warning threshold | Warn if fallback costs >3x the primary provider | Prevents surprise bills; 3x is a reasonable "significantly more expensive" threshold |
| Embedding migration scheduling | Synchronous on startup if model changed; background re-embed with progress bar | Detect model mismatch on boot, run re-embedding before first query |
| File chunking parameters | 512 tokens per chunk, 50 token overlap, paragraph boundary preference | Matches standard RAG best practices; text-splitter handles boundary detection |
| KV store implementation | SQLite table (new `bot_kv_store` table) | Already using sqlx; avoids new dependency. JSON column with SQLite JSON functions. |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `async-openai` | 0.32.4 | OpenAI-compatible API client (OpenAI, Gemini, Mistral, GLM 4.7) | De facto Rust OpenAI client. 2.6M+ downloads. Configurable base URLs via `OpenAIConfig::with_api_base()`, built-in SSE streaming via `create_stream()`, type-safe request/response. Published 2026-01-25. |
| `lancedb` | 0.26.2 | Embedded vector database for memory embeddings | Embedded (no server), Lance columnar format, IVF-PQ indexing, SQL-like filter expressions with pre/post-filtering, multi-version concurrency control. Published 2026-02-09. |
| `fastembed` | 5.x | Local embedding model inference (ONNX runtime) | 44+ text models, local ONNX inference, no API keys. BGESmallENV15 default (384 dims). Configurable cache directory. |
| `arrow-schema` | 57.x (match lancedb transitive) | Arrow schema definitions for LanceDB tables | Required for defining LanceDB table schemas with FixedSizeList vector columns. Version MUST match lancedb's transitive dependency (57.2 for lancedb 0.26.2). |
| `arrow-array` | 57.x (match lancedb transitive) | Arrow array types for LanceDB data ingestion | Required for creating RecordBatch data. Version MUST match lancedb's transitive dependency. |
| `text-splitter` | 0.29.3 | Semantic text chunking for file storage indexing | Splits at paragraph/sentence/word boundaries. Supports character and token-based chunk sizing. MarkdownSplitter for .md files. tiktoken integration via feature flag. |

### Supporting (already in workspace)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `reqwest` | 0.12.x | HTTP client | Provider health checks, Claude subscription proxy |
| `reqwest-eventsource` | 0.6.0 | SSE streaming | Anthropic provider (Phase 2), fallback if async-openai insufficient |
| `serde` / `serde_json` | 1.x | Serialization | All API types, KV store JSON values |
| `tokio` | 1.x | Async runtime | Everything async, `spawn_blocking` for fastembed |
| `futures-util` | 0.3.x | Stream combinators | Processing async-openai ChatCompletionResponseStream |
| `secrecy` | 0.10.x | API key wrapping | All provider API keys |
| `tracing` | 0.1.x | Structured logging | Provider health events, failover tracing, audit log |
| `chrono` | 0.4.x | Timestamps | Memory provenance, health check timestamps, time decay |
| `uuid` | 1.20.x | Unique IDs | Memory entries, file storage entries |
| `sha2` | 0.10.x | SHA-256 hashing | Shared memory tamper detection |
| `sqlx` | 0.8.x | SQLite async | KV store, file metadata, audit logs, provider health |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `async-openai` for all OpenAI-compat providers | Raw `reqwest` + custom types | Full control but rebuilds SSE streaming, type safety, retry. async-openai handles all this. |
| `async-openai` for all OpenAI-compat providers | Individual provider crates | More maintenance, no shared code. async-openai's configurable base URL handles all. |
| `fastembed` (local) | OpenAI Embeddings API | Requires API key, network, costs money. Local is free, offline-capable, consistent. |
| `lancedb` | `qdrant` (embedded mode) | Qdrant requires running a separate server process. LanceDB is truly embedded (in-process). |
| `lancedb` | `sqlite-vec` (SQLite extension) | sqlite-vec lacks IVF-PQ indexing, pre/post-filtering, and scales poorly past ~50K vectors. |
| Custom circuit breaker | `tower-circuitbreaker` | Our LLM providers implement LlmProvider, not Tower Service. Custom is simpler. |
| Custom circuit breaker | `failsafe` crate | failsafe is more mature but has fewer recent updates. Our needs are simple. |
| `text-splitter` | Custom chunking logic | text-splitter handles Unicode boundaries, paragraph detection, token counting. Don't rebuild. |
| SQLite KV table | `sled` embedded KV | Adds a new dependency. SQLite already in workspace, well-understood, supports JSON. |
| SQLite KV table | LanceDB for KV | LanceDB is optimized for vector search, not key-value lookups. Wrong tool. |

**Installation (workspace Cargo.toml additions):**
```toml
# New workspace dependencies for Phase 3
async-openai = "0.32"
lancedb = "0.26"
fastembed = "5"
text-splitter = { version = "0.29", features = ["markdown", "tiktoken-rs"] }

# Arrow -- MUST match lancedb's transitive deps (57.x for lancedb 0.26.2)
# After adding lancedb, verify with: cargo tree -p lancedb | grep arrow
arrow-schema = "57"
arrow-array = "57"
```

## Architecture Patterns

### Recommended Module Structure (Phase 3 additions)
```
crates/
  boternity-types/
    src/
      llm.rs                    # Extended: add ProviderConfig, FallbackChainConfig
      memory.rs                 # Extended: add VectorMemoryEntry, SharedMemoryEntry, TrustLevel
      storage.rs                # NEW: per-bot file storage + KV types

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
        vector.rs               # NEW: VectorMemoryStore trait (search, add, delete)
        shared.rs               # NEW: SharedMemoryStore trait with trust partitioning
        embedder.rs             # NEW: Embedder trait (abstracts fastembed vs API embeddings)
      storage/
        mod.rs                  # NEW: re-exports
        file_store.rs           # NEW: FileStore trait
        kv_store.rs             # NEW: KvStore trait

  boternity-infra/
    src/
      llm/
        anthropic/              # Existing: AnthropicProvider (unchanged from Phase 2)
        bedrock/                # Existing: BedrockProvider (extended for failover)
        openai_compat/
          mod.rs                # NEW: OpenAiCompatibleProvider
          config.rs             # NEW: provider-specific configs (Gemini, Mistral, GLM, etc.)
          streaming.rs          # NEW: OpenAI SSE stream adapter -> StreamEvent
        claude_sub/
          mod.rs                # NEW: ClaudeSubscriptionProvider (experimental)
      vector/
        mod.rs                  # NEW: re-exports
        lance.rs                # NEW: LanceDB vector store implementation
        embedder.rs             # NEW: FastEmbedEmbedder implementing Embedder trait
        schema.rs               # NEW: Arrow schema definitions for memory tables
      storage/
        mod.rs                  # NEW: re-exports
        filesystem.rs           # NEW: Per-bot file storage on local filesystem
        chunker.rs              # NEW: text-splitter wrapper for file indexing
        metadata.rs             # NEW: SQLite metadata for file storage + versions
      sqlite/
        kv.rs                   # NEW: SQLite KV store implementation
        audit.rs                # NEW: Memory audit log
        provider_health.rs      # NEW: Persistent provider health state
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
    ChatCompletionRequestUserMessageArgs,
    ChatCompletionRequestSystemMessageArgs,
    ChatCompletionStreamOptions,
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
                streaming: true, tool_calling: true, vision: true,
                extended_thinking: false,
                max_context_tokens: 128_000,
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
                streaming: true, tool_calling: true, vision: true,
                extended_thinking: false,
                max_context_tokens: 1_000_000,
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
                streaming: true, tool_calling: true, vision: true,
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
                streaming: true, tool_calling: true, vision: false,
                extended_thinking: false,
                max_context_tokens: 200_000,
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
            api_key: "dummy-key".into(),
            model: model.into(),
            capabilities: ProviderCapabilities {
                streaming: true, tool_calling: true, vision: true,
                extended_thinking: true,
                max_context_tokens: 200_000,
                max_output_tokens: 128_000,
            },
        })
    }
}
```

### Pattern 2: Streaming Adapter (OpenAI -> StreamEvent)
**What:** Map `async-openai`'s streaming types to the Phase 2 `StreamEvent` enum.
**When to use:** Inside `OpenAiCompatibleProvider::stream()`.

Key OpenAI streaming chunk structure:
```json
{"id":"chatcmpl-xxx","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}
{"id":"chatcmpl-xxx","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],
 "usage":{"prompt_tokens":10,"completion_tokens":20,"total_tokens":30}}
```

**CRITICAL:** Set `stream_options: { include_usage: true }` in request. Without this, streaming responses do NOT report token usage.

```rust
// Mapping: async-openai types -> StreamEvent
// ChatChoiceStream.delta.content  -> StreamEvent::TextDelta { index, text }
// ChatChoiceStream.delta.tool_calls -> accumulate, emit StreamEvent::ToolUseComplete on finish
// ChatChoiceStream.finish_reason: Stop -> StreamEvent::MessageDelta { StopReason::EndTurn }
// ChatChoiceStream.finish_reason: Length -> StreamEvent::MessageDelta { StopReason::MaxTokens }
// ChatChoiceStream.finish_reason: ToolCalls -> StreamEvent::MessageDelta { StopReason::ToolUse }
// chunk.usage -> StreamEvent::Usage { input_tokens, output_tokens }
```

### Pattern 3: Existing Bedrock Provider Extension
**What:** The Phase 2 `BedrockProvider` already works with `reqwest` + Bearer token auth + AWS event stream binary protocol. For Phase 3, we keep it as-is and integrate it into the fallback chain.
**When to use:** When Bedrock is configured as a provider in the fallback chain.
**Why:** The existing implementation is tested and working. No need to switch to the AWS SDK.

NOTE: The existing `BedrockProvider` uses Anthropic-compatible Messages API format (same JSON shape as `AnthropicProvider` minus the `model` field, plus `anthropic_version`). This is specific to Claude models on Bedrock. If non-Claude Bedrock models are needed in the future, a Bedrock ConverseStream implementation can be added separately.

### Pattern 4: Fallback Chain with Circuit Breaker
**What:** A `FallbackChain` that wraps multiple `BoxLlmProvider` instances with per-provider circuit breaker state.
**When to use:** When the bot has a configured fallback chain.

```rust
// In boternity-core/src/llm/health.rs

#[derive(Debug, Clone)]
pub enum CircuitState {
    /// Normal: requests pass through.
    Closed { consecutive_failures: u32 },
    /// Failing fast: requests rejected. Check timer for half-open transition.
    Open { opened_at: Instant, wait_duration: Duration },
    /// Testing: one probe request allowed.
    HalfOpen,
}

pub struct ProviderHealth {
    pub name: String,
    pub priority: u32,              // User-configured priority number
    pub state: CircuitState,
    pub last_error: Option<String>,
    pub last_success: Option<Instant>,
    pub last_latency_ms: Option<u64>,
    pub total_calls: u64,
    pub total_failures: u64,
    pub uptime_since: Option<Instant>,
    // Config
    failure_threshold: u32,         // Default: 3
    success_threshold: u32,         // Default: 1
    open_duration: Duration,        // Default: 30s
    // Rate limit queuing
    rate_limit_until: Option<Instant>,
}

impl ProviderHealth {
    /// Can this provider accept a request right now?
    pub fn is_available(&mut self) -> bool {
        // Check rate limit wait first
        if let Some(until) = self.rate_limit_until {
            if Instant::now() < until {
                return false;
            }
            self.rate_limit_until = None;
        }
        match &self.state {
            CircuitState::Closed { .. } => true,
            CircuitState::Open { opened_at, wait_duration } => {
                if opened_at.elapsed() >= *wait_duration {
                    self.state = CircuitState::HalfOpen;
                    true
                } else { false }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Queue for rate limit: wait up to N seconds before failover
    pub fn set_rate_limited(&mut self, retry_after_ms: Option<u64>, max_wait_ms: u64) {
        let wait = retry_after_ms.unwrap_or(1000).min(max_wait_ms);
        self.rate_limit_until = Some(Instant::now() + Duration::from_millis(wait));
    }

    /// Should we fail over based on this error type?
    pub fn is_failover_error(error: &LlmError) -> bool {
        matches!(error,
            LlmError::Provider { .. } |
            LlmError::Stream(_) |
            LlmError::RateLimited { .. } |
            LlmError::Overloaded(_)
        )
        // AuthenticationFailed, InvalidRequest, ContextLengthExceeded -> do NOT failover
    }
}
```

```rust
// In boternity-core/src/llm/fallback.rs

pub struct FallbackChain {
    providers: Vec<(ProviderHealth, BoxLlmProvider)>,
    // Cost table: estimated $/1K tokens per provider
    cost_per_1k_tokens: HashMap<String, f64>,
    primary_provider_name: String,
}

impl FallbackChain {
    /// Returns (response, provider_name, failover_warning).
    /// failover_warning is Some("msg") if we had to fall back.
    pub async fn complete(&mut self, request: &CompletionRequest)
        -> Result<(CompletionResponse, String, Option<String>), LlmError>
    {
        // ... try providers in priority order, track failover ...
    }

    /// Get health status of all providers for `bnity provider status`.
    pub fn health_status(&self) -> Vec<ProviderStatusInfo> {
        // Returns name, state, last_error, uptime, total_calls, total_failures
    }
}
```

**User-decided behaviors:**
- Rate-limited: queue for up to 5 seconds (configurable), then fail over
- Auto-recover: on each request, re-check primary if circuit is half-open
- Warn on capability downgrade: if fallback model is weaker, include warning
- Warn on cost increase: if fallback costs >3x primary, include cost warning
- Stderr notice: print failover events to stderr during chat
- Test on configure: send small "Hello" request when new provider is set up

### Pattern 5: LanceDB Vector Memory with fastembed
**What:** LanceDB for vector storage + fastembed for local embedding generation.
**When to use:** All long-term vector memory operations.

**LanceDB key APIs (v0.26.2):**
- `lancedb::connect(path).execute().await` -- connect to local DB
- `connection.create_table(name, data).execute().await` -- create table
- `connection.open_table(name).execute().await` -- open existing
- `connection.drop_table(name).execute().await` -- PERMANENTLY delete
- `connection.table_names().execute().await` -- list tables
- `table.query().nearest_to(vector).limit(n).execute().await` -- vector search
- `table.query().only_if("sql expression")` -- metadata filter
- `table.add(data).execute().await` -- add records
- `table.delete("sql predicate").await` -- delete rows
- `table.count_rows(None).await` -- row count
- `table.create_index(&["vector"], Index::Auto).execute().await` -- create index

**Distance metrics:** `DistanceType::Cosine` (use for text embeddings), L2, Dot, Hamming
**Filter expressions:** SQL-like, e.g., `"importance >= 3 AND category = 'fact'"`
**Pre-filtering (default):** Filters BEFORE vector search -- faster for metadata-heavy queries
**Results:** Returns `SendableRecordBatchStream`, use `try_collect::<Vec<RecordBatch>>()` to consume

```rust
// Schema for bot memory table
fn bot_memory_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("bot_id", DataType::Utf8, false),
        Field::new("fact", DataType::Utf8, false),
        Field::new("category", DataType::Utf8, false),
        Field::new("importance", DataType::Int32, false),
        Field::new("session_id", DataType::Utf8, true),
        Field::new("created_at", DataType::Utf8, false),       // ISO 8601
        Field::new("last_accessed_at", DataType::Utf8, true),   // For time decay tracking
        Field::new("access_count", DataType::Int32, false),      // Reinforcement counter
        Field::new("embedding_model", DataType::Utf8, false),    // Track model version
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                384, // BGESmallENV15
            ),
            false,
        ),
    ])
}
```

### Pattern 6: Time Decay Scoring for Memory Retrieval
**What:** Combine cosine similarity with time decay to prioritize recent/reinforced memories.
**When to use:** Every memory retrieval (user decided: search on every user message).

```rust
/// Compute final relevance score combining similarity + time decay + importance.
fn compute_relevance_score(
    cosine_distance: f32,      // From LanceDB (0.0 = identical, 2.0 = opposite)
    importance: u8,            // 1-5
    created_at: DateTime<Utc>,
    last_accessed_at: Option<DateTime<Utc>>,
    access_count: u32,
) -> f32 {
    let similarity = 1.0 - (cosine_distance / 2.0); // Convert distance to similarity [0, 1]

    // Time decay: half-life of 30 days. Accessed memories reset the clock.
    let reference_time = last_accessed_at.unwrap_or(created_at);
    let days_old = Utc::now().signed_duration_since(reference_time).num_days() as f32;
    let half_life_days = 30.0;
    let time_factor = (0.5_f32).powf(days_old / half_life_days);

    // Reinforcement bonus: each access adds a small boost (diminishing returns)
    let reinforcement = 1.0 + (access_count as f32).ln_1p() * 0.1;

    // Importance weight: scale 1-5 to 0.6-1.0
    let importance_factor = 0.6 + (importance as f32 - 1.0) * 0.1;

    similarity * time_factor * reinforcement * importance_factor
}

// Relevance threshold: user decided "minimum similarity score filter"
// Recommend: filter out memories with cosine distance > 1.2 (similarity < 0.4)
const MIN_SIMILARITY_THRESHOLD: f32 = 0.4;
const MAX_COSINE_DISTANCE: f32 = 1.2;
```

### Pattern 7: Semantic Dedup via Vector Similarity
**What:** Before storing a new memory, check if a near-duplicate already exists.
**When to use:** Every memory storage operation (user decided: semantic dedup).

```rust
/// Check for near-duplicate memories before storing.
/// Returns the existing memory if similarity exceeds threshold.
async fn check_duplicate(
    table: &LanceTable,
    embedding: &[f32],
    dedup_threshold: f32,  // e.g., cosine distance < 0.15 = near-duplicate
) -> Result<Option<MemoryEntry>, VectorStoreError> {
    let results = table.query()
        .nearest_to(embedding)?
        .distance_type(DistanceType::Cosine)
        .limit(1)
        .execute()
        .await?;

    let batches: Vec<RecordBatch> = results.try_collect().await?;
    // Check if closest match is within dedup threshold
    // LanceDB includes a "_distance" column in results
    // If _distance < dedup_threshold, it's a near-duplicate
    // ...
}
```

### Pattern 8: Shared Memory with Trust Partitioning
**What:** Single shared memory LanceDB table with trust metadata columns.
**When to use:** For cross-bot memory sharing (MEMO-03, MEMO-04).

```rust
// Shared memory schema
fn shared_memory_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("fact", DataType::Utf8, false),
        Field::new("category", DataType::Utf8, false),
        Field::new("importance", DataType::Int32, false),
        Field::new("author_bot_id", DataType::Utf8, false),
        Field::new("author_bot_name", DataType::Utf8, false),
        Field::new("trust_level", DataType::Utf8, false),       // "public", "trusted", "private"
        Field::new("created_at", DataType::Utf8, false),
        Field::new("write_hash", DataType::Utf8, false),         // SHA-256 tamper detection
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

// Trust-filtered query for Bot B:
// Bot B can see: public memories + memories from bots in its trust list + its own
fn build_trust_filter(reading_bot_id: &str, trusted_bot_ids: &[&str]) -> String {
    let trusted_list = trusted_bot_ids.iter()
        .map(|id| format!("'{}'", id))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "trust_level = 'public' OR author_bot_id = '{}' OR (trust_level = 'trusted' AND author_bot_id IN ({}))",
        reading_bot_id, trusted_list
    )
}

// Merged query: private memories + shared memories, ranked by relevance
async fn merged_memory_query(
    bot_table: &LanceTable,       // Bot's own memory table
    shared_table: &LanceTable,     // Shared memory table
    query_vec: &[f32],
    bot_id: &str,
    trusted_bot_ids: &[&str],
    limit: usize,
) -> Result<Vec<RankedMemory>, Error> {
    // Query both tables in parallel
    let (private, shared) = tokio::join!(
        bot_table.query().nearest_to(query_vec)?.distance_type(DistanceType::Cosine).limit(limit).execute(),
        shared_table.query().nearest_to(query_vec)?.distance_type(DistanceType::Cosine)
            .only_if(&build_trust_filter(bot_id, trusted_bot_ids)).limit(limit).execute(),
    );
    // Merge and re-rank by relevance score
    // Annotate shared memories with provenance: "Written by BotX"
}
```

### Pattern 9: File Storage with Semantic Chunking
**What:** Per-bot file storage on filesystem + text-splitter for semantic chunking + LanceDB for file chunk embeddings.
**When to use:** For MEMO-06 (per-bot persistent storage).

File storage path: `~/.boternity/bots/{slug}/files/`
Version history path: `~/.boternity/bots/{slug}/files/.versions/`
Metadata in SQLite: file_id, bot_id, filename, mime_type, size_bytes, created_at, updated_at, version, is_indexed

```rust
// Text file chunking for semantic search
use text_splitter::{TextSplitter, MarkdownSplitter, ChunkConfig};

/// Chunk a text file for embedding and indexing.
fn chunk_text_file(content: &str, is_markdown: bool) -> Vec<String> {
    let config = ChunkConfig::new(512); // 512 characters per chunk

    if is_markdown {
        let splitter = MarkdownSplitter::new(config);
        splitter.chunks(content).map(|s| s.to_string()).collect()
    } else {
        let splitter = TextSplitter::new(config);
        splitter.chunks(content).map(|s| s.to_string()).collect()
    }
}

// File chunk schema in LanceDB (separate table per bot)
fn file_chunks_schema() -> Schema {
    Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),
        Field::new("file_id", DataType::Utf8, false),
        Field::new("bot_id", DataType::Utf8, false),
        Field::new("filename", DataType::Utf8, false),
        Field::new("chunk_index", DataType::Int32, false),
        Field::new("chunk_text", DataType::Utf8, false),
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
```

### Pattern 10: SQLite KV Store
**What:** Simple key-value store per bot using SQLite JSON column.
**When to use:** For structured data (settings, state, counters) alongside file storage.

```sql
-- Migration: bot_kv_store
CREATE TABLE IF NOT EXISTS bot_kv_store (
    bot_id  TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    key     TEXT NOT NULL,
    value   TEXT NOT NULL,           -- JSON text (objects, arrays, nested structures)
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (bot_id, key)
);

CREATE INDEX IF NOT EXISTS idx_kv_bot_id ON bot_kv_store(bot_id);
```

```rust
// KvStore trait in boternity-core
pub trait KvStore: Send + Sync {
    fn get(&self, bot_id: &Uuid, key: &str)
        -> impl Future<Output = Result<Option<serde_json::Value>, RepositoryError>> + Send;
    fn set(&self, bot_id: &Uuid, key: &str, value: &serde_json::Value)
        -> impl Future<Output = Result<(), RepositoryError>> + Send;
    fn delete(&self, bot_id: &Uuid, key: &str)
        -> impl Future<Output = Result<(), RepositoryError>> + Send;
    fn list_keys(&self, bot_id: &Uuid)
        -> impl Future<Output = Result<Vec<String>, RepositoryError>> + Send;
}
```

### Pattern 11: Memory Audit Log
**What:** Record all memory additions and deletions for accountability.
**When to use:** Every memory write/delete operation (user decided: audit log).

```sql
-- Migration: memory_audit_log
CREATE TABLE IF NOT EXISTS memory_audit_log (
    id          TEXT PRIMARY KEY NOT NULL,   -- UUIDv7
    bot_id      TEXT NOT NULL,
    memory_id   TEXT NOT NULL,
    action      TEXT NOT NULL CHECK (action IN ('add', 'delete', 'share', 'revoke', 'merge')),
    actor       TEXT NOT NULL,               -- "system", "user", bot slug
    details     TEXT,                        -- Optional JSON context
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_bot ON memory_audit_log(bot_id, created_at DESC);
```

### Anti-Patterns to Avoid
- **Separate implementations for each OpenAI-compatible provider:** OpenAI, Gemini, Mistral, and GLM all speak OpenAI chat completions. Use one `OpenAiCompatibleProvider` with different base URLs.
- **Using raw reqwest for OpenAI-compatible providers:** `async-openai` already handles SSE parsing, retry, type safety. Don't rebuild it.
- **Running fastembed on the Tokio runtime thread:** Embedding generation is CPU-intensive ONNX inference. Always use `tokio::task::spawn_blocking`.
- **Storing embeddings in SQLite:** SQLite is not designed for vector similarity search. Use LanceDB for vectors, SQLite for relational metadata.
- **Creating one global LanceDB table for all bots:** Use per-bot tables for isolation. Shared memory gets its own table.
- **Hard-coding provider URLs:** Base URLs should come from configuration.
- **Ignoring `stream_options.include_usage`:** Without this, OpenAI-compatible streaming does NOT report token usage.
- **Replacing the existing BedrockProvider with AWS SDK:** The Phase 2 Bedrock implementation using reqwest + Bearer token + event stream protocol is working and tested. Don't replace it.
- **Trusting all bots equally in shared memory:** Every write must include provenance and validation hash.
- **Relying on claude-max-api-proxy for production:** Anthropic blocks this as of Jan 2026.
- **Ignoring `embedding_model` column:** Always store which model produced the embedding. Mismatch = garbage results.
- **Single-record inserts to LanceDB:** Creates disk fragments. Batch inserts (10+ records or periodic flush).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| OpenAI-compatible API client | Custom reqwest + SSE parser | `async-openai` 0.32.4 | Handles streaming, retry, types, auth, stream_options. |
| Vector similarity search | Custom cosine over SQLite | `lancedb` 0.26.2 | IVF-PQ indexing, ANN search, SQL-like filters. |
| Embedding generation | Custom model loading + inference | `fastembed` 5.x | ONNX runtime, 44+ models, configurable cache. |
| Text chunking for indexing | Custom paragraph splitter | `text-splitter` 0.29.3 | Unicode boundaries, Markdown support, token-based sizing. |
| Arrow RecordBatch creation | Manual byte buffer | `arrow-array` / `arrow-schema` | Type-safe batch creation, schema validation. |
| Content hash for tamper detection | Custom algorithm | `sha2` (already in workspace) | SHA-256, already used in Phase 1. |
| Circuit breaker state machine | -- | Custom 3-state machine (~80 lines) | Simple enough to own. Tower middleware doesn't fit our trait. |
| KV store | New embedded KV crate | SQLite table (already in workspace) | Avoid new dependency. JSON column handles arbitrary values. |

**Key insight:** The biggest win is `async-openai` with configurable base URLs -- four providers from one implementation. The second is LanceDB for vector search. The third is using SQLite for everything relational (KV, audit, metadata) since it's already in the workspace.

## Common Pitfalls

### Pitfall 1: Blocking Tokio Runtime with fastembed
**What goes wrong:** `fastembed::TextEmbedding::embed()` on the Tokio thread blocks all async tasks.
**Why it happens:** ONNX inference is CPU-intensive (10-50ms per batch).
**How to avoid:** Always `tokio::task::spawn_blocking`. Create `TextEmbedding` once at startup, share via `Arc<TextEmbedding>`.
**Warning signs:** Streaming stutters during memory operations. Health check timeouts.

### Pitfall 2: Embedding Model Mismatch
**What goes wrong:** Memories stored with one model's vectors queried with another.
**Why it happens:** Changing embedding model without re-embedding.
**How to avoid:** Store `embedding_model` in every LanceDB row. On startup, check if model changed. If yes, re-embed ALL data before first query (user decided: auto re-embed).
**Warning signs:** Irrelevant search results after upgrade.

### Pitfall 3: OpenAI-Compatible Provider Quirks
**Known quirks (verified 2026-02-12):**
- **Gemini:** Beta. `reasoning_effort` maps differently. Safety filters may block unexpectedly.
- **Mistral:** Standard. Supports parallel function calling.
- **GLM (z.ai):** Uses `/api/paas/v4` path (not `/v1`). JWT auth option.
- **Claude subscription proxy:** Depends on Claude Code CLI auth. Only Opus 4, Sonnet 4, Haiku 4.
**How to avoid:** Provider-specific integration tests. Per-provider quirk config.

### Pitfall 4: Fallback Masking Provider Issues
**What goes wrong:** Users on weaker fallback model without knowing.
**How to avoid:** User decided: warn about limitations, show in stderr, include in stats footer. Also warn if fallback costs >3x more.

### Pitfall 5: Shared Memory Trust Bypass
**What goes wrong:** Bug allows reading another bot's private memories.
**How to avoid:** `SharedMemoryStore` trait is the ONLY access path. Raw LanceTable handle never exposed.

### Pitfall 6: LanceDB Concurrent Write Conflicts
**What goes wrong:** Multiple bots writing shared memory simultaneously.
**How to avoid:** Serialize shared memory writes through `tokio::sync::mpsc` channel. Per-bot tables don't have this issue.

### Pitfall 7: LanceDB Single-Record Insert Fragmentation
**What goes wrong:** One-at-a-time inserts create many small Lance fragments.
**How to avoid:** Batch writes. Buffer 10 memories or flush every 60 seconds.

### Pitfall 8: Claude.ai Subscription ToS Violation
**What goes wrong:** Anthropic blocks the proxy. Provider stops working.
**Why:** Anthropic explicitly prohibits this as of January 2026.
**How to avoid:** Mark EXPERIMENTAL. Require official API as fallback. Clear warnings.

### Pitfall 9: Missing stream_options for Token Usage
**What goes wrong:** Token usage never reported for streaming calls.
**How to avoid:** Always set `stream_options: { include_usage: true }`.

### Pitfall 10: Arrow Version Mismatch with LanceDB
**What goes wrong:** Compile error from type incompatibilities between arrow crates.
**Why it happens:** `lancedb` 0.26.2 depends on `arrow-*` 57.2. Using a different version causes mismatches.
**How to avoid:** After adding `lancedb` to Cargo.toml, run `cargo tree -p lancedb | grep arrow` and pin to the exact version.

## Provider-Specific Reference

### Provider Endpoint Summary (verified 2026-02-12)

| Provider | Type | Base URL | Auth | Streaming | Models |
|----------|------|----------|------|-----------|--------|
| Anthropic | Custom (Phase 2) | `https://api.anthropic.com/v1` | x-api-key header | SSE | Claude Sonnet/Opus/Haiku |
| Bedrock | Custom (Phase 2) | Regional AWS endpoint | Bearer token (presigned) | Binary event stream | Claude on Bedrock |
| OpenAI | OpenAI-compat | `https://api.openai.com/v1` | Bearer token | SSE | GPT-4o, o3-mini, etc. |
| Gemini | OpenAI-compat | `https://generativelanguage.googleapis.com/v1beta/openai` | API key as Bearer | SSE (beta) | Gemini 2.5/3.x |
| Mistral | OpenAI-compat | `https://api.mistral.ai/v1` | Bearer token | SSE | Large, Small, Pixtral |
| GLM (z.ai) | OpenAI-compat | `https://api.z.ai/api/paas/v4` | Bearer token | SSE | GLM-4.7, 4.7-Flash |
| Claude.ai sub | OpenAI-compat (proxy) | `http://localhost:3456/v1` | Dummy key | SSE | Opus 4, Sonnet 4, Haiku 4 |

### Per-Provider Cost Estimates (for failover warnings)

| Provider | Model | Input ($/1M tokens) | Output ($/1M tokens) |
|----------|-------|---------------------|----------------------|
| Anthropic | Claude Sonnet 4 | $3.00 | $15.00 |
| Anthropic | Claude Opus 4 | $15.00 | $75.00 |
| OpenAI | GPT-4o | $2.50 | $10.00 |
| OpenAI | o3-mini | $1.10 | $4.40 |
| Gemini | 2.5 Pro | $1.25 | $10.00 |
| Mistral | Large | $2.00 | $6.00 |
| GLM | 4.7 | Free tier / ~$0.50 | Free tier / ~$2.00 |
| Bedrock | Claude Sonnet 4 | ~$3.00 | ~$15.00 |

NOTE: Prices approximate as of Feb 2026. Hard-code in a cost table that gets updated with version bumps.

### Rate Limit Detection per Provider

| Provider | Signal | Retry-After | Queue Duration |
|----------|--------|-------------|----------------|
| OpenAI | HTTP 429 | Header | Up to 5s |
| Gemini | HTTP 429 | Header | Up to 5s |
| Mistral | HTTP 429 | Header | Up to 5s |
| GLM | HTTP 429 | Header | Up to 5s |
| Bedrock | HTTP 429 / ThrottlingException | -- | Up to 5s |
| Anthropic | `rate_limit_error` / HTTP 429 | Header | Up to 5s |

## Code Examples

### fastembed Embedding with spawn_blocking
```rust
use fastembed::{TextEmbedding, EmbeddingModel, InitOptions};

let cache_dir = dirs::data_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("boternity").join("models");

let model = Arc::new(TextEmbedding::try_new(InitOptions {
    model_name: EmbeddingModel::BGESmallENV15, // 384 dimensions, ~23MB
    show_download_progress: true,
    cache_dir,
    ..Default::default()
})?);

// ALWAYS use spawn_blocking
let model_clone = model.clone();
let text = "User prefers concise responses".to_string();
let embeddings = tokio::task::spawn_blocking(move || {
    model_clone.embed(vec![&text], None)
}).await??;
assert_eq!(embeddings[0].len(), 384);
```

### LanceDB Full Lifecycle
```rust
use lancedb::connect;
use arrow_schema::{Schema, Field, DataType};
use arrow_array::{RecordBatch, RecordBatchIterator, StringArray, Float32Array, FixedSizeListArray, Int32Array};
use futures_util::TryStreamExt;

// Connect
let db = connect("~/.boternity/vector_store").execute().await?;

// Create table with initial data
let schema = Arc::new(bot_memory_schema());
let batch = create_memory_batch(&schema, &entries, &embeddings)?;
let batches = RecordBatchIterator::new(vec![Ok(batch)], schema.clone());
let table = db.create_table("bot_memory_abc123", Box::new(batches)).execute().await?;

// Search with filter
let results = table.query()
    .nearest_to(&query_vec)?
    .distance_type(lancedb::DistanceType::Cosine)
    .only_if("importance >= 3")
    .limit(10)
    .execute()
    .await?;
let batches: Vec<RecordBatch> = results.try_collect().await?;

// Add more data
let new_batch = create_memory_batch(&schema, &new_entries, &new_embeddings)?;
let batches = RecordBatchIterator::new(vec![Ok(new_batch)], schema.clone());
table.add(Box::new(batches)).execute().await?;

// Delete by predicate
table.delete("id = 'mem_001'").await?;

// Create index when table grows past ~10K rows
table.create_index(&["vector"], lancedb::index::Index::Auto).execute().await?;
```

### async-openai Streaming with Token Usage
```rust
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
    .messages(vec![...])
    .stream_options(ChatCompletionStreamOptions {
        include_usage: Some(true),  // CRITICAL: enable token usage in streaming
    })
    .build()?;

let mut stream = client.chat().create_stream(request).await?;
while let Some(result) = stream.next().await {
    // Map to StreamEvent...
}
```

### Provider Health Check on Configuration
```rust
/// Test provider connectivity when newly configured.
async fn test_provider_connection(provider: &BoxLlmProvider) -> Result<(), LlmError> {
    let request = CompletionRequest {
        model: String::new(), // Provider uses default
        messages: vec![Message {
            role: MessageRole::User,
            content: "Hello".to_string(),
        }],
        system: None,
        max_tokens: 10,
        temperature: Some(0.0),
        stream: false,
        stop_sequences: None,
    };
    provider.complete(&request).await?;
    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate client per LLM provider | OpenAI-compatible endpoints + configurable base URL | 2025-2026 | One client serves 4+ providers |
| External vector DB server | Embedded vector DB (LanceDB) | 2025-2026 | Zero infrastructure, in-process |
| API-based embeddings | Local ONNX embeddings (fastembed) | 2025-2026 | Free, offline, consistent |
| AWS Bedrock InvokeModel | Bedrock via reqwest + Bearer token (existing Phase 2) | Phase 2 | Already implemented and working |
| Qdrant/Milvus for vectors | LanceDB on Lance columnar format | 2025-2026 | True embedded, Arrow native |
| Claude.ai subscription as API | Official API only | January 2026 | Subscription proxies actively blocked |

**Deprecated/outdated:**
- `vectordb` crate: Renamed to `lancedb`.
- Claude.ai subscription proxies for production: Blocked by Anthropic (January 2026).
- `tower-circuitbreaker` for LLM fallback: Not a fit for LlmProvider trait.

## Open Questions

1. **Arrow version pinning with lancedb** (MEDIUM confidence)
   - What we know: lancedb 0.26.2 uses arrow-* 57.2. Must match exactly.
   - Recommendation: First task should verify with `cargo tree -p lancedb | grep arrow`. Pin arrow-schema and arrow-array to discovered version.

2. **LanceDB empty table creation** (MEDIUM confidence)
   - What we know: `create_table` requires initial data. No documented `create_empty_table`.
   - Recommendation: Try `RecordBatch::new_empty(schema)`. If fails, create with one row and delete it.

3. **fastembed Send + Sync for Arc sharing** (MEDIUM-HIGH confidence)
   - What we know: TextEmbedding should be Send + Sync. ONNX supports concurrent inference.
   - Recommendation: Verify at compile time. If not Send+Sync, use a dedicated embedding thread.

4. **LanceDB _distance column in results** (MEDIUM confidence)
   - What we know: LanceDB includes distance in results for ranked output. Need to verify the exact column name and how to extract it from RecordBatch.
   - Recommendation: Test during implementation. Check for `_distance` or similar column in result schema.

5. **async-openai FinishReason enum variants** (MEDIUM confidence)
   - What we know: Need Stop, Length, ToolCalls. async-openai may have additional variants (ContentFilter, FunctionCall).
   - Recommendation: Check async-openai source at implementation time for exact enum definition.

## Sources

### Primary (HIGH confidence)
- [LanceDB v0.26.2 Rust API](https://docs.rs/lancedb/latest/lancedb/) -- Connection, Table, Query, DistanceType. Published 2026-02-09.
- [LanceDB DistanceType](https://docs.rs/lancedb/latest/lancedb/enum.DistanceType.html) -- L2, Cosine, Dot, Hamming.
- [LanceDB Table methods](https://docs.rs/lancedb/latest/lancedb/table/struct.Table.html) -- add, query, create_index, delete, count_rows.
- [LanceDB Vector Search](https://docs.lancedb.com/search/vector-search) -- Distance metrics, pre/post-filtering.
- [async-openai v0.32.4](https://github.com/64bit/async-openai) -- Configurable base URLs, streaming API. Published 2026-01-25.
- [async-openai OpenAIConfig](https://docs.rs/crate/async-openai/latest/source/src/config.rs) -- with_api_base, with_api_key methods.
- [Google Gemini OpenAI Compatibility](https://ai.google.dev/gemini-api/docs/openai) -- Base URL, models, beta status.
- [Mistral AI API](https://docs.mistral.ai/api) -- OpenAI-compatible at `https://api.mistral.ai/v1`.
- [Z.ai GLM-4.7 API](https://docs.z.ai/guides/llm/glm-4.7) -- Base URL, Bearer auth, 200K context, OpenAI compat.
- [text-splitter v0.29.3](https://github.com/benbrandt/text-splitter) -- Semantic chunking, MarkdownSplitter, tiktoken integration.
- [text-splitter docs.rs](https://docs.rs/text-splitter/latest/text_splitter/) -- TextSplitter, ChunkConfig, boundary hierarchy.
- [OpenAI Embeddings API](https://platform.openai.com/docs/api-reference/embeddings) -- Endpoint, request/response format, dimensions parameter.
- Phase 2 codebase -- Existing LlmProvider trait, BoxLlmProvider, AnthropicProvider, BedrockProvider, StreamEvent types.

### Secondary (MEDIUM confidence)
- [failsafe-rs](https://github.com/dmexe/failsafe-rs) -- Circuit breaker patterns reference.
- [claude-max-api-proxy](https://docs.openclaw.ai/providers/claude-max-api-proxy) -- OpenAI-compat proxy at localhost:3456.
- [Anthropic ToS Enforcement](https://generativeai.pub/stop-using-claudes-api-for-moltbot-and-opencode-52f8febd1137) -- January 2026 crackdown.
- [RAG similarity thresholds](https://meisinlee.medium.com/better-rag-retrieval-similarity-with-threshold-a6dbb535ef9e) -- Cosine similarity > 0.3 recommended threshold.
- LanceDB lib.rs listing -- Confirmed version 0.26.2, arrow 57.2 dependencies.

### Tertiary (LOW confidence)
- Claude.ai subscription viability -- Actively blocked. May stop working at any time.
- GLM 4.7 Rust SDK -- No dedicated crate. OpenAI-compatible endpoint is the only path.
- Provider pricing estimates -- Subject to change. Hard-coded table needs periodic review.

## Metadata

**Confidence breakdown:**
- Standard stack (OpenAI/Gemini/Mistral via async-openai): HIGH -- verified docs, configurable base URLs
- Standard stack (Bedrock): HIGH -- existing Phase 2 implementation
- Standard stack (LanceDB + fastembed): MEDIUM-HIGH -- APIs verified, Arrow bridging needs compile-time validation
- Architecture (OpenAI-compat provider): HIGH -- pattern verified against async-openai docs
- Architecture (fallback chain + circuit breaker): HIGH -- custom 3-state design, error classification defined
- Architecture (shared memory trust): MEDIUM-HIGH -- LanceDB filter expressions verified
- Architecture (KV store via SQLite): HIGH -- simple table, existing sqlx patterns
- Architecture (file storage + chunking): MEDIUM-HIGH -- text-splitter verified, filesystem patterns standard
- Claude subscription provider: LOW -- Anthropic ToS violation confirmed
- Pitfalls: HIGH -- all verified against library docs and official sources

**Research date:** 2026-02-12 (post-discussion comprehensive)
**Valid until:** 2026-03-12 (30 days; core patterns are stable)
