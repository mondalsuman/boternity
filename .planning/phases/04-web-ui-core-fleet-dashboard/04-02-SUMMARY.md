---
phase: 04-web-ui-core-fleet-dashboard
plan: 02
subsystem: ui
tags: [react, vite, tanstack-router, tanstack-query, shadcn-ui, tailwind-v4, zustand, pwa, sidebar, command-palette]

# Dependency graph
requires:
  - phase: 04-web-ui-core-fleet-dashboard
    plan: 01
    provides: Backend API endpoints (SSE streaming, session CRUD, identity, stats, SPA serving)
provides:
  - React 19 app scaffolded with Vite 7, TypeScript 5.9, Tailwind v4
  - 17 shadcn/ui components installed and configured
  - Full app shell with collapsible sidebar, command palette, breadcrumbs, toaster
  - File-based routing with 10 routes (auto-code-split)
  - Typed API client with envelope unwrapping
  - TypeScript types mirroring Rust domain model
  - Theme system (dark/light/system) with Zustand persistence
  - Vite proxy for /api -> Rust backend
affects: [04-03 (chat UI), 04-04 (fleet dashboard), 04-05 (soul editor), 04-06 (bot detail), 04-07 (PWA)]

# Tech tracking
tech-stack:
  added: [react@19, vite@7, typescript@5.9, tailwindcss@4, @tanstack/react-router@1, @tanstack/react-query@5, zustand@5, shadcn-ui, lucide-react, sonner, cmdk, date-fns@4, react-markdown@9, rehype-highlight@7, remark-gfm@4, @monaco-editor/react@4.7, vite-plugin-pwa@0.21, tw-animate-css]
  patterns: [file-based routing with TanStack Router, envelope unwrapping API client, Zustand theme persistence, shadcn sidebar with collapsible icon rail]

key-files:
  created:
    - apps/web/package.json
    - apps/web/vite.config.ts
    - apps/web/tsconfig.json
    - apps/web/tsconfig.app.json
    - apps/web/index.html
    - apps/web/components.json
    - apps/web/src/main.tsx
    - apps/web/src/index.css
    - apps/web/src/lib/api-client.ts
    - apps/web/src/lib/query-client.ts
    - apps/web/src/lib/utils.ts
    - apps/web/src/types/api.ts
    - apps/web/src/types/bot.ts
    - apps/web/src/types/chat.ts
    - apps/web/src/types/soul.ts
    - apps/web/src/stores/theme-store.ts
    - apps/web/src/stores/sidebar-store.ts
    - apps/web/src/routes/__root.tsx
    - apps/web/src/routes/index.tsx
    - apps/web/src/routes/settings.tsx
    - apps/web/src/routes/bots/$botId/route.tsx
    - apps/web/src/routes/bots/$botId/index.tsx
    - apps/web/src/routes/bots/$botId/chat.tsx
    - apps/web/src/routes/bots/$botId/soul.tsx
    - apps/web/src/routes/bots/$botId/settings.tsx
    - apps/web/src/routes/chat/index.tsx
    - apps/web/src/routes/chat/$sessionId.tsx
    - apps/web/src/components/layout/app-sidebar.tsx
    - apps/web/src/components/layout/command-palette.tsx
    - apps/web/src/components/layout/breadcrumbs.tsx
  modified:
    - package.json
    - .gitignore
    - apps/web/src/components/ui/sonner.tsx

key-decisions:
  - "Sonner component rewritten to use Zustand theme store instead of next-themes (avoiding unnecessary dependency)"
  - "Dark theme as :root default with .light class override (not .dark class, matches user decision for dark-first)"
  - "Bot detail uses TanStack Router layout route (route.tsx) for shared tab navigation across child routes"
  - "SidebarProvider wraps entire app for consistent sidebar state across all routes"
  - "TooltipProvider at root level for sidebar tooltip support on collapsed rail"
  - "TanStack Router/Query devtools lazy-loaded only in development mode"

