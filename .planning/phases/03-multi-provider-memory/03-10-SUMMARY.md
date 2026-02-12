---
phase: 03-multi-provider-memory
plan: 10
subsystem: infra, storage
tags: [file-storage, text-splitter, lancedb, chunking, semantic-search, embeddings, versioning]

# Dependency graph
requires:
  - phase: 03-multi-provider-memory (plan 04)
    provides: LanceVectorStore, FastEmbedEmbedder, Arrow schemas for file_chunks table
  - phase: 03-multi-provider-memory (plan 05)
    provides: SqliteFileMetadataStore with file CRUD and version tracking
  - phase: 03-multi-provider-memory (plan 01)
    provides: FileStore trait, StorageFile/FileVersion/FileChunk types, Embedder trait
provides:
  - LocalFileStore implementing FileStore trait with version history on disk
  - Text chunking via text-splitter (TextSplitter for plain text, MarkdownSplitter for .md)
  - FileIndexer for chunking, embedding, and storing file content in LanceDB
  - MIME detection and text content classification
  - Semantic search over file chunks via vector similarity
affects: [03-11, 03-12, 03-13, all plans needing bot file storage or file-based knowledge retrieval]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Module-level detect_mime/is_text_mime for cross-module MIME utilities"
    - "RecordBatchIterator wrapping Vec<RecordBatch> for LanceDB add() (IntoArrow trait)"
    - "FixedSizeListArray::try_new with explicit Field for vector column construction"
    - "MockEmbedder pattern for testing vector operations without model download"
    - "Bot files stored under {base_dir}/bots/{bot_id_simple}/files/ with .versions/ subdirectory"

key-files:
  created:
    - crates/boternity-infra/src/storage/mod.rs
    - crates/boternity-infra/src/storage/filesystem.rs
    - crates/boternity-infra/src/storage/chunker.rs
    - crates/boternity-infra/src/storage/indexer.rs
  modified:
    - crates/boternity-infra/src/lib.rs

key-decisions:
  - "MIME detection and is_text_mime extracted to module-level public functions (not struct methods) for cross-module access"
  - "RecordBatchIterator used instead of Box<Vec<RecordBatch>> because Vec<RecordBatch> does not implement RecordBatchReader"
  - "QueryBase and ExecutableQuery traits must be imported for LanceDB .limit() and .execute() methods"
  - "FixedSizeListArray::try_new replaces try_new_from_values (arrow-array 57.3 API)"
  - "Bot files directory uses bot_id.simple() (no hyphens) as slug for filesystem paths"
  - "Closure type annotations required for Rust 2024 edition iterator maps with clone()"

patterns-established:
  - "MockEmbedder: deterministic vectors from text length + index, no ONNX model needed"
  - "RecordBatchIterator pattern: wrap Vec<Ok(batch)> + schema for LanceDB table.add()"
  - "get_string_col helper: extract typed column from RecordBatch with error propagation"

# Metrics
duration: 13m 58s
completed: 2026-02-12
---

# Phase 3 Plan 10: File Storage with Versioning and Semantic Indexing Summary

**LocalFileStore with disk versioning in .versions/ directory, text-splitter chunking (TextSplitter + MarkdownSplitter at 512 chars), and FileIndexer embedding chunks in LanceDB for semantic search**

## Performance

- **Duration:** 13m 58s
- **Started:** 2026-02-12T22:24:17Z
- **Completed:** 2026-02-12T22:38:15Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Implemented LocalFileStore with full FileStore trait compliance: save, get, delete, list, get_file_info, get_versions
- Version history preserved on disk in .versions/ subdirectory with automatic archival on re-save
- Text chunking via text-splitter: MarkdownSplitter for .md files, TextSplitter for all other text, 512-char default
- FileIndexer with index_file, deindex_file, reindex_file, and search_file_chunks operations on LanceDB
- MIME detection from file extension covering text, code, documents, images, and archives
- 50MB file size limit and path traversal rejection for security
- 29 tests covering filesystem operations, chunking, indexing, deindexing, reindexing, and semantic search
- MockEmbedder pattern for testing vector operations without downloading embedding model

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement file storage with version history** - `c4a6323` (feat)
2. **Task 2: Implement text chunking and file indexing** - `7215c9e` (feat)

## Files Created/Modified

