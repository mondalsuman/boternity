---
phase: 06-skill-system-wasm-sandbox
plan: 11
subsystem: cli
tags: [clap, ratatui, crossterm, tui, skill-management, registry]

# Dependency graph
requires:
  - phase: 06-02
    provides: SkillStore filesystem operations
  - phase: 06-04
    provides: inspect_resolved_capabilities inheritance resolver
  - phase: 06-09
    provides: GitHubRegistryClient, SkillRegistry trait, DiscoveredSkill
  - phase: 06-08
    provides: OS sandbox executor (referenced by skill types)
  - phase: 06-10
    provides: AgentEngine skill integration, SkillStore on AppState
provides:
  - bnity skill subcommand tree with 11 subcommands
  - Interactive TUI skill browser with 3-pane layout
  - CLI skill create/install/remove/list/inspect/attach/detach/enable/disable/publish/update
affects: [06-12, future-phases]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SkillCommand clap derive enum with async handle_skill_command dispatcher"
    - "ratatui 3-pane browser with BrowserState, category filtering, live search"
    - "Clone-on-read in TUI detail panel to avoid borrow lifetime issues"

key-files:
  created:
    - crates/boternity-api/src/cli/skill.rs
    - crates/boternity-api/src/cli/skill_browser.rs
  modified:
    - crates/boternity-api/src/cli/mod.rs
    - crates/boternity-api/src/main.rs
    - crates/boternity-api/src/http/handlers/skill.rs

key-decisions:
  - "SkillCommand as top-level bnity skill subcommand (not verb-noun like create skill)"
  - "Interactive registry selection when multiple search results found"
  - "Capability approval prompt before install (interactive Confirm dialog)"
  - "Clone-on-read for TUI detail panel (avoids borrow checker issues with Span lifetimes)"
  - "Categories pane uses deduplicated list from all skill.categories fields"

patterns-established:
  - "handle_skill_command async dispatch pattern matching SkillCommand enum"
  - "BrowserState with refilter() for category + search combo filtering"
  - "TUI raw mode with clean restore on exit via crossterm EnterAlternateScreen/LeaveAlternateScreen"

# Metrics
duration: 6m
completed: 2026-02-14
---

# Phase 6 Plan 11: CLI Skill Management + TUI Browser Summary

**Clap-based bnity skill subcommand with 11 operations (create/install/remove/list/inspect/attach/detach/enable/disable/publish/update/browse) and ratatui 3-pane TUI browser**

## Performance

- **Duration:** 6m 1s
- **Started:** 2026-02-14T00:23:04Z
- **Completed:** 2026-02-14T00:29:05Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Full skill management CLI: create local skills, install from registries with capability approval, remove/list/inspect/attach/detach/enable/disable
- Inspect shows resolved capabilities via inheritance resolution (own vs inherited vs combined)
- Interactive ratatui TUI browser with category filtering, live search, trust tier badges, and skill detail panel
- Publish validates manifest and prints submission instructions

## Task Commits

Each task was committed atomically:

1. **Task 1: CLI skill management commands** - `0b39fa6` (feat)
2. **Task 2: Interactive TUI skill browser** - `f53fbb4` (feat)

## Files Created/Modified
- `crates/boternity-api/src/cli/skill.rs` - SkillCommand enum + 11 handler functions (create, install, remove, list, inspect, attach, detach, enable/disable, publish, browse, update)
- `crates/boternity-api/src/cli/skill_browser.rs` - ratatui 3-pane TUI browser with BrowserState, category/search filtering, detail panel
- `crates/boternity-api/src/cli/mod.rs` - Added `pub mod skill; pub mod skill_browser;` and Skill variant to Commands enum
- `crates/boternity-api/src/main.rs` - Wired `Commands::Skill` dispatch to `handle_skill_command`
- `crates/boternity-api/src/http/handlers/skill.rs` - Fixed pre-existing private type visibility to pub(crate) [deviation]

## Decisions Made
- SkillCommand as top-level `bnity skill` subcommand (consistent with `bnity soul`, `bnity provider`, `bnity storage` pattern)
- Interactive selection prompt when multiple registry search results match
- Capability approval required before install (Confirm dialog, defaults to deny)
- Update command reports current versions (full remote version check deferred to future enhancement)
- Clone-on-read for TUI detail panel strings to satisfy Span lifetime requirements

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed private type visibility on HTTP skill handler DTOs**
- **Found during:** Task 1 (cargo check)
- **Issue:** Pre-existing SkillListItem, SkillDetail, BotSkillItem, RegistrySearchItem structs in http/handlers/skill.rs were private but used in pub(crate) functions, causing compilation errors
- **Fix:** Changed visibility to pub(crate) on all four structs
- **Files modified:** crates/boternity-api/src/http/handlers/skill.rs
- **Verification:** cargo check passes
- **Committed in:** 0b39fa6 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Auto-fix necessary to unblock compilation. Pre-existing issue from parallel plan 06-12. No scope creep.

## Issues Encountered
- Borrow lifetime issue in TUI detail panel: Span borrows from local strings that go out of scope. Solved by cloning strings before creating Lines.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- CLI skill management complete, ready for end-to-end testing
- TUI browser functional, loads skills from configured registries
- Plan 06-12 (REST API + Web UI) runs in parallel and is independent

## Self-Check: PASSED

---
*Phase: 06-skill-system-wasm-sandbox*
*Completed: 2026-02-14*
