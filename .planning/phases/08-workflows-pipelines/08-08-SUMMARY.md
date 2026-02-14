---
phase: 08-workflows-pipelines
plan: 08
subsystem: cli
tags: [clap, workflow, messaging, bot-to-bot, cli-subcommands]

# Dependency graph
requires:
  - phase: 08-04
    provides: workflow executor and step runners
  - phase: 08-06
    provides: bot-to-bot message bus with direct messaging and pub/sub
  - phase: 08-07
    provides: trigger manager with cron, webhook, event, and file triggers
provides:
  - "bnity workflow CLI subcommands (create, trigger, list, status, logs, delete, approve, cancel)"
  - "bnity message CLI subcommands (send, history, channels, subscribe, unsubscribe, channel-history)"
affects: [08-09, 08-10, 08-11]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Lazy repository instantiation in CLI handlers (SqliteWorkflowRepository, SqliteMessageRepository created per-command)"
    - "JSON body parsing with fallback: try JSON first, wrap in {text: body} if invalid"

key-files:
  created:
    - crates/boternity-api/src/cli/workflow.rs
    - crates/boternity-api/src/cli/message.rs
  modified:
    - crates/boternity-api/src/cli/mod.rs
    - crates/boternity-api/src/main.rs

key-decisions:
  - "Lazy repo creation per CLI command rather than adding to AppState (avoids coupling with REST API state wiring)"
  - "Body parsing: JSON-first with plain text fallback wrapping in {text: body} for UX"
  - "Channel auto-creation on first subscribe (no separate create-channel step needed)"

patterns-established:
  - "Workflow CLI pattern: resolve owner via --bot flag or default to Global"
  - "Status command accepts both workflow name and run UUID for flexibility"

# Metrics
duration: 7min
completed: 2026-02-14
---

# Phase 8 Plan 8: CLI Workflow and Message Commands Summary

**CLI subcommands for workflow management (create/trigger/list/status/logs/delete/approve/cancel) and bot-to-bot messaging (send/history/channels/subscribe/unsubscribe) using clap derive macros**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-14T15:38:23Z
- **Completed:** 2026-02-14T15:45:42Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Full `bnity workflow` CLI with 8 subcommands covering workflow CRUD, manual triggering, run status/logs, and run lifecycle management (approve/cancel)
- Full `bnity message` CLI with 6 subcommands covering direct messaging, pub/sub channels, message history, and subscription management
- Both JSON (`--json`) and styled table output modes for all commands

## Task Commits

Each task was committed atomically:

1. **Task 1: CLI workflow commands** - `31c4086` (feat)
2. **Task 2: CLI bot-to-bot message commands** - `2b22084` (feat, from concurrent 08-09 execution)

**Plan metadata:** [below]

## Files Created/Modified
- `crates/boternity-api/src/cli/workflow.rs` - WorkflowCommand enum with 8 variants and handlers for CRUD, trigger, status, logs, approve, cancel
- `crates/boternity-api/src/cli/message.rs` - MessageCommand enum with 6 variants and handlers for send, history, channels, subscribe, unsubscribe, channel-history
- `crates/boternity-api/src/cli/mod.rs` - Added workflow and message module declarations and Commands enum variants
- `crates/boternity-api/src/main.rs` - Added dispatch for Workflow and Message commands

## Decisions Made
- Lazy repository instantiation: SqliteWorkflowRepository and SqliteMessageRepository are created per CLI command invocation rather than stored on AppState, keeping the CLI path lightweight
- Workflow status command accepts both a workflow name (shows recent runs) and a run UUID (shows specific run detail) for flexibility
- Send-and-wait mode is persisted but noted as requiring a running message bus (bnity serve) for reply delivery
- Channel auto-creation: subscribing to a non-existent channel creates it automatically, reducing friction

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed BotId newtype conversion**
- **Found during:** Task 1 (workflow create handler)
- **Issue:** WorkflowOwner expects Uuid but bot.id is BotId(Uuid) newtype
- **Fix:** Used bot.id.0 to extract inner Uuid
- **Files modified:** crates/boternity-api/src/cli/workflow.rs
- **Verification:** cargo check compiles cleanly
- **Committed in:** 31c4086

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor type conversion fix, necessary for compilation. No scope creep.

## Issues Encountered
- Git stash/pop during investigation reverted mod.rs and main.rs edits, requiring re-application. Resolved by re-editing and amending the commit.
- Task 2 (message commands) was already committed by a concurrent 08-09 plan execution. The code was identical, so no additional commit was needed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- CLI commands ready for workflow and message management
- REST API handlers (08-09) already wired for HTTP-based workflow/message operations
- Ready for integration testing and end-to-end workflow execution

## Self-Check: PASSED

---
*Phase: 08-workflows-pipelines*
*Completed: 2026-02-14*
