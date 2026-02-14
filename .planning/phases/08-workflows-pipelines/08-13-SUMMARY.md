---
phase: 08-workflows-pipelines
plan: 13
subsystem: workflow, ui, api
tags: [websocket, react-flow, events, crash-recovery, execution-visualization]

# Dependency graph
requires:
  - phase: 08-09
    provides: REST API workflow handlers, AppState Phase 8 fields
  - phase: 08-11
    provides: Workflow builder page with canvas, toolbar, and node palette
provides:
  - 7 workflow lifecycle event variants on AgentEvent enum
  - StepExecutionContext trait for real service wiring
  - Crash recovery on AppState startup (Running -> Crashed)
  - WebSocket workflow subscription filtering
  - useWorkflowEvents hook for live step status tracking
  - Live node status colors and animated edge data flow
  - Run/Cancel/Test Step buttons on builder toolbar
  - ExecutionStatusBar for run progress and completion summary
affects: [09-mcp-integration, 10-polish-deploy]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Object-safe StepExecutionContext trait with boxed futures for dynamic dispatch"
    - "WebSocket subscription filtering by workflow run_id"
    - "Crash recovery via list_crashed_runs() marking interrupted runs on startup"
    - "React Flow animated edges for data flow visualization between steps"

key-files:
  created:
    - apps/web/src/hooks/use-workflow-events.ts
  modified:
    - crates/boternity-types/src/event.rs
    - crates/boternity-core/src/workflow/executor.rs
    - crates/boternity-core/src/workflow/step_runner.rs
    - crates/boternity-api/src/state.rs
    - crates/boternity-api/src/http/handlers/ws.rs
    - apps/web/src/components/workflow/WorkflowCanvas.tsx
    - apps/web/src/routes/workflows/builder/$workflowId.tsx

key-decisions:
  - "StepExecutionContext uses boxed futures (not RPITIT) for Arc<dyn> object safety"
  - "PlaceholderExecutionContext as default StepRunner context (real services wired via with_context)"
  - "WebSocket workflow subscription is opt-in via SubscribeWorkflow command"
  - "Non-workflow events always forwarded regardless of workflow subscriptions"
  - "Edge animation uses React Flow animated prop with green stroke for flowing data"
  - "Crash recovery at AppState::init() time, not a background task"

patterns-established:
  - "StepExecutionContext: boxed-future trait for wiring real agent/skill/HTTP execution"
  - "WebSocket subscription filtering: should_forward_event checks run_id membership"
  - "useWorkflowEvents: hook pattern for real-time execution state from WebSocket"

# Metrics
duration: 8m 26s
completed: 2026-02-14
---

# Phase 8 Plan 13: Live Execution, Events, Service Wiring, Crash Recovery Summary

**7 workflow lifecycle events on AgentEvent, live node status visualization with animated edges, StepExecutionContext trait for service wiring, crash recovery on startup**

## Performance

- **Duration:** 8m 26s
- **Started:** 2026-02-14T16:09:17Z
- **Completed:** 2026-02-14T16:17:43Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments

- Added 7 WorkflowEvent variants to AgentEvent enum with serde roundtrip tests, wired DagExecutor to publish events at each lifecycle point (run start/complete/fail/pause, step start/complete/fail)
- Defined StepExecutionContext trait with PlaceholderExecutionContext default and StepRunner::with_context() for real service wiring
- Added crash recovery to AppState::init() detecting and marking interrupted workflow runs as Crashed
- Built useWorkflowEvents hook filtering WebSocket events by run_id with per-step status tracking
- Wired Run/Cancel/Test Step buttons on builder toolbar with ExecutionStatusBar showing live progress
- Added animated edge data flow between completed and running steps on React Flow canvas

## Task Commits

Each task was committed atomically:

1. **Task 1: Workflow events, service wiring, and crash recovery** - `6685629` (feat)
2. **Task 2: Live execution visualization on React Flow canvas** - `15ddb36` (feat)

## Files Created/Modified

- `crates/boternity-types/src/event.rs` - 7 WorkflowEvent variants, agent_id() None arms, serde tests
- `crates/boternity-core/src/workflow/executor.rs` - Event publishing at all lifecycle points with timing
- `crates/boternity-core/src/workflow/step_runner.rs` - StepExecutionContext trait, PlaceholderExecutionContext, with_context()
- `crates/boternity-api/src/state.rs` - Crash recovery in init(), marking Running runs as Crashed
- `crates/boternity-api/src/http/handlers/ws.rs` - SubscribeWorkflow/UnsubscribeWorkflow commands, event filtering
- `apps/web/src/hooks/use-workflow-events.ts` - Hook tracking live step statuses from WebSocket events
- `apps/web/src/components/workflow/WorkflowCanvas.tsx` - stepStatuses prop, status-colored nodes, animated edges
- `apps/web/src/routes/workflows/builder/$workflowId.tsx` - Run/Cancel/Test buttons, ExecutionStatusBar

## Decisions Made

- **StepExecutionContext uses boxed futures (not RPITIT):** RPITIT traits are not object-safe in Rust. Since StepRunner needs `Arc<dyn StepExecutionContext>` for flexibility, used `Pin<Box<dyn Future>>` return types following the BoxLlmProvider pattern.
- **PlaceholderExecutionContext as default:** StepRunner::new() uses placeholder context so all existing code continues to work. Real services wired via StepRunner::with_context() when AppState provides them.
- **WebSocket subscription is opt-in:** Clients send SubscribeWorkflow to filter workflow events by run_id. Without subscription, all events (including workflow) are forwarded for backward compatibility.
- **Crash recovery at init time:** Simple approach -- mark all Running runs as Crashed on startup. No background heartbeat needed yet (noted in STATE.md from 08-02 decision).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- All Phase 8 plans (01-13) are now complete
- Workflow system fully integrated: types, core engine, infra persistence, CLI, REST API, WebSocket events, web UI builder with live visualization
- StepExecutionContext trait ready for Phase 9/10 to wire real LLM agent and skill execution
- Phase 9 (MCP Integration) can build on the workflow event system for MCP tool invocation tracking

## Self-Check: PASSED

---
*Phase: 08-workflows-pipelines*
*Completed: 2026-02-14*
