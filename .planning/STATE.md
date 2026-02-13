# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-10)

**Core value:** A user can create a bot with a distinct identity, give it skills through an interactive builder, and have meaningful parallel conversations with it -- all running locally with full observability.
**Current focus:** Phase 6 (Skill System + WASM Sandbox) - In progress

## Current Position

Phase: 6 of 10 (Skill System + WASM Sandbox)
Plan: 1 of 12 in current phase
Status: In progress
Last activity: 2026-02-14 -- Completed 06-01-PLAN.md (Skill domain types + workspace deps)

Progress: [███████████████████████████████████████████████░░░░░░] 43/54 (~80%)

## Performance Metrics

**Velocity:**
- Total plans completed: 43
- Average duration: 6m 25s
- Total execution time: 276m 40s

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. Foundation + Bot Identity | 6/6 | 49m 14s | 8m 12s |
| 2. Single-Agent Chat + LLM | 7/8 | 31m 46s | 4m 32s |
| 3. Multi-Provider + Memory | 13/13 | 127m 31s | 9m 49s |
| 4. Web UI Core + Fleet Dashboard | 8/8 | 40m 37s | 5m 05s |
| 5. Agent Hierarchy + Event System | 8/8 | 28m 00s | 3m 30s |
| 6. Skill System + WASM Sandbox | 1/12 | 3m 32s | 3m 32s |

