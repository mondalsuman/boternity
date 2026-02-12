---
phase: 03-multi-provider-memory
plan: 01
subsystem: types, core
tags: [domain-types, traits, rpitit, circuit-breaker, vector-memory, shared-memory, embedder, kv-store, file-storage, fallback-chain]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: boternity-types and boternity-core crate structure, RepositoryError, MemoryCategory, MemoryEntry
  - phase: 02-single-agent-chat
    provides: LlmProvider trait, BoxLlmProvider, LlmError, ProviderCapabilities, CompletionRequest/Response
provides:
  - ProviderType, ProviderConfig, FallbackChainConfig, ProviderCostInfo, ProviderStatusInfo in boternity-types
  - TrustLevel, VectorMemoryEntry, SharedMemoryEntry, MemoryAuditEntry, AuditAction, RankedMemory in boternity-types
  - StorageFile, FileVersion, FileChunk, KvEntry, MAX_FILE_SIZE_BYTES in boternity-types
  - CircuitState, ProviderHealth with circuit breaker logic in boternity-core
  - FallbackChain stub in boternity-core
  - ProviderRegistry in boternity-core
  - VectorMemoryStore trait in boternity-core
  - SharedMemoryStore trait in boternity-core
  - Embedder trait in boternity-core
  - FileStore trait in boternity-core
  - KvStore trait in boternity-core
affects: [03-02 through 03-13, all Phase 3 implementation plans]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Circuit breaker pattern for provider health tracking (Closed/Open/HalfOpen)"
    - "RPITIT traits for all new core abstractions (no async_trait)"
    - "Trust levels for cross-bot memory sharing (Private by default)"
    - "SHA-256 tamper detection for shared memories"

key-files:
  created:
    - crates/boternity-types/src/storage.rs
    - crates/boternity-core/src/llm/health.rs
    - crates/boternity-core/src/llm/fallback.rs
    - crates/boternity-core/src/llm/registry.rs
    - crates/boternity-core/src/memory/vector.rs
    - crates/boternity-core/src/memory/shared.rs
    - crates/boternity-core/src/memory/embedder.rs
    - crates/boternity-core/src/storage/mod.rs
    - crates/boternity-core/src/storage/file_store.rs
    - crates/boternity-core/src/storage/kv_store.rs
  modified:
    - crates/boternity-types/src/llm.rs
    - crates/boternity-types/src/memory.rs
    - crates/boternity-types/src/lib.rs
    - crates/boternity-core/src/llm/mod.rs
    - crates/boternity-core/src/memory/mod.rs
    - crates/boternity-core/src/lib.rs

key-decisions:
  - "ProviderType uses explicit serde rename for OpenAiCompatible to get 'openai_compatible' (not 'open_ai_compatible')"
  - "FallbackChainConfig defaults: 5000ms rate_limit_queue_timeout, 3.0x cost_warning_multiplier (per user decision)"
  - "TrustLevel::Private as default (per user decision: private by default)"
  - "CircuitState uses Instant for timing (not DateTime) for monotonic clock correctness"
  - "ProviderHealth failure_threshold=3, success_threshold=1, open_duration=30s defaults"
  - "is_failover_error classifies Provider/Stream/RateLimited/Overloaded as failover, AuthenticationFailed/InvalidRequest/ContextLengthExceeded as non-failover"

patterns-established:
  - "Circuit breaker pattern: Closed -> Open (after N failures) -> HalfOpen (after timeout) -> Closed (after success)"
  - "Health tracking types live in core (not infra) because FallbackChain depends on them"
  - "Stub methods with todo!() for deferred implementation across plans"

# Metrics
duration: 5m 7s
completed: 2026-02-12
---

# Phase 3 Plan 01: Domain Types and Core Traits Summary

**Phase 3 type foundation: ProviderType/Config/Health, VectorMemory/SharedMemory/Embedder traits, FileStore/KvStore traits with circuit breaker pattern**

## Performance

- **Duration:** 5m 7s
- **Started:** 2026-02-12T21:54:43Z
- **Completed:** 2026-02-12T21:59:50Z
- **Tasks:** 2
- **Files modified:** 16

