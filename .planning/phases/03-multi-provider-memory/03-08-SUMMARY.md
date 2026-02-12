---
phase: 03-multi-provider-memory
plan: 08
subsystem: core, memory, agent
tags: [vector-search, embeddings, long-term-memory, system-prompt, semantic-recall, dedup, re-embed, box-pattern]

# Dependency graph
requires:
  - phase: 03-multi-provider-memory (plan 01)
    provides: VectorMemoryStore trait, VectorMemoryEntry, RankedMemory types
  - phase: 03-multi-provider-memory (plan 04)
    provides: LanceVectorStore, FastEmbedEmbedder, EMBEDDING_DIMENSION
  - phase: 03-multi-provider-memory (plan 07)
    provides: LanceVectorMemoryStore with search, add, dedup, re-embed
  - phase: 02-single-agent-chat-llm (plan 05)
    provides: AgentEngine, AgentContext, SystemPromptBuilder, ChatService
provides:
  - SystemPromptBuilder with <long_term_memory> section from vector search results
  - AgentContext.recalled_memories and set_recalled_memories() for per-message injection
  - ChatService.search_memories_for_message() for vector memory search per user message
  - ChatService.embed_and_store_memories() for embedding extracted memories into LanceDB
  - ChatService.check_and_reembed() for auto re-embed on embedding model change
  - BoxVectorMemoryStore for type-erased dynamic dispatch of VectorMemoryStore trait
  - BoxEmbedder for type-erased dynamic dispatch of Embedder trait
  - Verbose mode logging of injected memories in AgentEngine
affects: [03-11, 03-12, 03-13, all plans using memory recall in chat]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "BoxVectorMemoryStore: VectorMemoryStoreDyn blanket impl + Box<dyn> wrapper (same as BoxLlmProvider)"
    - "BoxEmbedder: EmbedderDyn blanket impl + Box<dyn> wrapper (same pattern)"
    - "Caller-does-search: ChatService searches vector memory, passes results to AgentContext"
    - "Graceful degradation: embed/search failures return empty results with warning log"
    - "System prompt rebuild on set_recalled_memories() to keep prompt in sync"

key-files:
  created:
    - crates/boternity-core/src/memory/box_vector.rs
    - crates/boternity-core/src/memory/box_embedder.rs
  modified:
    - crates/boternity-core/src/agent/context.rs
    - crates/boternity-core/src/agent/engine.rs
    - crates/boternity-core/src/agent/prompt.rs
    - crates/boternity-core/src/chat/service.rs
    - crates/boternity-core/src/memory/mod.rs

key-decisions:
  - "Caller-does-search pattern: ChatService does vector search, passes Vec<RankedMemory> to AgentContext via set_recalled_memories()"
  - "BoxEmbedder and BoxVectorMemoryStore passed as method params not struct fields (optional components)"
  - "System prompt rebuilt on each set_recalled_memories() call to keep <long_term_memory> section current"
  - "DEFAULT_MEMORY_SEARCH_LIMIT=10, DEFAULT_MIN_SIMILARITY=0.3, DEFAULT_DEDUP_THRESHOLD=0.15"
  - "Memories formatted as natural-language facts without scores or metadata (provenance only for shared)"

patterns-established:
  - "set_recalled_memories() triggers rebuild_system_prompt() to keep system prompt in sync"
  - "BoxVectorMemoryStore and BoxEmbedder follow identical Dyn+Box pattern as BoxLlmProvider"
  - "search_memories_for_message() returns Vec (not Result) for graceful degradation"
  - "embed_and_store_memories() batch-embeds then individually dedup-checks and stores"
  - "Verbose mode: INFO-level per-memory logging; normal mode: DEBUG-level count-only"

# Metrics
duration: 6m 31s
completed: 2026-02-12
---

# Phase 3 Plan 08: Vector Memory Recall Integration Summary

**Agent engine and chat service wired to search vector memory per user message, inject recalled facts into system prompt via <long_term_memory> XML section, embed extracted memories into LanceDB with semantic dedup, and auto re-embed on model change -- with BoxVectorMemoryStore and BoxEmbedder for dynamic dispatch**