**Recent Trend:**
- Last 5 plans: 05-06 (3m 00s), 05-07 (4m 00s), 05-08 (4m 00s), 06-01 (3m 32s)
- Trend: Consistent ~3-4m for foundational/types plans

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
- [03-07]: delete+insert for update_embedding: LanceDB UpdateBuilder uses SQL expressions only, cannot set vector column from Vec<f32>
- [03-07]: Search over-fetches limit*2 then filters by min_similarity and truncates
- [03-07]: update_embedding scans all bot_memory_* tables since trait lacks bot_id parameter
- [03-07]: DEFAULT_DEDUP_THRESHOLD = 0.15 cosine distance (~92.5% similarity)
- [03-07]: lancedb::query::{ExecutableQuery, QueryBase} must be imported for .execute()/.limit()/.only_if()
- [03-10]: detect_mime and is_text_mime as module-level functions in storage/mod.rs (not struct methods)
- [03-10]: RecordBatchIterator wraps Vec<Ok(batch)> for LanceDB table.add() (Vec<RecordBatch> lacks RecordBatchReader)
- [03-10]: FixedSizeListArray::try_new(field, size, values, None) for arrow-array 57.3 (try_new_from_values is lance_arrow extension)
- [03-10]: Bot files stored under {base_dir}/bots/{bot_id.simple()}/files/ with .versions/ subdirectory
- [03-10]: MockEmbedder pattern for testing vector operations without embedding model download
- [03-10]: Rust 2024 edition requires explicit type annotations on closure params with .clone() in iterator chains
- [03-09]: SHA-256 write_hash covers id+fact+category+importance+author_bot_id+trust_level+created_at (not embedding)
- [03-09]: Share/revoke uses delete+insert pattern (same as 03-07 update_embedding) to preserve vector column
- [03-09]: Shared memory search uses raw similarity as relevance (no time-decay or access tracking)
- [03-09]: Per-bot cap checked on every add() via count_by_author query
- [03-09]: Trust filter SQL: OR-combined clauses for public, author-self, trusted-list access
- [03-09]: extract_embedding_from_batch reads FixedSizeListArray for delete+insert ops
- [03-08]: Caller-does-search: ChatService searches vector memory, passes Vec<RankedMemory> to AgentContext
- [03-08]: BoxEmbedder and BoxVectorMemoryStore passed as method params not struct fields (optional components)
- [03-08]: System prompt rebuilt on each set_recalled_memories() call to keep <long_term_memory> section current
- [03-08]: DEFAULT_MEMORY_SEARCH_LIMIT=10, DEFAULT_MIN_SIMILARITY=0.3, DEFAULT_DEDUP_THRESHOLD=0.15
- [03-08]: Memories formatted as natural-language facts without scores/metadata (provenance only for shared)
- [03-12]: shared-memory is a separate top-level subcommand (bnity shared-memory), not nested under bnity memory
- [03-12]: Optional vector/embedder parameters on remember() and delete_memory() for graceful degradation
- [03-12]: Similarity score color-coding: green>=0.7, yellow>=0.4, red<0.4 in CLI search output
- [03-12]: Export outputs structured JSON with bot metadata (slug, name, exported_at, count, memories array)
- [03-12]: Zero-embedding for shared-memory list (no semantic filtering, returns all visible entries)
- [03-13]: KV set parses value as JSON with fallback to string (natural UX for both simple and structured values)
- [03-13]: Storage delete deindexes from vector store before file removal (prevents orphaned chunks)
- [03-13]: Two FastEmbedEmbedder instances in AppState: one concrete for FileIndexer<E>, one boxed for BoxEmbedder
- [03-13]: Three separate LanceVectorStore connections for vector_store/vector_memory/shared_memory (each takes ownership)
- [03-11]: Provider configs persisted in ~/.boternity/providers.json (simple JSON array, not SQLite)
- [03-11]: Circuit breaker state is session-scoped (resets each chat), not persisted in provider status
- [03-11]: Verbose mode uses short flag -V (not -v which is taken by global verbosity counter)
- [03-11]: Vector memory search integrated directly in chat loop with Option<BoxVectorMemoryStore> for graceful fallback
- [03-11]: Provider add tests connection by default, --skip-test to bypass
- [04-01]: async_stream::stream! for SSE (avoids complex Pin<Box> manual construction, produces Send stream)
- [04-01]: Direct SQL for stats endpoint (efficient COUNT with conditional aggregation instead of service-layer list+count)
- [04-01]: ChatRepository trait extended with clear_messages, count_sessions, count_messages (rather than separate stats repository)
- [04-01]: SPA fallback via BOTERNITY_WEB_DIR env var with graceful degradation when dir absent
- [04-01]: Conversation history loaded into AgentContext for session continuation in streaming endpoint
- [04-02]: Sonner component rewritten to use Zustand theme store instead of next-themes (avoiding unnecessary dependency)
- [04-02]: Dark theme as :root default with .light class override (not .dark class, dark-first design)
- [04-02]: Bot detail uses TanStack Router layout route (route.tsx) for shared tab navigation across child routes
- [04-02]: SidebarProvider wraps entire app for consistent sidebar state across all routes
- [04-02]: TooltipProvider at root level for sidebar tooltip support on collapsed rail
- [04-02]: TanStack Router/Query devtools lazy-loaded only in development mode
- [04-03]: Client-side search/sort for bot grid (single-user app, small bot counts, immediate responsiveness)
- [04-03]: DropdownMenu RadioGroup for sort picker instead of Select component (simpler, consistent)
- [04-03]: placeholderData: (prev) => prev in useBots for smooth filter transitions
- [04-03]: Simple 16-emoji grid picker instead of full emoji picker library
- [04-03]: AlertDialog added as shadcn component for destructive action confirmations
- [04-04]: ChatLayout shared wrapper for sibling routes (/chat/ and /chat/$sessionId are siblings not nested under TanStack Router)
- [04-04]: fetch + ReadableStream for SSE (POST body required, EventSource only supports GET)
- [04-04]: Functional updater setStreamedContent(prev => prev + text) avoids stale closure during rapid token updates
- [04-04]: Isolated StreamingMessage component prevents full message list re-render on each token delta
- [04-04]: AbortController for stop generation + unmount cleanup pattern
- [04-06]: Active tab detection via useMatchRoute instead of defaultValue on Tabs (tracks URL changes)
- [04-06]: Identity form rebuilds raw IDENTITY.md frontmatter on every change (preserves body content)
- [04-06]: Local editor buffers populated once from fetch data, then managed locally (prevents overwrite on refetch)
- [04-06]: shadcn/ui primitives (Label, Slider, Select, Switch) added for identity form controls
- [04-05]: github-dark highlight.js theme for code block syntax coloring (matches dark-first design)
- [04-05]: extractTextContent helper traverses React node tree for code copy (no DOM refs needed)
- [04-05]: data-copied attribute drives icon swap on copy button (avoids re-render for visual feedback)
- [04-05]: highlight.js added as direct dependency for CSS import (transitive dep through lowlight not importable in pnpm strict mode)
- [04-07]: Version timeline as collapsible right panel (280px) with smooth width transition, collapsed by default
- [04-07]: DiffViewer in large Dialog overlay (max-w-6xl, 80vh) rather than replacing editor pane
- [04-07]: Rollback uses AlertDialog for destructive confirmation with scrollable content preview
- [04-07]: Version actions (Compare, Restore) visible only when version is selected in timeline
- [04-07]: useSoulVersion has staleTime: Infinity since versions are immutable
- [05-01]: tokio-util 0.7 without feature gates (CancellationToken available by default, no sync feature exists)
- [05-01]: toml as dev-dependency on boternity-types (only tests need TOML parsing)
- [05-01]: source_agent_id: Option<Uuid> on MemoryEntry with None default for backward compatibility
- [05-01]: AgentEvent serde tagged union: #[serde(tag = "type", rename_all = "snake_case")] for event bus
- [05-02]: Clone-on-read for SharedWorkspace::get() to prevent DashMap Ref held across await
- [05-02]: HashMap<u64, usize> for CycleDetector instead of HashSet (tracks repetition count, not just presence)
- [05-02]: RequestContext.child() uses saturating_add for depth to prevent u8 overflow
- [05-02]: EventBus publish silently drops events when no subscribers (let _ = sender.send())
- [05-03]: Only first <spawn_agents> block parsed per response (single spawn per turn)
- [05-03]: Default spawn mode is Parallel when no mode attribute present
- [05-03]: Sub-agent prompts exclude user_context/session_memory/long_term_memory (fresh context)
- [05-03]: Depth < 3 includes agent_capabilities for recursive spawning; depth 3 excludes it
- [05-04]: Orchestrator is stateless coordinator: no fields beyond max_depth, all state via parameters
- [05-04]: BoxLlmProvider streams created before JoinSet spawn (stream is 'static, provider is not Clone)
- [05-04]: Token estimation via 4 chars/token heuristic for streaming budget, corrected by real Usage events
- [05-04]: Sequential sub-agents see only immediately prior result (not full chain) per user decision
- [05-04]: Memory extraction deferred to chat handler via AgentMemoryContext (orchestrator surfaces data, caller extracts)
- [05-05]: default_pricing_table() is private (not pub) since external callers use estimate_cost()
- [05-05]: OpenAI gpt-4o-mini entry ordered before gpt-4o for correct prefix matching
- [05-05]: Bedrock uses contains() fallback for region-prefixed model IDs
- [05-05]: Minimum budget floor of 10,000 tokens in resolve_request_budget()
- [05-06]: tokio::select! single-loop for WebSocket instead of socket.split() two-task (enables Ping/Pong in same scope)
- [05-06]: WsCommand serde tagged enum matches AgentEvent convention (#[serde(tag = "type", rename_all = "snake_case")])
- [05-06]: DashMap for agent_cancellations and budget_responses (concurrent access from WebSocket + orchestrator)
- [05-06]: WebSocket disconnect does NOT auto-cancel agents (reconnection-safe, per research pitfall 4)
- [05-07]: Two-path execution in loop_runner: parse_spawn_instructions() on initial response decides orchestrator vs direct stream
- [05-07]: HTTP orchestrator runs in tokio::spawn with mpsc channel result delivery, EventBus subscriber in SSE stream loop
- [05-07]: Budget warning auto-continues in CLI (stdin reading during orchestrator execution deferred as TODO)
- [05-07]: --quiet / -q flag on Chat command suppresses sub-agent detail, showing only final synthesis
- [05-08]: Native WebSocket API (no npm dep) with exponential backoff 1s-30s, 30% jitter, max 10 attempts
- [05-08]: Map-based Zustand store with functional updates instead of immer for agent tree
- [05-08]: AgentEvent forwarded from SSE via useAgentStore.getState().handleEvent() (outside React lifecycle)
- [05-08]: AgentBlock auto-collapses on completion, auto-expands on running (useEffect on status)
- [05-08]: Recursive TreeNode component with depth-based paddingLeft for tree indentation
- [05-08]: Blended $9/1M cost estimate for budget indicator (rough hint, not exact billing)
- [06-01]: TrustTier::Untrusted as Default (secure by default)
- [06-01]: Capability enum with 8 variants matching CONTEXT.md fine-grained operations
- [06-01]: SkillMeta uses semver::Version for version field (strong typing)
- [06-01]: SkillSource tagged enum with type=local or type=registry
- [06-01]: ResourceLimits defaults: 64MB memory, 1M fuel, 30s duration
- [06-01]: landlock declared in workspace but NOT wired to any crate (Plan 08 will add with cfg gates)

### Pending Todos

None yet.

### Blockers/Concerns

- [Research]: Dual-GraphQL architecture (Yoga+Pothos BFF vs async-graphql alone) needs validation in Phase 4
- [Research]: `llm` crate (graniet) v1.2.4 is newer -- may need fallback to thin reqwest wrapper if API unstable
- [Resolved]: LanceDB selected for vector storage (03-04 implemented LanceVectorStore)

## Session Continuity

Last session: 2026-02-14
Stopped at: Completed 06-01-PLAN.md (Skill domain types + workspace deps)
Resume file: None
