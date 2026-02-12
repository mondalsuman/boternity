# Phase 3: Multi-Provider + Memory - Context

**Gathered:** 2026-02-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Bots can use any of multiple LLM providers (OpenAI, Gemini, Mistral, Bedrock, GLM 4.7, Claude.ai subscription) with automatic failover, remember things long-term via vector embeddings, share knowledge across bots with trust-level partitioning, and store files and structured data per-bot. This phase does NOT include web UI (Phase 4), agent hierarchy (Phase 5), or skill system (Phase 6).

</domain>

<decisions>
## Implementation Decisions

### Failover experience
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

### Memory recall in chat
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

### Shared memory trust model
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

### Per-bot file storage
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

</decisions>

<specifics>
## Specific Ideas

- Provider health CLI: something like `bnity provider status` showing all providers with circuit state, last error, uptime
- Memory search should show cosine similarity scores (e.g., 0.87) in CLI output
- File storage acts as a "personal knowledge base" -- text files chunked, embedded, and semantically searchable
- Shared memory browsable via dedicated `bnity shared-memory` subcommand, not mixed into `bnity memory`
- In-chat "forget" command alongside CLI deletion gives two paths for memory management
- Provider priority numbers with auto tie-breaking (by latency or cost) rather than explicit ordering

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope

</deferred>

---

*Phase: 03-multi-provider-memory*
*Context gathered: 2026-02-12*
