---
phase: 05-agent-hierarchy-event-system
plan: 07
subsystem: api
tags: [orchestrator, cli, sse, tree-rendering, budget-display, agent-events, websocket]

# Dependency graph
requires:
  - phase: 05-04
    provides: AgentOrchestrator with execute(), sub-agent lifecycle, synthesis
  - phase: 05-05
    provides: resolve_request_budget(), estimate_cost() for budget and pricing
  - phase: 05-06
    provides: WebSocket infrastructure, EventBus, budget_responses DashMap
provides:
  - CLI tree renderer with Unicode box-drawing characters and colored agent labels
  - CLI budget display with live counter, warning prompt, exhaustion message, completion stats
  - Orchestrator integration in CLI chat loop with two-path execution
  - HTTP SSE handler with orchestrator and sub-agent event streaming
  - --quiet flag for CLI to suppress sub-agent detail
  - Cancel support via Ctrl+C (tree) and agent_cancellations DashMap
affects: [05-08, 06-testing, 07-security]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two-path execution: simple direct-stream vs orchestrator sub-agent path based on spawn detection"
    - "EventBus subscriber pattern for real-time CLI rendering of agent events"
    - "agent_event_to_sse() converter for mapping AgentEvent variants to named SSE events"
    - "Background orchestrator task with mpsc channel result delivery for SSE streaming"

key-files:
  created:
    - crates/boternity-api/src/cli/chat/tree_renderer.rs
    - crates/boternity-api/src/cli/chat/budget_display.rs
  modified:
    - crates/boternity-api/src/cli/chat/loop_runner.rs
    - crates/boternity-api/src/cli/chat/mod.rs
    - crates/boternity-api/src/http/handlers/chat.rs
    - crates/boternity-api/src/cli/mod.rs
    - crates/boternity-api/src/main.rs

key-decisions:
  - "Two-path execution in loop_runner: parse_spawn_instructions() on initial response decides orchestrator vs direct stream"
  - "HTTP orchestrator runs in tokio::spawn with mpsc channel result delivery, EventBus subscriber in SSE stream loop"
  - "Budget warning auto-continues in CLI (stdin reading during orchestrator execution deferred as TODO)"
  - "--quiet flag added to Chat command as --quiet / -q (does not conflict with global -q)"

patterns-established:
  - "agent_event_to_sse: centralized AgentEvent to SSE Event mapper returning Option<Event>"
  - "Cancellation token registration in agent_cancellations DashMap for Ctrl+C tree cancel"
  - "Sub-agent memory extraction with source_agent_id tagging from memory_contexts"

# Metrics
duration: 12min
completed: 2026-02-13
---

# Phase 5 Plan 7: CLI + HTTP Orchestrator Integration Summary

**CLI tree renderer with Unicode box-drawing, live budget counter, and orchestrator integration in both CLI loop and HTTP SSE handler with 11 new sub-agent SSE event types**

## Performance

- **Duration:** 12 min
- **Started:** 2026-02-13T22:05:26Z
- **Completed:** 2026-02-13T22:17:24Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Tree renderer module with box-drawing characters, colored agent labels, completion stats, depth/cycle warnings
- Budget display module with live token counter (yellow at 80%), warning prompt, exhaustion message, cost estimate stats
- CLI chat loop integration with AgentOrchestrator: two-path execution (simple direct-stream vs orchestrator), EventBus subscriber for real-time tree rendering, cancellation token registration, sub-agent memory extraction with source_agent_id
- HTTP SSE handler integration with orchestrator in background task, 11 new SSE event types for agent hierarchy (agent_spawned, agent_text_delta, agent_completed, agent_failed, agent_cancelled, budget_update, budget_warning, budget_exhausted, depth_limit, cycle_detected, synthesis_started)
- --quiet flag suppresses sub-agent detail in CLI, showing only final synthesis

## Task Commits

Each task was committed atomically:

1. **Task 1: CLI tree renderer and budget display** - `bf464c8` (feat)
2. **Task 2: Integrate orchestrator into CLI loop and HTTP SSE handler** - `2f52ead` (feat)

## Files Created/Modified
- `crates/boternity-api/src/cli/chat/tree_renderer.rs` - Unicode box-drawing tree output with colored agent labels, completion stats, format_tokens_human
- `crates/boternity-api/src/cli/chat/budget_display.rs` - Live budget counter, warning prompt, exhaustion message, completion stats with cost
- `crates/boternity-api/src/cli/chat/loop_runner.rs` - Orchestrator integration with two-path execution, EventBus rendering, cancel support
- `crates/boternity-api/src/cli/chat/mod.rs` - Added tree_renderer and budget_display module declarations
- `crates/boternity-api/src/http/handlers/chat.rs` - Orchestrator integration with background task, agent_event_to_sse converter, 11 new SSE events
- `crates/boternity-api/src/cli/mod.rs` - Added --quiet flag to Chat command
- `crates/boternity-api/src/main.rs` - Updated Chat command dispatch to pass quiet flag

## Decisions Made
- Two-path execution: the initial LLM response is streamed via FallbackChain as before. After streaming, `parse_spawn_instructions()` checks for sub-agent directives. If found, the orchestrator takes over; otherwise the simple path persists messages directly. This preserves the existing zero-overhead path for simple messages.
- HTTP orchestrator runs in `tokio::spawn` with a `mpsc::channel(1)` for result delivery. The SSE stream uses `tokio::select!` between EventBus events and the result channel, with biased selection favoring events.
- CLI budget warning auto-continues rather than blocking on stdin, since reading stdin during orchestrator execution with rustyline-async is complex. Marked as TODO for future refinement.
- `--quiet / -q` added to Chat command. Does not conflict with global `-q` (which is for log-level quiet).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- CLI and HTTP interfaces are now orchestrator-aware
- Ready for 05-08 (final phase plan) which will add end-to-end testing and polish
- Budget pause via stdin in CLI is deferred (auto-continues for now)
- WebSocket budget_continue/budget_stop flow is wired on the state side; frontend integration happens in a future phase

## Self-Check: PASSED

---
*Phase: 05-agent-hierarchy-event-system*
*Completed: 2026-02-13*
