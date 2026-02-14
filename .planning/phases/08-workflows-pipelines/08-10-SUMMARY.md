---
phase: 08-workflows-pipelines
plan: 10
subsystem: ui
tags: [react-flow, dagre, workflow-builder, canvas, custom-nodes, zustand, tanstack-router]

# Dependency graph
requires:
  - phase: 08-01
    provides: Workflow domain types (WorkflowDefinition, StepConfig, etc.)
  - phase: 08-09
    provides: Workflow HTTP API endpoints for CRUD, trigger, runs
  - phase: 08-12
    provides: TypeScript type conventions matching Rust serde
  - phase: 04-02
    provides: Web UI shell with sidebar, routing, shadcn/ui components
provides:
  - Workflow list and detail pages with run history
  - React Flow visual builder canvas with 8 custom node types
  - Typed edges with color coding by data type
  - Workflow API client for frontend
  - Zustand workflow store for canvas state
  - Dagre auto-layout for DAG visualization
affects: [08-11, 08-13]

# Tech tracking
tech-stack:
  added: ["@xyflow/react ^12.10.0", "@dagrejs/dagre ^2.0.4", "@types/dagre ^0.7.53"]
  patterns: ["Custom React Flow nodeTypes as module-level constant", "definitionToFlow/flowToDefinition converter pair", "nodeStatusClass shared utility for status border coloring"]

key-files:
  created:
    - apps/web/src/components/workflow/WorkflowCanvas.tsx
    - apps/web/src/components/workflow/nodes/AgentNode.tsx
    - apps/web/src/components/workflow/nodes/SkillNode.tsx
    - apps/web/src/components/workflow/nodes/CodeNode.tsx
    - apps/web/src/components/workflow/nodes/HttpNode.tsx
    - apps/web/src/components/workflow/nodes/ConditionalNode.tsx
    - apps/web/src/components/workflow/nodes/LoopNode.tsx
    - apps/web/src/components/workflow/nodes/ApprovalNode.tsx
    - apps/web/src/components/workflow/nodes/SubWorkflowNode.tsx
    - apps/web/src/components/workflow/nodes/shared.ts
    - apps/web/src/components/workflow/edges/TypedEdge.tsx
    - apps/web/src/routes/workflows/index.tsx
    - apps/web/src/routes/workflows/$workflowId.tsx
    - apps/web/src/routes/workflows/builder/$workflowId.tsx
    - apps/web/src/lib/api/workflows.ts
    - apps/web/src/stores/workflow-store.ts
    - apps/web/src/types/workflow.ts
  modified:
    - apps/web/package.json
    - apps/web/src/components/layout/app-sidebar.tsx

key-decisions:
  - "ConditionalNode uses dual source handles at 30%/70% bottom position for then/else branches"
  - "TypedEdge uses invisible 20px-wide path for easier hover targeting"
  - "nodeTypes/edgeTypes defined as module-level constants to avoid React re-renders"
  - "definitionToFlow auto-applies dagre layout when no UI positions exist in definition"
  - "Workflow types in separate types/workflow.ts matching Rust serde representation exactly"

patterns-established:
  - "Custom React Flow node pattern: memo-wrapped, typed NodeProps cast, Handle at Top/Bottom"
  - "nodeStatusClass shared utility for consistent status border coloring across all node types"
  - "definitionToFlow/flowToDefinition converter pair for bidirectional definition-canvas mapping"

# Metrics
duration: 7m 33s
completed: 2026-02-14
---

# Phase 8 Plan 10: Web UI Workflow Builder Summary

**React Flow visual builder canvas with 8 custom nodes, typed edges, dagre auto-layout, and workflow list/detail pages with run history**

## Performance

- **Duration:** 7m 33s
- **Started:** 2026-02-14T15:49:33Z
- **Completed:** 2026-02-14T15:57:06Z
- **Tasks:** 2
- **Files created:** 17
- **Files modified:** 2

