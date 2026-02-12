---
phase: 03-multi-provider-memory
plan: 05
subsystem: database, infra
tags: [sqlite, kv-store, audit-log, circuit-breaker, file-metadata, migrations, sqlx]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: SQLite pool infrastructure (DatabasePool, split read/write), bots table, RepositoryError
  - phase: 03-01
    provides: Domain types (KvEntry, MemoryAuditEntry, AuditAction, StorageFile, FileVersion, ProviderHealth/CircuitState)
  - phase: 03-01
    provides: KvStore trait in boternity-core/src/storage/kv_store.rs
provides:
  - SQLite migration for Phase 3 tables (bot_kv_store, memory_audit_log, provider_health, bot_files, bot_file_versions)
  - SqliteKvStore implementing KvStore trait with JSON value support
  - SqliteAuditLog for memory audit trail (log, get_for_bot, get_for_memory)
  - SqliteProviderHealthStore for persistent circuit breaker state (save, load, load_all)
  - SqliteFileMetadataStore for file CRUD with versioning (save_file, update_file, get_file, list_files, delete_file, save_version, get_versions)
affects: [03-06, 03-07, 03-08, 03-09, 03-10, 03-11, 03-12, 03-13]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ON CONFLICT upsert pattern for KV store and provider health (INSERT ... ON CONFLICT DO UPDATE)"
    - "ProviderHealthRow as persistence-only struct separate from runtime ProviderHealth (Instant not serializable)"
    - "Private Row structs with from_row/into_domain pattern for all SQLite repositories"
    - "Cascade delete on bot_file_versions via FK to bot_files"

key-files:
  created:
    - migrations/20260212_003_phase3_storage.sql
    - crates/boternity-infra/src/sqlite/kv.rs
    - crates/boternity-infra/src/sqlite/audit.rs
    - crates/boternity-infra/src/sqlite/provider_health.rs
    - crates/boternity-infra/src/sqlite/file_metadata.rs
  modified:
    - crates/boternity-infra/src/sqlite/mod.rs

key-decisions:
  - "bot_kv_store uses composite PK (bot_id, key) instead of surrogate ID -- natural key is sufficient and enforces uniqueness"
  - "memory_audit_log.memory_id is TEXT not FK -- memory may be deleted while audit trail persists"
  - "ProviderHealthRow is a separate persistence struct from core ProviderHealth -- Instant (monotonic clock) cannot be serialized"
  - "provider_health table keyed by provider name (not UUIDv7) -- names are unique across providers"
  - "bot_files uses UNIQUE(bot_id, filename) constraint -- same filename per bot is upserted, not duplicated"

patterns-established:
  - "Persistence Row structs for types with non-serializable fields (Instant, Duration)"
  - "Full UUID in test bot slugs to prevent collision in parallel test execution"

# Metrics
duration: 12m 45s
completed: 2026-02-12
---

# Phase 3 Plan 05: SQLite Phase 3 Migrations and Repositories Summary

**Five new SQLite tables (KV store, audit log, provider health, file metadata, file versions) with four repository implementations and 65 unit tests**

## Performance

- **Duration:** 12m 45s
- **Started:** 2026-02-12T22:04:12Z
- **Completed:** 2026-02-12T22:16:57Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Created SQLite migration with 5 new tables and 6 indexes for Phase 3 structured data
- Implemented SqliteKvStore with full KvStore trait compliance (get/set/delete/list_keys/get_entry with JSON upsert)
- Built SqliteAuditLog for memory operation auditing with bot-scoped and memory-scoped queries
- Built SqliteProviderHealthStore to persist circuit breaker state across application restarts
- Built SqliteFileMetadataStore with file CRUD, version tracking, and cascade delete verification
- Added 65 new unit tests across all four repositories (total infra tests: 193)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create SQLite migrations for Phase 3 tables** - `f0c359c` (feat)
2. **Task 2: Implement KV store, audit log, provider health, and file metadata repositories** - `e9e8019` (feat)

## Files Created/Modified

**Created:**
- `migrations/20260212_003_phase3_storage.sql` - 5 tables: bot_kv_store, memory_audit_log, provider_health, bot_files, bot_file_versions
- `crates/boternity-infra/src/sqlite/kv.rs` - SqliteKvStore implementing KvStore trait with JSON value support
- `crates/boternity-infra/src/sqlite/audit.rs` - SqliteAuditLog for memory add/delete/share/revoke/merge audit trail
- `crates/boternity-infra/src/sqlite/provider_health.rs` - SqliteProviderHealthStore for persistent circuit breaker state
- `crates/boternity-infra/src/sqlite/file_metadata.rs` - SqliteFileMetadataStore for file CRUD with versioning

**Modified:**
- `crates/boternity-infra/src/sqlite/mod.rs` - Added audit, file_metadata, kv, provider_health modules

## Decisions Made
- **Composite PK for KV store:** `(bot_id, key)` as natural primary key instead of surrogate UUID -- simpler, enforces uniqueness at the schema level
- **Audit log memory_id as TEXT:** Not a foreign key because memories may be deleted while audit trail must persist
- **ProviderHealthRow separation:** Runtime `ProviderHealth` uses `Instant` (monotonic, non-serializable); `ProviderHealthRow` stores only the serializable subset for persistence
- **Provider name as PK:** Provider health keyed by `name` TEXT, not UUIDv7 -- names are unique identifiers from ProviderConfig
- **UNIQUE(bot_id, filename) on bot_files:** Prevents duplicate filenames per bot, enables upsert on re-upload

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed slug collision in bot isolation tests**
- **Found during:** Task 2 (KV store and file metadata tests)
- **Issue:** `setup_bot()` used `&bot_id.to_string()[..8]` for slug -- UUIDv7's time-based prefix causes collisions when two bots created within the same millisecond
- **Fix:** Changed to `format!("bot-{}", bot_id)` using full UUID for test slug uniqueness
- **Files modified:** kv.rs, audit.rs, file_metadata.rs (test helpers)
- **Verification:** `test_bot_isolation` tests pass in both kv and file_metadata modules
- **Committed in:** e9e8019 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Trivial test helper fix. No scope creep.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All Phase 3 SQLite tables and repositories are in place
- Ready for Plan 03-06 (provider implementations) and all subsequent plans that need structured storage
- SqliteKvStore ready for bot-scoped settings and state persistence
- SqliteProviderHealthStore ready for FallbackChain integration (Plan 03-03)
- SqliteFileMetadataStore ready for FileStore implementation (Plan 03-10)

## Self-Check: PASSED

---
*Phase: 03-multi-provider-memory*
*Completed: 2026-02-12*
