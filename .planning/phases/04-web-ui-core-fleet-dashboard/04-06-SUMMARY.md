---
phase: 04-web-ui-core-fleet-dashboard
plan: 06
subsystem: ui
tags: [monaco-editor, react, tanstack-query, debounce, markdown, soul-editor, auto-save]

# Dependency graph
requires:
  - phase: 04-02
    provides: Scaffold with bot detail layout, tabs, route structure, shadcn/ui components
  - phase: 01-06
    provides: Soul versioning and integrity verification API
  - phase: 04-01
    provides: REST API identity/user endpoints, soul endpoints
provides:
  - Soul editor with Monaco for SOUL.md/IDENTITY.md/USER.md
  - Identity form with model/temperature/max_tokens controls
  - Markdown split preview panel
  - TanStack Query hooks for soul, identity, user context CRUD
  - Debounced auto-save (2s inactivity)
  - Active tab detection on bot detail layout
affects: [04-07, 05-web-ui-advanced]

# Tech tracking
tech-stack:
  added: [@monaco-editor/react (already in deps), react-markdown, remark-gfm]
  patterns:
    - "Debounced auto-save with useDebouncedCallback hook (2s delay)"
    - "File tab navigation within soul editor (soul/identity/user)"
    - "Form/raw toggle for IDENTITY.md editing"
    - "Active tab detection via useMatchRoute in layout routes"

key-files:
  created:
    - apps/web/src/components/soul/soul-editor.tsx
    - apps/web/src/components/soul/identity-form.tsx
    - apps/web/src/components/soul/markdown-preview.tsx
    - apps/web/src/hooks/use-soul-queries.ts
    - apps/web/src/hooks/use-debounce.ts
    - apps/web/src/components/ui/label.tsx
    - apps/web/src/components/ui/slider.tsx
    - apps/web/src/components/ui/select.tsx
    - apps/web/src/components/ui/switch.tsx
  modified:
    - apps/web/src/routes/bots/$botId/route.tsx
    - apps/web/src/routes/bots/$botId/soul.tsx

key-decisions:
  - "Active tab detection via useMatchRoute instead of defaultValue on Tabs (tracks URL changes)"
  - "Identity form rebuilds raw IDENTITY.md frontmatter on every change (preserves body content)"
  - "Local editor buffers populated once from fetch data, then managed locally (prevents overwrite on refetch)"
  - "shadcn/ui primitives (Label, Slider, Select, Switch) added for identity form controls"

patterns-established:
  - "useDebouncedCallback: stable debounced callback with cancel, ref-based latest callback"
  - "Soul query hooks: separate read/write hooks per file type with toast notifications"
  - "Form/raw toggle pattern: Switch component toggles between IdentityForm and Monaco editor"

# Metrics
duration: 4min
completed: 2026-02-13
---

# Phase 4 Plan 06: Soul Editor Summary

**Monaco-based soul editor with file tabs, identity form (model/temperature/max_tokens), debounced auto-save, and split markdown preview**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-12T23:52:21Z
- **Completed:** 2026-02-12T23:56:10Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments
- Full soul editor at `/bots/$botId/soul` with Monaco markdown editing for SOUL.md, IDENTITY.md, and USER.md
- Identity form view with model dropdown (5 models), temperature slider (0-2), and max_tokens input with form/raw toggle
- Split preview layout: Monaco editor left, rendered markdown right via react-markdown + remark-gfm
- Auto-save with 2-second debounce after inactivity via `useDebouncedCallback` hook
- TanStack Query hooks for all soul file CRUD operations with toast notifications
- Active tab detection on bot detail layout via `useMatchRoute`

## Task Commits

Each task was committed atomically:

1. **Task 1: Bot detail page tabs + soul/identity/user query hooks + debounce** - `7e77c58` (feat)
2. **Task 2: Soul editor with Monaco, file tabs, auto-save, identity form, split preview** - `f3a8d31` (feat)

## Files Created/Modified
- `apps/web/src/components/soul/soul-editor.tsx` - Monaco editor wrapper with file tabs, auto-save, split preview
- `apps/web/src/components/soul/identity-form.tsx` - Form view for IDENTITY.md with model/temperature/max_tokens controls
- `apps/web/src/components/soul/markdown-preview.tsx` - Rendered markdown preview using react-markdown
- `apps/web/src/hooks/use-soul-queries.ts` - TanStack Query hooks for soul, identity, user context CRUD
- `apps/web/src/hooks/use-debounce.ts` - Debounced callback hook for auto-save
- `apps/web/src/components/ui/label.tsx` - shadcn/ui Label primitive
- `apps/web/src/components/ui/slider.tsx` - shadcn/ui Slider primitive
- `apps/web/src/components/ui/select.tsx` - shadcn/ui Select primitive
- `apps/web/src/components/ui/switch.tsx` - shadcn/ui Switch primitive
- `apps/web/src/routes/bots/$botId/route.tsx` - Updated with active tab detection via useMatchRoute
- `apps/web/src/routes/bots/$botId/soul.tsx` - Replaced placeholder with SoulEditor component

## Decisions Made
- Used `useMatchRoute` for active tab detection instead of `defaultValue` on Tabs (tracks URL changes properly)
- Identity form rebuilds raw IDENTITY.md frontmatter on every field change to preserve body content below the frontmatter
- Local editor buffers populated once from fetch data via refs, then managed locally to prevent overwrite during refetch
- Added shadcn/ui primitives (Label, Slider, Select, Switch) as they were needed for the identity form and did not exist yet

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added missing shadcn/ui primitives (Label, Slider, Select, Switch)**
- **Found during:** Task 2 (Identity form creation)
- **Issue:** Identity form requires Label, Slider, Select, Switch components that were not yet in the UI component library
- **Fix:** Created standard shadcn/ui components using radix-ui primitives
- **Files created:** label.tsx, slider.tsx, select.tsx, switch.tsx
- **Verification:** TypeScript compilation passes, components render correctly
- **Committed in:** f3a8d31 (Task 2 commit)

**2. [Rule 1 - Bug] Fixed tab active state not tracking route changes**
- **Found during:** Task 1 (Route layout review)
- **Issue:** Tabs used `defaultValue="overview"` which is uncontrolled and does not update when navigating between tabs
- **Fix:** Added `useActiveTab` hook using `useMatchRoute` to derive active tab from current route, switched Tabs to controlled `value` prop
- **Files modified:** route.tsx
- **Verification:** Tab highlights correctly follow route navigation
- **Committed in:** 7e77c58 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for correct operation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Soul editor fully functional, ready for real-time collaboration features in future phases
- Identity form pattern can be extended with additional frontmatter fields
- Query hooks available for any component needing soul/identity/user data

## Self-Check: PASSED

---
*Phase: 04-web-ui-core-fleet-dashboard*
*Completed: 2026-02-13*
