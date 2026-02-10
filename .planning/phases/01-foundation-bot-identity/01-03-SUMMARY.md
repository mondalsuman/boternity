---
phase: 01-foundation-bot-identity
plan: 03
subsystem: core
tags: [rust, service-layer, filesystem, sha256, clean-architecture, identity-files, soul]

# Dependency graph
requires:
  - "01-01 (domain types, repository traits, workspace structure)"
  - "01-02 (SQLite repository implementations for bot and soul)"
provides:
  - "BotService orchestrating full bot creation with SOUL.md, IDENTITY.md, USER.md on disk"
  - "SoulService for soul content generation, hashing, and integrity verification"
  - "FileSystem trait in core, LocalFileSystem adapter in infra"
  - "ContentHasher trait in core, Sha256ContentHasher adapter in infra"
  - "SOUL.md frontmatter parser (YAML between --- delimiters)"
  - "IDENTITY.md frontmatter parser with config extraction"
  - "USER.md read/write helpers with customization detection"
  - "resolve_data_dir() with BOTERNITY_DATA_DIR env var support"
  - "UpdateBotRequest type in boternity-types"
affects:
  - "01-05 (CLI and REST API wire BotService and SoulService)"
  - "01-06 (Soul versioning and integrity checks use SoulService)"
  - "All future phases requiring bot creation or identity file I/O"

# Tech tracking
tech-stack:
  added: [dirs 6]
  patterns:
    - "Generic services over trait bounds (BotService<B, S, F, H>) instead of Arc<dyn Trait> for zero-cost abstraction"
    - "Free functions for stateless operations (generate_default_soul, generate_default_identity, generate_default_user)"
    - "FileSystem trait in core, LocalFileSystem in infra (dependency inversion for file I/O)"
    - "ContentHasher trait in core, Sha256ContentHasher in infra (dependency inversion for hashing)"
    - "YAML frontmatter parsing without external YAML library (simple line-based parser)"

key-files:
  created:
    - "crates/boternity-core/src/service/bot.rs"
    - "crates/boternity-core/src/service/soul.rs"
    - "crates/boternity-core/src/service/fs.rs"
    - "crates/boternity-core/src/service/hash.rs"
    - "crates/boternity-infra/src/filesystem/mod.rs"
    - "crates/boternity-infra/src/filesystem/soul.rs"
    - "crates/boternity-infra/src/filesystem/identity.rs"
    - "crates/boternity-infra/src/filesystem/user.rs"
    - "crates/boternity-infra/src/crypto/hash.rs"
  modified:
    - "crates/boternity-core/src/service/mod.rs"
    - "crates/boternity-core/Cargo.toml"
    - "crates/boternity-types/src/bot.rs"
    - "crates/boternity-types/src/error.rs"
    - "crates/boternity-infra/src/lib.rs"
    - "crates/boternity-infra/src/crypto/mod.rs"
    - "crates/boternity-infra/Cargo.toml"
    - "Cargo.toml"

key-decisions:
  - "Generic services (BotService<B, S, F, H>) over trait objects: RPITIT traits are not object-safe, generics give zero-cost abstraction"
  - "Free functions for content generation: generate_default_soul() etc. are stateless, avoids needing trait bounds just to call them"
  - "Simple line-based YAML parser for frontmatter: avoids adding serde_yaml dependency for a narrow use case"
  - "LocalFileSystem creates parent directories automatically on write: prevents errors from missing intermediate dirs"

patterns-established:
  - "Service generic pattern: pub struct FooService<R: RepoTrait, F: FileSystem, H: ContentHasher>"
  - "Free function content generators: soul::generate_default_soul(name) callable without service instance"
  - "Path helpers as static methods on LocalFileSystem: bot_dir, soul_path, identity_path, user_path"
  - "Data dir resolution: BOTERNITY_DATA_DIR env var > dirs::home_dir()/.boternity > .boternity"

# Metrics
duration: 9min 25s
completed: 2026-02-10
---

# Phase 1 Plan 3: Bot Identity System Summary

**BotService with full bot creation lifecycle (slug deduplication, DB save, SOUL.md/IDENTITY.md/USER.md on disk with SHA-256 integrity), SoulService with human-like default personality generation, FileSystem/ContentHasher trait abstractions maintaining zero infra deps in core**

## Performance

- **Duration:** 9 min 25s
- **Started:** 2026-02-10T21:33:44Z
- **Completed:** 2026-02-10T21:43:09Z
- **Tasks:** 2/2
- **Files created:** 9
- **Files modified:** 8
- **Tests:** 74 passing (8 core + 66 infra)

## Accomplishments

