---
phase: 03-multi-provider-memory
plan: 13
subsystem: api, cli
tags: [storage, kv-store, file-upload, json, cli, clap, comfy-table, auto-index]

# Dependency graph
requires:
  - phase: 03-multi-provider-memory (plan 10)
    provides: LocalFileStore, FileIndexer, detect_mime/is_text_mime utilities
  - phase: 03-multi-provider-memory (plan 05)
    provides: SqliteKvStore, SqliteAuditLog, SqliteProviderHealthStore, SqliteFileMetadataStore
  - phase: 03-multi-provider-memory (plan 04)
    provides: LanceVectorStore, FastEmbedEmbedder
  - phase: 03-multi-provider-memory (plan 07)
    provides: LanceVectorMemoryStore
  - phase: 03-multi-provider-memory (plan 09)
    provides: LanceSharedMemoryStore
provides:
  - Storage CLI subcommands (bnity storage upload/download/list/info/delete)
  - KV store CLI subcommands (bnity kv set/get/delete/list)
  - AppState wired with all Phase 3 services (vector_store, embedder, vector_memory, shared_memory, file_store, file_indexer, kv_store, audit_log, provider_health_store)
affects: [04-web-ui-core-fleet-dashboard, all plans needing file or KV operations via CLI]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "JSON value fallback: parse as JSON, fallback to string for CLI UX"
    - "format_size helper for human-readable byte display"
    - "Auto-index on upload: text files detected by MIME, chunked and embedded automatically"

key-files:
  created:
    - crates/boternity-api/src/cli/storage.rs
    - crates/boternity-api/src/cli/kv.rs
  modified:
    - crates/boternity-api/src/state.rs
    - crates/boternity-api/src/cli/mod.rs
    - crates/boternity-api/src/main.rs

key-decisions:
  - "KV set parses value as JSON with fallback to string -- good UX for simple values like `bnity kv set bot1 name Alice`"
  - "Storage delete deindexes from vector store before removing file -- prevents orphaned chunks"
  - "Two FastEmbedEmbedder instances: one concrete for FileIndexer<E>, one boxed for BoxEmbedder -- avoids needing Arc<T>: Embedder blanket impl"
  - "Three separate LanceVectorStore connections for vector_store/vector_memory/shared_memory -- each store takes ownership, not Arc"

patterns-established:
  - "handle_*_command dispatch pattern: top-level handler matches subcommand enum, delegates to private async functions"
  - "Bot lookup pattern: state.bot_service.get_bot_by_slug(slug).with_context() for consistent error messages"

# Metrics
duration: 7m 35s
completed: 2026-02-12
---

# Phase 3 Plan 13: Storage and KV CLI Commands + Full AppState Wiring Summary

**Storage CLI with upload/download/list/info/delete and auto-indexing, KV CLI with JSON-aware set/get/delete/list, and AppState wired with all 9 Phase 3 services**

## Performance

- **Duration:** 7m 35s
- **Started:** 2026-02-12T22:52:59Z
- **Completed:** 2026-02-12T23:00:34Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Implemented Storage CLI with 5 subcommands: upload (with auto-index), download, list (metadata table), info (version history), delete (file + chunks + index)
- Implemented KV CLI with 4 subcommands: set (JSON-aware), get (pretty-print), delete, list (with value previews)
- Wired all 9 Phase 3 services into AppState: vector_store, embedder, vector_memory, shared_memory, file_store, file_indexer, kv_store, audit_log, provider_health_store
- Embedding model name and dimension logged on startup via tracing::info
- All commands support --json flag for machine-readable output

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire all Phase 3 services into AppState** - `25f75cc` (feat, committed alongside 03-11 plan)
2. **Task 2: Implement storage and KV CLI commands** - `1fad4d3` (feat)

## Files Created/Modified

**Created:**
- `crates/boternity-api/src/cli/storage.rs` - StorageCommand with upload, download, list, info, delete handlers
- `crates/boternity-api/src/cli/kv.rs` - KvCommand with set, get, delete, list handlers

**Modified:**
- `crates/boternity-api/src/state.rs` - Added 9 Phase 3 service fields and initialization
- `crates/boternity-api/src/cli/mod.rs` - Registered storage and kv modules, added Storage and Kv command variants
- `crates/boternity-api/src/main.rs` - Added dispatch arms for Storage and Kv commands

## Decisions Made
- **JSON fallback for KV set:** When the value string is not valid JSON, it is stored as a JSON string. This provides natural UX: `bnity kv set bot1 name Alice` stores `"Alice"`, while `bnity kv set bot1 config '{"theme":"dark"}'` stores the parsed object.
- **Deindex before delete:** Storage delete deindexes from the vector store first (if file was indexed), then deletes the file. This prevents orphaned chunk vectors in LanceDB.
- **Two embedder instances:** Created one concrete `FastEmbedEmbedder` for `FileIndexer<FastEmbedEmbedder>` (generic type parameter) and one via `BoxEmbedder::new(FastEmbedEmbedder::new())` for dynamic dispatch. The model files are cached on disk so the second initialization is fast.
- **Three LanceVectorStore connections:** `LanceVectorMemoryStore::new()` and `LanceSharedMemoryStore::new()` take ownership of `LanceVectorStore`, so separate instances are created pointing to the same directory.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added Embedder trait import for model_name() access**
- **Found during:** Task 1 (AppState wiring)
- **Issue:** `FastEmbedEmbedder::model_name()` and `dimension()` are trait methods from `Embedder`, not inherent methods. Calling them without the trait in scope produces "private field, not a method" error.
- **Fix:** Added `use boternity_core::memory::embedder::Embedder` to state.rs imports
- **Files modified:** crates/boternity-api/src/state.rs
- **Verification:** `cargo check --workspace` passes, embedding model info logged on startup
- **Committed in:** 25f75cc (Task 1 commit)

**2. [Rule 3 - Blocking] Fixed run_chat_loop verbose parameter mismatch**
- **Found during:** Task 2 (compilation verification)
- **Issue:** Concurrent plan (03-12) added `verbose: bool` parameter to `run_chat_loop()` but the dispatch in main.rs still called with 3 arguments
- **Fix:** Updated call to pass `false` as default verbose value
- **Files modified:** crates/boternity-api/src/main.rs
- **Verification:** `cargo check --workspace` compiles without errors
- **Committed in:** 7a327f1 (committed by concurrent 03-12 plan)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes are trait import and function signature mismatches. No scope creep.

## Issues Encountered
- Concurrent plan executions (03-11, 03-12) modified the same files (state.rs, mod.rs, main.rs) during this plan's execution. Task 1 state.rs changes were captured by the 03-11 commit, and mod.rs/main.rs dispatch changes were captured by the 03-12 commit. Only the new files (storage.rs, kv.rs) needed explicit committing as Task 2.

## User Setup Required
None - no external service configuration required. All services use local infrastructure (SQLite, LanceDB, fastembed).

## Next Phase Readiness
- Phase 3 is now complete: all 13 plans executed
- All services wired into AppState and accessible from CLI and REST API
- File storage with auto-indexing operational via `bnity storage` commands
- KV store for structured bot data operational via `bnity kv` commands
- Ready for Phase 4 (Web UI Core + Fleet Dashboard)

## Self-Check: PASSED

---
*Phase: 03-multi-provider-memory*
*Completed: 2026-02-12*
