---
phase: 07-builder-system
plan: 06
subsystem: builder
tags: [skill-builder, skill-manifest, llm-generation, capability-suggestion, bot-assembly]

# Dependency graph
requires:
  - phase: 07-04
    provides: "BotAssembler with skills_attached placeholder"
  - phase: 07-05
    provides: "LlmBuilderAgent with structured output and output_config wiring"
  - phase: 06-02
    provides: "SkillManifest types, parse_skill_md, validate_manifest, skill store"
provides:
  - "SkillBuilder for LLM-driven skill generation from natural language"
  - "Heuristic capability suggestion (suggest_capabilities)"
  - "SKILL.md validation with warnings"
  - "BotAssembler skill attachment (attach_skills, skills.toml writing)"
  - "format_assembly_summary for CLI post-create output"
affects: [07-07, 07-08, 07-09, 07-10]

# Tech tracking
tech-stack:
  added: [tempfile (dev-dependency)]
  patterns: [stateless-utility-with-structured-output, keyword-heuristic-capability-suggestion]

key-files:
  created:
    - crates/boternity-core/src/builder/skill_builder.rs
  modified:
    - crates/boternity-core/src/builder/mod.rs
    - crates/boternity-core/src/builder/assembler.rs
    - crates/boternity-core/Cargo.toml

key-decisions:
  - "SkillBuilder follows stateless utility pattern (no fields, provider passed per-call) consistent with 02-06"
  - "suggest_capabilities uses keyword-based heuristics (8 capability types) for fast inline suggestions without LLM"
  - "validate_skill does structural validation only (parse + manifest checks + heuristic warnings); LLM semantic validation deferred"
  - "attach_skills writes SKILL.md files and skills.toml with builder-created origin tag in overrides"
  - "skill_request_to_build_result converts lightweight SkillRequest to SkillBuildResult without LLM call"
  - "Rust 2024 reserves 'gen' keyword -- use 'generated' as variable name (Rule 3 blocking fix)"

patterns-established:
  - "SkillBuilder: stateless LLM utility with structured output schema for skill generation"
  - "Capability heuristics: keyword matching with deduplication for fast inline suggestions"
  - "builder-created origin tag: overrides map in BotSkillConfig for tracking skill provenance"

# Metrics
duration: 5min
completed: 2026-02-14
---

# Phase 7 Plan 6: Skill Builder Summary

**SkillBuilder generates SKILL.md from natural language via LLM structured output with heuristic capability suggestions and BotAssembler skill attachment**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-14T13:28:22Z
- **Completed:** 2026-02-14T13:33:22Z
- **Tasks:** 2/2
- **Files modified:** 4

## Accomplishments

- SkillBuilder with generate_skill (LLM-driven), suggest_capabilities (heuristic), and validate_skill (structural)
- 8 capability types auto-suggested from description keywords with deduplication
- BotAssembler attach_skills writes SKILL.md, optional src/lib.rs, and skills.toml with builder-created metadata
- format_assembly_summary produces detailed CLI post-create output with all bot details

## Task Commits

Each task was committed atomically:

1. **Task 1: SkillBuilder for LLM-driven skill creation** - `c0c2eab` (feat)
2. **Task 2: Wire skill attachment into BotAssembler** - `04d6c0e` (feat)

## Files Created/Modified

- `crates/boternity-core/src/builder/skill_builder.rs` - SkillBuilder utility with generate_skill, suggest_capabilities, validate_skill + 13 tests
- `crates/boternity-core/src/builder/mod.rs` - Added pub mod skill_builder
- `crates/boternity-core/src/builder/assembler.rs` - attach_skills, format_assembly_summary, skill_request_to_build_result + 5 new tests
- `crates/boternity-core/Cargo.toml` - Added tempfile dev-dependency

## Decisions Made

- SkillBuilder follows stateless utility pattern (consistent with 02-06 pattern)
- Heuristic capability suggestion for 8 types: http_get, read_file, write_file, read_env, exec_command, get_secret, recall_memory (database maps to http_get)
- validate_skill does structural validation only (parse SKILL.md + validate_manifest + body/metadata heuristic warnings); reserved provider parameter for future semantic validation
- Skills attached with builder-created origin tag in BotSkillConfig.overrides for provenance tracking
- skill_request_to_build_result creates minimal SkillBuildResult from SkillRequest without LLM call (for skills requested but not yet fully generated)
- All builder-created skills default to TrustTier::Local

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Rust 2024 `gen` reserved keyword**
- **Found during:** Task 1 (SkillBuilder implementation)
- **Issue:** Used `gen` as variable name; Rust 2024 edition reserves `gen` as keyword
- **Fix:** Renamed variable from `gen` to `generated`
- **Files modified:** crates/boternity-core/src/builder/skill_builder.rs
- **Verification:** cargo check -p boternity-core passes
- **Committed in:** c0c2eab (part of Task 1 commit)

**2. [Rule 3 - Blocking] Missing tempfile dev-dependency**
- **Found during:** Task 2 (BotAssembler tests)
- **Issue:** Tests use tempfile::tempdir() for filesystem tests, but tempfile not in Cargo.toml
- **Fix:** Added tempfile as dev-dependency (already in workspace)
- **Files modified:** crates/boternity-core/Cargo.toml
- **Verification:** cargo test -p boternity-core builder::assembler passes
- **Committed in:** 04d6c0e (part of Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both were minimal fixes required for compilation. No scope creep.

## Issues Encountered

None beyond the auto-fixed deviations.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- SkillBuilder ready for CLI integration in `bnity skill create` command
- BotAssembler skill attachment wired and tested
- format_assembly_summary ready for CLI post-create display
- suggest_capabilities available for inline builder flow suggestions

## Self-Check: PASSED

---
*Phase: 07-builder-system*
*Completed: 2026-02-14*
