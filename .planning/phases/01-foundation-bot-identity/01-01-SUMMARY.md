---
phase: 01-foundation-bot-identity
plan: 01
subsystem: infra
tags: [rust, cargo-workspace, turborepo, domain-types, repository-traits, clean-architecture]

# Dependency graph
requires: []
provides:
  - "Cargo workspace with four crates (types, core, infra, api)"
  - "Domain types: Bot, BotId, BotStatus, BotCategory, Soul, SoulId, SoulVersion, Identity, SecretKey, SecretEntry, Redacted"
  - "Repository traits: BotRepository, SoulRepository, SecretProvider (native async fn)"
  - "Error types: BotError, SoulError, SecretError, RepositoryError"
  - "Slug generation utility (slugify)"
  - "Turborepo config for future JS/TS packages"
affects:
  - "01-02 (SQLite storage implements repository traits)"
  - "01-03 (BotService and SoulService use domain types)"
  - "01-04 (SecretService uses SecretProvider trait)"
  - "01-05 (CLI and REST API use all types and traits)"
  - "01-06 (Soul versioning extends Soul types)"
  - "All future phases (everything builds on this crate structure)"

# Tech tracking
tech-stack:
  added: [axum 0.8, sqlx 0.8, clap 4.5, tokio 1, serde 1, uuid 1.20, chrono 0.4, tracing 0.1, tower-http 0.6, keyring 3.6, aes-gcm 0.10, argon2 0.5, sha2 0.10, thiserror 1, anyhow 1]
  patterns:
    - "Workspace dependency sharing via [workspace.dependencies] + { workspace = true }"
    - "Clean architecture: core depends only on types, never on infra"
    - "Native async fn in traits (Rust 2024 edition, no async_trait macro)"
    - "UUID v7 for time-sortable identifiers"
    - "Newtype pattern for domain IDs (BotId, SoulId)"
    - "Redacted wrapper for secret values (custom Debug/Display)"

key-files:
  created:
    - "Cargo.toml"
    - "rust-toolchain.toml"
    - "turbo.json"
    - "package.json"
    - "pnpm-workspace.yaml"
    - ".gitignore"
    - "crates/boternity-types/Cargo.toml"
    - "crates/boternity-types/src/lib.rs"
    - "crates/boternity-types/src/bot.rs"
    - "crates/boternity-types/src/soul.rs"
    - "crates/boternity-types/src/identity.rs"
    - "crates/boternity-types/src/secret.rs"
    - "crates/boternity-types/src/error.rs"
    - "crates/boternity-core/Cargo.toml"
    - "crates/boternity-core/src/lib.rs"
    - "crates/boternity-core/src/repository/mod.rs"
    - "crates/boternity-core/src/repository/bot.rs"
    - "crates/boternity-core/src/repository/soul.rs"
    - "crates/boternity-core/src/repository/secret.rs"
    - "crates/boternity-core/src/service/mod.rs"
    - "crates/boternity-infra/Cargo.toml"
    - "crates/boternity-infra/src/lib.rs"
    - "crates/boternity-api/Cargo.toml"
    - "crates/boternity-api/src/main.rs"
  modified: []

key-decisions:
  - "Rust 2024 edition with resolver 3 and native async fn in traits"
  - "UUID v7 for all entity IDs (time-sortable, process-local ordering)"
  - "Newtype pattern for domain IDs with Display/FromStr/Serialize"
  - "BotStatus: Active/Disabled/Archived (not Created/Running/Stopped from research examples)"
  - "BotCategory: Assistant/Creative/Research/Utility (system categories)"
  - "Identity defaults: claude-sonnet-4-20250514, temperature 0.7, max_tokens 4096"
  - "Redacted<T> wrapper with custom Debug/Display for secret values"
  - "Repository traits return impl Future (RPITIT) not Box<dyn Future>"

patterns-established:
  - "Workspace deps: all shared deps in root [workspace.dependencies], consumed via { workspace = true }"
  - "Crate hierarchy: types -> core -> infra -> api (strict, one-directional)"
  - "Domain types: derive Debug, Clone, Serialize, Deserialize; never sqlx::FromRow"
  - "Repository traits: Send + Sync bound, async fn via RPITIT, Result<_, RepositoryError>"
  - "Error types: per-domain (BotError, SoulError) + cross-cutting (RepositoryError)"
  - "Testing: unit tests co-located in each module, doctests for public API examples"

# Metrics
duration: 4min 30s
completed: 2026-02-10
---

# Phase 1 Plan 1: Monorepo Scaffold + Domain Types Summary

