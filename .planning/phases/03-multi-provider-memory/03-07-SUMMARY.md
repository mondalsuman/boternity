---
phase: 03-multi-provider-memory
plan: 07
subsystem: infra, vector
tags: [lancedb, vector-search, cosine-similarity, time-decay, semantic-dedup, memory, arrow, embeddings]

# Dependency graph
requires:
  - phase: 03-multi-provider-memory (plan 01)
    provides: VectorMemoryStore trait, VectorMemoryEntry, RankedMemory types
  - phase: 03-multi-provider-memory (plan 04)
    provides: LanceVectorStore wrapper, Arrow schemas (bot_memory_schema), EMBEDDING_DIMENSION
  - phase: 03-multi-provider-memory (plan 05)
    provides: SQLite memory tables (MemoryEntry linked via source_memory_id)
provides:
  - LanceVectorMemoryStore implementing VectorMemoryStore trait
  - Cosine distance vector search with time-decay relevance scoring
  - Semantic dedup detection via check_duplicate (configurable distance threshold)
  - Embedding model mismatch detection for re-embedding workflows
  - Per-bot table isolation for memory storage
  - Arrow RecordBatch<->VectorMemoryEntry bidirectional conversion
  - DEFAULT_DEDUP_THRESHOLD constant (0.15) and DECAY_HALF_LIFE_DAYS (30)
affects: [03-08, 03-11, 03-12, 03-13, all plans needing bot memory retrieval]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "RecordBatchIterator for LanceDB table.add() -- wraps Vec<RecordBatch> with schema"
    - "column_by_name('_distance') to extract LanceDB vector search distance column"
    - "delete+insert for update_embedding since LanceDB update() uses SQL expressions only"
    - "compute_relevance_score: similarity * time_factor * reinforcement * importance_factor"
    - "Exponential time decay with configurable half-life (30 days)"

key-files:
  created:
    - crates/boternity-infra/src/vector/memory.rs
  modified:
    - crates/boternity-infra/src/vector/mod.rs

key-decisions:
  - "delete+insert for update_embedding: LanceDB UpdateBuilder uses SQL expressions, cannot set vector column from Vec<f32>"
  - "Search fetches limit*2 candidates then filters by min_similarity and truncates to limit"
  - "Access stats updated via LanceDB update() with SQL expressions (access_count, last_accessed_at)"
  - "update_embedding scans all bot_memory_* tables since trait signature lacks bot_id parameter"
  - "DEFAULT_DEDUP_THRESHOLD = 0.15 cosine distance (~92.5% similarity)"

patterns-established:
  - "Time-decay scoring: 30-day half-life exponential decay for memory relevance"
  - "Access reinforcement: 1.0 + 0.1 * min(access_count, 10), capped at 2.0x"
  - "Importance factor: maps 1-5 to 0.6-1.0 range"
  - "RecordBatch column access by index (0-10) matching bot_memory_schema field order"
  - "lancedb::query::{ExecutableQuery, QueryBase} must be imported for .execute() and .limit()/.only_if()"

# Metrics
duration: 10m 45s
completed: 2026-02-12
---

# Phase 3 Plan 07: LanceDB Vector Memory Store Summary

**LanceVectorMemoryStore implementing VectorMemoryStore with cosine search, time-decay scoring (30-day half-life), semantic dedup, embedding model mismatch detection, and per-bot table isolation -- 20 integration tests**

## Performance

- **Duration:** 10m 45s
- **Started:** 2026-02-12T22:25:47Z
- **Completed:** 2026-02-12T22:36:32Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Implemented LanceVectorMemoryStore with all 8 VectorMemoryStore trait methods (search, add, delete, delete_all, count, check_duplicate, get_all_for_reembedding, update_embedding)
- Built compute_relevance_score combining cosine similarity, exponential time decay (30-day half-life), access reinforcement (capped at 2.0x), and importance factor (0.6-1.0)
- Arrow RecordBatch bidirectional conversion (build_record_batch and record_batch_to_entries) for 11-column bot_memory schema
- 20 integration tests covering search ranking, dedup detection, model mismatch, CRUD, scoring formula, bot isolation, and RecordBatch roundtrip

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement LanceVectorMemoryStore with search and CRUD** - `a1a6ee1` (feat)

