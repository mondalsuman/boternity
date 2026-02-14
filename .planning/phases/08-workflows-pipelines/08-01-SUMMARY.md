---
phase: 08-workflows-pipelines
plan: 01
subsystem: types
tags: [workflow, dag, cron, webhook, bot-messaging, serde, yaml, triggers]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: domain type patterns (serde, uuid, chrono derives)
  - phase: 06-skill-system
    provides: serde_yaml_ng, petgraph, semver already in workspace
provides:
  - WorkflowDefinition canonical IR (YAML/visual/SDK interchangeable)
  - StepDefinition with 8 step types (Agent, Skill, Code, Http, Conditional, Loop, Approval, SubWorkflow)
  - TriggerConfig with 5 trigger types (Manual, Cron, Webhook, Event, FileWatch)
  - WebhookAuth (HmacSha256, BearerToken)
  - WorkflowRun and WorkflowStepLog execution tracking types
  - BotMessage envelope with Direct and Channel recipients
  - Channel and BotSubscription pub/sub types
  - Workspace deps: tokio-cron-scheduler, jexl-eval, notify, notify-debouncer-mini, hmac
affects: [08-02 through 08-13, all Phase 8 plans depend on these types]

# Tech tracking
tech-stack:
  added: [tokio-cron-scheduler 0.15, jexl-eval 0.4, notify 8.2, notify-debouncer-mini 0.5, hmac 0.12]
  patterns: [canonical WorkflowDefinition IR, internally-tagged serde enums for step/trigger configs, YAML+JSON roundtrip]

key-files:
  created:
    - crates/boternity-types/src/workflow.rs
    - crates/boternity-types/src/message.rs
  modified:
    - Cargo.toml
    - crates/boternity-core/Cargo.toml
    - crates/boternity-infra/Cargo.toml
    - crates/boternity-types/src/lib.rs

key-decisions:
  - "WorkflowOwner tagged enum (Bot/Global) for scoped vs cross-bot workflows"
  - "StepConfig internally tagged by type for clean YAML representation"
  - "7-state WorkflowRunStatus (Pending/Running/Paused/Completed/Failed/Crashed/Cancelled)"
  - "6-state WorkflowStepStatus (Pending/Running/Completed/Failed/Skipped/WaitingApproval)"
  - "RetryConfig defaults to max_attempts=3 via serde default function"
  - "StepUiMetadata optional with skip_serializing_if for clean YAML when no visual data"
  - "BotMessage uses serde_json::Value for flexible body (not typed enum)"

patterns-established:
  - "Canonical IR pattern: all representations convert to/from WorkflowDefinition"
  - "Internally-tagged serde enums for polymorphic config types"
  - "Optional UI metadata on domain types for visual builder round-trip"

# Metrics
duration: 4min
completed: 2026-02-14
---

# Phase 8 Plan 01: Workflow & Message Domain Types Summary

**Canonical WorkflowDefinition IR with 8 step types, 5 trigger types, bot-to-bot BotMessage envelope, and 5 new workspace dependencies (tokio-cron-scheduler, jexl-eval, notify, hmac)**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-14T14:50:10Z
- **Completed:** 2026-02-14T14:54:10Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- All Phase 8 domain types defined in boternity-types with full Serialize/Deserialize support
- YAML and JSON roundtrip tests verify correctness (37 tests across workflow + message modules)
- New workspace dependencies compiled successfully across all crates
- 122 total boternity-types tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Phase 8 workspace dependencies** - `171009a` (chore)
2. **Task 2: Create workflow and message domain types** - `fb69dd1` (feat)

## Files Created/Modified
- `crates/boternity-types/src/workflow.rs` - WorkflowDefinition, StepDefinition, StepConfig, TriggerConfig, RetryConfig, WorkflowRun, WorkflowStepLog (1031 lines)
- `crates/boternity-types/src/message.rs` - BotMessage, MessageRecipient, Channel, BotSubscription (206 lines)
- `crates/boternity-types/src/lib.rs` - Added `pub mod workflow` and `pub mod message`
- `Cargo.toml` - Added tokio-cron-scheduler, jexl-eval, notify, notify-debouncer-mini, hmac to workspace deps
- `crates/boternity-core/Cargo.toml` - Added tokio-cron-scheduler, jexl-eval, notify, notify-debouncer-mini
- `crates/boternity-infra/Cargo.toml` - Added hmac, notify, notify-debouncer-mini

## Decisions Made
- WorkflowOwner as tagged enum (Bot with bot_id+slug, or Global) matching CONTEXT.md scoping rules
- StepConfig internally tagged (`#[serde(tag = "type")]`) for clean YAML: `config: { type: agent, bot: ..., prompt: ... }`
- 7-state WorkflowRunStatus includes Crashed (infra failure) separate from Failed (app error), per research deep dive 2
- RetryConfig defaults max_attempts to 3 via `#[serde(default = "default_max_attempts")]`
- BotMessage body is `serde_json::Value` for maximum flexibility (typed envelope, flexible payload)
- StepUiMetadata uses `skip_serializing_if = "Option::is_none"` to keep YAML clean when no visual builder data present

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed test assertion for pretty-printed JSON**
- **Found during:** Task 2 (serde roundtrip tests)
- **Issue:** `serde_json::to_string_pretty()` adds whitespace between key and value, breaking substring assertion `"type":"direct"`
- **Fix:** Changed to `serde_json::to_string()` (compact) for assertions that check exact substring patterns
- **Files modified:** crates/boternity-types/src/message.rs
- **Verification:** All 37 tests pass
- **Committed in:** fb69dd1

**2. [Rule 1 - Bug] Fixed YAML manual trigger syntax**
- **Found during:** Task 2 (realistic YAML parse test)
- **Issue:** `- type: manual {}` is invalid YAML for serde tagged enum; the `{}` is parsed as the value of type
- **Fix:** Changed to `- type: manual` (no trailing `{}`)
- **Files modified:** crates/boternity-types/src/workflow.rs
- **Verification:** YAML parse test passes
- **Committed in:** fb69dd1

---

**Total deviations:** 2 auto-fixed (2 bugs in test code)
**Impact on plan:** Both were test assertion bugs, not design issues. No scope creep.

## Issues Encountered
None - plan executed smoothly.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All domain types are available for Plans 02-13 to build upon
- Workspace dependencies are wired and compile across all crates
- Serde roundtrip verified for both YAML (workflow definitions) and JSON (messages, API responses)
- No blockers for next plans

## Self-Check: PASSED

---
*Phase: 08-workflows-pipelines*
*Completed: 2026-02-14*
