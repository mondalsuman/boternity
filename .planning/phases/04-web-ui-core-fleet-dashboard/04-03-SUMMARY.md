---
phase: 04-web-ui-core-fleet-dashboard
plan: 03
subsystem: ui
tags: [react, tanstack-query, dashboard, bot-card, stats-bar, shadcn, radix-ui]

# Dependency graph
requires:
  - phase: 04-02
    provides: React app shell with routing, sidebar, command palette, theme
  - phase: 04-01
    provides: Backend API endpoints for /bots and /stats
provides:
  - Fleet dashboard landing page with stats bar, bot grid, search/sort
  - TanStack Query hooks for bot CRUD operations
  - Create bot dialog with emoji picker and form validation
  - Empty state for zero-bot onboarding flow
  - AlertDialog shadcn component
affects: [04-04, 04-05, 04-06, 04-07, 04-08]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "TanStack Query hooks pattern: dedicated hook files per resource (use-bot-queries, use-stats-query)"
    - "Client-side search/sort/filter with useMemo for small datasets"
    - "Shared dialog state between desktop header button, mobile FAB, and empty state CTA"
    - "AlertDialog for destructive confirmations (delete bot)"

key-files:
  created:
    - apps/web/src/hooks/use-bot-queries.ts
    - apps/web/src/hooks/use-stats-query.ts
    - apps/web/src/components/dashboard/stats-bar.tsx
    - apps/web/src/components/dashboard/bot-card.tsx
    - apps/web/src/components/dashboard/bot-grid.tsx
    - apps/web/src/components/dashboard/empty-state.tsx
    - apps/web/src/components/dashboard/create-bot-dialog.tsx
    - apps/web/src/components/ui/alert-dialog.tsx
  modified:
    - apps/web/src/routes/index.tsx

key-decisions:
  - "Client-side search/sort instead of server-side: single-user app has small bot counts, immediate responsiveness preferred"
  - "DropdownMenu with RadioGroup for sort picker instead of Select component: simpler, consistent with existing components"
  - "placeholderData: (prev) => prev in useBots for smooth filter transitions (keepPreviousData equivalent)"
  - "Emoji picker as a simple grid of 16 common emojis rather than a full picker library"

patterns-established:
  - "Dashboard component pattern: stats-bar + grid with shared filter state lifted to page"
  - "Card action pattern: overflow menu on hover + prominent CTA button"
  - "Empty state pattern: Lucide icon composition for illustrations"
  - "Mobile FAB pattern: fixed bottom-right button hidden on md+ breakpoint"

# Metrics
duration: 3m 24s
completed: 2026-02-13
---

# Phase 4 Plan 3: Fleet Dashboard Summary

**Fleet dashboard with clickable stats bar, bot card grid (search/sort/filter), create bot dialog with emoji picker, empty state CTA, and responsive mobile FAB**

## Performance

- **Duration:** 3m 24s
- **Started:** 2026-02-12T23:51:11Z
- **Completed:** 2026-02-12T23:54:35Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- TanStack Query hooks for stats and full bot CRUD with cache invalidation and toast notifications
- Clickable stats bar with aggregate metrics that filter the bot grid (e.g., click "Active" to show only active bots)
- Bot cards with emoji avatar, color-coded status badge, model info, relative timestamps, overflow menu (edit/disable/delete), and Chat CTA
- Client-side search, sort (name/activity/status), and responsive 1-4 column grid
- Empty state with Lucide icon illustration and prominent "Create your first bot" CTA
- Create bot dialog with name validation (3-50 chars), description textarea, emoji picker grid
- Mobile FAB and desktop header button sharing the same dialog state

## Task Commits

Each task was committed atomically:

1. **Task 1: Stats bar + bot card grid with search and sort** - `d794c0c` (feat)
2. **Task 2: Empty state + create bot dialog + mobile FAB** - `f47b5ed` (feat)

## Files Created/Modified
- `apps/web/src/hooks/use-stats-query.ts` - Stats TanStack Query hook with Stats type definition
- `apps/web/src/hooks/use-bot-queries.ts` - Bot CRUD hooks (useBots, useBot, useCreateBot, useUpdateBot, useDeleteBot)
- `apps/web/src/components/dashboard/stats-bar.tsx` - Clickable stats bar with filter callback
- `apps/web/src/components/dashboard/bot-card.tsx` - Bot card with STATUS_COLORS, overflow menu, delete confirmation
- `apps/web/src/components/dashboard/bot-grid.tsx` - Responsive grid with search input and sort dropdown
- `apps/web/src/components/dashboard/empty-state.tsx` - Zero-bot onboarding with Lucide icon illustration
- `apps/web/src/components/dashboard/create-bot-dialog.tsx` - Create bot form with emoji picker and validation
- `apps/web/src/components/ui/alert-dialog.tsx` - AlertDialog shadcn component for destructive confirmations
- `apps/web/src/routes/index.tsx` - Dashboard page composition with stats bar, grid, empty state, FAB

## Decisions Made
- Client-side search/sort instead of server-side: single-user app has small bot counts, immediate responsiveness preferred over network round-trips
- Used DropdownMenu with RadioGroup for sort picker instead of adding Select component: simpler, consistent with existing UI components
- `placeholderData: (prev) => prev` in useBots for smooth transitions between filter states (TanStack Query v5 equivalent of keepPreviousData)
- Emoji picker as a simple 16-emoji grid rather than a full picker library: lightweight, sufficient for bot personality selection
- AlertDialog added as new shadcn component (was missing from scaffold) for delete confirmation pattern

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added AlertDialog shadcn component**
- **Found during:** Task 1 (Bot card implementation)
- **Issue:** Bot card requires AlertDialog for delete confirmation but the component was not in the scaffold
- **Fix:** Created alert-dialog.tsx following the same shadcn/radix-ui pattern as existing dialog.tsx
- **Files modified:** apps/web/src/components/ui/alert-dialog.tsx
- **Verification:** TypeScript compilation passes, component renders correctly
- **Committed in:** d794c0c (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential component needed for delete confirmation UX. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Fleet dashboard complete, serves as landing page at /
- Bot CRUD hooks (useCreateBot, useUpdateBot, useDeleteBot) ready for reuse in bot detail pages
- Stats query hook ready for any page that needs aggregate metrics
- AlertDialog component available for any future destructive action confirmations
- Ready for 04-04 (bot detail pages) which will use useBot hook and navigate from bot cards

## Self-Check: PASSED

---
*Phase: 04-web-ui-core-fleet-dashboard*
*Completed: 2026-02-13*
