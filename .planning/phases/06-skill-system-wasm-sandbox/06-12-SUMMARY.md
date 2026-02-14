---
phase: 06-skill-system-wasm-sandbox
plan: 12
subsystem: api, web
tags: [axum, rest-api, react, tanstack-query, skill-management, web-ui]

# Dependency graph
requires:
  - phase: 06-10
    provides: AppState with SkillStore, WasmRuntime, SkillAuditLog
  - phase: 06-09
    provides: GitHubRegistryClient, SkillRegistry trait, DiscoveredSkill
provides:
  - REST API endpoints for skill CRUD (8 endpoints)
  - Web UI Skills tab in bot detail page
  - TypeScript types matching Rust skill types
  - TanStack Query hooks for skill data fetching
affects: [future-phases]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Axum handlers with AppState for skill store access"
    - "TanStack Query custom hooks (useBotSkills, useAllSkills)"
    - "Trust tier color-coded badges in web UI"

key-files:
  created:
    - crates/boternity-api/src/http/handlers/skill.rs
    - apps/web/src/types/skill.ts
    - apps/web/src/hooks/use-skill-queries.ts
    - apps/web/src/hooks/use-debounce.ts
    - apps/web/src/routes/bots/$botId/skills.tsx
  modified:
    - crates/boternity-api/src/http/handlers/mod.rs
    - crates/boternity-api/src/http/router.rs
    - apps/web/src/lib/api.ts
    - apps/web/src/routes/bots/$botId/route.tsx
    - apps/web/src/components/ui/switch.tsx

key-decisions:
  - "8 REST endpoints covering full skill CRUD plus registry search and install"
  - "Skills tab added to bot detail route alongside Overview, Chat, Soul, Settings"
  - "Three sections: Attached Skills, Available Skills, Discover Skills"
  - "Trust tier badges color-coded: green (local), yellow (verified), red (untrusted)"
  - "Switch toggle fixed for visibility in both light and dark themes"

patterns-established:
  - "Skill API handler pattern with AppState.skill_store access"
  - "TanStack Query invalidation on skill mutations"
  - "Debounce hook for registry search input"

# Metrics
duration: 8m
completed: 2026-02-14
---

# Phase 6 Plan 12: REST API + Web UI Skill Management Summary

**REST API handlers for skill CRUD and web UI Skills tab in bot detail page**

## Performance

- **Duration:** ~8m (auto tasks) + human verification
- **Started:** 2026-02-14
- **Completed:** 2026-02-14
- **Tasks:** 2 auto + 1 checkpoint (human-verify)
- **Files modified:** 10

## Accomplishments
- REST API with 8 endpoints: list all skills, get skill details, list bot skills, attach/detach skills, update skill config, search registries, install from registry
- Web UI Skills tab with three sections: Attached Skills, Available Skills, Discover Skills
- Trust tier badges (color-coded), skill type badges (prompt/tool icons), enable/disable toggle
- TanStack Query hooks for data fetching with mutation invalidation
- Registry search with debounced input
- Fixed switch toggle visibility in both light and dark themes (checkpoint feedback)

## Task Commits

Each task was committed atomically:

1. **Task 1: REST API handlers for skill management** - `0b39fa6` (feat)
2. **Task 2: Web UI skill management page** - `dbc15e8` (feat)
3. **Checkpoint fix: Switch toggle visibility** - `b741c5b` (fix)

## Files Created/Modified
- `crates/boternity-api/src/http/handlers/skill.rs` - 8 REST API handler functions
- `crates/boternity-api/src/http/handlers/mod.rs` - Added `pub mod skill;`
- `crates/boternity-api/src/http/router.rs` - Registered skill routes under /api/skills and /api/bots/{id}/skills
- `apps/web/src/types/skill.ts` - TypeScript interfaces matching Rust skill types
- `apps/web/src/hooks/use-skill-queries.ts` - TanStack Query hooks for skill API
- `apps/web/src/hooks/use-debounce.ts` - Debounce hook for search input
- `apps/web/src/routes/bots/$botId/skills.tsx` - Skills tab page component
- `apps/web/src/routes/bots/$botId/route.tsx` - Added Skills tab to bot detail navigation
- `apps/web/src/components/ui/switch.tsx` - Fixed unchecked state visibility

## Decisions Made
- Eight REST endpoints covering complete skill lifecycle
- Skills tab positioned after Soul in bot detail tab bar
- Three-section layout: Attached (with toggle/detach), Available (with attach), Discover (with search/install)
- AlertDialog confirmation for detach operations
- Switch component uses muted-foreground for unchecked track (better contrast)

## Deviations from Plan

### User-Requested Fix
**Switch toggle visibility** - User reported toggle nearly invisible in both themes during checkpoint verification. Fixed by changing unchecked track color from `bg-input` to `bg-muted-foreground/30` (light) / `bg-muted-foreground/40` (dark).

---

**Total deviations:** 1 user-requested fix
**Impact on plan:** Minor UI polish. No scope change.

## Human Verification

Checkpoint verified by user:
- CLI skill create, list, inspect: Working
- Web UI Skills tab: Renders correctly
- Attach/detach/toggle: Functional
- Switch visibility fix: Approved

## Self-Check: PASSED

---
*Phase: 06-skill-system-wasm-sandbox*
*Completed: 2026-02-14*
