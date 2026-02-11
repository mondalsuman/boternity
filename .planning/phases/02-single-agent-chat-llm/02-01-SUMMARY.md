---
phase: 02-single-agent-chat-llm
plan: 01
subsystem: types, api
tags: [llm, chat, memory, agent, rpitit, domain-types, traits, dynamic-dispatch]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: "boternity-types/boternity-core crate structure, RPITIT pattern, BoxSecretProvider pattern, RepositoryError"
provides:
  - "LLM domain types (CompletionRequest, CompletionResponse, StreamEvent, Usage, LlmError, ProviderCapabilities)"
  - "Chat domain types (ChatSession, ChatMessage, ContextSummary, SessionStatus)"
  - "Agent domain types (AgentConfig)"
  - "Memory domain types (MemoryEntry, MemoryCategory, PendingExtraction)"
  - "LlmProvider trait with RPITIT async methods"
  - "BoxLlmProvider for runtime provider selection via dynamic dispatch"
  - "TokenBudget for context window allocation"
  - "ChatRepository trait for session/message/context-summary CRUD"
  - "MemoryRepository trait for memory entry and pending extraction CRUD"
affects:
  - 02-02 (Anthropic provider implements LlmProvider)
  - 02-03 (SQLite persistence implements ChatRepository, MemoryRepository)
  - 02-04 (Agent engine uses LlmProvider, ChatRepository, AgentConfig, TokenBudget)
  - 02-05 (CLI uses ChatSession, ChatMessage types)
  - 02-06 (Memory extraction uses MemoryRepository, PendingExtraction)
  - 02-07 (REST API uses all domain types)
  - 02-08 (Integration tests use all traits and types)

# Tech tracking
tech-stack:
  added: [futures-util (workspace, added to boternity-core)]
  patterns:
    - "LlmProviderDyn blanket impl for object-safe dynamic dispatch of RPITIT traits"
    - "Pin<Box<dyn Stream>> for streaming responses (not RPITIT, needs object safety)"
    - "TokenBudget percentage allocation: soul 15%, memory 10%, user_context 5%, conversation 70%"
    - "ContextSummary on ChatRepository (session-scoped, not memory-scoped)"

key-files:
  created:
    - "crates/boternity-types/src/llm.rs"
    - "crates/boternity-types/src/chat.rs"
    - "crates/boternity-types/src/agent.rs"
    - "crates/boternity-types/src/memory.rs"
    - "crates/boternity-core/src/llm/mod.rs"
    - "crates/boternity-core/src/llm/provider.rs"
    - "crates/boternity-core/src/llm/box_provider.rs"
    - "crates/boternity-core/src/llm/token_budget.rs"
    - "crates/boternity-core/src/llm/types.rs"
    - "crates/boternity-core/src/chat/mod.rs"
    - "crates/boternity-core/src/chat/repository.rs"
    - "crates/boternity-core/src/memory/mod.rs"
    - "crates/boternity-core/src/memory/store.rs"
  modified:
    - "crates/boternity-types/src/lib.rs"
    - "crates/boternity-core/src/lib.rs"
    - "crates/boternity-core/Cargo.toml"

key-decisions:
  - "MessageRole defined in llm.rs and re-exported from chat.rs (single source of truth)"
  - "StreamEvent uses tagged enum with serde(tag=type) for JSON parsing"
  - "BoxLlmProvider delegates via LlmProviderDyn blanket impl (same pattern as BoxSecretProvider)"
  - "stream() returns Pin<Box<dyn Stream>> not RPITIT (needs object safety for BoxLlmProvider)"
  - "ContextSummary on ChatRepository not MemoryRepository (session-scoped)"
  - "TokenBudget allocation: soul 15%, memory 10%, user_context 5%, conversation 70%"
  - "Summarization triggers at 80% of conversation budget"
  - "uuid added to boternity-core dependencies for Uuid parameters in new repository traits"

patterns-established:
  - "LlmProviderDyn + blanket impl pattern for boxing RPITIT traits with streams"
  - "TokenBudget percentage-based context window partitioning"
  - "Chat/Memory repository traits alongside existing repository module"

# Metrics
duration: 5min
completed: 2026-02-11
---

# Phase 2 Plan 01: Domain Types and Trait Abstractions Summary