- BotService orchestrates complete bot creation: slug generation with uniqueness enforcement (-2, -3 suffixes), database save, directory creation, and three identity files on disk
- SoulService generates human-like default SOUL.md (curious, warm, direct personality -- never generic assistant), IDENTITY.md with LLM defaults, USER.md as curated briefing template
- Clean architecture preserved: FileSystem and ContentHasher traits in core, LocalFileSystem and Sha256ContentHasher in infra -- boternity-core has zero infra/sqlx dependencies
- SOUL.md frontmatter parser handles both YAML list and inline array syntax for traits
- SHA-256 hashing produces consistent lowercase hex digests, verified with known test vectors

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement BotService and SoulService in boternity-core** - `2db3128` (feat)
2. **Task 2: Implement filesystem and hashing adapters in boternity-infra** - `f24549c` (feat)

## Files Created/Modified

- `crates/boternity-core/src/service/bot.rs` - BotService with create/get/update/delete/clone/list operations
- `crates/boternity-core/src/service/soul.rs` - SoulService + free functions for generating default SOUL.md/IDENTITY.md/USER.md
- `crates/boternity-core/src/service/fs.rs` - FileSystem trait (write_file, read_file, create_dir_all, exists, remove_dir_all)
- `crates/boternity-core/src/service/hash.rs` - ContentHasher trait (compute_hash)
- `crates/boternity-core/src/service/mod.rs` - Module declarations for bot, soul, fs, hash
- `crates/boternity-infra/src/filesystem/mod.rs` - LocalFileSystem impl + resolve_data_dir() + path helpers
- `crates/boternity-infra/src/filesystem/soul.rs` - SOUL.md frontmatter parsing and composition
- `crates/boternity-infra/src/filesystem/identity.rs` - IDENTITY.md frontmatter parsing
- `crates/boternity-infra/src/filesystem/user.rs` - USER.md read/write and customization detection
- `crates/boternity-infra/src/crypto/hash.rs` - Sha256ContentHasher implementation
- `crates/boternity-types/src/bot.rs` - Added UpdateBotRequest
- `crates/boternity-types/src/error.rs` - Added FileSystemError and InvalidContent variants

## Decisions Made

- **Generic services over Arc<dyn Trait>:** The RPITIT-based repository traits (using `impl Future`) are not object-safe, so `Arc<dyn BotRepository>` is impossible. Used generics (`BotService<B: BotRepository, S: SoulRepository, F: FileSystem, H: ContentHasher>`) which gives zero-cost static dispatch.
- **Free functions for content generation:** `generate_default_soul()`, `generate_default_identity()`, `generate_default_user()` are stateless free functions in the `soul` module. This avoids requiring trait bounds just to call static generation methods, making tests simpler.
- **No serde_yaml dependency:** Frontmatter parsing uses a simple line-based parser rather than adding serde_yaml. The YAML subset we parse is narrow (name, traits list, tone, and identity config fields).
- **Auto-create parent directories on write:** `LocalFileSystem::write_file` creates parent dirs automatically. `SoulService::write_and_save_soul` also creates the bot directory. This prevents errors from missing intermediate directories.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added chrono dependency to boternity-core**
- **Found during:** Task 1 (SoulService implementation)
- **Issue:** SoulService needs `chrono::Utc::now()` for soul version timestamps, but boternity-core only had types+thiserror+tracing
- **Fix:** Added `chrono = { workspace = true }` to boternity-core/Cargo.toml
- **Files modified:** crates/boternity-core/Cargo.toml
- **Verification:** cargo build passes, no infra deps introduced
- **Committed in:** 2db3128

**2. [Rule 3 - Blocking] Added dirs crate for home directory resolution**
- **Found during:** Task 2 (resolve_data_dir implementation)
- **Issue:** Plan specified "Support BOTERNITY_DATA_DIR env var with dirs crate" but dirs was not in workspace deps
- **Fix:** Added `dirs = "6"` to workspace root and boternity-infra Cargo.toml
- **Files modified:** Cargo.toml, crates/boternity-infra/Cargo.toml
- **Verification:** cargo build passes, resolve_data_dir test passes
- **Committed in:** f24549c

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both were missing dependencies required by the plan. No scope creep.

## Issues Encountered

- **Rust 2024 raw string delimiter conflict:** The default IDENTITY.md template contains `"#6366f1"` which conflicts with `r#"..."#` delimiters. Fixed by using `r##"..."##` for the identity template.
- **Rust 2024 unsafe env var mutation:** `std::env::set_var` and `std::env::remove_var` are unsafe in edition 2024. Wrapped test calls in `unsafe {}` blocks with safety comments.
- **Parallel plan file contention:** Plan 01-04 was executing concurrently and intermittently overwrote shared files (lib.rs, crypto/mod.rs). Re-applied changes when detected.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- BotService and SoulService ready for Plan 01-05 (CLI and REST API wiring)
- FileSystem and ContentHasher traits ready to be instantiated by the application layer
- Soul integrity verification ready for Plan 01-06 (startup hash check, hard block on mismatch)
- Default SOUL.md template ready for CLI interactive wizard customization
- Identity file parsing ready for `bnity show <bot>` command output

## Self-Check: PASSED

---
*Phase: 01-foundation-bot-identity*
*Completed: 2026-02-10*
