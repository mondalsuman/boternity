---
phase: 07-builder-system
plan: 02
subsystem: core
tags: [builder, forge, rpitit, extension-trait, system-prompt, xml-tags]

# Dependency graph
requires:
  - phase: 07-01
    provides: "Builder domain types (BuilderTurn, BuilderState, BuilderPhase, BuilderAnswer, BuilderConfig)"
provides:
  - "BuilderAgent trait with RPITIT surface-agnostic interface"
  - "BuilderStateExt extension trait for state lifecycle management"
  - "Forge system prompt builder with XML tags and memory recall"
  - "RecalledBuilderMemory and BuilderMode types for prompt construction"
affects: ["07-03", "07-04", "07-05", "07-06", "07-07"]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Extension trait pattern (BuilderStateExt) for cross-crate impl"
    - "XML-tagged system prompt sections for builder LLM"
    - "Embedded const personality (Forge not stored in DB)"

key-files:
  created:
    - crates/boternity-core/src/builder/mod.rs
    - crates/boternity-core/src/builder/agent.rs
    - crates/boternity-core/src/builder/state.rs
    - crates/boternity-core/src/builder/prompt.rs
  modified:
    - crates/boternity-core/src/lib.rs

key-decisions:
  - "BuilderStateExt extension trait instead of inherent impl (Rust orphan rules prevent impl on foreign type)"
  - "new_builder_state() free function instead of BuilderState::new() (constructors on extension traits return Self which is awkward)"
  - "build_forge_system_prompt takes &BuilderMode not owned (no need to consume)"

patterns-established:
  - "Extension trait pattern: when boternity-core needs methods on boternity-types structs, use XyzExt trait"
  - "Forge prompt XML sections: forge_identity, builder_instructions, accumulated_context, current_config, past_sessions"

# Metrics
duration: 4min
completed: 2026-02-14
---

# Phase 7 Plan 2: Core Builder Agent Summary

**BuilderAgent RPITIT trait, BuilderStateExt accumulator with back navigation, and Forge XML system prompt with cross-session memory recall**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-14T13:06:53Z
- **Completed:** 2026-02-14T13:11:14Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- BuilderAgent trait with start/next_turn/resume/reconfigure methods using RPITIT
- BuilderStateExt extension trait with full lifecycle: record_exchange, advance_phase, go_back (truncates), conversation_summary (last 5), update_config_field, is_complete
- Forge system prompt builder producing XML-tagged sections with mode-specific instructions and recalled builder memory injection
- 21 unit tests covering state lifecycle and prompt construction

## Task Commits

Each task was committed atomically:

1. **Task 1: BuilderAgent trait and BuilderState accumulator** - `668db0f` (feat)
2. **Task 2: Forge system prompt builder with memory recall** - `7235b7b` (feat)

## Files Created/Modified
- `crates/boternity-core/src/builder/mod.rs` - Builder module root with agent, state, prompt sub-modules
- `crates/boternity-core/src/builder/agent.rs` - BuilderAgent RPITIT trait + BuilderError enum
- `crates/boternity-core/src/builder/state.rs` - BuilderStateExt extension trait + new_builder_state constructor + 10 tests
- `crates/boternity-core/src/builder/prompt.rs` - Forge system prompt builder with RecalledBuilderMemory, BuilderMode, XML sections + 11 tests
- `crates/boternity-core/src/lib.rs` - Added `pub mod builder;`

## Decisions Made
- **Extension trait pattern for BuilderState**: Rust orphan rules prevent inherent `impl BuilderState` in boternity-core since the struct is defined in boternity-types. Used `BuilderStateExt` trait instead, consistent with how the project handles cross-crate method additions.
- **Free function constructor**: `new_builder_state()` instead of `BuilderState::new()` because constructors returning `Self` on extension traits are possible but less ergonomic than a standalone function.
- **BuilderMode by reference**: `build_forge_system_prompt` takes `&BuilderMode` not owned, since the mode is typically reused and cloning is unnecessary.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Used extension trait instead of inherent impl for BuilderState**
- **Found during:** Task 1 (BuilderState accumulator)
- **Issue:** `impl BuilderState` in boternity-core fails with E0116 (cannot define inherent impl for type outside its crate)
- **Fix:** Created `BuilderStateExt` extension trait with all methods, and `new_builder_state()` free function for construction
- **Files modified:** crates/boternity-core/src/builder/state.rs
- **Verification:** `cargo check -p boternity-core` compiles, all 10 state tests pass
- **Committed in:** 668db0f (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Extension trait is the idiomatic Rust pattern for this situation. No scope creep. All planned functionality preserved.

## Issues Encountered
None beyond the orphan rule deviation handled above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- BuilderAgent trait ready for LlmBuilderAgent implementation (Plan 07-05)
- BuilderStateExt ready for use in draft persistence (Plan 07-03) and CLI adapter (Plan 07-06)
- Forge prompt builder ready for LLM integration with memory recall
- Consumers must `use crate::builder::state::BuilderStateExt` to access state methods

## Self-Check: PASSED

---
*Phase: 07-builder-system*
*Completed: 2026-02-14*
