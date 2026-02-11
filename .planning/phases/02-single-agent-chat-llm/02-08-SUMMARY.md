---
phase: 02-single-agent-chat-llm
plan: 08
subsystem: cli
tags: [clap, comfy-table, session-browser, memory-management, cli, export]

# Dependency graph
requires:
  - phase: 02-04
    provides: "SqliteChatRepository, SqliteMemoryRepository, chat_sessions/session_memories tables"
  - phase: 01-foundation
    provides: "CLI patterns (comfy-table, dialoguer, console), BotService for bot resolution"
provides:
  - "bnity sessions <bot> -- session browser with rich table"
  - "bnity export session <id> -- Markdown or JSON session export"
  - "bnity delete session <id> -- session deletion with confirmation"
  - "bnity memories <bot> -- memory browser with provenance"
  - "bnity remember <bot> 'fact' -- manual memory injection"
  - "bnity forget <bot> -- wipe all memories with confirmation"
  - "bnity delete memory <id> -- individual memory deletion"
  - "ChatService wired into AppState (ConcreteChatService type alias)"
affects:
  - 02-07 (chat loop can dispatch to session/memory commands via slash commands)
  - Phase 3 (memory browser extends to semantic search)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ChatService exposed on AppState for CLI access to sessions and memories"
    - "ExportResource enum for extensible export commands"
    - "Importance formatting as star ratings (*****/---)"

key-files:
  created:
    - "crates/boternity-api/src/cli/session.rs"
    - "crates/boternity-api/src/cli/memory.rs"
  modified:
    - "crates/boternity-api/src/cli/mod.rs"
    - "crates/boternity-api/src/main.rs"
    - "crates/boternity-api/src/state.rs"

key-decisions:
  - "Manual memories use Uuid::nil() session_id (not linked to any session)"
  - "ConcreteChatService type alias pins ChatService<SqliteChatRepository, SqliteMemoryRepository>"
  - "Session/memory IDs parsed from String args via Uuid::parse (user-friendly error on invalid UUID)"

patterns-established:
  - "ExportResource subcommand pattern for resource-specific export"
  - "Bot slug resolution before session/memory operations (consistent bot-not-found error)"
  - "format_importance() renders 1-5 importance as star/dash bars"

# Metrics
duration: 6min
completed: 2026-02-12
---

# Phase 2 Plan 08: Session and Memory Management CLI Summary

**Session browser, export, and delete commands plus memory browser, inject, and wipe -- completing the CLI lifecycle for conversations and bot knowledge**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-11T23:09:33Z
- **Completed:** 2026-02-11T23:16:02Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Session browser (`bnity sessions <bot>`) with rich table showing title, started, duration, messages, and status
- Session export to Markdown (default) or JSON with full conversation transcript, metadata, and timing
- Session delete with confirmation prompt and --force bypass, using CASCADE on chat_sessions
- Memory browser (`bnity memories <bot>`) with fact, category, importance stars, source provenance, and date
- Manual memory injection (`bnity remember <bot> 'fact'`) stored as MemoryCategory::Fact with is_manual=true
- Memory wipe (`bnity forget <bot>`) with count display and confirmation
- Individual memory deletion (`bnity delete memory <id>`) with confirmation
- ChatService wired into AppState via ConcreteChatService type alias

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement session browser, export, and delete commands** - `70ec24e` (feat)
2. **Task 2: Implement memory CLI commands and wire all commands** - `ebeb2a3` (feat)

## Files Created/Modified
- `crates/boternity-api/src/cli/session.rs` - Session list, export (Markdown/JSON), and delete commands
- `crates/boternity-api/src/cli/memory.rs` - Memory list, remember, forget, and delete commands
- `crates/boternity-api/src/cli/mod.rs` - Added memory/session modules, ExportResource, DeleteResource::Session/Memory, Sessions/Memories/Remember/Forget commands
- `crates/boternity-api/src/main.rs` - Dispatch for all new commands with UUID parsing
- `crates/boternity-api/src/state.rs` - Added ConcreteChatService type alias and ChatService wiring in AppState::init()

## Decisions Made
- **Manual memory session_id:** Manual memories (`bnity remember`) use `Uuid::nil()` as session_id since they aren't linked to any conversation. The schema requires a session_id, and nil UUID clearly distinguishes manual from extracted memories.
- **ConcreteChatService on AppState:** Wired `ChatService<SqliteChatRepository, SqliteMemoryRepository>` as a new field on AppState, following the same `Arc<ConcreteXxxService>` pattern as BotService and SoulService.
- **UUID parsing from CLI args:** Session and memory IDs are accepted as strings in clap args and parsed to UUID in main.rs dispatch, providing user-friendly error messages on invalid UUIDs.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Wired ChatService into AppState**
- **Found during:** Task 1 (session commands need ChatService)
- **Issue:** AppState did not have ChatService or memory repository access. Session/memory commands cannot function without it.
- **Fix:** Added ConcreteChatService type alias, added chat_service field to AppState, wired SqliteChatRepository + SqliteMemoryRepository in AppState::init()
- **Files modified:** crates/boternity-api/src/state.rs
- **Verification:** cargo check passes, session commands can access chat_service
- **Committed in:** 70ec24e (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential for functionality -- session/memory commands require ChatService access. No scope creep.

## Issues Encountered
- BotId newtype vs Uuid: ChatService methods take `&Uuid` but Bot type uses `BotId(pub Uuid)`. Fixed by accessing inner field via `bot.id.0`.
- ChatRepository trait not in scope for `delete_session` call through chat_repo() reference. Fixed by importing `boternity_core::chat::repository::ChatRepository`.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All Phase 2 CLI commands are complete (session browsing, memory management, export)
- ChatService is wired and accessible from any CLI or API handler
- Ready for Phase 2 completion -- all 8 plans executed
- No blockers or concerns
- Workspace compiles cleanly

## Self-Check: PASSED

---
*Phase: 02-single-agent-chat-llm*
*Completed: 2026-02-12*
