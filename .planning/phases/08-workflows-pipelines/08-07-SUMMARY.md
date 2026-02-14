---
phase: 08-workflows-pipelines
plan: 07
subsystem: workflow
tags: [cron, scheduler, trigger, webhook, hmac, file-watcher, notify, jexl]

# Dependency graph
requires:
  - phase: 08-04
    provides: workflow executor and step runners
  - phase: 08-05
    provides: JEXL expression evaluator for when clauses
provides:
  - CronScheduler with human-readable schedules and missed-run catch-up
  - TriggerManager coordinating cron, webhook, event, and file triggers
  - TriggerContext struct for trigger metadata
  - WebhookRegistry with HMAC-SHA256 and bearer token auth
  - File watcher with glob pattern filtering
affects: [08-08, 08-09, 08-10, 08-11, 08-13]

# Tech tracking
tech-stack:
  added: [croner (cron expression parsing for missed-run detection)]
  patterns:
    - "RAII WatcherHandle for filesystem watcher lifecycle"
    - "Constant-time comparison for security-sensitive token/HMAC checks"
    - "DashMap-backed concurrent registry for webhook routes"
    - "Human-readable schedule normalization to standard cron"

key-files:
  created:
    - crates/boternity-core/src/workflow/scheduler.rs
    - crates/boternity-core/src/workflow/trigger.rs
    - crates/boternity-infra/src/workflow/mod.rs
    - crates/boternity-infra/src/workflow/webhook_handler.rs
    - crates/boternity-infra/src/workflow/file_trigger.rs
  modified:
    - crates/boternity-core/src/workflow/mod.rs
    - crates/boternity-core/Cargo.toml
    - crates/boternity-infra/src/lib.rs
    - crates/boternity-infra/Cargo.toml
    - Cargo.toml

key-decisions:
  - "Used croner crate directly for missed-run detection (iter_after for computing missed occurrences between last_fired and now)"
  - "Used notify-debouncer-mini re-exported notify types to avoid version conflict (debouncer uses notify 7, workspace has notify 8)"
  - "Constant-time XOR comparison for bearer tokens instead of pulling in a separate crate"
  - "Human-readable schedules normalized at registration time, not at fire time"

patterns-established:
  - "TriggerContext as universal trigger metadata carrier across all trigger types"
  - "When-clause evaluation via WorkflowEvaluator before workflow launch"
  - "WebhookRegistry.verify_request() combining lookup + auth in single call"

# Metrics
duration: 13min
completed: 2026-02-14
---

# Phase 8 Plan 7: Trigger System Summary

**CronScheduler with human-readable schedules, missed-run catch-up, WebhookRegistry with HMAC-SHA256/bearer auth, and file watcher with glob filtering**

## Performance

- **Duration:** 13 min
- **Started:** 2026-02-14T15:22:04Z
- **Completed:** 2026-02-14T15:35:15Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Complete trigger system for workflow engine: cron scheduling, webhook auth, event bus listeners, file watchers
- Human-readable schedule support ("every 5 minutes", "daily", "every day at 09:30") alongside standard cron
- Missed-run catch-up detection for cron workflows that should have fired while scheduler was down
- HMAC-SHA256 webhook verification with RFC 4231 test vectors and constant-time comparison
- Glob pattern matching for file watcher events with full character class support

## Task Commits

Each task was committed atomically:

1. **Task 1: CronScheduler with missed-run catch-up and TriggerManager** - `7eb7add` (feat)
2. **Task 2: Webhook handler and file watcher infrastructure** - `408f7ed` (feat)

## Files Created/Modified
- `crates/boternity-core/src/workflow/scheduler.rs` - CronScheduler wrapping tokio-cron-scheduler, normalize_schedule(), check_missed_runs()
- `crates/boternity-core/src/workflow/trigger.rs` - TriggerManager, TriggerContext, when-clause evaluation, trigger registration/validation
- `crates/boternity-core/src/workflow/mod.rs` - Added scheduler and trigger module exports
- `crates/boternity-core/Cargo.toml` - Added croner dependency
- `crates/boternity-infra/src/workflow/mod.rs` - New workflow module for infra layer
- `crates/boternity-infra/src/workflow/webhook_handler.rs` - HMAC-SHA256/bearer verification, WebhookRegistry with DashMap
- `crates/boternity-infra/src/workflow/file_trigger.rs` - Debounced file watcher, glob matching, WatcherHandle RAII
- `crates/boternity-infra/src/lib.rs` - Added pub mod workflow
- `crates/boternity-infra/Cargo.toml` - Added dashmap dependency
- `Cargo.toml` - Added croner to workspace dependencies

## Decisions Made
- Used `croner` crate directly (transitive dep from tokio-cron-scheduler) for missed-run detection since tokio-cron-scheduler doesn't expose a "list occurrences between two times" API
- Used `notify_debouncer_mini::notify` re-exports instead of direct `notify` 8.x import to avoid version conflict (notify-debouncer-mini 0.5 requires notify 7.x)
- Implemented constant-time bearer token comparison with XOR rather than adding a dependency -- simple enough for inline implementation
- Human-readable schedule normalization happens at registration time (fail-fast) rather than at cron fire time

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] notify version conflict between direct dep and debouncer**
- **Found during:** Task 2 (File watcher implementation)
- **Issue:** `notify` 8.2 (workspace dep) incompatible with `notify-debouncer-mini` 0.5 which uses notify 7.x -- two versions of the crate produced type mismatches
- **Fix:** Used `notify_debouncer_mini::notify::*` re-exports instead of importing `notify` directly, ensuring all types come from the same version
- **Files modified:** crates/boternity-infra/src/workflow/file_trigger.rs
- **Verification:** All 50 infra tests pass
- **Committed in:** 408f7ed (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Auto-fix necessary for compilation. No scope creep.

## Issues Encountered
- `croner` 3.0 API changed from 2.x: `Cron::new().parse()` replaced by `Cron::from_str()`, `iter_from()` now requires `Direction` parameter -- used `iter_after()` instead for simpler forward-only iteration

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Trigger system ready for integration with workflow executor
- CronScheduler can be started and workflows scheduled
- WebhookRegistry ready for HTTP endpoint integration in API crate
- File watcher ready for per-workflow filesystem monitoring
- TriggerManager provides central coordination point for all trigger types

## Self-Check: PASSED

---
*Phase: 08-workflows-pipelines*
*Completed: 2026-02-14*
