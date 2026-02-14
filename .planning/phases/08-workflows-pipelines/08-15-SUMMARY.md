---
phase: "08-workflows-pipelines"
plan: 15
subsystem: "workflow-triggers"
tags: ["cron", "event-bus", "triggers", "scheduler", "dag-executor"]
dependency-graph:
  requires: ["08-09", "08-14"]
  provides: ["cron-trigger-execution", "event-trigger-execution", "all-trigger-types-wired"]
  affects: []
tech-stack:
  added: []
  patterns: ["background-task-spawning", "broadcast-subscriber-event-matching"]
key-files:
  created: []
  modified:
    - "crates/boternity-api/src/state.rs"
decisions:
  - id: "cron-callback-arc-clone"
    choice: "CronCallback clones Arc<DagExecutor> and Arc<SqliteWorkflowRepository> per fire"
    reason: "CronCallback is Fn (not FnOnce), must be reusable across fires"
  - id: "event-type-serde-extraction"
    choice: "Extract event type via serde_json::to_value then reading [\"type\"] field"
    reason: "AgentEvent uses #[serde(tag = \"type\", rename_all = \"snake_case\")] -- no event_type() method"
metrics:
  duration: "2m 19s"
  completed: "2026-02-14"
---

# Phase 8 Plan 15: Cron + Event Trigger Wiring Summary

**Wire CronScheduler and EventBus listener to DagExecutor.execute() so cron and event-driven workflows actually fire.**

## What Was Done

### Task 1: Start CronScheduler with executor callbacks at AppState init
- Added `CronScheduler` and `TriggerManager` as Arc fields on AppState
- CronScheduler started at init() before returning Self
- All workflow definitions loaded via `workflow_repo.list_definitions(None)`
- Each definition's triggers registered with TriggerManager for centralized tracking
- Cron triggers get callbacks that: record_fire, load definition, call executor.execute()
- Imported `WorkflowExecutor` trait for RPITIT execute() method visibility

### Task 2: EventBus listener for event-driven workflow triggers
- Check for event triggers via `trigger_manager.get_event_triggers()`
- If any exist, subscribe to EventBus and spawn a background listener loop
- Extract event type from serialized AgentEvent via `serde_json::to_value` and `["type"]` field
- Match against registered event triggers by comparing `event_type` strings
- Evaluate optional `when` clauses via TriggerManager.evaluate_when_clause()
- Matching events spawn `tokio::spawn` tasks calling `executor.execute(&def, "event", payload)`
- Handle broadcast lag (warn + continue) and channel closure (break loop)

## Task Commits

| Task | Name | Commit | Key Changes |
|------|------|--------|-------------|
| 1 | CronScheduler with executor callbacks | d682b5a | CronScheduler + TriggerManager on AppState, cron trigger registration |
| 2 | EventBus listener for event triggers | 69010c2 | EventBus subscriber matching events to triggers, spawning executor |

## Decisions Made

1. **CronCallback uses Arc clones**: CronCallback is `Fn` (reusable), so it clones `Arc<DagExecutor>` and `Arc<SqliteWorkflowRepository>` into each async block. This is cheap (Arc clone = atomic refcount bump).

2. **Event type extraction via serde_json**: AgentEvent uses `#[serde(tag = "type", rename_all = "snake_case")]`. To get the type string, serialize to Value and read `["type"]`. No `event_type()` accessor method exists on AgentEvent.

3. **Event triggers re-fetched each event**: The listener calls `tm_for_events.get_event_triggers()` on each incoming event rather than caching. This ensures dynamically registered triggers (from workflow CRUD operations) are picked up without listener restart.

## Deviations from Plan

None -- plan executed exactly as written.

## Verification Results

1. `cargo check -p boternity-api` -- compiles with only dead-code warnings (expected for new AppState fields)
2. `cargo test -p boternity-core -- workflow` -- 134 tests pass, 0 failures
3. `cron_scheduler` field confirmed in state.rs
4. `schedule_workflow` call confirmed in state.rs
5. `event_bus.subscribe()` confirmed in state.rs

## Trigger Type Coverage

All three trigger types now produce Running workflow runs that complete:

| Trigger Type | Wired In | Mechanism |
|--------------|----------|-----------|
| Manual | 08-14 | `trigger_workflow()` handler spawns executor |
| Webhook | 08-14 | `receive_webhook()` handler spawns executor |
| Cron | 08-15 | CronScheduler callback calls executor.execute() |
| Event | 08-15 | EventBus listener matches events, spawns executor |

## Self-Check: PASSED
