---
phase: 08-workflows-pipelines
plan: 02
subsystem: database
tags: [sqlite, sqlx, workflow, messaging, repository, migrations]

# Dependency graph
requires:
  - phase: 08-01
    provides: WorkflowDefinition, WorkflowRun, WorkflowStepLog, BotMessage, Channel, BotSubscription types
provides:
  - WorkflowRepository trait (CRUD + run/step queries)
  - MessageRepository trait (send, query, channel, subscription)
  - SqliteWorkflowRepository with migration 006
  - SqliteMessageRepository with migration 006
affects: [08-03, 08-04, 08-05, 08-06, 08-07, 08-08]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "JSON blob storage for WorkflowDefinition (full IR stored as JSON TEXT column)"
    - "COALESCE-based unique index for nullable owner columns"

key-files:
  created:
    - crates/boternity-core/src/repository/workflow.rs
    - crates/boternity-core/src/repository/message.rs
    - crates/boternity-infra/src/sqlite/workflow.rs
    - crates/boternity-infra/src/sqlite/message.rs
    - migrations/20260216_006_workflow_messages.sql
  modified:
    - crates/boternity-core/src/repository/mod.rs
    - crates/boternity-infra/src/sqlite/mod.rs

key-decisions:
  - "WorkflowDefinition stored as full JSON blob (not normalized columns) for schema flexibility"
  - "COALESCE unique index instead of inline UNIQUE constraint (SQLite limitation)"
  - "Crashed runs detected by querying status='running' (no heartbeat needed yet)"

patterns-established:
  - "Workflow repo pattern: definition JSON blob + denormalized run/step rows"
  - "Message repo pattern: flattened recipient_type/recipient_bot_id/recipient_channel columns"

# Metrics
duration: 7min
completed: 2026-02-14
---

# Phase 8 Plan 2: Workflow & Message Persistence Summary

**SQLite repositories for workflow definitions (JSON blob), run/step execution tracking, bot-to-bot messages, channels, and subscriptions with 23 integration tests**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-14T14:58:19Z
- **Completed:** 2026-02-14T15:05:00Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- WorkflowRepository and MessageRepository traits with RPITIT (14+9 methods)
- SqliteWorkflowRepository: full definition CRUD, run lifecycle, step log tracking, crash recovery query
- SqliteMessageRepository: direct/channel message persistence, channel CRUD, subscription management
- Migration 006 creates 6 tables with proper indexes and foreign keys
- 23 integration tests covering all operations and edge cases

## Task Commits

Each task was committed atomically:

1. **Task 1: Define workflow and message repository traits** - `8ae9030` (feat)
2. **Task 2: Implement SQLite workflow and message repositories with migrations** - `2cee6a2` (feat)

## Files Created/Modified
- `crates/boternity-core/src/repository/workflow.rs` - WorkflowRepository trait (14 methods: definition CRUD, run lifecycle, step logs)
- `crates/boternity-core/src/repository/message.rs` - MessageRepository trait (9 methods: messages, channels, subscriptions)
- `crates/boternity-core/src/repository/mod.rs` - Added workflow and message module exports
- `crates/boternity-infra/src/sqlite/workflow.rs` - SqliteWorkflowRepository (1025 lines, full impl + 14 tests)
- `crates/boternity-infra/src/sqlite/message.rs` - SqliteMessageRepository (612 lines, full impl + 9 tests)
- `crates/boternity-infra/src/sqlite/mod.rs` - Added workflow and message module exports
- `migrations/20260216_006_workflow_messages.sql` - 6 tables: workflows, workflow_runs, workflow_steps, bot_messages, bot_channels, bot_subscriptions

## Decisions Made
- Stored WorkflowDefinition as full JSON blob rather than normalizing columns -- preserves schema flexibility for evolving workflow IR without migration changes
- Used `COALESCE(owner_bot_id, '')` in a CREATE UNIQUE INDEX instead of inline UNIQUE constraint because SQLite prohibits expressions in table-level UNIQUE constraints
- Crash recovery uses simple status='running' query -- no heartbeat mechanism needed yet, can be added when executor is built

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed SQLite UNIQUE constraint expression limitation**
- **Found during:** Task 2 (migration execution)
- **Issue:** `UNIQUE(name, owner_type, COALESCE(owner_bot_id, ''))` is invalid in SQLite -- expressions are prohibited in inline UNIQUE constraints
- **Fix:** Changed to `CREATE UNIQUE INDEX idx_workflows_name_owner ON workflows(name, owner_type, COALESCE(owner_bot_id, ''))` which SQLite supports
- **Files modified:** `migrations/20260216_006_workflow_messages.sql`
- **Verification:** All 23 tests pass
- **Committed in:** 2cee6a2 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential fix for SQLite compatibility. No scope creep.

## Issues Encountered
None beyond the migration syntax fix documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Repository traits and SQLite implementations ready for workflow executor (08-03+)
- Message bus can build on MessageRepository for runtime pub/sub (08-05+)
- All 6 tables migrated and indexed for production use

## Self-Check: PASSED

---
*Phase: 08-workflows-pipelines*
*Completed: 2026-02-14*
