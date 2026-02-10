---
phase: 01-foundation-bot-identity
plan: 02
subsystem: infra
tags: [rust, sqlite, sqlx, wal-mode, repository-pattern, split-pools, migrations]

# Dependency graph
requires:
  - "01-01 (domain types, repository traits, workspace structure)"
provides:
  - "SQLite database with WAL mode and split read/write pools"
  - "SqliteBotRepository implementing BotRepository trait (full CRUD + filters)"
  - "SqliteSoulRepository implementing SoulRepository trait (versioned storage)"
  - "SqliteSecretRepository implementing SecretProvider trait (encrypted BLOB)"
  - "Database migration: bots, soul_versions, secrets, api_keys tables"
  - "default_database_url() helper using BOTERNITY_DATA_DIR env var"
affects:
  - "01-03 (BotService and SoulService will use these repositories)"
  - "01-04 (SecretService vault encryption feeds into SqliteSecretRepository)"
  - "01-05 (CLI and REST API wire repositories via DatabasePool)"
  - "01-06 (Soul versioning and integrity checks use SoulRepository)"

# Tech tracking
tech-stack:
  added: [tempfile 3]
  patterns:
    - "Split read/write SQLite pools (8 readers, 1 writer)"
    - "WAL journal mode on ALL connections (reader and writer)"
    - "Foreign key enforcement via SqliteConnectOptions::foreign_keys(true)"
    - "sqlx::migrate!() for auto-migration on pool creation"
    - "Private row structs for SQLite-to-domain type mapping (no sqlx::FromRow on domain types)"
    - "Hex encoding for encrypted BLOB round-trip through SecretProvider trait"
    - "UPSERT via ON CONFLICT for secret set operations"
    - "Transaction for soul save (INSERT + UPDATE bots.version_count)"

key-files:
  created:
    - "migrations/20260210_001_initial.sql"
    - "crates/boternity-infra/src/sqlite/mod.rs"
    - "crates/boternity-infra/src/sqlite/pool.rs"
    - "crates/boternity-infra/src/sqlite/bot.rs"
    - "crates/boternity-infra/src/sqlite/soul.rs"
    - "crates/boternity-infra/src/sqlite/secret.rs"
  modified:
    - "crates/boternity-infra/Cargo.toml"
    - "crates/boternity-infra/src/lib.rs"
    - "Cargo.lock"

key-decisions:
  - "Split reader/writer pools: 8 concurrent readers, 1 serialized writer (SQLite allows only one writer)"
  - "WAL mode on both pools to prevent journal mode reset on reconnection"
  - "Private BotRow struct for SQLite mapping (domain types stay sqlx-free)"
  - "Hex encoding for encrypted secret BLOB transport through string-based SecretProvider trait"
  - "Secrets scope stored as string (not FK to bots) -- allows pre-provisioned keys for bots that don't exist yet"
  - "SQL injection prevention: sort field whitelist in list() queries"
  - "Transaction for soul save: INSERT soul_version + UPDATE bots.version_count atomically"

patterns-established:
  - "DatabasePool::new() auto-runs migrations -- no separate migration step needed"
  - "Repository methods use writer pool for mutations, reader pool for queries"
  - "Error mapping: sqlx UNIQUE constraint -> RepositoryError::Conflict"
  - "Error mapping: zero rows affected on UPDATE/DELETE -> RepositoryError::NotFound"
  - "Integration tests use tempfile::tempdir() with std::mem::forget() for pool lifetime"

# Metrics
duration: 5min 58s
completed: 2026-02-10
---

# Phase 1 Plan 2: SQLite Storage Layer Summary

**SQLite storage with WAL-mode split read/write pools, auto-migrations, and repository implementations for bots (full CRUD + filtering/sorting/pagination), soul versions (transactional save with version_count update), and secrets (encrypted BLOB with upsert)**

## Performance

- **Duration:** 5 min 58s
- **Started:** 2026-02-10T21:24:26Z
- **Completed:** 2026-02-10T21:30:24Z
- **Tasks:** 2/2
- **Files created:** 6
- **Files modified:** 3
- **Tests:** 23 passing

## Accomplishments

- Initial SQLite migration creating 4 tables (bots, soul_versions, secrets, api_keys) with indexes and constraints
- DatabasePool with split reader (8 connections) / writer (1 connection) pools, both in WAL journal mode
- SqliteBotRepository: full CRUD with filtering by status/category, sorting by whitelisted fields, offset pagination
- SqliteSoulRepository: versioned soul storage with transactional version_count increment on parent bot
- SqliteSecretRepository: encrypted BLOB storage with upsert semantics, global and bot-scoped access
- Foreign key cascade verified: deleting a bot cascades to soul_versions
- No sqlx types in boternity-types or boternity-core (architectural constraint maintained)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create SQLite migration and database pool with WAL mode** - `6bff683` (feat)
2. **Task 2: Implement repository trait adapters for SQLite** - `3a90f22` (feat)

## Files Created/Modified

- `migrations/20260210_001_initial.sql` - Schema: bots, soul_versions, secrets, api_keys with indexes
- `crates/boternity-infra/src/sqlite/mod.rs` - Module declarations for pool, bot, soul, secret
- `crates/boternity-infra/src/sqlite/pool.rs` - DatabasePool with split read/write, WAL mode, auto-migrations
- `crates/boternity-infra/src/sqlite/bot.rs` - SqliteBotRepository with full CRUD, filtering, sorting, pagination
- `crates/boternity-infra/src/sqlite/soul.rs` - SqliteSoulRepository with versioned storage, transactional save
- `crates/boternity-infra/src/sqlite/secret.rs` - SqliteSecretRepository with encrypted BLOB, upsert, scoped access
- `crates/boternity-infra/Cargo.toml` - Added serde_json, uuid, serde, tempfile dev-dependency
- `crates/boternity-infra/src/lib.rs` - Added `pub mod sqlite` declaration
- `Cargo.lock` - Updated with new dependencies

## Decisions Made

- **Split pool sizing (8 readers, 1 writer):** 8 reader connections balances concurrency with SQLite file-level locking. Single writer is mandatory for SQLite correctness.
- **WAL mode on both pools:** Prevents journal mode reset on pool reconnection (documented pitfall from research).
- **Private BotRow struct:** Maps SQLite rows to domain types without putting sqlx derives on domain types. Maintains clean architecture boundary.
- **Hex encoding for secrets:** SecretProvider trait uses String values. Encrypted bytes are hex-encoded for transport, stored as BLOB. The vault service (Plan 01-04) will handle actual AES-256-GCM encryption/decryption.
- **Secrets scope as string, not FK:** Bot-scoped secrets use the bot UUID as a string scope value. No foreign key to bots table. This allows pre-provisioning secrets for bots that don't exist yet and avoids cascade delete of secrets (application layer handles cleanup).
- **Sort field whitelist:** The list() method only allows known column names for ORDER BY to prevent SQL injection through the sort_by parameter.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Repositories ready for Plan 01-03 (BotService, SoulService wrapping repository calls)
- SecretProvider ready for Plan 01-04 (vault encryption layer will call set/get with encrypted data)
- DatabasePool ready for Plan 01-05 (CLI and REST API wire repositories via pool)
- Soul versioning infrastructure ready for Plan 01-06 (integrity checks, rollback, diff)

## Self-Check: PASSED

---
*Phase: 01-foundation-bot-identity*
*Completed: 2026-02-10*