## Files Created/Modified

**Created:**
- `crates/boternity-infra/src/vector/memory.rs` - LanceVectorMemoryStore implementing VectorMemoryStore trait with all CRUD ops, search with ranking, semantic dedup, and embedding model tracking

**Modified:**
- `crates/boternity-infra/src/vector/mod.rs` - Added `pub mod memory` to vector module

## Decisions Made
- **delete+insert for update_embedding:** LanceDB's `UpdateBuilder` only accepts SQL expression strings for column values -- cannot pass a `Vec<f32>` for the vector column. Delete the old row and re-insert with new embedding.
- **Search over-fetches by 2x:** Fetches `limit * 2` candidates from LanceDB before applying min_similarity filtering, since time-decay scoring can reorder results relative to raw distance.
- **Table scan for update_embedding:** The `VectorMemoryStore::update_embedding` trait method doesn't accept `bot_id`, so implementation scans all `bot_memory_*` tables to find the memory by ID. Acceptable for the small number of bot tables expected.
- **DEFAULT_DEDUP_THRESHOLD = 0.15:** Cosine distance of 0.15 corresponds to ~92.5% similarity, which catches near-duplicates while allowing legitimately similar but distinct memories.
- **QueryBase and ExecutableQuery imports required:** LanceDB 0.26 requires explicit trait imports for `.limit()`, `.only_if()`, and `.execute()` methods on Query/VectorQuery builders.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed FixedSizeListArray construction for arrow-array 57.3**
- **Found during:** Task 1 (build_record_batch implementation)
- **Issue:** Plan referenced `FixedSizeListArray::try_new_from_values()` which does not exist in arrow-array 57.3. That method is from the lance_arrow extension trait, not the core arrow-array crate.
- **Fix:** Used `FixedSizeListArray::new(field, EMBEDDING_DIMENSION, Arc::new(values), None)` with explicit Field construction.
- **Files modified:** crates/boternity-infra/src/vector/memory.rs
- **Verification:** RecordBatch roundtrip test passes
- **Committed in:** a1a6ee1 (Task 1 commit)

**2. [Rule 3 - Blocking] Imported lancedb query traits for method resolution**
- **Found during:** Task 1 (VectorMemoryStore impl)
- **Issue:** `.limit()`, `.only_if()`, and `.execute()` methods on LanceDB Query/VectorQuery are defined in traits `QueryBase` and `ExecutableQuery`, not inherent methods. Without importing these traits, the methods are invisible.
- **Fix:** Added `use lancedb::query::{ExecutableQuery, QueryBase};` to imports.
- **Files modified:** crates/boternity-infra/src/vector/memory.rs
- **Verification:** All query-based methods compile and tests pass
- **Committed in:** a1a6ee1 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking)
**Impact on plan:** API compatibility fixes for actual library versions. No scope creep.

## Issues Encountered
- Parallel plan execution (03-10) had uncommitted changes in `storage/indexer.rs` that temporarily blocked compilation. Resolved by staging only the files for this plan's commit. The parallel plan's changes were not affected.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- LanceVectorMemoryStore is operational and ready for integration
- Ready for Plan 03-08 (memory service integration with ChatService)
- Ready for Plan 03-11 (memory CLI commands)
- compute_relevance_score parameters (half-life, reinforcement cap, importance range) can be tuned later via KV store settings
- DEFAULT_DEDUP_THRESHOLD exported for callers to use as default

## Self-Check: PASSED

---
*Phase: 03-multi-provider-memory*
*Completed: 2026-02-12*
