---
phase: 05-agent-hierarchy-event-system
plan: 02
subsystem: core
tags: [atomic, dashmap, broadcast, cancellation-token, concurrent, agent-hierarchy]

# Dependency graph
requires:
  - phase: 05-agent-hierarchy-event-system
    provides: "AgentEvent enum, dashmap and tokio-util workspace deps"
  - phase: 01-foundation-bot-identity
    provides: "boternity-core crate structure, uuid, serde_json"
provides:
  - "RequestBudget with atomic token tracking and 80% warning threshold"
  - "SharedWorkspace with DashMap concurrent key-value store"
  - "CycleDetector with normalized task signature counting"
  - "RequestContext bundling budget + workspace + cancellation + cycle_detector"
  - "EventBus wrapping tokio::sync::broadcast for multi-consumer event distribution"
affects:
  - 05-agent-hierarchy-event-system (plans 03-08 use RequestContext and EventBus for orchestrator)
  - 06-sub-agent-ui-observability (EventBus feeds WebSocket and UI)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "AtomicU32 + AtomicBool CAS for lock-free budget tracking with exactly-once warning"
    - "DashMap with clone-on-read to prevent lock-across-await deadlocks"
    - "CancellationToken tree for hierarchical cancellation (parent cancels children)"
    - "HashMap<u64, usize> counter approach for cycle detection by normalized task hash"

key-files:
  created:
    - crates/boternity-core/src/agent/budget.rs
    - crates/boternity-core/src/agent/workspace.rs
    - crates/boternity-core/src/agent/cycle_detector.rs
    - crates/boternity-core/src/agent/request_context.rs
    - crates/boternity-core/src/event/mod.rs
    - crates/boternity-core/src/event/bus.rs
  modified:
    - crates/boternity-core/src/agent/mod.rs
    - crates/boternity-core/src/lib.rs

key-decisions:
  - "Clone-on-read for SharedWorkspace::get() to prevent DashMap Ref held across await"
  - "HashMap<u64, usize> for CycleDetector instead of HashSet (tracks repetition count, not just presence)"
  - "RequestContext.child() uses saturating_add for depth to prevent overflow"
  - "EventBus publish silently drops events when no subscribers (let _ = sender.send())"

patterns-established:
  - "Budget threshold detection via AtomicBool compare_exchange for exactly-once semantics"
  - "Child context pattern: shared Arc-backed state + child CancellationToken + depth+1"
  - "Broadcast channel event bus with manual Clone impl"

# Metrics
duration: 5min
completed: 2026-02-13
---

# Phase 5 Plan 2: Core Primitives Summary

**RequestBudget with atomic 80% warning, SharedWorkspace with DashMap clone-on-read, CycleDetector with normalized hash counting, RequestContext hierarchy propagation, and EventBus broadcast channel**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-13T21:47:44Z
- **Completed:** 2026-02-13T21:52:18Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments

- Implemented lock-free RequestBudget with AtomicU32 token tracking and exactly-once 80% warning via AtomicBool CAS
- Built SharedWorkspace on DashMap with clone-on-read semantics to prevent deadlocks across await points
- Created CycleDetector with normalized task hashing and configurable repetition threshold
- Assembled RequestContext that bundles all shared state with hierarchical child() derivation
- Added EventBus wrapping tokio broadcast channel for multi-consumer AgentEvent distribution
- 43 new unit tests including concurrent/parallel safety tests, all passing alongside 122 existing tests (165 total)

## Task Commits

Each task was committed atomically:

1. **Task 1: RequestBudget, SharedWorkspace, and CycleDetector** - `cc346ec` (feat)
2. **Task 2: RequestContext and EventBus** - `b086cc9` (feat)

## Files Created/Modified

- `crates/boternity-core/src/agent/budget.rs` - RequestBudget with AtomicU32 token tracking and BudgetStatus enum
- `crates/boternity-core/src/agent/workspace.rs` - SharedWorkspace with DashMap-backed concurrent key-value store
- `crates/boternity-core/src/agent/cycle_detector.rs` - CycleDetector with normalized hash counting and CycleCheckResult enum
- `crates/boternity-core/src/agent/request_context.rs` - RequestContext bundling budget + workspace + cancellation + cycle_detector + depth
- `crates/boternity-core/src/event/mod.rs` - Event module declaration re-exporting EventBus
- `crates/boternity-core/src/event/bus.rs` - EventBus wrapping tokio::sync::broadcast for AgentEvent distribution
- `crates/boternity-core/src/agent/mod.rs` - Added budget, cycle_detector, request_context, workspace module declarations
- `crates/boternity-core/src/lib.rs` - Added event module declaration

## Decisions Made

- **Clone-on-read for SharedWorkspace:** `get()` clones the `serde_json::Value` immediately rather than returning a `DashMap` `Ref` guard. This prevents the guard from being held across `.await` points which would deadlock.
- **HashMap counter for CycleDetector:** Used `HashMap<u64, usize>` instead of `HashSet<u64>` to track how many times each normalized task hash has been seen, enabling configurable thresholds.
- **Saturating depth increment:** `RequestContext::child()` uses `saturating_add(1)` for depth to prevent u8 overflow at extreme nesting.
- **Silent drop on no subscribers:** `EventBus::publish()` uses `let _ = sender.send()` to silently handle the case when no subscribers exist, which is valid during startup.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed budget test that expected Ok for 999 tokens (crosses 80% threshold)**
- **Found during:** Task 1 (budget tests)
- **Issue:** Test `add_tokens_returns_exhausted_at_or_over_budget` added 999 tokens in one call expecting `BudgetStatus::Ok`, but 999 > 800 (80% of 1000) so the warning threshold was crossed, returning `Warning`.
- **Fix:** Restructured test to add tokens in steps that correctly reflect threshold crossings (500 Ok, 300 Warning, 199 Ok, 1 Exhausted).
- **Files modified:** crates/boternity-core/src/agent/budget.rs
- **Verification:** All budget tests pass
- **Committed in:** cc346ec (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug in test logic)
**Impact on plan:** Test was incorrectly asserting expectations. No code logic change needed.

## Issues Encountered

None beyond the auto-fixed test deviation above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- All five core primitives are implemented, tested, and exported from boternity-core
- Plans 03-08 can import RequestBudget, SharedWorkspace, CycleDetector, RequestContext, and EventBus
- RequestContext.child() is ready for AgentOrchestrator sub-agent spawning
- EventBus is ready to distribute AgentEvent instances to WebSocket and logging subscribers
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 05-agent-hierarchy-event-system*
*Completed: 2026-02-13*