## Performance

- **Duration:** 6m 31s
- **Started:** 2026-02-12T22:42:02Z
- **Completed:** 2026-02-12T22:48:33Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Added `<long_term_memory>` XML section to SystemPromptBuilder that formats recalled memories as natural-language facts without scores or metadata, with provenance annotation for shared memories
- Added `recalled_memories: Vec<RankedMemory>` and `verbose: bool` to AgentContext with `set_recalled_memories()` that rebuilds the system prompt on each call
- Created BoxVectorMemoryStore and BoxEmbedder following the established Dyn+Box blanket-impl pattern for object-safe dynamic dispatch of RPITIT traits
- Added `search_memories_for_message()`, `embed_and_store_memories()`, and `check_and_reembed()` to ChatService for the full vector memory lifecycle
- AgentEngine logs recalled memories at INFO level in verbose mode (fact, category, relevance score, distance, provenance) and DEBUG level otherwise
- All 108 core tests pass, full workspace compiles and all tests green

## Task Commits

Each task was committed atomically:

1. **Task 1: Add vector memory search to agent engine and system prompt** - `2702529` (feat)
2. **Task 2: Update memory extraction to embed and store in vector DB** - `8230c0a` (feat)

## Files Created/Modified

**Created:**
- `crates/boternity-core/src/memory/box_vector.rs` - BoxVectorMemoryStore with VectorMemoryStoreDyn blanket impl for type-erased dynamic dispatch
- `crates/boternity-core/src/memory/box_embedder.rs` - BoxEmbedder with EmbedderDyn blanket impl for type-erased dynamic dispatch

**Modified:**
- `crates/boternity-core/src/agent/context.rs` - Added recalled_memories, verbose fields, set_recalled_memories(), with_verbose(), rebuild_system_prompt()
- `crates/boternity-core/src/agent/engine.rs` - Added log_recalled_memories() for verbose/debug logging before each LLM call
- `crates/boternity-core/src/agent/prompt.rs` - Added <long_term_memory> section with natural-language formatting and provenance
- `crates/boternity-core/src/chat/service.rs` - Added search_memories_for_message(), embed_and_store_memories(), check_and_reembed() with constants
- `crates/boternity-core/src/memory/mod.rs` - Registered box_embedder and box_vector modules

## Decisions Made
- **Caller-does-search pattern:** ChatService performs vector search and passes results to AgentContext via `set_recalled_memories()`, rather than having AgentEngine do the search. This keeps the engine focused on LLM calls and lets the caller control when/whether to search.
- **Box wrappers as method params:** BoxEmbedder and BoxVectorMemoryStore are passed as method parameters to ChatService (not struct fields), since vector memory is an optional capability that may not be available.
- **System prompt rebuild on each set_recalled_memories():** The system prompt is rebuilt each time recalled memories change, ensuring the `<long_term_memory>` section always reflects the latest search results.
- **Constants for search/dedup:** DEFAULT_MEMORY_SEARCH_LIMIT=10 memories per search, DEFAULT_MIN_SIMILARITY=0.3 cosine distance, DEFAULT_DEDUP_THRESHOLD=0.15 cosine distance (~92.5% similarity).
- **Natural-language formatting:** Recalled memories appear as plain facts ("- User loves Rust programming") without relevance scores, distances, or metadata. Provenance is appended only for shared memories.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Vector memory recall is fully integrated: ChatService can search, embed, store, and re-embed memories
- Ready for Plan 03-11 (memory CLI commands: `bnity memory list/search/delete/add`)
- Ready for Plan 03-12 (shared memory store implementation)
- Ready for Plan 03-13 (audit log and memory lifecycle)
- The chat loop runner in boternity-api needs to call `search_memories_for_message()` before each LLM request and pass results to `agent_context.set_recalled_memories()` -- this wiring happens in a later integration plan
- BoxVectorMemoryStore and BoxEmbedder are ready for use in AppState construction

## Self-Check: PASSED

---
*Phase: 03-multi-provider-memory*
*Completed: 2026-02-12*
