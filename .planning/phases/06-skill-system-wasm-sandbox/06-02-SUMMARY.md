---
phase: 06-skill-system-wasm-sandbox
plan: 02
subsystem: skill
tags: [skill, manifest, yaml, toml, parsing, filesystem, skill-store, serde_yaml_ng]

# Dependency graph
requires:
  - phase: 06-skill-system-wasm-sandbox
    provides: "Skill domain types (SkillManifest, InstalledSkill, BotSkillsFile, SkillMeta) in boternity_types::skill"
  - phase: 01-foundation-bot-identity
    provides: "Workspace structure, serde/chrono/uuid dependencies"
provides:
  - "SKILL.md manifest parser (extract_frontmatter, parse_skill_md, validate_manifest) in boternity_core::skill::manifest"
  - "Filesystem-based SkillStore (install/list/get/remove) in boternity_infra::skill::skill_store"
  - "Per-bot skills.toml parse/serialize (parse_bot_skills_config, serialize_bot_skills_config)"
affects:
  - 06-skill-system-wasm-sandbox (Plans 03-12 consume manifest parsing and skill storage)

# Tech tracking
tech-stack:
  added:
    - "anyhow 1 (added to boternity-core for manifest parsing error handling)"
    - "toml 0.8 (added to boternity-core for skills.toml serialization)"
  patterns:
    - "YAML frontmatter extraction with --- delimiters for SKILL.md format"
    - "Slug validation pattern: lowercase alphanumeric + hyphens, no leading/trailing hyphens"
    - "SkillStore filesystem layout: {base_dir}/skills/{name}/SKILL.md + .boternity-meta.toml + skill.wasm"
    - "Graceful degradation on corrupted skills in list_skills() (warn and skip)"

key-files:
  created:
    - "crates/boternity-core/src/skill/manifest.rs"
    - "crates/boternity-infra/src/skill/skill_store.rs"
  modified:
    - "crates/boternity-core/src/skill/mod.rs"
    - "crates/boternity-core/Cargo.toml"
    - "crates/boternity-infra/src/skill/mod.rs"

key-decisions:
  - "anyhow for manifest parsing errors (utility functions, not repository traits)"
  - "Slug validation: lowercase alphanumeric + hyphens, no leading/trailing hyphens"
  - "SkillStore returns empty Vec on missing skills directory (graceful for fresh installs)"
  - "SkillSource::Local as default when .boternity-meta.toml absent"
  - "get_bot_skills_config returns empty BotSkillsFile when skills.toml missing"

patterns-established:
  - "YAML frontmatter parsing: split on opening/closing --- delimiters"
  - "Skill directory convention: {base}/skills/{slug}/SKILL.md as canonical presence check"
  - "Meta TOML alongside SKILL.md for registry provenance tracking"

# Metrics
duration: 10m 14s
completed: 2026-02-14
---

# Phase 6 Plan 02: SKILL.md Manifest Parsing + Filesystem Skill Store Summary

**YAML frontmatter SKILL.md parser with validation in boternity-core, filesystem SkillStore with install/list/get/remove in boternity-infra, per-bot skills.toml round-trip support**

## Performance

- **Duration:** 10m 14s
- **Started:** 2026-02-13T23:32:39Z
- **Completed:** 2026-02-13T23:42:53Z
- **Tasks:** 2/2
- **Files modified:** 5

## Accomplishments

- Complete SKILL.md manifest parser: extract_frontmatter, parse_skill_md, validate_manifest with slug pattern, semver, self-conflict, and depth checks
- Filesystem SkillStore: install/list/get/remove skills at configurable base directory with SKILL.md + .boternity-meta.toml + skill.wasm layout
- Per-bot skills.toml parse/serialize round-trip via TOML
- 19 unit tests total (11 manifest parsing + 8 skill store) all passing

## Task Commits

Each task was committed atomically:

1. **Task 1: SKILL.md manifest parser in boternity-core** - `b89c87f` (feat)
2. **Task 2: Filesystem-based skill store in boternity-infra** - `a95d89f` (feat)

## Files Created/Modified

- `crates/boternity-core/src/skill/manifest.rs` - SKILL.md frontmatter extraction, YAML parsing into SkillManifest, validation (slug, semver, self-conflict, depth), skills.toml parse/serialize
- `crates/boternity-infra/src/skill/skill_store.rs` - SkillStore struct with new/list_skills/get_skill/install_skill/remove_skill/skill_exists/resolve_skill_path/get_bot_skills_config/save_bot_skills_config
- `crates/boternity-core/src/skill/mod.rs` - Added pub mod manifest alongside existing permission module
- `crates/boternity-core/Cargo.toml` - Added anyhow and toml dependencies
- `crates/boternity-infra/src/skill/mod.rs` - Added pub mod skill_store alongside existing audit and wasm_runtime modules

## Decisions Made

- **anyhow for manifest parsing:** Used anyhow::Result for manifest parsing functions (utility/parsing, not repository trait implementations which use RepositoryError).
- **Slug validation pattern:** Lowercase alphanumeric + hyphens only, must not start/end with hyphen. Matches common package naming conventions.
- **Graceful empty results:** SkillStore::list_skills() returns empty Vec when skills directory doesn't exist. get_bot_skills_config() returns empty BotSkillsFile when skills.toml is missing. Both avoid errors on fresh installations.
- **SkillSource::Local default:** When .boternity-meta.toml is absent, skill source defaults to Local (locally created skill, not from registry).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added anyhow and toml to boternity-core Cargo.toml**
- **Found during:** Task 1 (manifest parser implementation)
- **Issue:** Plan uses anyhow::Result and toml parsing but these weren't in boternity-core dependencies
- **Fix:** Added `anyhow = { workspace = true }` and `toml = { workspace = true }` to [dependencies]
- **Files modified:** crates/boternity-core/Cargo.toml
- **Verification:** cargo test compiles and passes
- **Committed in:** b89c87f (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential dependency addition for manifest parsing. No scope creep.

## Issues Encountered

- Linter/template engine auto-generated additional skill modules (resolver.rs, inheritance.rs, permission.rs) and committed them alongside plan 06-02 files. These are for future plans and were not part of this plan's scope. The mod.rs was kept to only reference manifest + permission (the pre-existing module).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Manifest parsing ready for Plan 03 (permission system) and Plan 04 (dependency resolver)
- SkillStore ready for Plan 06 (skill CLI) and Plan 07 (skill registry client)
- Per-bot skills.toml support ready for bot-skill association management
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 06-skill-system-wasm-sandbox*
*Completed: 2026-02-14*