## Accomplishments
- Extended boternity-types with 15 new domain types across llm, memory, and storage modules
- Defined 7 new RPITIT traits in boternity-core (VectorMemoryStore, SharedMemoryStore, Embedder, FileStore, KvStore, plus FallbackChain and ProviderRegistry structs)
- Implemented full circuit breaker logic in ProviderHealth with 7 unit tests
- Maintained dependency inversion: boternity-core has zero dependency on boternity-infra

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend domain types in boternity-types** - `6f1bc50` (feat)
2. **Task 2: Define core traits in boternity-core** - `553d49d` (feat)

## Files Created/Modified

**Created:**
- `crates/boternity-types/src/storage.rs` - StorageFile, FileVersion, FileChunk, KvEntry, MAX_FILE_SIZE_BYTES
- `crates/boternity-core/src/llm/health.rs` - CircuitState, ProviderHealth with circuit breaker logic
- `crates/boternity-core/src/llm/fallback.rs` - FallbackChain stub (complete/stream deferred to 03-03)
- `crates/boternity-core/src/llm/registry.rs` - ProviderRegistry for name-indexed provider lookup
- `crates/boternity-core/src/memory/vector.rs` - VectorMemoryStore trait (search, add, delete, dedup, reembedding)
- `crates/boternity-core/src/memory/shared.rs` - SharedMemoryStore trait (cross-bot sharing with trust levels)
- `crates/boternity-core/src/memory/embedder.rs` - Embedder trait (text-to-vector conversion)
- `crates/boternity-core/src/storage/mod.rs` - Storage module root
- `crates/boternity-core/src/storage/file_store.rs` - FileStore trait (versioned file CRUD)
- `crates/boternity-core/src/storage/kv_store.rs` - KvStore trait (bot-scoped key-value store)

**Modified:**
- `crates/boternity-types/src/llm.rs` - Added ProviderType, ProviderConfig, FallbackChainConfig, ProviderCostInfo, ProviderStatusInfo
- `crates/boternity-types/src/memory.rs` - Added TrustLevel, VectorMemoryEntry, SharedMemoryEntry, MemoryAuditEntry, AuditAction, RankedMemory
- `crates/boternity-types/src/lib.rs` - Added `pub mod storage`
- `crates/boternity-core/src/llm/mod.rs` - Added health, fallback, registry submodules
- `crates/boternity-core/src/memory/mod.rs` - Added vector, shared, embedder submodules
- `crates/boternity-core/src/lib.rs` - Added `pub mod storage`

## Decisions Made
- ProviderType uses explicit `#[serde(rename = "openai_compatible")]` because `rename_all = "snake_case"` produces `open_ai_compatible` from `OpenAiCompatible`
- CircuitState timing uses `std::time::Instant` (monotonic clock) not `DateTime` for correctness
- `is_failover_error` is a static method on ProviderHealth for clear classification of transient vs permanent errors
- ProviderHealth defaults: failure_threshold=3, success_threshold=1, open_duration=30s (from RESEARCH.md)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed ProviderType serde serialization mismatch**
- **Found during:** Task 1 (test_provider_type_serde)
- **Issue:** `#[serde(rename_all = "snake_case")]` on ProviderType serialized `OpenAiCompatible` as `open_ai_compatible` instead of `openai_compatible`
- **Fix:** Added explicit `#[serde(rename = "openai_compatible")]` on the `OpenAiCompatible` variant
- **Files modified:** crates/boternity-types/src/llm.rs
- **Verification:** test_provider_type_serde passes
- **Committed in:** 6f1bc50 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Trivial serde naming fix. No scope creep.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All Phase 3 domain types and core traits are in place
- Ready for Plan 03-02 (vector memory infra implementation) and all subsequent Phase 3 plans
- FallbackChain::complete and FallbackChain::stream are stubbed with todo!(), awaiting Plan 03-03

## Self-Check: PASSED

---
*Phase: 03-multi-provider-memory*
*Completed: 2026-02-12*