## Accomplishments

- Workflow list page with search, trigger, and delete-with-confirmation actions
- Workflow detail page with run history tabs, expandable step logs, approve/cancel controls
- Full-viewport React Flow canvas with MiniMap, Controls, Background, and dagre auto-layout
- 8 custom node components with step-type-specific icons, rich previews, and status-colored borders
- Typed edges with color coding by data type (text=blue, json=green, file=orange) and animated execution state
- API client covering all workflow CRUD, trigger, run, approve, and cancel endpoints

## Task Commits

Each task was committed atomically:

1. **Task 1: Install deps, API client, store, and workflow list/detail pages** - `16af33d` (feat)
2. **Task 2: React Flow canvas with 8 custom nodes and typed edges** - `d155d4c` (feat)

## Files Created/Modified

- `apps/web/src/types/workflow.ts` - TypeScript types mirroring Rust workflow domain
- `apps/web/src/lib/api/workflows.ts` - API client for workflow CRUD, trigger, runs
- `apps/web/src/stores/workflow-store.ts` - Zustand store for canvas UI state
- `apps/web/src/routes/workflows/index.tsx` - Workflow list page with search/trigger/delete
- `apps/web/src/routes/workflows/$workflowId.tsx` - Workflow detail with run history tabs
- `apps/web/src/routes/workflows/builder/$workflowId.tsx` - Visual builder page with save
- `apps/web/src/components/workflow/WorkflowCanvas.tsx` - React Flow canvas with dagre layout
- `apps/web/src/components/workflow/nodes/AgentNode.tsx` - Bot name, prompt preview
- `apps/web/src/components/workflow/nodes/SkillNode.tsx` - Skill name, input preview
- `apps/web/src/components/workflow/nodes/CodeNode.tsx` - Language badge, source preview
- `apps/web/src/components/workflow/nodes/HttpNode.tsx` - Method badge, URL display
- `apps/web/src/components/workflow/nodes/ConditionalNode.tsx` - Dual then/else handles
- `apps/web/src/components/workflow/nodes/LoopNode.tsx` - Condition, max iterations
- `apps/web/src/components/workflow/nodes/ApprovalNode.tsx` - Yellow accent, prompt preview
- `apps/web/src/components/workflow/nodes/SubWorkflowNode.tsx` - Referenced workflow name
- `apps/web/src/components/workflow/nodes/shared.ts` - nodeStatusClass shared utility
- `apps/web/src/components/workflow/edges/TypedEdge.tsx` - Color-coded edges by data type
- `apps/web/package.json` - Added @xyflow/react, @dagrejs/dagre
- `apps/web/src/components/layout/app-sidebar.tsx` - Added Workflows sidebar navigation

## Decisions Made

- ConditionalNode uses dual source handles positioned at 30%/70% of bottom edge for clear then/else branching
- TypedEdge renders an invisible 20px-wide path overlay for easier hover interaction targeting
- nodeTypes and edgeTypes are defined as module-level constants (outside components) to prevent React re-renders on every render cycle
- definitionToFlow automatically applies dagre auto-layout when no step UI position metadata exists in the definition
- Workflow TypeScript types placed in dedicated types/workflow.ts file (matching pattern from types/bot.ts, types/chat.ts)
- flowToDefinition preserves the original WorkflowDefinition structure and only updates node positions from canvas

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- TanStack Router type checking failed when workflow list/detail pages referenced the builder route before the builder file existed. Created the builder route stub in Task 1 to enable route tree generation, then replaced with full implementation in Task 2.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Visual builder canvas ready for Plan 11 (NodePalette drag-to-add, property panels)
- All 8 node types render correctly and are wired to nodeTypes constant
- definitionToFlow/flowToDefinition converters ready for integration with builder save/load flow
- Workflow routes registered in TanStack Router and sidebar navigation

## Self-Check: PASSED

---
*Phase: 08-workflows-pipelines*
*Completed: 2026-02-14*
