# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-10)

**Core value:** A user can create a bot with a distinct identity, give it skills through an interactive builder, and have meaningful parallel conversations with it -- all running locally with full observability.
**Current focus:** Phase 3 (Multi-Provider + Memory) - In progress

## Current Position

Phase: 3 of 10 (Multi-Provider + Memory)
Plan: 6 of 13 in current phase (03-01, 03-02, 03-03, 03-04, 03-05, 03-06 complete)
Status: In progress
Last activity: 2026-02-12 -- Completed 03-06-PLAN.md (Provider wiring + fallback chain integration)

Progress: [███████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 19/53 (~36%)

## Performance Metrics

**Velocity:**
- Total plans completed: 19
- Average duration: 7m 51s
- Total execution time: 149m 13s

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. Foundation + Bot Identity | 6/6 | 49m 14s | 8m 12s |
| 2. Single-Agent Chat + LLM | 7/8 | 31m 46s | 4m 32s |
| 3. Multi-Provider + Memory | 6/13 | 68m 13s | 11m 22s |

**Recent Trend:**
- Last 5 plans: 03-02 (13m 36s), 03-03 (9m 30s), 03-05 (12m 45s), 03-04 (16m 27s), 03-06 (10m 48s)
- Trend: Phase 3 plans longer due to external dep compilation and larger scope

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Roadmap]: 10 phases derived from 109 requirements following dependency chain: types -> core -> infra -> api
- [Roadmap]: SOUL.md immutability enforced from Phase 1 (CVE-2026-25253 mitigation)
- [Roadmap]: boternity-core must never depend on boternity-infra (dependency inversion)
- [Roadmap]: Security concerns front-loaded into the phase where their attack surface first appears
- [01-01]: Rust 2024 edition with resolver 3 and native async fn in traits (RPITIT, no async_trait)
- [01-01]: UUID v7 for all entity IDs (time-sortable, process-local ordering)
- [01-01]: BotStatus: Active/Disabled/Archived (lifecycle states from CONTEXT.md)
- [01-01]: Identity defaults: claude-sonnet-4-20250514, temperature 0.7, max_tokens 4096
- [01-01]: Redacted wrapper pattern for secret values (custom Debug/Display)
- [01-01]: Repository traits return impl Future (RPITIT) not Box<dyn Future>
- [01-02]: Split read/write SQLite pools (8 readers, 1 writer) with WAL mode on both
- [01-02]: Private BotRow struct for SQLite-to-domain mapping (no sqlx derives on domain types)
- [01-02]: Secrets scope stored as string not FK (allows pre-provisioned keys)
- [01-02]: Sort field whitelist in list() to prevent SQL injection
- [01-02]: Transaction for soul save (INSERT + UPDATE version_count atomically)
- [01-03]: Generic services (BotService<B, S, F, H>) over trait objects -- RPITIT traits not object-safe
- [01-03]: Free functions for content generation (generate_default_soul, etc.) -- no trait bounds needed for static calls
- [01-03]: Simple line-based YAML frontmatter parser -- avoids serde_yaml dep for narrow use case
- [01-03]: LocalFileSystem auto-creates parent dirs on write -- prevents missing dir errors
- [01-04]: BoxSecretProvider with blanket impl for object-safe dynamic dispatch of RPITIT traits
- [01-04]: Fixed Argon2id salt "boternity-vault-v1" for password KDF (password provides entropy)
- [01-04]: Auto-generated master key in OS keychain as zero-friction default
- [01-04]: Secret<T> generic wrapper alongside existing Redacted(String)
- [01-06]: LCS-based line diff in pure Rust (no external diff library)
- [01-06]: Message field on Soul struct for version commit messages
- [01-06]: update_soul saves DB first then file (DB failure leaves disk unchanged)
- [01-06]: bnity check enhanced with soul integrity verification
- [02-01]: MessageRole defined in llm.rs, re-exported from chat.rs (single source of truth)
- [02-01]: stream() returns Pin<Box<dyn Stream>> not RPITIT (needs object safety for BoxLlmProvider)
- [02-01]: BoxLlmProvider follows same LlmProviderDyn blanket impl pattern as BoxSecretProvider
- [02-01]: ContextSummary on ChatRepository not MemoryRepository (session-scoped)
- [02-01]: TokenBudget allocation: soul 15%, memory 10%, user_context 5%, conversation 70%
- [02-01]: Summarization triggers at 80% of conversation budget
- [02-02]: OnceLock for OTel provider storage -- opentelemetry 0.31 removed global shutdown, store in OnceLock
- [02-02]: stdout exporter for dev -- opentelemetry-stdout for local development, swappable for OTLP
- [02-03]: SSE event dispatch via match on event type string, not serde tag on outer enum
- [02-03]: Model capabilities derived from model name substring matching (sonnet/opus/haiku)
- [02-03]: Empty tool use JSON buffer produces empty JSON object (not null or parse error)
- [02-03]: AnthropicProvider does not derive Debug (defense-in-depth for API key)
- [02-04]: save_message atomically increments session message_count (prevents drift)
- [02-04]: get_pending_extractions filters attempt_count < 3 (max retry policy in query)
- [02-04]: ON DELETE CASCADE on chat_sessions cascades to messages and summaries (not memories)
- [02-05]: XML tag boundaries for system prompt sections (<soul>, <identity>, <user_context>, <session_memory>, <instructions>)
- [02-05]: Character-based token estimation (4 chars/token) for should_summarize()
- [02-05]: StreamInSpan unsafe pin projection to keep OTel span alive during streaming
- [02-05]: Memory extraction interval: every 10 turns via SessionManager
- [02-05]: ChatService<C, M> generic over ChatRepository + MemoryRepository
- [02-06]: Stateless LLM utility pattern: struct with no fields, BoxLlmProvider passed per-call
- [02-06]: Extraction prompt returns JSON array of {fact, category, importance}; empty array for nothing worth extracting
- [02-06]: Graceful JSON parse degradation: log warning, return empty result, caller queues retry
- [02-06]: Title generation: temperature 0.3, max_tokens 50, trims whitespace and quotes
- [02-08]: Manual memories use Uuid::nil() session_id (not linked to any session)
- [02-08]: ConcreteChatService type alias pins ChatService<SqliteChatRepository, SqliteMemoryRepository> on AppState
- [02-08]: Session/memory IDs parsed from String CLI args via Uuid::parse for user-friendly errors
- [03-01]: ProviderType explicit serde rename for OpenAiCompatible to get 'openai_compatible'
- [03-01]: FallbackChainConfig defaults: 5000ms rate_limit_queue_timeout, 3.0x cost_warning_multiplier
- [03-01]: TrustLevel::Private as default (private by default for shared memories)
- [03-01]: CircuitState uses Instant for timing (monotonic clock correctness)
- [03-01]: ProviderHealth defaults: failure_threshold=3, success_threshold=1, open_duration=30s
- [03-01]: is_failover_error: Provider/Stream/RateLimited/Overloaded trigger failover; AuthenticationFailed/InvalidRequest/ContextLengthExceeded do not
- [03-02]: async-openai requires chat-completion feature to enable _api gate (Client, Chat, streaming types)
- [03-02]: OpenAI types under async_openai::types::chat not async_openai::types
- [03-02]: async_stream::try_stream! in Rust 2024 needs explicit type annotations (no ref patterns)
- [03-02]: max_tokens maps to max_completion_tokens in OpenAI API
- [03-02]: OpenAiCompatibleProvider does not derive Debug (defense-in-depth, same as AnthropicProvider)
- [03-03]: FallbackChain::complete() returns FallbackResult struct (response + provider_name + failover_warning)
- [03-03]: select_stream() instead of stream() -- separates provider selection from stream consumption for borrow checker compliance
- [03-03]: record_stream_success/failure for caller to report stream outcome (can't track from 'static stream)
- [03-03]: Priority tiebreaking: latency first, then alphabetical name
- [03-03]: Cost warning uses average of input+output cost per million for ratio comparison
- [03-05]: bot_kv_store uses composite PK (bot_id, key) -- natural key enforces uniqueness
- [03-05]: memory_audit_log.memory_id is TEXT not FK -- memory may be deleted while audit persists
- [03-05]: ProviderHealthRow separate from runtime ProviderHealth -- Instant not serializable
- [03-05]: provider_health keyed by name TEXT not UUIDv7 -- names are unique identifiers
- [03-05]: bot_files UNIQUE(bot_id, filename) enables upsert on re-upload
- [03-04]: Arrow version 57.3 pinned to match lancedb 0.26 transitive dep (Pitfall 10)
- [03-04]: Arc<Mutex<TextEmbedding>> not Arc<TextEmbedding> because fastembed embed() requires &mut self
- [03-04]: RepositoryError::Query used for embedding/vector errors (no Internal variant)
- [03-04]: TextInitOptions builder pattern required (non_exhaustive struct)
- [03-04]: drop_table is idempotent (returns Ok on TableNotFound)
- [03-06]: ClaudeSubscriptionProvider as thin wrapper over OpenAiCompatibleProvider (no separate protocol needed)
- [03-06]: Provider factory create_provider() in boternity-infra, not CLI layer (infrastructure concern)
- [03-06]: FallbackChain built lazily via build_fallback_chain() not at AppState::init() (requires API key)
- [03-06]: create_single_provider() retained for utility calls (title gen, memory extraction)
- [03-06]: Chat loop uses FallbackChain directly instead of AgentEngine for streaming
- [03-06]: build_completion_request() free function replicates AgentEngine request building for chain use
- [03-06]: Stats footer shows 'model via provider_name' only during failover

### Pending Todos

None yet.

### Blockers/Concerns

- [Research]: Dual-GraphQL architecture (Yoga+Pothos BFF vs async-graphql alone) needs validation in Phase 4
- [Research]: `llm` crate (graniet) v1.2.4 is newer -- may need fallback to thin reqwest wrapper if API unstable
- [Resolved]: LanceDB selected for vector storage (03-04 implemented LanceVectorStore)

## Session Continuity

Last session: 2026-02-12T22:34:33Z
Stopped at: Completed 03-06-PLAN.md (Provider wiring + fallback chain integration)
Resume file: None