**LLM/Chat/Memory/Agent domain types in boternity-types with LlmProvider (RPITIT + BoxLlmProvider), ChatRepository, and MemoryRepository traits in boternity-core**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-11T22:41:45Z
- **Completed:** 2026-02-11T22:46:37Z
- **Tasks:** 3
- **Files modified:** 16

## Accomplishments
- Complete LLM domain type vocabulary: request/response, streaming events, usage tracking, error handling, provider capabilities
- Chat session and message types with ContextSummary for sliding window context management
- Memory types with category-based classification and pending extraction jobs
- LlmProvider trait with RPITIT async + BoxLlmProvider for runtime provider selection
- ChatRepository and MemoryRepository traits with full CRUD using RPITIT pattern
- TokenBudget allocating context window across soul/memory/user_context/conversation priorities

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Phase 2 domain types in boternity-types** - `a1f74bb` (feat)
2. **Task 2: Create LlmProvider trait, BoxLlmProvider, and TokenBudget** - `e73041c` (feat)
3. **Task 3: Create ChatRepository and MemoryRepository traits** - `88db7ef` (feat)

## Files Created/Modified
- `crates/boternity-types/src/llm.rs` - LLM request/response types, StreamEvent, LlmError, ProviderCapabilities
- `crates/boternity-types/src/chat.rs` - ChatSession, ChatMessage, ContextSummary, SessionStatus
- `crates/boternity-types/src/agent.rs` - AgentConfig for bot identity + LLM parameters
- `crates/boternity-types/src/memory.rs` - MemoryEntry, MemoryCategory, PendingExtraction
- `crates/boternity-types/src/lib.rs` - Added agent, chat, llm, memory modules
- `crates/boternity-core/src/llm/provider.rs` - LlmProvider trait with RPITIT
- `crates/boternity-core/src/llm/box_provider.rs` - BoxLlmProvider with LlmProviderDyn blanket impl
- `crates/boternity-core/src/llm/token_budget.rs` - TokenBudget context window allocation
- `crates/boternity-core/src/llm/types.rs` - Re-exports from boternity_types::llm
- `crates/boternity-core/src/llm/mod.rs` - LLM module root
- `crates/boternity-core/src/chat/repository.rs` - ChatRepository trait
- `crates/boternity-core/src/chat/mod.rs` - Chat module root
- `crates/boternity-core/src/memory/store.rs` - MemoryRepository trait
- `crates/boternity-core/src/memory/mod.rs` - Memory module root
- `crates/boternity-core/src/lib.rs` - Added chat, llm, memory modules
- `crates/boternity-core/Cargo.toml` - Added futures-util, uuid dependencies

## Decisions Made
- **MessageRole single source of truth:** Defined in llm.rs, re-exported from chat.rs. Both LLM and chat contexts use the same enum.
- **stream() not RPITIT:** Returns `Pin<Box<dyn Stream>>` because streams need to be object-safe for BoxLlmProvider. The `complete()` and `count_tokens()` methods use RPITIT.
- **ContextSummary on ChatRepository:** Context summaries are session-scoped (belong to chat conversations), not bot-scoped (like memories). Placed on ChatRepository accordingly.
- **TokenBudget 80% summarization threshold:** Triggers context summarization when conversation tokens exceed 80% of the conversation budget, leaving 20% headroom.
- **uuid dependency added to boternity-core:** New repository traits use `Uuid` directly for session/message/memory IDs (not wrapped ID types like `BotId`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added uuid dependency to boternity-core**
- **Found during:** Task 3 (ChatRepository and MemoryRepository traits)
- **Issue:** ChatRepository and MemoryRepository traits use `Uuid` parameters but uuid was not in boternity-core's Cargo.toml dependencies
- **Fix:** Added `uuid = { workspace = true }` to boternity-core/Cargo.toml
- **Files modified:** crates/boternity-core/Cargo.toml
- **Verification:** `cargo check -p boternity-core` passes
- **Committed in:** 88db7ef (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minimal -- uuid was already a workspace dependency, just needed to be referenced from boternity-core. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All domain types and trait contracts are defined for Phase 2
- Next plans can implement: Anthropic provider (02-02), SQLite persistence (02-03), agent engine (02-04)
- No blockers or concerns
- Workspace compiles cleanly with 80 tests passing (41 types + 39 core)

## Self-Check: PASSED

---
*Phase: 02-single-agent-chat-llm*
*Completed: 2026-02-11*
