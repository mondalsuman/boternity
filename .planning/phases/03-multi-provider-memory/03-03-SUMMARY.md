---
phase: 03-multi-provider-memory
plan: 03
subsystem: core, llm
tags: [circuit-breaker, fallback-chain, failover, health-tracking, rate-limiting, provider-routing]

# Dependency graph
requires:
  - phase: 02-single-agent-chat
    provides: LlmProvider trait, BoxLlmProvider, LlmError, ProviderCapabilities, CompletionRequest/Response
  - phase: 03-multi-provider-memory (plan 01)
    provides: CircuitState, ProviderHealth struct, FallbackChain stub, ProviderCostInfo, ProviderStatusInfo
provides:
  - FallbackChain with complete() and select_stream() methods for multi-provider routing
  - FallbackResult and StreamSelection return types with failover warnings
  - Complete circuit breaker state machine with all transitions tested
  - Cost and capability downgrade warnings on failover
  - Rate limit queuing with configurable timeout
  - record_stream_success/failure for post-stream health updates
  - primary_available() for auto-switch-back detection
affects: [03-04 through 03-13, Phase 4 web UI provider status display]

# Tech tracking
tech-stack:
  added:
    - "async-stream 0.3 (added to boternity-core for mock provider streams in tests)"
    - "tokio time feature (added to boternity-core for rate limit queuing)"
  patterns:
    - "Fallback chain routing: priority-sorted providers with circuit breaker gating"
    - "Failover warning composition: cost + capability checks on non-primary provider"
    - "Stream health tracking: select_stream returns provider name, caller reports success/failure"

key-files:
  created: []
  modified:
    - crates/boternity-core/src/llm/health.rs
    - crates/boternity-core/src/llm/fallback.rs
    - crates/boternity-core/Cargo.toml

key-decisions:
  - "complete() returns FallbackResult struct (response + provider_name + failover_warning) instead of tuple for clarity"
  - "select_stream() instead of stream() -- separates provider selection from stream consumption, avoids &mut self in 'static stream"
  - "record_stream_success/failure methods for caller to report stream outcome (cannot track automatically from 'static stream)"
  - "Priority tiebreaking: latency first, then alphabetical name"
  - "Cost warning uses average of input+output cost per million for ratio comparison"

patterns-established:
  - "Mock provider pattern: MockProvider struct with MockResult enum for testing LlmProvider consumers"
  - "Stream health reporting: select provider, get stream, report outcome separately"
  - "Failover warning composition: base message + capability check + cost check, joined by periods"

# Metrics
duration: 9m 30s
completed: 2026-02-12
---

# Phase 3 Plan 03: Circuit Breaker and Fallback Chain Summary

**FallbackChain with priority-based provider routing, circuit breaker gating, rate limit queuing, cost/capability warnings, and mock-tested failover scenarios**

## Performance

- **Duration:** 9m 30s
- **Started:** 2026-02-12T22:03:18Z
- **Completed:** 2026-02-12T22:12:48Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Completed circuit breaker test coverage (7 -> 17 tests) covering all state transitions
- Implemented FallbackChain::complete() with priority-based routing, rate limit queuing, and failover on transient errors
- Implemented FallbackChain::select_stream() with same provider selection logic for streaming
- Added cost warning (>3x multiplier) and capability downgrade warnings on failover
- Auth/config errors correctly bypass failover and return immediately
- Clear "all providers down" error message with `bnity provider status` CLI hint
- 16 mock-based unit tests covering all fallback scenarios

## Task Commits

Each task was committed atomically:

1. **Task 1: Complete circuit breaker state machine in ProviderHealth** - `4018ea9` (test)
2. **Task 2: Implement FallbackChain complete() and select_stream()** - `f2b2956` (feat)

## Files Created/Modified

**Modified:**
- `crates/boternity-core/src/llm/health.rs` - Added 10 new tests for all circuit breaker state transitions
- `crates/boternity-core/src/llm/fallback.rs` - Full FallbackChain implementation replacing todo!() stubs
- `crates/boternity-core/Cargo.toml` - Added async-stream and tokio (time) dependencies

## Decisions Made
- `complete()` returns `FallbackResult` struct instead of a 3-tuple for readability and named access
- Renamed `stream()` to `select_stream()` -- returns `StreamSelection` with stream + metadata; avoids borrow checker issues with `&mut self` captured in `'static` stream
- Added `record_stream_success/failure` methods so callers can report stream outcome after consumption
- Cost warning ratio computed from average of input + output cost per million tokens
- Priority tiebreaking order: priority number (ascending) -> last latency (ascending) -> name (alphabetical)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added async-stream and tokio dependencies to boternity-core**
- **Found during:** Task 2 (FallbackChain implementation)
- **Issue:** `async-stream` needed for mock provider stream tests; `tokio::time::sleep` needed for rate limit queuing
- **Fix:** Added `async-stream = { workspace = true }` and `tokio = { version = "1", features = ["time"] }` to Cargo.toml
- **Files modified:** crates/boternity-core/Cargo.toml
- **Verification:** `cargo check --workspace` compiles
- **Committed in:** f2b2956 (Task 2 commit)

**2. [Rule 1 - Bug] Changed stream() to select_stream() to avoid borrow checker conflict**
- **Found during:** Task 2 (stream implementation design)
- **Issue:** Original `stream(&mut self)` signature cannot return `'static` stream while borrowing `self` mutably. The plan's `async_stream::stream!` approach would capture `&mut self` in a non-`'static` closure.
- **Fix:** Renamed to `select_stream()` which does provider selection synchronously, returns owned stream + metadata. Added `record_stream_success/failure` for post-stream health updates.
- **Files modified:** crates/boternity-core/src/llm/fallback.rs
- **Verification:** All 16 fallback tests pass including stream tests
- **Committed in:** f2b2956 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both changes necessary for correct compilation and Rust borrow checker compliance. No scope creep.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- FallbackChain is fully functional with complete(), select_stream(), health_status(), primary_available()
- Ready for Plan 03-04+ to wire FallbackChain into ChatService and build provider CLI commands
- All provider routing logic is in boternity-core (no infra dependency needed)
- Mock provider pattern established for future LlmProvider consumer tests

## Self-Check: PASSED

---
*Phase: 03-multi-provider-memory*
*Completed: 2026-02-12*
