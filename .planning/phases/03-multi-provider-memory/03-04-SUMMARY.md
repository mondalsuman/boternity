---
phase: 03-multi-provider-memory
plan: 04
subsystem: infra, vector
tags: [lancedb, fastembed, arrow, vector-database, embeddings, onnx, bge-small]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: boternity-types and boternity-core crate structure, RepositoryError
  - phase: 03-multi-provider-memory (plan 01)
    provides: Embedder trait in boternity-core, VectorMemoryStore trait, SharedMemoryStore trait
provides:
  - LanceVectorStore wrapper with connection management and table lifecycle (ensure, open, drop)
  - FastEmbedEmbedder implementing Embedder trait using BGESmallENV15 (384-dim vectors)
  - Arrow schemas for bot_memory, shared_memory, and file_chunks tables
  - Table naming conventions (bot_memory_{id}, shared_memory, file_chunks_{id})
  - EMBEDDING_DIMENSION constant (384)
affects: [03-07, 03-09, 03-10, 03-11, all plans needing vector memory or embeddings]

# Tech tracking
tech-stack:
  added:
    - "lancedb 0.26 (embedded vector database)"
    - "fastembed 5 (local ONNX embedding inference)"
    - "text-splitter 0.29 (semantic text chunking)"
    - "arrow-schema 57.3 (Arrow schema definitions)"
    - "arrow-array 57.3 (Arrow array types)"
  patterns:
    - "Arc<Mutex<TextEmbedding>> for thread-safe mutable fastembed access in spawn_blocking"
    - "create_empty_table for schema-first LanceDB table creation"
    - "ensure_table pattern: open_table -> fallback to create_empty_table on TableNotFound"
    - "Per-bot table naming: bot_memory_{uuid_simple}, file_chunks_{uuid_simple}"

key-files:
  created:
    - crates/boternity-infra/src/vector/mod.rs
    - crates/boternity-infra/src/vector/lance.rs
    - crates/boternity-infra/src/vector/embedder.rs
    - crates/boternity-infra/src/vector/schema.rs
  modified:
    - Cargo.toml
    - crates/boternity-infra/Cargo.toml
    - crates/boternity-infra/src/lib.rs

key-decisions:
  - "Arrow version 57.3 pinned to match lancedb 0.26 transitive dependency (Pitfall 10)"
  - "Arc<Mutex<TextEmbedding>> not Arc<TextEmbedding> because fastembed embed() requires &mut self"
  - "RepositoryError::Query used for embedding/vector errors (no Internal variant exists)"
  - "TextInitOptions builder pattern (non_exhaustive struct cannot be constructed directly)"
  - "drop_table is idempotent (returns Ok on TableNotFound)"

patterns-established:
  - "spawn_blocking for all CPU-intensive fastembed ONNX inference"
  - "ensure_table: try open, create on NotFound -- idempotent table setup"
  - "Table name convention: bot_memory_{bot_id_simple} for per-bot isolation"

# Metrics
duration: 16m 27s
completed: 2026-02-12
---

# Phase 3 Plan 04: LanceDB + fastembed Infrastructure Summary

**LanceDB vector store with ensure_table lifecycle, FastEmbedEmbedder using BGESmallENV15 (384-dim) via spawn_blocking, and Arrow schemas for bot/shared/file-chunks tables**

## Performance

- **Duration:** 16m 27s
- **Started:** 2026-02-12T22:03:38Z
- **Completed:** 2026-02-12T22:20:05Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Added 5 new workspace dependencies (lancedb, fastembed, text-splitter, arrow-schema, arrow-array) with arrow version pinned to match lancedb transitive dep
- Created LanceVectorStore with full table lifecycle (ensure, exists, drop, list) and per-bot/shared table naming
- Created FastEmbedEmbedder implementing the core Embedder trait with BGESmallENV15 model (384-dim vectors)
- Defined Arrow schemas for all three Phase 3 vector tables (bot_memory, shared_memory, file_chunks)
- 14 tests covering schema validation, table CRUD, name generation, and embedding integration

## Task Commits

Each task was committed atomically:

