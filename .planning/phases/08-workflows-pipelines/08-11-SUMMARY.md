---
phase: 08-workflows-pipelines
plan: 11
subsystem: ui
tags: [react, react-flow, monaco, yaml, undo-redo, drag-drop, workflow-builder, templates]

# Dependency graph
requires:
  - phase: 08-10
    provides: React Flow canvas with 8 custom nodes and typed edges
provides:
  - StepConfigPanel with dynamic forms for all 8 step types
  - NodePalette with categorized draggable step types
  - use-undo-redo hook for canvas state management
  - YamlEditor with Monaco and inline validation
  - WorkflowTemplates dialog with 4 built-in templates
  - Full builder toolbar with canvas/YAML toggle, undo/redo, grouping
affects: [08-13, 09-mcp-integration]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Imperative ref handle for canvas methods (autoLayout, undo, redo, group)"
    - "HTML5 drag-and-drop with dataTransfer for palette -> canvas"
    - "structuredClone-based undo/redo with past/future stacks"
    - "Canvas/YAML toggle with unidirectional sync (canvas -> yaml on switch)"

key-files:
  created:
    - apps/web/src/components/workflow/panels/StepConfigPanel.tsx
    - apps/web/src/components/workflow/panels/NodePalette.tsx
    - apps/web/src/components/workflow/YamlEditor.tsx
    - apps/web/src/components/workflow/WorkflowTemplates.tsx
    - apps/web/src/hooks/use-undo-redo.ts
  modified:
    - apps/web/src/components/workflow/WorkflowCanvas.tsx
    - apps/web/src/routes/workflows/builder/$workflowId.tsx

key-decisions:
  - "Canvas-to-YAML sync is unidirectional (canvas -> YAML on toggle); full bidirectional parse-back deferred as future enhancement"
  - "Undo/redo uses structuredClone with max 50 entries to cap memory"
  - "Node grouping uses React Flow parentId API with visual dashed-border group nodes"
  - "Template steps are hardcoded constants (no backend dependency)"

patterns-established:
  - "WorkflowCanvasHandle imperative ref for parent-to-canvas commands"
  - "Palette drag uses 'application/boternity-step-type' MIME type"

# Metrics
duration: 5min
completed: 2026-02-14
---

# Phase 8 Plan 11: Builder Panels and YAML Editor Summary

**StepConfigPanel with 8 dynamic forms, NodePalette drag-and-drop, Monaco YAML editor, 4 workflow templates, and undo/redo canvas operations**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-14T16:00:32Z
- **Completed:** 2026-02-14T16:05:35Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Right-side step config panel with dynamic form per step type plus common fields (name, timeout, retry, depends_on, condition)
- Left sidebar node palette with categorized draggable items (AI, Logic, Integration, Control) using HTML5 drag-and-drop
- Undo/redo hook with past/future stacks, max 50 entries, structuredClone deep copies
- WorkflowCanvas enhanced with onDrop, node selection, keyboard shortcuts (Ctrl+Z/Y), and node grouping/ungrouping
- Monaco YAML editor with debounced onChange (500ms) and inline validation
- 4 built-in templates: Data Pipeline, Approval Flow, Multi-Bot Collaboration, Scheduled Report
- Full builder toolbar: Save, Canvas/YAML toggle, Auto-layout, Undo/Redo, Group/Ungroup, Templates, Test Step/Run placeholders

## Task Commits

Each task was committed atomically:

1. **Task 1: Step config panel, node palette, and undo/redo** - `eabd2e6` (feat)
2. **Task 2: YAML editor, templates, and builder page integration** - `4dbdcb9` (feat)

## Files Created/Modified
- `apps/web/src/components/workflow/panels/StepConfigPanel.tsx` - Right-side panel with dynamic forms for all 8 step types
- `apps/web/src/components/workflow/panels/NodePalette.tsx` - Left sidebar with categorized draggable step types
- `apps/web/src/hooks/use-undo-redo.ts` - Generic undo/redo hook with past/future ref stacks
- `apps/web/src/components/workflow/WorkflowCanvas.tsx` - Enhanced with drop handler, node selection, keyboard shortcuts, grouping
- `apps/web/src/components/workflow/YamlEditor.tsx` - Monaco editor with YAML mode and inline validation
- `apps/web/src/components/workflow/WorkflowTemplates.tsx` - Template picker dialog with 4 built-in templates
- `apps/web/src/routes/workflows/builder/$workflowId.tsx` - Full builder page with toolbar, panels, and editor toggle

## Decisions Made
- Canvas-to-YAML sync is unidirectional: YAML is generated from canvas state when toggling to YAML mode. Full bidirectional parse-back deferred as future enhancement to avoid complex parsing.
- Undo/redo stores max 50 snapshots using structuredClone for deep copies. Version counter forces re-renders when stacks change.
- Node grouping uses React Flow parentId API. Group nodes are type="group" with dashed border styling. Child positions recalculated relative to group.
- Template data is hardcoded as constants, no backend dependency needed.
- Test Step and Run buttons are placeholder (disabled) -- will be wired in plan 08-13.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Full n8n-like builder experience complete with config panels, palette, YAML editing, templates, and undo/redo
- Plan 08-13 can wire up test step execution and run triggers
- All builder UI foundation established for future MCP integration

## Self-Check: PASSED

---
*Phase: 08-workflows-pipelines*
*Completed: 2026-02-14*
