---
phase: 05-agent-hierarchy-event-system
plan: 08
subsystem: ui
tags: [websocket, zustand, agent-tree, streaming, react, budget, collapsible, sse]

# Dependency graph
requires:
  - phase: 05-agent-hierarchy-event-system
    provides: "AgentEvent enum and WebSocket /ws/events endpoint"
  - phase: 04-web-ui-core-fleet-dashboard
    provides: "Chat UI components, SSE chat hook, Zustand stores, shadcn/ui primitives"
provides:
  - "WebSocket hook with exponential backoff reconnection at /ws/events"
  - "Zustand agent store processing 8 AgentEvent types into tree structure"
  - "AgentBlock: collapsible inline sub-agent blocks with streaming text and metadata"
  - "AgentTreePanel: process-manager tree view with per-agent stop buttons"
  - "BudgetIndicator: usage bar with cost estimate and Continue/Stop prompt"
  - "WsStatus: connection status indicator (connected/reconnecting/disconnected)"
  - "SSE chat hook extended with agent event forwarding to store"
affects:
  - 06-sub-agent-ui-observability (observability dashboards may consume agent store data)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Native WebSocket with exponential backoff (1s-30s, 30% jitter, max 10 attempts)"
    - "Map-based Zustand store for agent tree with functional updates (no immer)"
    - "Agent events forwarded from SSE stream to Zustand via getState().handleEvent()"
    - "Memo-wrapped components for per-agent rendering isolation"

key-files:
  created:
    - apps/web/src/types/agent.ts
    - apps/web/src/hooks/use-websocket.ts
    - apps/web/src/hooks/use-agent-tree.ts
    - apps/web/src/stores/agent-store.ts
    - apps/web/src/components/chat/agent-block.tsx
    - apps/web/src/components/chat/agent-tree-panel.tsx
    - apps/web/src/components/chat/budget-indicator.tsx
    - apps/web/src/components/chat/ws-status.tsx
  modified:
    - apps/web/src/components/chat/message-list.tsx
    - apps/web/src/hooks/use-sse-chat.ts

key-decisions:
  - "Native WebSocket API (no npm dependency) with exponential backoff 1s-30s, 30% jitter, max 10 attempts"
  - "Map-based Zustand store with functional updates instead of immer (Map ops are straightforward)"
  - "AgentEvent forwarded from SSE via useAgentStore.getState().handleEvent() (outside React lifecycle)"
  - "AgentBlock auto-collapses on completion, auto-expands on running (via useEffect on status)"
  - "TreeNode recursive component with depth-based paddingLeft for tree indentation"
  - "Blended $9/1M cost estimate for budget indicator (rough hint, not exact billing)"

patterns-established:
  - "useAgentWebSocket: reusable WebSocket hook with exponential backoff reconnection"
  - "Agent store as central event processor: any component subscribing sees real-time tree state"
  - "SSE hook as agent event bridge: SSE stream events forwarded to Zustand for UI consumption"

# Metrics
duration: 4min
completed: 2026-02-13
---

# Phase 5 Plan 8: Web UI Agent Hierarchy Summary

**Real-time agent hierarchy UI with WebSocket hook, Zustand tree store, collapsible inline AgentBlocks with streaming text, process-manager tree panel, budget indicator with Continue/Stop, and WsStatus dot**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-13T22:05:05Z
- **Completed:** 2026-02-13T22:09:33Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments

- Created complete TypeScript type definitions matching Rust AgentEvent enum (14 variants tagged union)
- Built WebSocket hook with native API and exponential backoff reconnection (1s-30s, 30% jitter)
- Implemented Zustand agent store processing 8 event types into a live agent tree with budget tracking
- Created AgentBlock component with collapsible body, status-colored borders, streaming markdown, and token/duration footer
- Built AgentTreePanel with recursive depth indentation, status badges, and per-agent stop buttons
- Added BudgetIndicator with color-coded progress bar, cost estimate, and Continue/Stop buttons on warning
- Created WsStatus connection indicator with green/yellow/red dot and optional label
- Extended SSE chat hook with 9 new agent event types forwarded to agent store
- Integrated inline AgentBlock rendering in message list for active sub-agents

## Task Commits

Each task was committed atomically:

1. **Task 1: Agent types, WebSocket hook, and agent store** - `263dbe4` (feat)
2. **Task 2: Agent UI components and SSE chat hook integration** - `769d556` (feat)

## Files Created/Modified

- `apps/web/src/types/agent.ts` - AgentEvent tagged union, AgentNode, AgentStatus, WsConnectionStatus types
- `apps/web/src/hooks/use-websocket.ts` - useAgentWebSocket hook with exponential backoff reconnection
- `apps/web/src/hooks/use-agent-tree.ts` - Bridge hook connecting WebSocket events to agent store
- `apps/web/src/stores/agent-store.ts` - Zustand store for agent tree and budget state management
- `apps/web/src/components/chat/agent-block.tsx` - Collapsible inline sub-agent block with streaming text
- `apps/web/src/components/chat/agent-tree-panel.tsx` - Process-manager tree view with cancel buttons
- `apps/web/src/components/chat/budget-indicator.tsx` - Budget usage bar with cost estimate and Continue/Stop
- `apps/web/src/components/chat/ws-status.tsx` - WebSocket connection status indicator dot
- `apps/web/src/components/chat/message-list.tsx` - Added inline AgentBlock rendering and agent store imports
- `apps/web/src/hooks/use-sse-chat.ts` - Extended with 9 agent event types and agent store reset

## Decisions Made

- **Native WebSocket API:** No npm dependency per research decision. Exponential backoff with 30% jitter prevents thundering herd on reconnection. Max 10 attempts then disconnected.
- **Map-based store without immer:** Zustand store uses `new Map(state.agents)` spread pattern for immutable updates. Simple enough that immer middleware adds overhead without benefit.
- **SSE event bridge pattern:** Agent events arrive via SSE (same HTTP connection as chat) and are forwarded to the Zustand store using `useAgentStore.getState().handleEvent()` (safe to call outside React render lifecycle).
- **Auto-collapse behavior:** AgentBlock components auto-collapse when agent completes and auto-expand when running, using a useEffect watching agent status. This matches the Claude Code tool-use pattern where completed steps collapse.
- **Recursive TreeNode:** AgentTreePanel uses a recursive TreeNode component with `paddingLeft = (depth + 1) * 16px` for tree indentation. Children retrieved via `getAgentChildren()` store selector.
- **Blended cost estimate:** Budget indicator uses $9/1M as a rough blended rate (mix of input and output pricing). This is a UI hint, not exact billing. Displayed as "~$0.04" format.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- All web UI agent hierarchy components are complete and buildable
- Components are ready to be wired into chat routes when orchestrator integration is complete
- AgentTreePanel and BudgetIndicator can be placed in chat header/sidebar via the useAgentTree hook
- WsStatus ready for chat header integration
- Phase 5 is now fully complete (all 8 plans executed)
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 05-agent-hierarchy-event-system*
*Completed: 2026-02-13*
