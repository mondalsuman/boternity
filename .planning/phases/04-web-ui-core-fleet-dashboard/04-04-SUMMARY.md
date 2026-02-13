---
phase: 04-web-ui-core-fleet-dashboard
plan: 04
subsystem: ui
tags: [react, sse, streaming, chat, zustand, tanstack-query]

# Dependency graph
requires:
  - phase: 04-01
    provides: REST API endpoints, SSE streaming endpoint, session CRUD handlers
  - phase: 04-02
    provides: SPA scaffold, shadcn components, sidebar layout, TanStack Router/Query
provides:
  - SSE streaming chat hook (useSSEChat) for real-time token-by-token responses
  - Chat session/message query hooks (useChatQueries) for CRUD operations
  - Chat UI store (useChatStore) for active session state
  - Session sidebar grouped by bot name
  - Message display with streaming, typing indicator, and auto-scroll
  - Auto-expanding chat input with Enter-to-send
  - Chat header with bot info and session actions
affects: [04-05-markdown-rendering, 04-06-bot-detail, 04-07-settings]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SSE via fetch + ReadableStream (not EventSource) for POST body support"
    - "Functional updater for streaming state (avoids stale closure)"
    - "Isolated StreamingMessage component to prevent list re-renders per token"
    - "Shared ChatLayout wrapper for sibling TanStack Router routes"
    - "AbortController for stop generation and cleanup on unmount"

key-files:
  created:
    - apps/web/src/hooks/use-sse-chat.ts
    - apps/web/src/hooks/use-chat-queries.ts
    - apps/web/src/stores/chat-store.ts
    - apps/web/src/components/chat/session-sidebar.tsx
    - apps/web/src/components/chat/chat-empty-state.tsx
    - apps/web/src/components/chat/chat-layout.tsx
    - apps/web/src/components/chat/message-list.tsx
    - apps/web/src/components/chat/message-bubble.tsx
    - apps/web/src/components/chat/chat-input.tsx
    - apps/web/src/components/chat/streaming-indicator.tsx
    - apps/web/src/components/chat/chat-header.tsx
  modified:
    - apps/web/src/routes/chat/index.tsx
    - apps/web/src/routes/chat/$sessionId.tsx

key-decisions:
  - "ChatLayout shared component for sibling routes: TanStack Router /chat/ and /chat/$sessionId are siblings not nested, so shared layout wraps both"
  - "fetch + ReadableStream over EventSource for SSE: enables POST with JSON body required by streaming endpoint"
  - "Functional updater setStreamedContent(prev => prev + text) to avoid stale closure during rapid token updates"
  - "Isolated StreamingMessage prevents full message list re-render on each token delta"
  - "AbortController for stop generation + unmount cleanup"
  - "Bot picker dialog for new chat creation from sidebar"

patterns-established:
  - "SSE streaming pattern: fetch POST -> ReadableStream reader -> parse SSE lines -> dispatch by event type"
  - "Chat layout pattern: shared ChatLayout wrapper with sidebar + content area for sibling routes"
  - "Streaming isolation: StreamingMessage as separate component receiving content prop"

# Metrics
duration: 5m 13s
completed: 2026-02-13
---

# Phase 4 Plan 4: Chat Interface Summary

**SSE streaming chat with session sidebar, auto-expanding input, typing indicator, and full send-stream-save lifecycle**

## Performance

- **Duration:** 5m 13s
- **Started:** 2026-02-12T23:51:58Z
- **Completed:** 2026-02-12T23:57:11Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments
- SSE streaming hook using fetch + ReadableStream with functional state updates and AbortController
- Session sidebar grouped by bot name with CRUD actions (delete, clear, new chat)
- Message display with auto-scroll, streaming indicator ("Bot is thinking..."), and isolated StreamingMessage
- Auto-expanding textarea input (1-6 lines) with Enter-to-send and stop button during streaming
- Chat header showing bot emoji, name, model badge with delete/clear confirmation dialogs
- Full send-stream-save lifecycle: user types -> POST SSE -> tokens stream live -> server saves -> refresh messages

## Task Commits

Each task was committed atomically:

1. **Task 1: SSE streaming hook + chat store + session sidebar** - `04d78d2` (feat)
2. **Task 2: Message display, chat input, streaming indicator, chat header** - `c9e2d11` (feat)

## Files Created/Modified
- `apps/web/src/hooks/use-sse-chat.ts` - SSE streaming hook via fetch + ReadableStream with AbortController
- `apps/web/src/hooks/use-chat-queries.ts` - TanStack Query hooks for sessions/messages CRUD
- `apps/web/src/stores/chat-store.ts` - Zustand store for active bot/session UI state
- `apps/web/src/components/chat/session-sidebar.tsx` - Session list grouped by bot with CRUD actions
- `apps/web/src/components/chat/chat-empty-state.tsx` - Bot grid for starting new conversations
- `apps/web/src/components/chat/chat-layout.tsx` - Shared two-panel layout for chat routes
- `apps/web/src/components/chat/message-list.tsx` - Scrollable message list with streaming support
- `apps/web/src/components/chat/message-bubble.tsx` - Individual message with alignment and timestamps
- `apps/web/src/components/chat/chat-input.tsx` - Auto-expanding textarea with send/stop buttons
- `apps/web/src/components/chat/streaming-indicator.tsx` - Animated "Bot is thinking..." dots
- `apps/web/src/components/chat/chat-header.tsx` - Bot info, model badge, session action buttons
- `apps/web/src/routes/chat/index.tsx` - Chat hub with sidebar and empty state
- `apps/web/src/routes/chat/$sessionId.tsx` - Session view wiring SSE streaming lifecycle

## Decisions Made
- **ChatLayout shared wrapper**: TanStack Router generates `/chat/` and `/chat/$sessionId` as sibling routes (not parent-child), so Outlet cannot be used. Created a shared ChatLayout component that both routes wrap around.
- **fetch + ReadableStream for SSE**: The streaming endpoint requires POST with a JSON body; EventSource only supports GET. Used fetch with ReadableStream reader and manual SSE line parsing.
- **Functional state updater**: `setStreamedContent(prev => prev + text)` is critical to avoid stale closure where rapid token deltas would overwrite each other.
- **StreamingMessage isolation**: The streaming content is rendered in a separate component to prevent the entire message list from re-rendering on every token delta (React only updates the StreamingMessage node).
- **Bot picker dialog**: New chat creation uses a dialog with active bots rather than inline selection, keeping the sidebar clean.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Created shared ChatLayout component for sibling routes**
- **Found during:** Task 1 (chat hub route)
- **Issue:** Plan specified Outlet-based parent-child layout for /chat/ and /chat/$sessionId, but TanStack Router file-based routing generates these as sibling routes under root, not nested routes
- **Fix:** Created a shared ChatLayout component that both routes use to wrap their content, providing consistent sidebar + content area
- **Files modified:** apps/web/src/components/chat/chat-layout.tsx, routes/chat/index.tsx, routes/chat/$sessionId.tsx
- **Verification:** Both routes render correctly with sidebar visible on both
- **Committed in:** 04d78d2 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary structural fix for TanStack Router routing model. No scope creep.

## Issues Encountered
None -- plan executed cleanly after the routing structure adjustment.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Chat interface fully functional for streaming conversations
- Ready for markdown rendering (Plan 05) which will enhance message display
- Session sidebar and message list support all CRUD operations
- SSE streaming pattern established for any future real-time features

## Self-Check: PASSED

---
*Phase: 04-web-ui-core-fleet-dashboard*
*Completed: 2026-02-13*
