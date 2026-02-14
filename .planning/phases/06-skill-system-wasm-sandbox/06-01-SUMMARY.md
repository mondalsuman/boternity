---
phase: 06-skill-system-wasm-sandbox
plan: 01
subsystem: types
tags: [skill, wasm, wasmtime, serde_yaml_ng, semver, petgraph, ratatui, trust-tier, capability]

# Dependency graph
requires:
  - phase: 01-foundation-bot-identity
    provides: "boternity-types crate, workspace structure, uuid/chrono/serde deps"
provides:
  - "Skill domain types (SkillManifest, TrustTier, Capability, etc.) in boternity_types::skill"
  - "Workspace dependencies for Phase 6 (wasmtime, serde_yaml_ng, semver, petgraph, ratatui, landlock)"
affects:
  - 06-skill-system-wasm-sandbox (all subsequent plans depend on these types and dependencies)

# Tech tracking
tech-stack:
  added:
    - "wasmtime 40 (component-model, async, cranelift)"
    - "wasmtime-wasi 40"
    - "serde_yaml_ng 0.10"
    - "semver 1 (serde)"
    - "petgraph 0.7"
    - "ratatui 0.30 (crossterm)"
    - "landlock 0.4 (workspace only, not yet wired to crate)"
  patterns:
    - "agentskills.io-compatible manifest types with boternity extensions in metadata section"
    - "serde rename_all snake_case for all skill enums"
    - "Capability enum with PartialEq+Eq+Hash for HashSet usage"
    - "TrustTier defaults to Untrusted (secure by default)"

key-files:
  created:
    - "crates/boternity-types/src/skill.rs"
  modified:
    - "crates/boternity-types/src/lib.rs"
    - "Cargo.toml"
    - "crates/boternity-types/Cargo.toml"
    - "crates/boternity-core/Cargo.toml"
    - "crates/boternity-infra/Cargo.toml"
    - "crates/boternity-api/Cargo.toml"

key-decisions:
  - "TrustTier::Untrusted as Default (secure by default)"
  - "Capability enum with 8 variants matching CONTEXT.md fine-grained operations"
  - "SkillMeta uses semver::Version for version field (strong typing)"
  - "SkillSource tagged enum with type=local or type=registry"
  - "ResourceLimits defaults: 64MB memory, 1M fuel, 30s duration"
  - "serde_yaml_ng added to types crate for future manifest parsing helpers"
  - "landlock declared in workspace but NOT wired to any crate (Plan 08 will add with cfg gates)"

patterns-established:
  - "agentskills.io manifest structure: SkillManifest top-level + SkillMetadata for extensions"
  - "serde rename with hyphens for YAML compatibility (allowed-tools, skill-type, trust-tier, conflicts-with)"

# Metrics
duration: 3m 32s
completed: 2026-02-14
---

# Phase 6 Plan 01: Skill Domain Types + Workspace Dependencies Summary

**All skill system domain types (manifest, trust tiers, capabilities, permissions, audit, resource limits) defined in boternity-types with Phase 6 workspace dependencies (wasmtime, serde_yaml_ng, semver, petgraph, ratatui) wired to crates**

## Performance

- **Duration:** 3m 32s
- **Started:** 2026-02-13T23:24:09Z
- **Completed:** 2026-02-13T23:27:41Z
- **Tasks:** 2/2
- **Files modified:** 7

## Accomplishments

- Complete skill domain type library in `boternity_types::skill` with 15 types covering the full skill lifecycle
- All three trust tiers (Local, Verified, Untrusted) with Display, Default, and serde support
- All 8 capability variants with Hash/Eq for HashSet usage in permission enforcement
- Phase 6 dependencies (wasmtime 40, serde_yaml_ng, semver, petgraph, ratatui) resolved and compiling across workspace

## Task Commits

Each task was committed atomically:

1. **Task 1: Skill domain types in boternity-types** - `b380ebd` (feat)
2. **Task 2: Add Phase 6 workspace dependencies** - `4c2cc34` (chore)

## Files Created/Modified

- `crates/boternity-types/src/skill.rs` - All skill domain types: SkillManifest, SkillMetadata, SkillType, TrustTier, Capability, PermissionGrant, SkillPermissions, BotSkillConfig, BotSkillsFile, SkillAuditEntry, InstalledSkill, SkillSource, SkillMeta, ResourceLimits
- `crates/boternity-types/src/lib.rs` - Added `pub mod skill`
- `Cargo.toml` - Added workspace deps: wasmtime, wasmtime-wasi, serde_yaml_ng, semver, petgraph, ratatui, landlock
- `crates/boternity-types/Cargo.toml` - Added semver, serde_yaml_ng
- `crates/boternity-core/Cargo.toml` - Added serde_yaml_ng, semver, petgraph
- `crates/boternity-infra/Cargo.toml` - Added wasmtime, wasmtime-wasi, serde_yaml_ng, semver
- `crates/boternity-api/Cargo.toml` - Added ratatui

## Decisions Made

- **TrustTier::Untrusted as Default:** Secure-by-default -- new skills start untrusted until explicitly promoted.
- **semver::Version for SkillMeta:** Strong typing prevents version string parsing issues downstream.
- **landlock workspace-only:** Declared in workspace Cargo.toml but not yet added to any crate. Plan 08 (OS sandbox) will wire it with proper `cfg(target_os = "linux")` gates.
- **serde_yaml_ng on types crate:** Added for potential manifest parsing helpers co-located with types, though primary YAML parsing will be in core.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- All domain types available for Plan 02 (manifest parser) and Plan 03 (permission system)
- All Phase 6 dependencies available for Plans 04-12 (WASM runtime, TUI browser, etc.)
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 06-skill-system-wasm-sandbox*
*Completed: 2026-02-14*
