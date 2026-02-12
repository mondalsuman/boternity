---
phase: 03-multi-provider-memory
plan: 09
subsystem: infra, vector
tags: [lancedb, shared-memory, trust-levels, sha256, provenance, cross-bot, vector-search]

# Dependency graph
requires:
  - phase: 03-multi-provider-memory (plan 01)
    provides: SharedMemoryStore trait, SharedMemoryEntry, TrustLevel, RankedMemory types
  - phase: 03-multi-provider-memory (plan 04)
    provides: LanceVectorStore wrapper, Arrow schemas (shared_memory_schema), EMBEDDING_DIMENSION
  - phase: 03-multi-provider-memory (plan 07)
    provides: LanceVectorMemoryStore patterns (RecordBatch build/parse, trust filter SQL, vector search)
provides:
  - LanceSharedMemoryStore implementing SharedMemoryStore trait
  - Trust-level partitioned vector search (public/trusted/private)
  - SHA-256 tamper detection with compute_write_hash
  - Per-bot contribution cap (default 500, configurable)
  - Provenance annotation ("Written by BotX") on all search results
  - Author-only deletion and revocation
  - Share/revoke via delete+insert pattern (preserves embedding vector)
  - DEFAULT_CONTRIBUTION_CAP constant (500)
affects: [03-11, 03-12, 03-13, all plans needing cross-bot memory sharing]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Trust filter SQL: OR-combined clauses for public, author-self, trusted-list access"
    - "extract_embedding_from_batch: reads FixedSizeListArray->Float32Array for delete+insert ops"
    - "to_vector_entry: converts SharedMemoryEntry to VectorMemoryEntry for RankedMemory compatibility"
    - "compute_write_hash: SHA-256 over id+fact+category+importance+author+trust_level+created_at"

key-files:
  created:
    - crates/boternity-infra/src/vector/shared.rs
  modified:
    - crates/boternity-infra/src/vector/mod.rs

key-decisions:
  - "SHA-256 write_hash covers: id, fact, category, importance, author_bot_id, trust_level, created_at -- not embedding_model or vector"
  - "Share/revoke uses delete+insert (same as update_embedding in 03-07) since LanceDB update() cannot set vector columns"
  - "Shared memory search uses raw similarity as relevance score (no time-decay or access tracking)"
  - "Per-bot cap checked on every add() via count_by_author query"
  - "Trust filter is OR-combined SQL: public OR author_self OR (trusted AND author IN list)"
  - "extract_embedding_from_batch needed to preserve vector during share/revoke operations"

patterns-established:
  - "Trust-level access control via SQL filter in LanceDB vector_search().only_if()"
  - "Provenance annotation in RankedMemory.provenance field"
  - "SharedMemoryEntry to VectorMemoryEntry conversion for unified ranking"

# Metrics
duration: 3m 31s
completed: 2026-02-12
---

# Phase 3 Plan 09: Shared Memory Store with Trust Partitioning Summary

**LanceSharedMemoryStore implementing SharedMemoryStore with 3-tier trust filtering (public/trusted/private), SHA-256 integrity hashes, per-bot caps (500), provenance tracking, and author-only revocation -- 23 integration tests**

## Performance

- **Duration:** 3m 31s
- **Started:** 2026-02-12T22:42:16Z
- **Completed:** 2026-02-12T22:45:47Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Implemented LanceSharedMemoryStore with all 7 SharedMemoryStore trait methods (add, search, delete, share, revoke, count_by_author, verify_integrity)
- Trust-level filtering via SQL OR clauses: public visible to all, trusted visible to explicit trust list, private visible to author only
- SHA-256 write_hash computed on every write, recomputed on share/revoke, verified via verify_integrity()
- Per-bot contribution cap (default 500, configurable via with_cap()) enforced on every add()
- Provenance annotation "Written by BotX" attached to all search results
- Share/revoke implemented as delete+insert to preserve embedding vectors (LanceDB limitation)
- Arrow RecordBatch bidirectional conversion for 11-column shared_memory schema
- 23 integration tests covering trust filtering (3 levels), integrity verification, share/revoke lifecycle, contribution caps, provenance, search ranking, and RecordBatch roundtrip

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement LanceSharedMemoryStore with trust partitioning** - `1d5370a` (feat)

## Files Created/Modified

**Created:**
- `crates/boternity-infra/src/vector/shared.rs` - LanceSharedMemoryStore implementing SharedMemoryStore trait with trust filtering, SHA-256 integrity, per-bot caps, and provenance

**Modified:**
- `crates/boternity-infra/src/vector/mod.rs` - Added `pub mod shared` to vector module

## Decisions Made
- **SHA-256 hash scope:** Hash covers id, fact, category, importance, author_bot_id, trust_level, and created_at. Deliberately excludes embedding_model and vector data (those are not content).
- **Delete+insert for share/revoke:** Same pattern as 03-07's update_embedding. LanceDB's UpdateBuilder cannot set vector columns from Vec<f32>, so share/revoke reads the existing embedding, deletes the row, and re-inserts with updated trust level and recomputed hash.
- **Relevance = raw similarity for shared memories:** Unlike per-bot memories (which use time-decay, access reinforcement, and importance factors), shared memory search uses cosine similarity directly as the relevance score. This avoids needing access tracking infrastructure on the shared table.
- **Cap enforcement on add():** Per-bot cap checked via count_by_author() on every add() call. Slightly more expensive than a cached counter but ensures accuracy without stale state.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed temporary value lifetime in extract_embedding_from_batch**
- **Found during:** Task 1 (compilation)
- **Issue:** `vector_col.value(row_index)` returns an owned `Arc<dyn Array>` that was being used as a temporary. Calling `.as_any().downcast_ref()` on it created a borrow that outlived the temporary.
- **Fix:** Bound the value to a `let value_array` binding before calling `.as_any().downcast_ref()`.
- **Files modified:** crates/boternity-infra/src/vector/shared.rs
- **Verification:** Compilation succeeds, all 23 tests pass
- **Committed in:** 1d5370a (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor Rust lifetime fix. No scope change.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- LanceSharedMemoryStore is operational and ready for integration
- Ready for Plan 03-11 (memory CLI commands with shared memory search/share/revoke)
- Ready for Plan 03-12 (cross-bot memory integration)
- DEFAULT_CONTRIBUTION_CAP exported for callers to override via with_cap()
- Trust filter pattern reusable for file sharing (if shared files need same trust model)

## Self-Check: PASSED

---
*Phase: 03-multi-provider-memory*
*Completed: 2026-02-12*
