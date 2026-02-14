---
phase: 06-skill-system-wasm-sandbox
plan: 08
subsystem: infra
tags: [sandbox, seatbelt, landlock, wasm, defense-in-depth, os-sandbox, subprocess]

# Dependency graph
requires:
  - phase: 06-07
    provides: WASM sandboxed skill executor (WasmSkillExecutor, WasmRuntime)
provides:
  - SandboxConfig struct with filesystem/network/resource controls
  - run_sandboxed() platform dispatch (macOS Seatbelt / Linux Landlock)
  - should_use_os_sandbox() trust tier gating
  - SandboxRequest/SandboxResponse JSON subprocess protocol
  - generate_seatbelt_profile() dynamic Seatbelt profile generation
  - apply_landlock() Landlock ABI v3 filesystem restrictions
affects: [06-10, 06-11, 06-12]

# Tech tracking
tech-stack:
  added: [landlock (cfg-gated for Linux)]
  patterns: [subprocess sandbox model, JSON stdin/stdout IPC, cfg-gated platform modules, deny-default security profiles]

key-files:
  created:
    - crates/boternity-infra/src/skill/sandbox.rs
    - crates/boternity-infra/src/skill/sandbox_macos.rs
    - crates/boternity-infra/src/skill/sandbox_linux.rs
  modified:
    - crates/boternity-infra/src/skill/mod.rs
    - crates/boternity-infra/Cargo.toml

key-decisions:
  - "Subprocess model: self --wasm-sandbox-exec spawns child that applies OS restrictions then runs WASM"
  - "should_use_os_sandbox: only Untrusted tier triggers OS sandbox (Verified/Local skip)"
  - "JSON IPC: SandboxRequest/SandboxResponse via stdin/stdout between parent and child process"
  - "Seatbelt deny-default with selective allow for system libs, WASM binary, configured paths"
  - "Landlock ABI v3 with best-effort fallback (partially enforced on older kernels)"
  - "Platform modules created alongside dispatch layer (deviation from task split) for compilation"

patterns-established:
  - "cfg-gated platform modules: #[cfg(target_os)] on mod declarations in mod.rs"
  - "Subprocess JSON IPC: SandboxRequest/SandboxResponse for parent-child communication"
  - "Deny-default security profiles: start restrictive, selectively allow"

# Metrics
duration: 4min
completed: 2026-02-14
---

# Phase 6 Plan 08: OS-Level Sandbox Summary

**Defense-in-depth OS sandbox layer with macOS Seatbelt profile generation and Linux Landlock filesystem restrictions, running WASM executor in restricted subprocess**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-14T00:12:29Z
- **Completed:** 2026-02-14T00:16:37Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- SandboxConfig struct with wasm_path, readable/writable paths, network flag, trust tier, and resource limits
- Platform dispatch via run_sandboxed() with macOS Seatbelt and Linux Landlock implementations
- 17 passing unit tests covering config construction, serialization, Seatbelt profile content, and trust tier gating

## Task Commits

Each task was committed atomically:

1. **Task 1: Sandbox configuration and dispatch layer** - `14c1da4` (feat)
2. **Task 2: Platform-specific sandbox implementations** - `7c58086` (feat)

**Plan metadata:** pending (docs: complete plan)

## Files Created/Modified
- `crates/boternity-infra/src/skill/sandbox.rs` - SandboxConfig, dispatch, should_use_os_sandbox, JSON protocol types
- `crates/boternity-infra/src/skill/sandbox_macos.rs` - Seatbelt profile generation, sandboxed subprocess spawning
- `crates/boternity-infra/src/skill/sandbox_linux.rs` - Landlock ABI v3 filesystem restrictions, subprocess spawning
- `crates/boternity-infra/src/skill/mod.rs` - Added sandbox, sandbox_macos (cfg-macOS), sandbox_linux (cfg-linux) modules
- `crates/boternity-infra/Cargo.toml` - Added landlock under cfg(target_os = "linux") dependencies

## Decisions Made
- Subprocess model spawns self with `--wasm-sandbox-exec` hidden flag; child applies OS restrictions then runs WASM component
- should_use_os_sandbox returns true only for Untrusted tier (Verified relies on WASM-only sandbox, Local runs natively)
- JSON IPC via SandboxRequest/SandboxResponse structs for clean parent-child communication
- Seatbelt profiles start with `(deny default)` and selectively allow system libraries, WASM binary, configured paths, and optional network
- Landlock uses ABI v3 with best-effort degradation (PartiallyEnforced/NotEnforced logged as warning, not error)
- Platform modules created in Task 1 alongside dispatch layer (deviation) to satisfy Rust compilation

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Platform modules created in Task 1 for dispatch compilation**
- **Found during:** Task 1 (Sandbox configuration and dispatch layer)
- **Issue:** sandbox.rs dispatch function references sandbox_macos/sandbox_linux modules via cfg-gated `super::` paths, but those modules didn't exist yet (planned for Task 2). Compilation failed with "could not find sandbox_macos in super".
- **Fix:** Created full platform implementations in Task 1 alongside the dispatch layer. Task 2 focused on adding the landlock Cargo.toml dependency and fixing test imports.
- **Files modified:** sandbox_macos.rs, sandbox_linux.rs (created), mod.rs (cfg-gated module declarations)
- **Verification:** `cargo check -p boternity-infra` passes
- **Committed in:** 14c1da4 (Task 1 commit)

**2. [Rule 1 - Bug] Missing TrustTier import in sandbox_macos test module**
- **Found during:** Task 2 (unit test verification)
- **Issue:** Test helper function referenced `TrustTier::Untrusted` directly but the type wasn't imported in the test module (only came indirectly through SandboxConfig field).
- **Fix:** Added `use boternity_types::skill::TrustTier;` to the test imports.
- **Files modified:** sandbox_macos.rs
- **Verification:** All 17 tests pass
- **Committed in:** 7c58086 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for compilation and testing. No scope creep -- same code was planned, just reordered across tasks.

## Issues Encountered
None beyond the deviations documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- OS sandbox layer complete and tested, ready for integration with skill executor pipeline
- WASM skills now have two isolation layers: WASM sandbox (Plan 07) inside OS sandbox (this plan)
- Subprocess model enables future hardening (seccomp, pledge) without changing calling code

## Self-Check: PASSED

---
*Phase: 06-skill-system-wasm-sandbox*
*Completed: 2026-02-14*
