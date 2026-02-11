---
phase: 01-foundation-bot-identity
plan: 06
subsystem: core, api
tags: [soul-versioning, sha256, immutability, rollback, integrity, diff, cli, rest-api]

# Dependency graph
requires:
  - phase: 01-02
    provides: "SQLite soul_versions table with message column, SoulRepository trait and implementation"
  - phase: 01-03
    provides: "SoulService with write_and_save_soul, ContentHasher trait, FileSystem trait"
provides:
  - "Soul versioning: every edit creates a new version with SHA-256 hash"
  - "Rollback: creates new version with old content (linear history preserved)"
  - "Integrity verification: SoulIntegrityResult with hash comparison"
  - "Simple line diff: LCS-based diffing with no external dependency"
  - "BotService.ensure_soul_integrity() startup guard"
  - "CLI: bnity soul {edit, history, diff, rollback, verify}"
  - "REST: PUT soul, GET version, POST rollback, GET verify endpoints"
  - "SoulIntegrityResult type in boternity-types"
  - "SoulIntegrityViolation error variant in BotError"
affects: ["02-chat", "03-memory", "10-security"]

# Tech tracking
tech-stack:
  added: [tempfile]
  patterns:
    - "LCS line diff without external dependency"
    - "Immutability invariant: only update_soul() and write_and_save_soul() write SOUL.md"
    - "Soul message field for version commit messages"

key-files:
  created:
    - "crates/boternity-api/src/cli/soul.rs"
  modified:
    - "crates/boternity-core/src/service/soul.rs"
    - "crates/boternity-core/src/service/bot.rs"
    - "crates/boternity-types/src/soul.rs"
    - "crates/boternity-types/src/error.rs"
    - "crates/boternity-infra/src/sqlite/soul.rs"
    - "crates/boternity-api/src/http/handlers/soul.rs"
    - "crates/boternity-api/src/http/router.rs"
    - "crates/boternity-api/src/cli/mod.rs"
    - "crates/boternity-api/src/main.rs"
    - "crates/boternity-api/src/http/error.rs"

key-decisions:
  - "LCS-based line diff in pure Rust instead of adding a diff library dependency"
  - "Added message field to Soul struct (not just SoulVersion) for end-to-end commit message tracking"
  - "update_soul saves to DB first then writes file (DB failure leaves disk unchanged)"
  - "bnity check enhanced with soul integrity verification (not just file existence)"

patterns-established:
  - "Immutability invariant: SOUL.md writes only through update_soul() or write_and_save_soul(), both create version entries"
  - "resolve_bot() helper in REST handlers to deduplicate ID/slug resolution"

# Metrics
duration: 9m 28s
completed: 2026-02-11
---

# Phase 1 Plan 6: Soul Versioning + Immutability Enforcement Summary

**Soul versioning with SHA-256 integrity verification, LCS line diff, rollback, and immutability enforcement via CLI and REST API**

## Performance

- **Duration:** 9m 28s
- **Started:** 2026-02-11T22:11:20Z
- **Completed:** 2026-02-11T22:20:48Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments

- Every SOUL.md edit now creates a new version with SHA-256 hash and optional commit message
- Rollback creates a NEW version with old content (linear history, never rewrites)
- Integrity verification returns SoulIntegrityResult with expected/actual hash comparison
- ensure_soul_integrity() in BotService is a hard block with clear error on mismatch
- CLI provides: `bnity soul {edit, history, diff, rollback, verify}` commands
- REST API provides: PUT soul (update), GET version, POST rollback, GET verify endpoints
- Simple LCS-based line diff with no external dependency
- 13 new unit tests covering versioning, rollback, integrity, and diffing (129 total)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement soul versioning and immutability in boternity-core** - `3ff2829` (feat)
2. **Task 2: Add soul CLI commands and update REST API handlers** - `5381e89` (feat)

## Files Created/Modified

- `crates/boternity-types/src/soul.rs` - Added SoulIntegrityResult struct, message field to Soul
- `crates/boternity-types/src/error.rs` - Added SoulIntegrityViolation to BotError
- `crates/boternity-core/src/service/soul.rs` - Added update_soul, rollback_soul, verify_soul_integrity, get_soul_diff, get_soul_version; 13 new tests
- `crates/boternity-core/src/service/bot.rs` - Added ensure_soul_integrity, data_dir/soul_service accessors
- `crates/boternity-infra/src/sqlite/soul.rs` - Updated save_version to persist message column, updated row_to_soul to read message
- `crates/boternity-api/src/cli/soul.rs` - New: edit, history, diff, rollback, verify CLI commands
- `crates/boternity-api/src/cli/mod.rs` - Added SoulCommand enum and soul module
- `crates/boternity-api/src/main.rs` - Wired Soul command dispatch, enhanced Check with integrity
- `crates/boternity-api/src/http/handlers/soul.rs` - Added update_soul, get_soul_version, rollback_soul, verify_soul REST handlers
- `crates/boternity-api/src/http/router.rs` - Added routes for PUT soul, GET version, POST rollback, GET verify
- `crates/boternity-api/src/http/error.rs` - Added SoulIntegrityViolation error mapping

## Decisions Made

- **LCS line diff in pure Rust:** Implemented a simple longest-common-subsequence based line diff rather than adding a diff library. The O(mn) LCS table is sufficient for soul files which are small (typically <200 lines).
- **Message field on Soul struct:** Added `message: Option<String>` to the `Soul` struct itself (not just `SoulVersion`) so commit messages flow through the entire save path from service to repository to database.
- **DB-first write order:** `update_soul()` saves to the database first, then writes the file. If the DB save fails, the disk is unchanged. This prevents orphaned file writes without version entries.
- **Enhanced bnity check:** The existing `bnity check <slug>` command was enhanced to include soul integrity verification alongside file existence checks, since the plan suggested integration.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added tempfile dependency for editor temp file**
- **Found during:** Task 2 (CLI edit command)
- **Issue:** `bnity soul edit` needs a temp file for the editor to modify (cannot edit SOUL.md in place as that would bypass versioning)
- **Fix:** Added `tempfile` crate to workspace and boternity-api dependencies
- **Files modified:** Cargo.toml, crates/boternity-api/Cargo.toml
- **Verification:** Build succeeds, edit command creates temp file
- **Committed in:** 5381e89 (Task 2 commit)

**2. [Rule 2 - Missing Critical] Enhanced bnity check with soul integrity**
- **Found during:** Task 2 (CLI commands)
- **Issue:** Existing `bnity check` only checked file existence, not integrity. Since integrity verification is now available, the check command should use it.
- **Fix:** Added soul integrity verification to the check command output
- **Files modified:** crates/boternity-api/src/main.rs
- **Verification:** Check command now shows soul integrity status
- **Committed in:** 5381e89 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing critical)
**Impact on plan:** Both auto-fixes necessary for correct operation. No scope creep.

## Issues Encountered

None - plan executed cleanly.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 1 (Foundation + Bot Identity) is now COMPLETE with all 6 plans executed
- Identity system: bot creation, soul versioning, integrity verification, secrets vault
- Ready for Phase 2 (Single-Agent Chat + LLM Integration)
- All 129 tests passing, clean architecture maintained (core never depends on infra)

## Self-Check: PASSED

---
*Phase: 01-foundation-bot-identity*
*Completed: 2026-02-11*
