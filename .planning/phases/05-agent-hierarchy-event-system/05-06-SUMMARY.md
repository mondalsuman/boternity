---
phase: 05-agent-hierarchy-event-system
plan: 06
subsystem: api
tags: [websocket, event-bus, axum-ws, broadcast, cancellation, budget, real-time]

# Dependency graph
requires:
  - phase: 05-agent-hierarchy-event-system
    provides: "EventBus broadcast channel and AgentEvent enum"
  - phase: 05-agent-hierarchy-event-system
    provides: "GlobalConfig type and load_global_config() from config.toml"
provides:
  - "WebSocket handler at /ws/events forwarding AgentEvent from EventBus to clients"
  - "Bidirectional WsCommand processing: CancelAgent, BudgetContinue, BudgetStop, Ping"
  - "EventBus on AppState shared between HTTP handlers and WebSocket"
  - "Agent cancellation registry (DashMap<Uuid, CancellationToken>) on AppState"
  - "Budget response channels (DashMap<Uuid, oneshot::Sender<bool>>) on AppState"
  - "GlobalConfig loaded from config.toml on AppState"
affects:
  - 05-agent-hierarchy-event-system (plans 07-08 for orchestrator and CLI rendering)
  - 06-sub-agent-ui-observability (frontend WebSocket client connects to /ws/events)

# Tech tracking
tech-stack:
  added:
    - "dashmap (workspace) added to boternity-api Cargo.toml"
  patterns:
    - "tokio::select! multiplexing EventBus broadcast + WebSocket receive in single loop"
    - "Graceful lagged-receiver recovery: log warning, continue (no disconnect)"
    - "WebSocket disconnect does NOT cancel agents (reconnection-safe)"

key-files:
  created:
    - crates/boternity-api/src/http/handlers/ws.rs
  modified:
    - crates/boternity-api/src/http/handlers/mod.rs
    - crates/boternity-api/src/http/router.rs
    - crates/boternity-api/src/state.rs
    - crates/boternity-api/Cargo.toml

key-decisions:
  - "tokio::select! single-loop instead of socket.split() two-task -- enables Ping/Pong without cross-task channel"
  - "WsCommand tagged union with serde rename_all snake_case matches AgentEvent serialization convention"
  - "DashMap for agent_cancellations and budget_responses -- concurrent access from WebSocket + orchestrator"
  - "WebSocket disconnect does NOT auto-cancel agents (per research pitfall 4)"

patterns-established:
  - "WsCommand serde-tagged enum for client-to-server WebSocket commands"
  - "process_command() extracted as testable free function for command dispatch"
  - "AppState as bridge: orchestrator writes to DashMap registries, WebSocket reads/removes"

# Metrics
duration: 3min
completed: 2026-02-13
---

# Phase 5 Plan 6: WebSocket Infrastructure Summary

**WebSocket at /ws/events forwarding AgentEvent via EventBus, with bidirectional CancelAgent/BudgetContinue/BudgetStop commands and DashMap-backed cancellation registry on AppState**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-13T21:59:03Z
- **Completed:** 2026-02-13T22:01:40Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Created WebSocket handler with tokio::select! multiplexing EventBus broadcast events and incoming client commands
- Implemented WsCommand enum (CancelAgent, BudgetContinue, BudgetStop, Ping) with graceful UUID parsing and error logging
- Added EventBus, GlobalConfig, agent cancellation registry, and budget response channels to AppState
- Registered /ws/events route outside /api/v1 namespace (WebSocket is not a REST endpoint)
- Lagged broadcast receivers handled gracefully (log skipped count, continue)

## Task Commits

Each task was committed atomically:

1. **Task 1: WebSocket handler with event forwarding and command receiving** - `6cbf88e` (feat)
2. **Task 2: Wire /ws/events route** - `2c1a5be` (feat)

## Files Created/Modified

- `crates/boternity-api/src/http/handlers/ws.rs` - WebSocket upgrade handler, event forwarding loop, command processing
- `crates/boternity-api/src/http/handlers/mod.rs` - Added `pub mod ws` declaration
- `crates/boternity-api/src/http/router.rs` - Added `/ws/events` route with GET handler
- `crates/boternity-api/src/state.rs` - Added EventBus, GlobalConfig, agent_cancellations, budget_responses fields and initialization
- `crates/boternity-api/Cargo.toml` - Added dashmap workspace dependency

## Decisions Made

- **tokio::select! single-loop pattern:** Instead of socket.split() with two separate async tasks, used a single loop with tokio::select! on both the EventBus receiver and WebSocket receiver. This keeps both sender and receiver accessible in the same scope, enabling the Ping command to send a pong response without cross-task communication.
- **WsCommand serde-tagged enum:** Matches the same `#[serde(tag = "type", rename_all = "snake_case")]` convention used by AgentEvent, so both directions of WebSocket traffic use consistent JSON format.
- **DashMap registries on AppState:** `agent_cancellations` and `budget_responses` use DashMap for lock-free concurrent access from both the orchestrator (which inserts entries) and the WebSocket handler (which reads/removes entries).
- **Disconnect does NOT cancel agents:** Per research pitfall 4, WebSocket disconnection is purely a UI event. The orchestrator manages its own lifecycle. Users must explicitly send CancelAgent commands.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added AppState fields in Task 1 instead of Task 2**
- **Found during:** Task 1 (WebSocket handler compilation)
- **Issue:** The ws.rs handler accesses `state.event_bus`, `state.agent_cancellations`, and `state.budget_responses`, but these fields were planned for Task 2. Task 1 could not compile without them.
- **Fix:** Moved the AppState field declarations and initialization from Task 2 into Task 1, so the handler compiles. Task 2 then only needed to add the route.
- **Files modified:** crates/boternity-api/src/state.rs
- **Verification:** `cargo check -p boternity-api` passes
- **Committed in:** 6cbf88e (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Task ordering adjusted for compilation dependency. No scope change.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- WebSocket infrastructure is complete and ready for frontend connection
- EventBus on AppState enables orchestrator to publish events that reach all connected clients
- Agent cancellation registry ready for orchestrator integration (plan 07)
- Budget response channels ready for pause/continue flow (plan 07)
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 05-agent-hierarchy-event-system*
*Completed: 2026-02-13*
