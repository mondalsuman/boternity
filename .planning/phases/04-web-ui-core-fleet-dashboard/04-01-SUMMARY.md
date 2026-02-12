---
phase: 04-web-ui-core-fleet-dashboard
plan: 01
subsystem: api
tags: [axum, sse, streaming, rest, chat, sessions, identity, stats, spa]

# Dependency graph
requires:
  - phase: 02-single-agent-chat-llm
    provides: ChatService, ChatRepository, AgentContext, FallbackChain streaming
  - phase: 03-multi-provider-memory
    provides: FallbackChain with multi-provider, vector memory search, AppState wiring
provides:
  - SSE streaming chat endpoint (POST /api/v1/bots/{id}/chat/stream)
  - Session CRUD endpoints (list, get, messages, delete, clear)
  - Identity/User file management endpoints (GET/PUT)
  - Dashboard statistics endpoint (GET /api/v1/stats)
  - SPA static file serving with client-side routing fallback
  - ChatRepository.clear_messages, count_sessions, count_messages methods
affects: [04-02 (React app will call these endpoints), 04-03 (chat UI), 04-04 (fleet dashboard)]

# Tech tracking
tech-stack:
  added: [async-stream, tokio-stream, tower-http/fs]
  patterns: [SSE streaming via async_stream::stream!, SPA fallback with ServeDir]

key-files:
  created:
    - crates/boternity-api/src/http/handlers/chat.rs
    - crates/boternity-api/src/http/handlers/session.rs
    - crates/boternity-api/src/http/handlers/identity.rs
    - crates/boternity-api/src/http/handlers/stats.rs
  modified:
    - crates/boternity-api/src/http/handlers/mod.rs
    - crates/boternity-api/src/http/router.rs
    - crates/boternity-api/Cargo.toml
    - crates/boternity-core/src/chat/repository.rs
    - crates/boternity-infra/src/sqlite/chat.rs
    - Cargo.toml

key-decisions:
  - "async_stream::stream! for SSE (avoids complex Pin<Box> manual construction, produces Send stream)"
  - "Direct SQL for stats endpoint (efficient COUNT with conditional aggregation instead of service-layer list+count)"
  - "ChatRepository trait extended with clear_messages, count_sessions, count_messages (rather than separate stats repository)"
  - "SPA fallback via BOTERNITY_WEB_DIR env var with graceful degradation when dir absent"
  - "Conversation history loaded into AgentContext for session continuation in streaming endpoint"

patterns-established:
  - "SSE handler pattern: resolve bot -> build context -> select_stream -> async_stream -> emit events"
  - "SPA serving pattern: API routes nested at /api/v1, fallback_service for static files"

# Metrics
duration: 9min
completed: 2026-02-13
---

# Phase 4 Plan 01: Backend API Endpoints Summary

**SSE streaming chat, session CRUD, identity/user file management, dashboard stats, and SPA serving for the web frontend**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-12T23:24:45Z
- **Completed:** 2026-02-12T23:33:23Z
- **Tasks:** 3/3
- **Files modified:** 10

## Accomplishments
- All 8 missing REST endpoints implemented and registered in the Axum router
- SSE streaming chat endpoint follows the exact same LLM interaction pattern as the CLI loop_runner
- SPA static file serving configured for production deployment with client-side routing fallback
- ChatRepository extended with clear_messages, count_sessions, count_messages for session management and stats

## Task Commits

Each task was committed atomically:

1. **Task 1: SSE streaming chat + session CRUD endpoints** - `9eccd6c` (feat)
2. **Task 2: Identity/User file endpoints + dashboard stats** - `b91efc3` (feat)
3. **Task 3: SPA static file serving for production** - `aac7d5f` (feat)

## Files Created/Modified
- `crates/boternity-api/src/http/handlers/chat.rs` - SSE streaming chat endpoint with async_stream
- `crates/boternity-api/src/http/handlers/session.rs` - Session CRUD: list, get, messages, delete, clear
- `crates/boternity-api/src/http/handlers/identity.rs` - Identity/User file read/write with frontmatter parsing
- `crates/boternity-api/src/http/handlers/stats.rs` - Dashboard aggregate statistics
- `crates/boternity-api/src/http/handlers/mod.rs` - Module registration for new handlers
- `crates/boternity-api/src/http/router.rs` - Route registration + SPA fallback serving
- `crates/boternity-api/Cargo.toml` - async-stream and tokio-stream dependencies
- `crates/boternity-core/src/chat/repository.rs` - clear_messages, count_sessions, count_messages trait methods
- `crates/boternity-infra/src/sqlite/chat.rs` - SQLite implementations for new trait methods
- `Cargo.toml` - tower-http fs feature for ServeDir/ServeFile

## Decisions Made
- Used `async_stream::stream!` macro for SSE stream construction (produces Send-safe streams without manual Pin gymnastics)
- Extended ChatRepository trait with count/clear methods instead of creating a separate stats repository (keeps the trait cohesive, stats are session-related)
- Direct SQL COUNT with conditional aggregation for bot stats (single query vs multiple service calls)
- Loaded conversation history into AgentContext on session continuation (user can resume sessions from the web UI)
- SPA serving is conditional on directory existence (API-only mode when frontend hasn't been built)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] ChatRepository trait import needed in session.rs**
- **Found during:** Task 1 (session CRUD handlers)
- **Issue:** `delete_session` and `clear_messages` methods not found because `ChatRepository` trait was not in scope
- **Fix:** Added `use boternity_core::chat::repository::ChatRepository;` import
- **Files modified:** crates/boternity-api/src/http/handlers/session.rs
- **Verification:** Build succeeds
- **Committed in:** 9eccd6c (part of Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minor import issue, standard Rust trait scoping. No scope creep.

## Issues Encountered
None - all planned work executed cleanly.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 8 backend endpoints ready for frontend consumption
- SPA serving infrastructure in place; React app can be built into `apps/web/dist/`
- Frontend can call: stream chat, manage sessions, read/write identity/user files, get dashboard stats
- No blockers for Phase 4 Plan 02 (React/Vite project setup)

## Self-Check: PASSED

---
*Phase: 04-web-ui-core-fleet-dashboard*
*Completed: 2026-02-13*