patterns-established:
  - "API client pattern: apiFetch<T>(path) -> typed data with envelope unwrapping and ApiError class"
  - "Route layout pattern: route.tsx for shared layout with Outlet, index.tsx for default tab content"
  - "Theme pattern: Zustand persist store + ThemeEffect component toggling class on documentElement"
  - "Sidebar nav pattern: shadcn Sidebar with collapsible='icon', SidebarMenuButton with tooltip, SidebarMenuSub for inline items"

# Metrics
duration: 8min
completed: 2026-02-13
---

# Phase 4 Plan 02: React App Shell Summary

**React 19 + Vite 7 app with collapsible sidebar, Cmd+K command palette, file-based routing (10 routes), dark theme, typed API client, and 17 shadcn/ui components**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-12T23:38:40Z
- **Completed:** 2026-02-12T23:46:30Z
- **Tasks:** 2/2
- **Files modified:** 56

## Accomplishments
- Complete React app scaffolded with Vite 7, TanStack Router, TanStack Query, Zustand, and Tailwind v4
- 17 shadcn/ui components installed (sidebar, command, card, tabs, dialog, sheet, breadcrumb, etc.)
- Full app shell: collapsible sidebar (4 sections with inline bot list), command palette (Cmd+K), breadcrumbs, Sonner toaster
- 10 file-based routes with auto-code-splitting: dashboard, settings, bot detail (4 tabs), chat hub, chat session
- Typed API client unwrapping ApiResponse envelopes from the Rust backend

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold React app with Vite, install all dependencies, configure build tooling** - `18ecf50` (feat)
2. **Task 2: App shell -- sidebar, command palette, breadcrumbs, theme, routing** - `3802fc3` (feat)

## Files Created/Modified
- `apps/web/package.json` - Web app dependencies and scripts (React 19, Vite 7, TanStack, Zustand, shadcn deps)
- `apps/web/vite.config.ts` - Vite configuration with TanStack Router plugin, Tailwind v4, PWA, /api proxy
- `apps/web/tsconfig.json` / `tsconfig.app.json` - TypeScript config with path aliases and strict mode
- `apps/web/index.html` - Entry HTML with dark class default
- `apps/web/components.json` - shadcn/ui configuration for Tailwind v4
- `apps/web/src/main.tsx` - App entry: QueryClientProvider + RouterProvider
- `apps/web/src/index.css` - Tailwind v4 import + shadcn CSS variables (dark default, light override)
- `apps/web/src/lib/api-client.ts` - Typed fetch wrapper with envelope unwrapping and ApiError
- `apps/web/src/lib/query-client.ts` - TanStack Query client with 10s staleTime
- `apps/web/src/lib/utils.ts` - cn() utility for class merging
- `apps/web/src/types/api.ts` - ApiEnvelope, ApiMeta, ApiErrorDetail matching Rust response.rs
- `apps/web/src/types/bot.ts` - Bot, BotStatus, CreateBotRequest, UpdateBotRequest
- `apps/web/src/types/chat.ts` - ChatSession, ChatMessage, StreamEvent types
- `apps/web/src/types/soul.ts` - Soul, SoulVersion, IdentityFile types
- `apps/web/src/stores/theme-store.ts` - Zustand persisted theme store (dark default)
- `apps/web/src/stores/sidebar-store.ts` - Zustand sidebar collapsed state
- `apps/web/src/routes/__root.tsx` - Root layout: SidebarProvider, TooltipProvider, Toaster, CommandPalette, ThemeEffect, devtools
- `apps/web/src/routes/index.tsx` - Dashboard page with skeleton placeholders
- `apps/web/src/routes/settings.tsx` - Settings page with theme toggle and API URL config
- `apps/web/src/routes/bots/$botId/route.tsx` - Bot detail layout with tabs (Overview/Chat/Soul/Settings)
- `apps/web/src/routes/bots/$botId/index.tsx` - Bot overview tab placeholder
- `apps/web/src/routes/bots/$botId/chat.tsx` - Bot chat tab placeholder
- `apps/web/src/routes/bots/$botId/soul.tsx` - Soul editor tab placeholder
- `apps/web/src/routes/bots/$botId/settings.tsx` - Bot settings tab placeholder
- `apps/web/src/routes/chat/index.tsx` - Chat hub page placeholder
- `apps/web/src/routes/chat/$sessionId.tsx` - Chat session page placeholder
- `apps/web/src/components/layout/app-sidebar.tsx` - Collapsible sidebar with 4 sections, inline bot list
- `apps/web/src/components/layout/command-palette.tsx` - Cmd+K palette with navigation, bots, actions
- `apps/web/src/components/layout/breadcrumbs.tsx` - Route-aware breadcrumbs from TanStack Router matches
- `apps/web/src/components/ui/sonner.tsx` - Modified to use Zustand theme instead of next-themes
- `apps/web/src/components/ui/*.tsx` - 17 shadcn/ui components (sidebar, command, card, tabs, dialog, etc.)

