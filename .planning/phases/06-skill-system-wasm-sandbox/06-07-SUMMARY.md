---
phase: 06-skill-system-wasm-sandbox
plan: 07
subsystem: infra
tags: [wasmtime, wasm, sandbox, capability, resource-limiter, fuel, security]

# Dependency graph
requires:
  - phase: 06-03
    provides: CapabilityEnforcer for permission checking
  - phase: 06-05
    provides: WasmRuntime with per-tier engines and bindgen! bindings
  - phase: 06-06
    provides: SkillExecutor trait and SkillExecutionResult type
provides:
  - WasmSkillExecutor implementing SkillExecutor for WASM sandboxed execution
  - SkillState with ResourceLimiter (memory + table caps)
  - Capability-gated Host trait implementation for all WIT imports
  - Async export bindings for SkillPlugin (call_execute async)
affects: [06-08, 06-10, 06-11, 06-12]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Fresh Store per WASM invocation (no state leaks)"
    - "ResourceLimiter on SkillState for memory/table caps"
    - "Capability HashSet check before every host function"
    - "Sync host imports with async exports via bindgen! config"

key-files:
  created:
    - crates/boternity-infra/src/skill/wasm_executor.rs
  modified:
    - crates/boternity-infra/src/skill/wasm_runtime.rs

key-decisions:
  - "bindgen! exports: { default: async } for async call_execute with async_support engine"
  - "Sync host imports (std::fs, std::env) -- no async I/O in host functions"
  - "Table entries capped at 1000 in ResourceLimiter"
  - "recall_memory returns empty Vec (not error) when capability missing -- graceful degradation"
  - "Stubs for http_get, http_post, get_secret -- to be wired in future plans"

patterns-established:
  - "Capability gating: every host import checks HashSet<Capability> before execution"
  - "Fresh Store pattern: new SkillState per execute() call, no shared mutable state"
  - "fuel_consumed = initial - remaining (saturating_sub for safety)"

# Metrics
duration: 7min
completed: 2026-02-14
---

# Phase 6 Plan 7: WASM Sandboxed Skill Executor Summary

**WasmSkillExecutor with fresh-Store-per-invocation, fuel limits, ResourceLimiter memory cap, and capability-gated Host imports for all WIT functions**

## Performance

- **Duration:** 7m 10s
- **Started:** 2026-02-14T00:01:37Z
- **Completed:** 2026-02-14T00:08:47Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- WasmSkillExecutor implementing SkillExecutor trait with full security boundary
- SkillState with ResourceLimiter: memory growth capped per trust tier, table entries at 1000
- Host trait implementation: 10 WIT import functions with capability checks (get_context/log always allowed, 8 others gated)
- Updated bindgen! to generate async exports (required for async_support engine)
- 14 unit tests covering ResourceLimiter deny/allow and every host function capability gate

## Task Commits

Each task was committed atomically:

1. **Task 1: WASM skill executor with capability-gated host imports** - `a1de3cd` (feat)

## Files Created/Modified

- `crates/boternity-infra/src/skill/wasm_executor.rs` - WasmSkillExecutor, SkillState, Host impl, ResourceLimiter, 14 tests
- `crates/boternity-infra/src/skill/wasm_runtime.rs` - Updated bindgen! with `exports: { default: async }` for async call_execute

## Decisions Made

- **Async exports only:** `exports: { default: async }` in bindgen! config. Host imports remain sync since they use `std::fs`/`std::env`, not tokio. This avoids async complexity in the Host trait while supporting async call_execute on the engine with async_support(true).
- **Table entries cap at 1000:** Prevents unbounded table growth from malicious WASM modules. Separate from the memory cap which is per-trust-tier.
- **recall_memory returns empty on denied:** Unlike other host functions that return `Err(String)`, recall_memory returns an empty Vec when capability is missing. This matches the WIT return type (`list<string>`) and provides graceful degradation.
- **Stub implementations for http_get/post, get_secret:** These require async I/O (reqwest, secret provider) that will be wired in future plans. Current stubs return descriptive errors.
- **bot_slug/bot_name empty in SkillExecutor trait path:** The SkillExecutor trait receives InstalledSkill which doesn't carry bot context. Empty strings used as placeholders until the caller provides bot context via a separate mechanism.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated bindgen! macro to generate async exports**
- **Found during:** Task 1 (compilation)
- **Issue:** Engine has async_support(true) but bindgen! generated sync call_execute. Calling sync functions on async-configured Store panics at runtime.
- **Fix:** Added `exports: { default: async }` to bindgen! macro in wasm_runtime.rs
- **Files modified:** crates/boternity-infra/src/skill/wasm_runtime.rs
- **Verification:** cargo check passes, call_execute properly awaitable
- **Committed in:** a1de3cd (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential fix for correctness. Sync calls on async engine would cause runtime panic.

## Issues Encountered

- Parallel plans 06-06 and 06-09 had already modified `mod.rs` to include `wasm_executor` module declaration and added `local_executor`/`registry_client` modules. No conflict -- our module was already declared.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- WasmSkillExecutor ready for integration with skill invocation pipeline (06-10)
- Stub host functions (http_get/post, get_secret) need wiring to real implementations
- Bot context (bot_slug, bot_name) needs to be threaded through from caller

## Self-Check: PASSED

---
*Phase: 06-skill-system-wasm-sandbox*
*Completed: 2026-02-14*