**Created:**
- `crates/boternity-infra/src/storage/mod.rs` - Storage module root with detect_mime/is_text_mime utilities
- `crates/boternity-infra/src/storage/filesystem.rs` - LocalFileStore implementing FileStore trait with versioning
- `crates/boternity-infra/src/storage/chunker.rs` - chunk_text_file() with TextSplitter/MarkdownSplitter
- `crates/boternity-infra/src/storage/indexer.rs` - FileIndexer with LanceDB vector operations

**Modified:**
- `crates/boternity-infra/src/lib.rs` - Added `pub mod storage`

## Decisions Made
- **Module-level MIME functions:** `detect_mime` and `is_text_mime` extracted from struct methods to module-level public functions, enabling use by both LocalFileStore and FileIndexer without visibility issues
- **RecordBatchIterator for LanceDB add:** `Vec<RecordBatch>` does not implement `RecordBatchReader`, so must be wrapped in `RecordBatchIterator::new(vec![Ok(batch)], schema)` for `table.add()`
- **QueryBase + ExecutableQuery imports:** LanceDB 0.26 requires explicit trait imports for `.limit()` (QueryBase) and `.execute()` (ExecutableQuery) on VectorQuery
- **Arrow FixedSizeListArray API:** `try_new_from_values` is from `lance_arrow` extension trait; used native `FixedSizeListArray::try_new(field, size, values, nulls)` from arrow-array 57.3
- **Bot ID as directory slug:** Files stored under `{bot_id.simple()}` (UUID without hyphens) to avoid filesystem path issues

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed FixedSizeListArray construction API**
- **Found during:** Task 2 (indexer implementation)
- **Issue:** `FixedSizeListArray::try_new_from_values` is from `lance_arrow::FixedSizeListArrayExt` extension trait, not available in plain `arrow-array`
- **Fix:** Used `FixedSizeListArray::try_new(field, size, values, None)` with explicit `Field::new("item", DataType::Float32, true)`
- **Files modified:** crates/boternity-infra/src/storage/indexer.rs
- **Verification:** Arrow RecordBatch construction works, all indexer tests pass
- **Committed in:** 7215c9e (Task 2 commit)

**2. [Rule 3 - Blocking] Added missing LanceDB trait imports**
- **Found during:** Task 2 (indexer implementation)
- **Issue:** `VectorQuery::limit()` requires `QueryBase` trait and `.execute()` requires `ExecutableQuery` trait in scope
- **Fix:** Added `use lancedb::query::{ExecutableQuery, QueryBase};`
- **Files modified:** crates/boternity-infra/src/storage/indexer.rs
- **Verification:** Vector search compiles and test_search_file_chunks passes
- **Committed in:** 7215c9e (Task 2 commit)

**3. [Rule 3 - Blocking] Fixed RecordBatch insertion into LanceDB**
- **Found during:** Task 2 (indexer implementation)
- **Issue:** `Box<Vec<RecordBatch>>` does not implement `IntoArrow` because `Vec<RecordBatch>` does not implement `RecordBatchReader`
- **Fix:** Wrapped in `RecordBatchIterator::new(vec![Ok(batch)], schema)` which implements `RecordBatchReader`
- **Files modified:** crates/boternity-infra/src/storage/indexer.rs
- **Verification:** test_index_text_file and test_index_markdown_file pass
- **Committed in:** 7215c9e (Task 2 commit)

**4. [Rule 3 - Blocking] Extracted MIME functions to module level**
- **Found during:** Task 2 (indexer referencing filesystem.rs)
- **Issue:** `detect_mime` was a private method on `LocalFileStore`; indexer needed access. Linter rejected `pub(crate)` visibility change.
- **Fix:** Moved `detect_mime` and `is_text_mime` to `storage/mod.rs` as public module-level functions
- **Files modified:** crates/boternity-infra/src/storage/mod.rs, filesystem.rs
- **Verification:** Both filesystem and indexer compile, all tests pass
- **Committed in:** 7215c9e (Task 2 commit)

---

**Total deviations:** 4 auto-fixed (1 bug, 3 blocking)
**Impact on plan:** All fixes address LanceDB/Arrow API compatibility and module visibility. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations.

## User Setup Required
None - no external service configuration required. Text-splitter runs locally.

## Next Phase Readiness
- File storage infrastructure is fully operational with disk storage, metadata tracking, and semantic indexing
- Ready for Plan 03-11 (memory store integration) and Plan 03-12 (agent/chat service integration)
- FileIndexer can be composed with FastEmbedEmbedder for production use
- MockEmbedder available for integration testing without model download

## Self-Check: PASSED

---
*Phase: 03-multi-provider-memory*
*Completed: 2026-02-12*
