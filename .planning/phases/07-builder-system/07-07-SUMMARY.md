---
phase: 07-builder-system
plan: 07
subsystem: api
tags: [cli, dialoguer, builder-wizard, interactive, skill-create]

# Dependency graph
requires:
  - phase: 07-05
    provides: LlmBuilderAgent with structured output and memory recall
  - phase: 07-06
    provides: SkillBuilder for LLM-driven skill generation and BotAssembler skill attachment
  - phase: 07-03
    provides: SqliteBuilderDraftStore and SqliteBuilderMemoryStore
provides:
  - CLI builder wizard (bnity build) with dialoguer Select, back navigation, live preview
  - Draft auto-save and resume (bnity build --resume)
  - Bot reconfiguration (bnity build --reconfigure <slug>)
  - Standalone skill creation wizard (bnity skill generate)
  - AppState wired with builder_draft_store and builder_memory_store
affects: [07-08, 07-09, 07-10]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Clone derive on SqliteBuilderMemoryStore for agent construction"
    - "Conversation loop pattern with BuilderTurn match and draft auto-save"
    - "slugify_description helper for auto-suggesting skill names"

key-files:
  created:
    - crates/boternity-api/src/cli/builder.rs
    - crates/boternity-api/src/cli/skill_create.rs
  modified:
    - crates/boternity-api/src/cli/mod.rs
    - crates/boternity-api/src/state.rs
    - crates/boternity-api/src/main.rs
    - crates/boternity-api/src/cli/skill.rs
    - crates/boternity-infra/src/builder/sqlite_memory_store.rs

key-decisions:
  - "Build as top-level command (bnity build) instead of overriding bnity create (preserves existing create bot workflow)"
  - "PurposeCategory serialized via serde_json::to_string for builder memory (consistent with 07-03 decision)"
  - "Generate as SkillCommand variant (bnity skill generate) alongside existing Create for LLM-powered wizard"
  - "SqliteBuilderMemoryStore gets Clone derive (DatabasePool is Clone, needed for LlmBuilderAgent construction)"

patterns-established:
  - "Conversation loop: match on BuilderTurn, dispatch to dialoguer, call builder.next_turn, auto-save draft"
  - "Builder wizard modes: new/resume/reconfigure as --flags on single command"

# Metrics
duration: 6m 41s
completed: 2026-02-14
---

# Phase 7 Plan 7: CLI Builder Wizard Summary

**Interactive CLI builder wizard (bnity build) with dialoguer Select, back navigation, draft auto-save, and standalone LLM-powered skill creation (bnity skill generate)**

## Performance

- **Duration:** 6m 41s
- **Started:** 2026-02-14T13:36:58Z
- **Completed:** 2026-02-14T13:43:39Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Full interactive builder wizard with multi-choice options, back navigation, live preview, and explicit confirmation
- Draft auto-save after each turn with resume support and builder memory recording
- Standalone skill creation wizard with LLM-powered generation, capability suggestions, validation, and disk write
- AppState wired with builder_draft_store and builder_memory_store

## Task Commits

Each task was committed atomically:

1. **Task 1: CLI builder wizard and AppState wiring** - `b83310a` (feat)
2. **Task 2: Standalone skill create command** - `cfe5f83` (feat)

## Files Created/Modified
- `crates/boternity-api/src/cli/builder.rs` - CLI builder wizard with run_builder_wizard, run_builder_resume, run_builder_reconfigure
- `crates/boternity-api/src/cli/skill_create.rs` - Standalone interactive skill creation with LLM generation and validation
- `crates/boternity-api/src/cli/mod.rs` - Added builder and skill_create module declarations
- `crates/boternity-api/src/state.rs` - Added builder_draft_store and builder_memory_store fields to AppState
- `crates/boternity-api/src/main.rs` - Added Build command dispatch
- `crates/boternity-api/src/cli/skill.rs` - Added Generate variant to SkillCommand
- `crates/boternity-infra/src/builder/sqlite_memory_store.rs` - Added Clone derive

## Decisions Made
- **Build vs Create:** Used `bnity build` as the builder wizard command rather than overriding `bnity create` which already serves the verb-noun `create bot` pattern
- **SkillCommand::Generate:** Added as separate variant alongside existing `Create` (which is basic scaffolding) for the LLM-powered interactive wizard
- **Clone on SqliteBuilderMemoryStore:** Added `#[derive(Clone)]` since `DatabasePool` is Clone and the memory store needs to be passed to `LlmBuilderAgent::new` which takes ownership
- **PurposeCategory serialization:** Used `serde_json::to_string` for builder memory recording, consistent with 07-03 SQL WHERE matching pattern

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added Clone derive to SqliteBuilderMemoryStore**
- **Found during:** Task 1 (builder wizard wiring)
- **Issue:** LlmBuilderAgent::new takes Option<M: BuilderMemoryStore> by value; SqliteBuilderMemoryStore didn't impl Clone
- **Fix:** Added `#[derive(Clone)]` to SqliteBuilderMemoryStore (DatabasePool is already Clone)
- **Files modified:** crates/boternity-infra/src/builder/sqlite_memory_store.rs
- **Verification:** cargo check passes, builder agent construction compiles
- **Committed in:** b83310a (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minimal -- Clone derive on a struct wrapping a Clone pool is a safe, necessary addition.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- CLI builder wizard complete, ready for HTTP API session handlers (07-08)
- Builder draft and memory stores wired into AppState, accessible from both CLI and HTTP
- All verification criteria met: compile, test, help output verified

## Self-Check: PASSED

---
*Phase: 07-builder-system*
*Completed: 2026-02-14*