## Decisions Made
- Used Zustand theme store for Sonner component instead of next-themes dependency (simpler, no Next.js dep for a Vite app)
- Dark theme set as :root default in CSS, with .light class override (not .dark class) to match "dark by default" decision
- Bot detail page uses TanStack Router's layout route pattern (route.tsx) for shared tab navigation that persists across child routes
- SidebarProvider wraps entire app at root level so sidebar state is consistent across all route navigations
- TooltipProvider added at root level to support sidebar tooltip behavior when collapsed to icon rail
- Devtools (Router + Query) lazy-loaded with dynamic import and only rendered in development mode

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Installed tw-animate-css dependency**
- **Found during:** Task 1 (build verification)
- **Issue:** shadcn init added `@import "tw-animate-css"` to index.css but didn't install the npm package
- **Fix:** `pnpm add tw-animate-css`
- **Files modified:** apps/web/package.json
- **Verification:** Build succeeds
- **Committed in:** 18ecf50 (part of Task 1 commit)

**2. [Rule 1 - Bug] Fixed Sonner toaster using next-themes**
- **Found during:** Task 2 (root layout assembly)
- **Issue:** shadcn's Sonner component imports `useTheme` from `next-themes`, which is a Next.js dependency we don't use
- **Fix:** Rewrote sonner.tsx to import `useThemeStore` from our Zustand store instead
- **Files modified:** apps/web/src/components/ui/sonner.tsx
- **Verification:** Build succeeds, Toaster renders correctly
- **Committed in:** 3802fc3 (part of Task 2 commit)

**3. [Rule 3 - Blocking] Added dist/ to .gitignore**
- **Found during:** Task 2 (post-build verification)
- **Issue:** Vite build output directory `dist/` was not in .gitignore, would be tracked
- **Fix:** Added `dist/` to .gitignore
- **Files modified:** .gitignore
- **Verification:** `git status` no longer shows dist/ as untracked
- **Committed in:** 3802fc3 (part of Task 2 commit)

**4. [Rule 3 - Blocking] Added pnpm onlyBuiltDependencies for esbuild**
- **Found during:** Task 1 (dependency installation)
- **Issue:** pnpm prompted interactively for esbuild build script approval, blocking CI/automation
- **Fix:** Added `pnpm.onlyBuiltDependencies: ["esbuild"]` to root package.json
- **Files modified:** package.json
- **Verification:** `pnpm install` runs without interactive prompts
- **Committed in:** 18ecf50 (part of Task 1 commit)

---

**Total deviations:** 4 auto-fixed (1 bug, 3 blocking)
**Impact on plan:** All auto-fixes necessary for build correctness and CI compatibility. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Complete app shell ready for feature development
- All shadcn components installed for fleet dashboard (cards, badges, dropdowns), chat (scroll area, skeleton), soul editor (tabs, dialog)
- API client and TypeScript types ready for data fetching
- Route skeletons in place -- future plans fill in the actual UI content
- Vite proxy configured for /api -> localhost:3000 backend
- No blockers for plans 04-03 (chat UI), 04-04 (fleet dashboard), 04-05 (soul editor)

## Self-Check: PASSED

---
*Phase: 04-web-ui-core-fleet-dashboard*
*Completed: 2026-02-13*
