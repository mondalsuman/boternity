---
phase: 04-web-ui-core-fleet-dashboard
plan: 07
subsystem: ui
tags: [react, monaco, diff-editor, version-history, timeline, rollback, tanstack-query]

# Dependency graph
requires:
  - phase: 04-06
    provides: Soul editor with Monaco, identity form, markdown preview, auto-save
provides:
  - Collapsible version history timeline panel with visual dots and connecting lines
  - Side-by-side diff viewer using Monaco DiffEditor
  - Rollback confirmation dialog with version content preview
  - useSoulVersion and useRollbackSoul query hooks
affects: [04-08]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Collapsible side panel with smooth width transition"
    - "Visual timeline with CSS dots and connecting lines"
    - "Monaco DiffEditor for version comparison in dialog overlay"
    - "AlertDialog for destructive rollback confirmation with preview"

key-files:
  created:
    - apps/web/src/components/soul/version-timeline.tsx
    - apps/web/src/components/soul/diff-viewer.tsx
    - apps/web/src/components/soul/rollback-dialog.tsx
  modified:
    - apps/web/src/hooks/use-soul-queries.ts
    - apps/web/src/routes/bots/$botId/soul.tsx
    - apps/web/src/components/soul/soul-editor.tsx

key-decisions:
  - "Version timeline as collapsible right panel (280px) with smooth width transition, collapsed by default"
  - "Timeline dots use border-2 circles with CSS positioning (not SVG) for simplicity"
  - "DiffViewer in a large Dialog (max-w-6xl, 80vh) overlay rather than replacing editor pane"
  - "Rollback uses AlertDialog (not Dialog) since it is a destructive confirmation action"
  - "Version actions (Compare, Restore) only visible when a version is selected in timeline"
  - "useSoulVersion has staleTime: Infinity since versions are immutable"

patterns-established:
  - "Collapsible panel pattern: width transition with overflow-hidden and inner fixed-width container"
  - "Timeline pattern: relative pl-6, absolute dots at left, connecting lines via pseudo-elements"
  - "Diff dialog pattern: DiffEditor in Dialog with version labels header bar"

# Metrics
duration: 3min
completed: 2026-02-13
---

# Phase 4 Plan 7: Version History Timeline, Diff Viewer, and Rollback Summary

**Collapsible version timeline with visual dots/lines, Monaco DiffEditor for side-by-side comparison, and rollback dialog with content preview**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-13T00:01:32Z
- **Completed:** 2026-02-13T00:04:17Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Collapsible right-side version history panel with visual timeline (dots + connecting lines)
- Auto-generated labels from timestamps with relative time via date-fns formatDistanceToNow
- Side-by-side diff viewer using Monaco DiffEditor with word-level highlighting
- Rollback confirmation dialog with version content preview and loading state
- Full version history workflow: browse -> select -> compare or restore

## Task Commits

Each task was committed atomically:

1. **Task 1: Version timeline panel with visual dots and connecting lines** - `72e3a8a` (feat)
2. **Task 2: Side-by-side diff viewer + rollback dialog** - `9674cd7` (feat)

## Files Created/Modified
- `apps/web/src/components/soul/version-timeline.tsx` - Collapsible timeline panel with dots, lines, skeleton loading, Compare/Restore actions
- `apps/web/src/components/soul/diff-viewer.tsx` - Monaco DiffEditor in Dialog overlay for side-by-side version comparison
- `apps/web/src/components/soul/rollback-dialog.tsx` - AlertDialog with version preview, Restore button with loading state
- `apps/web/src/hooks/use-soul-queries.ts` - Added useSoulVersion (single version fetch) and useRollbackSoul (POST rollback mutation)
- `apps/web/src/routes/bots/$botId/soul.tsx` - Added History button, timeline panel, wired diff/rollback state to SoulEditor
- `apps/web/src/components/soul/soul-editor.tsx` - Extended props for diffVersions and rollbackVersion, renders DiffViewer and RollbackDialog

## Decisions Made
- Version timeline collapsed by default (user decision: collapsible right side panel)
- Actions (Compare, Restore) appear only when a version card is selected to reduce visual clutter
- DiffViewer opens as a large dialog overlay (max-w-6xl, 80vh) rather than replacing the editor pane
- Rollback uses AlertDialog for destructive confirmation, includes scrollable content preview
- useSoulVersion query has staleTime: Infinity since versions are immutable once created

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Soul editor now fully featured: editing, auto-save, version history, diff comparison, rollback
- Ready for plan 04-08 (remaining web UI plans)
- Version timeline reactively refreshes after saves and rollbacks via TanStack Query invalidation

## Self-Check: PASSED

---
*Phase: 04-web-ui-core-fleet-dashboard*
*Completed: 2026-02-13*
