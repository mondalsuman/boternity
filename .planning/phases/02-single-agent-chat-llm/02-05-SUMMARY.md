---
phase: 02-single-agent-chat-llm
plan: 05
subsystem: core, agent, chat
tags: [agent-engine, system-prompt, xml-tags, streaming, otel, session-manager, chat-service, token-budget]

# Dependency graph
requires:
  - phase: 02-single-agent-chat-llm
    plan: 01
    provides: "LlmProvider/BoxLlmProvider, CompletionRequest/Response/StreamEvent, AgentConfig, TokenBudget, ChatRepository, MemoryRepository, MemoryEntry"
  - phase: 02-single-agent-chat-llm
    plan: 03
    provides: "AnthropicProvider implementing LlmProvider (usable via BoxLlmProvider)"
  - phase: 02-single-agent-chat-llm
    plan: 04
    provides: "SqliteChatRepository implementing ChatRepository, SqliteMemoryRepository implementing MemoryRepository"
provides:
  - "AgentEngine with execute() returning Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>>>>"
  - "SystemPromptBuilder assembling soul + identity + user + memories with XML tag boundaries"
  - "AgentContext holding conversation state, personality, and token budget"
  - "ChatService<C: ChatRepository, M: MemoryRepository> for session lifecycle management"
  - "SessionManager for turn tracking and memory extraction scheduling"
affects:
  - 02-06 (memory extraction uses AgentEngine for LLM-based extraction)
  - 02-07 (CLI chat uses AgentEngine.execute() for streaming and ChatService for persistence)
  - 02-08 (integration tests use AgentEngine + ChatService end-to-end)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "XML-tagged system prompt sections (<soul>, <identity>, <user_context>, <session_memory>, <instructions>)"
    - "StreamInSpan wrapper keeps OTel span alive for stream duration via unsafe pin projection"
    - "ChatService generic over ChatRepository + MemoryRepository (same pattern as BotService)"
    - "SessionManager turn-based memory extraction scheduling (every 10 turns)"

key-files:
  created:
    - "crates/boternity-core/src/agent/mod.rs"
    - "crates/boternity-core/src/agent/engine.rs"
    - "crates/boternity-core/src/agent/context.rs"
    - "crates/boternity-core/src/agent/prompt.rs"
    - "crates/boternity-core/src/chat/service.rs"
    - "crates/boternity-core/src/chat/session.rs"
  modified:
    - "crates/boternity-core/src/chat/mod.rs"
    - "crates/boternity-core/src/lib.rs"

key-decisions:
  - "XML tag boundaries for system prompt sections: <soul>, <identity>, <user_context>, <session_memory>, <instructions>"
  - "Character-based token estimation (4 chars/token) for should_summarize() -- exact counting deferred to API call"
  - "StreamInSpan uses unsafe pin projection to keep OTel span alive during streaming"
  - "Memory extraction interval: every 10 turns via SessionManager"
  - "ChatService generic over ChatRepository + MemoryRepository, matching BotService pattern"

patterns-established:
  - "XML-tagged system prompt: clear section boundaries for LLM to distinguish personality sources"
  - "StreamInSpan: unsafe pin projection pattern for wrapping streams with metadata"
  - "SessionManager: turn-based scheduling for periodic background tasks"
  - "ChatService lifecycle: create_session -> save_user_message -> save_assistant_message -> end_session"

# Metrics
duration: 4min
completed: 2026-02-11
---

# Phase 2 Plan 05: Agent Engine and Chat Service Summary

**AgentEngine streaming LLM calls through BoxLlmProvider with OTel spans, SystemPromptBuilder assembling XML-tagged personality prompt, and ChatService managing full session lifecycle**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-11T22:59:46Z
- **Completed:** 2026-02-11T23:03:46Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- AgentEngine with execute() (streaming), execute_non_streaming(), and generate_greeting() methods, all instrumented with OTel GenAI spans
- SystemPromptBuilder composing XML-tagged system prompt from SOUL.md + IDENTITY.md config + USER.md + session memories
- AgentContext holding conversation state with token budget awareness and should_summarize() threshold detection
- ChatService<C, M> orchestrating session lifecycle: create, message save (user/assistant), title update, end session, memory loading
- SessionManager tracking turns with memory extraction scheduling every 10 turns
- 22 new tests (10 prompt/context, 2 engine, 7 session, 1 service type-check), 61 total boternity-core tests passing

## Task Commits

Each task was committed atomically:

1. **Task 1: Create SystemPromptBuilder and AgentContext** - `8a44acd` (feat)
2. **Task 2: Create AgentEngine and ChatService** - `2bb2a32` (feat)

## Files Created/Modified
- `crates/boternity-core/src/agent/mod.rs` - Agent module root with context, engine, prompt submodules
- `crates/boternity-core/src/agent/engine.rs` - AgentEngine with streaming/non-streaming LLM execution and OTel spans
- `crates/boternity-core/src/agent/context.rs` - AgentContext holding conversation state, personality, and token budget
- `crates/boternity-core/src/agent/prompt.rs` - SystemPromptBuilder assembling XML-tagged system prompt
- `crates/boternity-core/src/chat/service.rs` - ChatService<C, M> orchestrating session lifecycle and persistence
- `crates/boternity-core/src/chat/session.rs` - SessionManager with turn tracking and memory extraction scheduling
- `crates/boternity-core/src/chat/mod.rs` - Added service and session module declarations
- `crates/boternity-core/src/lib.rs` - Added agent module declaration

## Decisions Made
- **XML tag boundaries for system prompt:** Each personality source (soul, identity, user context, memories, instructions) is wrapped in distinct XML tags. This gives the LLM clear section boundaries for understanding what each piece of the system prompt represents, matching the established pattern from CONTEXT.md.
- **Character-based token estimation:** `should_summarize()` uses a 4 chars/token heuristic rather than calling the provider's `count_tokens()`. This avoids an async API call in a sync context and is conservative enough for the 80% summarization threshold.
- **StreamInSpan with unsafe pin projection:** The OTel span must live as long as the stream. A `StreamInSpan` wrapper uses unsafe pin projection to borrow the span immutably while polling the inner stream mutably -- necessary because `Span::enter()` borrows and `poll_next` needs `&mut`.
- **Memory extraction every 10 turns:** `SessionManager::should_extract_memory()` returns true at turn 10, 20, 30, etc. This balances extraction frequency against LLM cost, matching CONTEXT.md's "periodic during session (every N messages)" requirement.
- **ChatService generic pattern:** `ChatService<C: ChatRepository, M: MemoryRepository>` follows the same generic-over-traits pattern as `BotService<B, S, F, H>`, maintaining clean architecture (core never depends on infra).

## Deviations from Plan

None -- plan executed exactly as written.

## Issues Encountered
- **Borrow conflict in StreamInSpan::poll_next:** Initial implementation triggered E0502 (cannot borrow self as mutable while immutably borrowed via span.enter()). Resolved with unsafe pin projection, separating the span and inner stream borrows through `get_unchecked_mut()`.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- AgentEngine is ready for the CLI chat command (02-06/02-07) to stream LLM responses
- ChatService provides all the persistence operations the CLI needs for session management
- SystemPromptBuilder will be called at session start to compose the full personality prompt
- SessionManager's should_extract_memory() is ready for the memory extraction plan (02-06)
- All 61 tests pass, workspace compiles cleanly
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 02-single-agent-chat-llm*
*Completed: 2026-02-11*