1. **Task 1: Add vector dependencies and create schema definitions** - `4aa01a9` (feat)
2. **Task 2: Implement LanceVectorStore and FastEmbedEmbedder** - `cd0b10d` (feat)

## Files Created/Modified

**Created:**
- `crates/boternity-infra/src/vector/mod.rs` - Vector module root (re-exports embedder, lance, schema)
- `crates/boternity-infra/src/vector/lance.rs` - LanceVectorStore wrapper with connection + table lifecycle
- `crates/boternity-infra/src/vector/embedder.rs` - FastEmbedEmbedder implementing Embedder trait via spawn_blocking
- `crates/boternity-infra/src/vector/schema.rs` - Arrow schemas for bot_memory, shared_memory, file_chunks + EMBEDDING_DIMENSION constant

**Modified:**
- `Cargo.toml` - Added lancedb, fastembed, text-splitter, arrow-schema, arrow-array workspace deps
- `crates/boternity-infra/Cargo.toml` - Added arrow-schema and arrow-array workspace deps
- `crates/boternity-infra/src/lib.rs` - Added `pub mod vector`

## Decisions Made
- Arrow version 57.3 pinned after verifying lancedb 0.26's transitive dependency via `cargo tree` (per RESEARCH.md Pitfall 10)
- `Arc<Mutex<TextEmbedding>>` chosen because fastembed's `embed()` requires `&mut self` -- cannot use `Arc<TextEmbedding>` directly
- Used `RepositoryError::Query(String)` for all vector/embedding errors since `RepositoryError::Internal` does not exist
- Used `TextInitOptions::new(model).with_cache_dir(dir)` builder pattern since `TextInitOptions` is `#[non_exhaustive]`
- `drop_table` is idempotent -- returns `Ok(())` when table does not exist

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed fastembed API usage for v5.x**
- **Found during:** Task 2 (FastEmbedEmbedder implementation)
- **Issue:** Plan referenced `InitOptions` struct with direct construction and `Arc<TextEmbedding>`, but fastembed v5.9 uses `TextInitOptions` (a type alias for `InitOptionsWithLength<EmbeddingModel>`) which is `#[non_exhaustive]` and requires builder pattern. Also `embed()` takes `&mut self` not `&self`.
- **Fix:** Used `TextInitOptions::new(model).with_cache_dir()` builder and wrapped in `Arc<Mutex<TextEmbedding>>` instead of `Arc<TextEmbedding>`
- **Files modified:** crates/boternity-infra/src/vector/embedder.rs
- **Verification:** All 3 embedder tests pass including integration test generating 384-dim vectors
- **Committed in:** cd0b10d (Task 2 commit)

**2. [Rule 1 - Bug] Used RepositoryError::Query instead of non-existent Internal variant**
- **Found during:** Task 2 (FastEmbedEmbedder implementation)
- **Issue:** Plan implied `RepositoryError::Internal(String)` but the enum only has Connection, Query(String), NotFound, Conflict(String)
- **Fix:** Used `RepositoryError::Query(String)` for all embedding/vector error wrapping
- **Files modified:** crates/boternity-infra/src/vector/embedder.rs
- **Verification:** Code compiles, error messages are descriptive
- **Committed in:** cd0b10d (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** API compatibility fixes for actual library versions. No scope creep.

## Issues Encountered
- Parallel plan executions (03-02, 03-03, 03-05) committed between Task 1 and Task 2, requiring temporary file management during verification. All resolved by the parallel commits completing.
- The `git checkout` during verification restored the embedder.rs placeholder, requiring a rewrite of the implementation. No code was lost.

## User Setup Required
None - no external service configuration required. Model downloads automatically on first embedding call (~23MB).

## Next Phase Readiness
- LanceDB and fastembed infrastructure is operational
- Ready for Plan 03-07 (VectorMemoryStore trait implementation over LanceDB)
- Ready for Plan 03-09 (SharedMemoryStore trait implementation)
- Ready for Plan 03-10 (file storage with semantic chunking via text-splitter)
- Embedding model auto-downloads to `{data_dir}/boternity/models` on first use

## Self-Check: PASSED

---
*Phase: 03-multi-provider-memory*
*Completed: 2026-02-12*