**Cargo workspace with 4 crates (types/core/infra/api), domain types for Bot/Soul/Identity/Secret, and repository traits using native async fn in Rust 2024 edition**

## Performance

- **Duration:** 4 min 30s
- **Started:** 2026-02-10T21:16:09Z
- **Completed:** 2026-02-10T21:20:39Z
- **Tasks:** 2/2
- **Files created:** 25

## Accomplishments

- Four-crate Cargo workspace compiling cleanly with strict dependency hierarchy (core never depends on infra)
- Complete domain type definitions: Bot with slug generation, Soul with SHA-256 hash field, Identity with LLM defaults, Secret with Redacted wrapper
- Repository trait definitions using native async fn (Rust 2024 RPITIT, no async_trait macro)
- Turborepo config coexisting with Cargo workspace for future JS/TS frontend packages
- 22 tests passing (unit tests for slugify, serde roundtrips, Display/FromStr, Redacted masking; doctest for slugify)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create monorepo scaffold with Cargo workspace and Turborepo config** - `eb3d186` (feat)
2. **Task 2: Define domain types and repository traits** - `492e457` (feat)

## Files Created/Modified

- `Cargo.toml` - Workspace root with resolver 3, edition 2024, all shared dependencies
- `Cargo.lock` - Generated lockfile (361 packages)
- `rust-toolchain.toml` - Pinned to stable channel
- `turbo.json` - Turborepo config for JS/TS packages (build, lint, test, dev tasks)
- `package.json` - Root package with Rust build convenience scripts
- `pnpm-workspace.yaml` - Workspace config for future apps/ and packages/ directories
- `.gitignore` - Rust, Node.js, SQLite, IDE, Turborepo exclusions
- `crates/boternity-types/Cargo.toml` - Domain types crate (serde, uuid, chrono, thiserror)
- `crates/boternity-types/src/lib.rs` - Module exports for bot, soul, identity, secret, error
- `crates/boternity-types/src/bot.rs` - BotId, Bot, BotStatus, BotCategory, CreateBotRequest, slugify()
- `crates/boternity-types/src/soul.rs` - SoulId, Soul, SoulFrontmatter, SoulVersion
- `crates/boternity-types/src/identity.rs` - Identity with LLM config defaults
- `crates/boternity-types/src/secret.rs` - SecretKey, SecretEntry, SecretProvider, SecretScope, Redacted
- `crates/boternity-types/src/error.rs` - BotError, SoulError, SecretError, RepositoryError
- `crates/boternity-core/Cargo.toml` - Business logic crate (types + thiserror + tracing only)
- `crates/boternity-core/src/lib.rs` - Module exports for repository and service
- `crates/boternity-core/src/repository/mod.rs` - SortOrder enum, module declarations
- `crates/boternity-core/src/repository/bot.rs` - BotRepository trait, BotFilter struct
- `crates/boternity-core/src/repository/soul.rs` - SoulRepository trait
- `crates/boternity-core/src/repository/secret.rs` - SecretProvider trait
- `crates/boternity-core/src/service/mod.rs` - Empty module declarations for future services
- `crates/boternity-infra/Cargo.toml` - Infrastructure crate (types + core + sqlx + crypto)
- `crates/boternity-infra/src/lib.rs` - Doc comment placeholder
- `crates/boternity-api/Cargo.toml` - Application crate (types + core + infra + axum + clap)
- `crates/boternity-api/src/main.rs` - Minimal placeholder main

## Decisions Made

- **BotStatus variants (Active/Disabled/Archived):** Aligned with CONTEXT.md lifecycle states rather than research example's Created/Running/Stopped which implied runtime process states
- **RPITIT over Box<dyn Future>:** Used native `impl Future<Output = ...> + Send` return type in traits for zero-allocation async, enabled by Rust 2024 edition
- **UUID v7 over UUID v4:** Time-sortable IDs provide natural ordering and better index performance in SQLite
- **Redacted wrapper pattern:** Custom Debug/Display that never exposes secret values, with `expose()` for explicit access and `masked()` for safe display
- **SortOrder in repository/mod.rs:** Shared enum rather than per-repository to avoid duplication across BotFilter, future SoulFilter, etc.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Crate structure ready for Plan 01-02 (SQLite storage layer implementing repository traits)
- Domain types ready for Plan 01-03 (BotService, SoulService using Bot/Soul types)
- Secret types ready for Plan 01-04 (vault encryption, keychain integration)
- All shared dependencies defined in workspace root for consistent versioning

## Self-Check: PASSED

---
*Phase: 01-foundation-bot-identity*
*Completed: 2026-02-10*
