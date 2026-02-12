# Phase 4: Web UI Core + Fleet Dashboard - Research

**Researched:** 2026-02-12
**Domain:** React SPA with Vite, ShadCN UI, TanStack Router/Query, SSE streaming chat, PWA
**Confidence:** HIGH

## Summary

Phase 4 introduces a React single-page application that communicates with the existing Rust/Axum backend via REST API. The frontend lives in the existing Turborepo monorepo at `apps/web` (pnpm workspace already configured for `apps/*`) and uses Vite 7 as the build tool, React 19 for the UI framework, TanStack Router for type-safe file-based routing, TanStack Query v5 for server state management with stale-while-revalidate, Zustand for client-side state, and shadcn/ui (Radix-based) for the component system with Tailwind CSS v4.

The three major UI features are: (1) a fleet dashboard showing all bots with status/activity, (2) a chat interface with real-time SSE streaming and parallel session support, and (3) a soul/config editor with Monaco-powered editing and diff view and version history. The backend already has the `StreamEvent` enum with tagged JSON serialization (`{"type":"text_delta","index":0,"text":"..."}`) -- the web API layer needs SSE endpoints that forward these events. The frontend consumes them via `fetch()` + `ReadableStream` (POST-based SSE, since native EventSource only supports GET).

The backend is MISSING several endpoints the frontend requires: chat session CRUD, SSE streaming, IDENTITY.md/USER.md file endpoints, and dashboard stats. These must be added as part of this phase.

**Primary recommendation:** Use the TanStack ecosystem (Router + Query) as the backbone, shadcn/ui for all base components, `@monaco-editor/react` for the soul editor and diff viewer, and `fetch()` + `ReadableStream` for SSE streaming chat. Keep the architecture simple -- this is a single-user local app, so no auth beyond API key, no SSR, no complex caching.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Fleet Dashboard
- Card grid layout (responsive, like Linear's project cards)
- Rich cards showing: bot name, emoji avatar (user-assignable or auto-generated from name), model, color-coded status badge (green/yellow/red for Active/Disabled/Archived), last activity time, session count, first line of SOUL.md as personality snippet
- Stats bar at the top: total bots, active sessions, total conversations -- stats are clickable to filter the grid (clicking "3 Active" filters to active bots)
- Quick actions on each card: "Chat" button + overflow menu (edit, disable, delete)
- Search box to filter by name + sort dropdown (name, last activity, status)
- CTA-focused empty state: illustration + "Create your first bot" with prominent button
- "Create Bot" accessible via FAB on mobile + header button on desktop

#### Chat Experience
- Session sidebar list (left panel, like Slack channels) -- sessions grouped under bot name headers
- Auto-expanding textarea for message input (grows up to ~6 lines, then scrolls)
- Streaming feedback: brief "Bot is thinking..." indicator, then tokens appear live as they stream
- "New chat" button for same bot + "+" to pick a different bot -- covers both use cases
- Full markdown rendering: headers, bold, lists, code blocks with syntax highlighting and copy button, tables
- "Stop generating" button during streaming responses
- Relative timestamps on messages ("2m ago", "yesterday")
- Delete session (removes entirely) + clear session (empties messages, keeps session)
- Chat header shows bot name, emoji, and model badge (e.g., "Claude Sonnet")
- Empty chat state: grid of available bots to start chatting with (functional empty state)

#### Soul Editor
- Lives as a tab on the bot detail page (alongside Chat, Overview, Settings)
- Supports editing all three files: SOUL.md, IDENTITY.md, USER.md (tabs or dropdown to switch)
- IDENTITY.md editing: form view by default (model dropdown, temperature slider, max_tokens input) with toggle to raw text editor
- Auto-save with debounce (saves after 2s of inactivity)
- Auto-generated version labels ("Edited Feb 12, 2026 at 3:45 PM") -- no commit message prompt
- Version history in a collapsible right side panel with visual timeline (vertical dots + connecting lines)
- Side-by-side diff view for comparing versions (read-only)
- Split preview: editor left, rendered markdown preview right
- Rollback via confirmation dialog: select version -> "Restore this version?" with preview -> confirm

#### App Shell & Navigation
- Sidebar with four sections: Dashboard, Bots, Chat, Settings
- "Bots" section shows last 5 bots inline in sidebar for quick access
- Sidebar collapses to icon-only rail (like VS Code)
- Mobile: sidebar becomes hamburger-triggered drawer
- Bot detail page: tab layout (Overview | Chat | Soul | Settings)
- Breadcrumbs showing navigation path (Dashboard > Bot Name > Soul)
- Dark theme by default with light mode toggle
- Global command palette (Cmd+K): search bots, navigate pages, trigger actions (new bot, new chat)
- Toast notifications (bottom-right) for all actions (bot created, soul saved, session deleted)
- Settings page: theme toggle + backend API connection config

### Claude's Discretion
- Grid pagination/infinite scroll strategy for bot list (10+ bots)
- Exact spacing, typography, and animation details
- Loading skeleton designs
- Error state handling and retry UX
- Code block language detection
- Monaco editor configuration details
- PWA icon design and manifest details
- Exact responsive breakpoints

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19.x | UI framework | Current stable, concurrent features, ref-as-prop |
| Vite | 7.x | Build tool + dev server | Current stable (7.3.x), fastest DX, native ESM |
| @vitejs/plugin-react | latest | React plugin for Vite | Official plugin |
| TypeScript | 5.7+ | Type safety | Required by TanStack Router for type-safe routes |
| Tailwind CSS | 4.x | Utility-first CSS | First-party Vite plugin, CSS-only config, used by shadcn/ui |
| @tailwindcss/vite | latest | Tailwind Vite plugin | Replaces PostCSS setup, better performance |
| shadcn/ui | latest (Radix variant) | Component library | Copy-paste components, Radix primitives, Tailwind v4, React 19 |
| TanStack Router | 1.x | Type-safe file-based routing | Best type safety for SPAs, auto code-splitting, search param management |
| TanStack Query | 5.x | Server state management | Built-in SWR, devtools, mutations, background refetch |
| Zustand | 5.x | Client state management | ~3KB, minimal boilerplate, React 19 compatible |

### Chat & Streaming
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| react-markdown | 9.x+ | Markdown rendering | Chat message display |
| remark-gfm | 4.x | GitHub Flavored Markdown | Tables, strikethrough, task lists in chat |
| rehype-highlight | 7.x | Code syntax highlighting | Code blocks in chat messages (uses highlight.js, fast and lightweight) |
| date-fns | 4.x | Date formatting | Relative timestamps ("2m ago", "yesterday") via formatDistanceToNow |

### Soul Editor & Diff
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| @monaco-editor/react | 4.7.x | Code/markdown editor | Soul editor with syntax highlighting, markdown preview split |
| @monaco-editor/react DiffEditor | (included) | Side-by-side diff | Version history diff comparison (read-only) |

### App Shell
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| sonner | 2.x | Toast notifications | All action feedback (integrated with shadcn Sonner component) |
| cmdk | 1.x | Command palette | Global Cmd+K (integrated with shadcn Command component) |
| lucide-react | latest | Icons | ShadCN UI uses Lucide icons throughout |

### PWA
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| vite-plugin-pwa | 0.21+ | PWA generation | Service worker, manifest, offline support, auto-update |

### Dev Tooling
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| @tanstack/react-query-devtools | latest | Query inspector | Development debugging |
| @tanstack/router-devtools | latest | Route inspector | Development debugging |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| TanStack Router | React Router v7 | RR v7 has closed the gap but TanStack Router has superior compile-time type safety for route params and search params -- critical for dashboard filter/sort state |
| Zustand | Jotai | Jotai's atom-based model is better for fine-grained reactivity, but Zustand's single-store model is simpler for the ~3 stores needed (theme, sidebar, chat UI) |
| rehype-highlight | react-syntax-highlighter | react-syntax-highlighter wraps Prism/highlight.js as React components (heavier); rehype-highlight integrates directly into the react-markdown pipeline via rehype plugin (lighter, no extra React wrapper) |
| rehype-highlight | rehype-pretty-code (shiki) | shiki produces better highlighting but is significantly heavier; highlight.js via rehype-highlight is fast and sufficient for chat |
| fetch + ReadableStream | @microsoft/fetch-event-source | fetch-event-source is battle-tested from Microsoft but hasn't been updated in 2+ years; native fetch with ReadableStream is zero-dependency and provides the same capability for POST-based SSE |
| Monaco DiffEditor | react-diff-viewer-continued | Monaco provides BOTH editor and diff in one package; react-diff-viewer is display-only. Since Monaco is already loaded for editing, reuse it for diffs too |
| No form library | react-hook-form + zod | For bot creation/edit forms, standard React state is sufficient given the small number of fields. Add react-hook-form only if forms grow complex |
| Custom ThemeProvider | next-themes | next-themes is Next.js-specific. A simple Zustand store + useEffect to toggle `dark` class on `<html>` works for Vite SPAs |

**Installation:**
```bash
# From project root -- create the web app
cd apps/web

# Core framework
pnpm add react react-dom @tanstack/react-router @tanstack/react-query zustand

# Build tooling
pnpm add -D vite @vitejs/plugin-react typescript @types/react @types/react-dom @types/node
pnpm add -D @tanstack/router-plugin tailwindcss @tailwindcss/vite
pnpm add -D vite-plugin-pwa
pnpm add -D @tanstack/react-query-devtools @tanstack/router-devtools

# Initialize shadcn/ui (interactive -- select Neutral base color)
pnpm dlx shadcn@latest init

# Chat & streaming
pnpm add react-markdown remark-gfm rehype-highlight date-fns

# Soul editor
pnpm add @monaco-editor/react

# Icons (shadcn dependency)
pnpm add lucide-react
```

## Architecture Patterns

### Recommended Project Structure
```
apps/web/
├── index.html
├── package.json
├── tsconfig.json
├── tsconfig.app.json
├── vite.config.ts
├── components.json              # shadcn/ui config
├── public/
│   ├── icons/                   # PWA icons (192x192, 512x512)
│   └── offline.html             # Offline fallback page
├── src/
│   ├── main.tsx                 # Entry point (QueryClient, Router)
│   ├── index.css                # @import 'tailwindcss' + CSS vars + shadcn theme
│   ├── routeTree.gen.ts         # Auto-generated by TanStack Router plugin
│   ├── routes/
│   │   ├── __root.tsx           # Root layout (sidebar, command palette, toaster, theme)
│   │   ├── index.tsx            # Dashboard (fleet overview)
│   │   ├── bots/
│   │   │   ├── index.tsx        # Bot list (may redirect to dashboard)
│   │   │   └── $botId/
│   │   │       ├── index.tsx    # Bot detail (overview tab)
│   │   │       ├── chat.tsx     # Bot-specific chat tab
│   │   │       ├── soul.tsx     # Soul editor tab
│   │   │       └── settings.tsx # Bot settings tab
│   │   ├── chat/
│   │   │   ├── index.tsx        # Chat hub (all sessions, sidebar + chat area)
│   │   │   └── $sessionId.tsx   # Specific session
│   │   └── settings.tsx         # App settings (theme, API config)
│   ├── components/
│   │   ├── ui/                  # shadcn/ui components (auto-generated by CLI)
│   │   ├── layout/
│   │   │   ├── app-sidebar.tsx
│   │   │   ├── sidebar-nav.tsx
│   │   │   ├── breadcrumbs.tsx
│   │   │   └── command-palette.tsx
│   │   ├── dashboard/
│   │   │   ├── bot-card.tsx
│   │   │   ├── bot-grid.tsx
│   │   │   ├── stats-bar.tsx
│   │   │   └── empty-state.tsx
│   │   ├── chat/
│   │   │   ├── message-list.tsx
│   │   │   ├── message-bubble.tsx
│   │   │   ├── chat-input.tsx
│   │   │   ├── session-sidebar.tsx
│   │   │   ├── streaming-indicator.tsx
│   │   │   └── markdown-renderer.tsx
│   │   └── soul/
│   │       ├── soul-editor.tsx
│   │       ├── identity-form.tsx
│   │       ├── version-timeline.tsx
│   │       ├── diff-viewer.tsx
│   │       └── markdown-preview.tsx
│   ├── hooks/
│   │   ├── use-sse-chat.ts      # SSE streaming hook (fetch + ReadableStream)
│   │   ├── use-bot-queries.ts   # TanStack Query hooks for bot API
│   │   ├── use-soul-queries.ts  # TanStack Query hooks for soul API
│   │   ├── use-chat-queries.ts  # TanStack Query hooks for session/message API
│   │   ├── use-debounce.ts      # Auto-save debounce for soul editor
│   │   └── use-keyboard.ts      # Cmd+K and keyboard shortcuts
│   ├── lib/
│   │   ├── api-client.ts        # Typed fetch wrapper with envelope unwrapping
│   │   ├── query-client.ts      # TanStack Query client configuration
│   │   └── utils.ts             # ShadCN cn() utility (auto-generated)
│   ├── stores/
│   │   ├── theme-store.ts       # Zustand: dark/light mode state (persisted)
│   │   ├── sidebar-store.ts     # Zustand: sidebar collapsed state
│   │   └── chat-store.ts        # Zustand: active sessions, streaming buffers
│   └── types/
│       ├── api.ts               # API response envelope types
│       ├── bot.ts               # Bot, BotStatus, BotCategory (mirrors Rust types)
│       ├── chat.ts              # ChatSession, ChatMessage, SessionStatus
│       └── soul.ts              # Soul, SoulVersion, SoulFrontmatter
```

### Pattern 1: Typed API Client with Envelope Unwrapping

The Rust backend wraps all responses in `ApiResponse<T>` envelopes. The frontend API client unwraps these.

**What:** A thin typed fetch wrapper that handles `{ data, meta, errors, _links }`.
**When to use:** Every REST API call.
**Example:**
```typescript
// src/lib/api-client.ts -- matches crates/boternity-api/src/http/response.rs
interface ApiEnvelope<T> {
  data?: T;
  meta: { request_id: string; timestamp: string; response_time_ms: number };
  errors?: Array<{ code: string; message: string; details?: unknown }>;
  _links?: Record<string, string>;
}

class ApiError extends Error {
  constructor(public code: string, message: string) {
    super(message);
  }
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api/v1${path}`, {
    ...init,
    headers: { 'Content-Type': 'application/json', ...init?.headers },
  });
  const envelope: ApiEnvelope<T> = await res.json();
  if (envelope.errors && envelope.errors.length > 0) {
    throw new ApiError(envelope.errors[0].code, envelope.errors[0].message);
  }
  return envelope.data as T;
}
```

### Pattern 2: SSE Streaming Chat with fetch + ReadableStream

The backend exposes an SSE endpoint that forwards `StreamEvent` as JSON. Use `fetch()` with `ReadableStream` since native EventSource only supports GET and we need POST with a body.

**What:** POST to `/api/v1/bots/{id}/chat/stream` with message payload, parse SSE events from response stream.
**When to use:** All chat interactions.
**Example:**
```typescript
// src/hooks/use-sse-chat.ts
export function useSSEChat(sessionId: string) {
  const [isStreaming, setIsStreaming] = useState(false);
  const [streamedContent, setStreamedContent] = useState('');
  const abortRef = useRef<AbortController | null>(null);

  const sendMessage = useCallback(async (botId: string, content: string) => {
    setIsStreaming(true);
    setStreamedContent('');
    abortRef.current = new AbortController();

    try {
      const res = await fetch(`/api/v1/bots/${botId}/chat/stream`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ session_id: sessionId, message: content }),
        signal: abortRef.current.signal,
      });

      const reader = res.body!.getReader();
      const decoder = new TextDecoder();
      let buffer = '';

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });

        // Parse SSE lines from buffer
        const lines = buffer.split('\n');
        buffer = lines.pop()!; // Keep incomplete line in buffer
        for (const line of lines) {
          if (line.startsWith('data: ')) {
            const event = JSON.parse(line.slice(6));
            if (event.type === 'text_delta') {
              // CRITICAL: use functional updater to avoid stale closure
              setStreamedContent(prev => prev + event.text);
            } else if (event.type === 'done') {
              break;
            }
          }
        }
      }
    } catch (err) {
      if ((err as Error).name !== 'AbortError') throw err;
    } finally {
      setIsStreaming(false);
    }
  }, [sessionId]);

  const stopGeneration = useCallback(() => {
    abortRef.current?.abort();
    setIsStreaming(false);
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => abortRef.current?.abort();
  }, []);

  return { sendMessage, stopGeneration, streamedContent, isStreaming };
}
```

### Pattern 3: TanStack Query with Stale-While-Revalidate

**What:** Query hooks with per-resource stale times.
**When to use:** All REST API data fetching.
**Example:**
```typescript
// src/hooks/use-bot-queries.ts
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiFetch } from '@/lib/api-client';
import { toast } from 'sonner';

export function useBots(filter?: { status?: string; sort?: string }) {
  const params = new URLSearchParams();
  if (filter?.status) params.set('status', filter.status);
  if (filter?.sort) params.set('sort', filter.sort);
  const qs = params.toString();
  return useQuery({
    queryKey: ['bots', filter],
    queryFn: () => apiFetch<Bot[]>(`/bots${qs ? '?' + qs : ''}`),
    staleTime: 10_000,  // 10s - bot status can change
  });
}

export function useCreateBot() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: CreateBotRequest) =>
      apiFetch<Bot>('/bots', { method: 'POST', body: JSON.stringify(data) }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bots'] });
      toast.success('Bot created');
    },
  });
}

export function useDeleteBot() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) =>
      apiFetch<{ deleted: boolean }>(`/bots/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bots'] });
      toast.success('Bot deleted');
    },
  });
}
```

### Pattern 4: Dark Mode with Zustand (Vite/non-Next.js)

**What:** Custom ThemeProvider using Zustand + class toggle on `<html>`.
**When to use:** Theme toggle (dark default, light option).
**Example:**
```typescript
// src/stores/theme-store.ts
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

type Theme = 'dark' | 'light' | 'system';

interface ThemeStore {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

export const useThemeStore = create<ThemeStore>()(
  persist(
    (set) => ({
      theme: 'dark', // dark by default per user decision
      setTheme: (theme) => set({ theme }),
    }),
    { name: 'boternity-theme' }
  )
);

// In __root.tsx, apply class to <html>
function ThemeEffect() {
  const theme = useThemeStore(s => s.theme);
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    if (theme === 'system') {
      const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      root.classList.add(systemDark ? 'dark' : 'light');
    } else {
      root.classList.add(theme);
    }
  }, [theme]);
  return null;
}
```

### Pattern 5: Vite Proxy for Backend API

**What:** Vite dev server proxies `/api` requests to Axum backend.
**When to use:** Development setup.
**Example:**
```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { TanStackRouterVite } from '@tanstack/router-plugin/vite';
import { VitePWA } from 'vite-plugin-pwa';
import path from 'path';

export default defineConfig({
  plugins: [
    // CRITICAL: TanStack Router plugin MUST come before react plugin
    TanStackRouterVite({ target: 'react', autoCodeSplitting: true }),
    tailwindcss(),
    react(),
    VitePWA({ registerType: 'autoUpdate' }),
  ],
  resolve: {
    alias: { '@': path.resolve(__dirname, './src') },
  },
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:3000', // Rust backend
        changeOrigin: true,
      },
    },
  },
});
```

### Pattern 6: Auto-save with Debounce

**What:** Debounced mutation for soul editor auto-save (2s after inactivity).
**When to use:** Soul/IDENTITY/USER.md editing.
**Example:**
```typescript
// src/hooks/use-debounce.ts
export function useDebouncedCallback<T extends (...args: any[]) => any>(
  callback: T,
  delay: number
) {
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>();
  useEffect(() => () => { if (timeoutRef.current) clearTimeout(timeoutRef.current); }, []);
  return useCallback((...args: Parameters<T>) => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => callback(...args), delay);
  }, [callback, delay]);
}
```

### Anti-Patterns to Avoid
- **Do NOT use native EventSource for chat:** It only supports GET requests. Chat messages need POST with a body.
- **Do NOT store server data in Zustand:** Use TanStack Query for all REST API data (bots, sessions, messages, souls). Zustand is only for ephemeral client state (active sessions, UI preferences, theme).
- **Do NOT use `useEffect` + `fetch` for data loading:** TanStack Query handles caching, deduplication, retries, and background refetch.
- **Do NOT use `tailwind.config.js`:** Tailwind v4 uses CSS-based config (`@import 'tailwindcss'`), not JS config files. Custom theme values go in CSS with `@theme`.
- **Do NOT hand-roll a sidebar:** Use shadcn/ui's Sidebar component which handles responsive, collapse, keyboard shortcuts, tooltips, and mobile sheet.
- **Do NOT create a monolithic chat component:** Split into MessageList, MessageInput, SessionSidebar, and StreamingMessage. The StreamingMessage must be isolated so token deltas only re-render that component, not the entire list.
- **Do NOT use `setStreamedContent(content + delta)`:** This captures a stale closure. Always use the functional updater: `setStreamedContent(prev => prev + delta)`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Component library | Custom buttons, inputs, dialogs | shadcn/ui + Radix | Accessibility, keyboard nav, focus management are extremely hard to get right |
| Sidebar navigation | Custom collapsible sidebar | shadcn/ui Sidebar (collapsible="icon") | Handles responsive, collapse modes, keyboard shortcuts, tooltips, mobile sheet |
| Command palette | Custom search modal | shadcn/ui Command (cmdk) | Fuzzy search, keyboard nav, grouping, empty states |
| Toast notifications | Custom toast system | shadcn Sonner | Positioning, stacking, animations, dismiss behavior |
| Diff viewer | Custom line-by-line diff | Monaco DiffEditor | Word-level diff, syntax highlighting, side-by-side + inline modes built-in |
| Markdown rendering | Custom markdown parser | react-markdown + remark-gfm + rehype-highlight | XSS-safe, extensible, code highlighting built-in |
| SSE parsing | Custom fetch + TextDecoder parser | Pattern in use-sse-chat.ts above | Manual parsing is fine here (simple SSE protocol), but use AbortController for cleanup |
| Data caching | Custom cache with timestamps | TanStack Query | Built-in SWR with configurable staleTime, background refetch, deduplication |
| Route code-splitting | Manual React.lazy + Suspense | TanStack Router autoCodeSplitting | File-based routing auto-splits by route, zero configuration |
| PWA manifest + SW | Manual service worker | vite-plugin-pwa | Auto-generates manifest, handles SW registration, update prompts, workbox strategies |
| Date formatting | Custom "2m ago" logic | date-fns formatDistanceToNow | Edge cases: timezones, locale, boundary conditions |

**Key insight:** ShadCN UI provides almost every component needed out of the box: Sidebar, Command, Card, Badge, Tabs, Dialog, Sheet, Skeleton, Sonner, Breadcrumb, Dropdown Menu, Avatar, Button, Input, Textarea, Select, Slider, Separator, Scroll Area, Toggle, Tooltip, Resizable, Collapsible. Use them.

## Common Pitfalls

### Pitfall 1: SSE with POST Requests
**What goes wrong:** Native `EventSource` API only supports GET requests. Chat streaming needs POST (to send user message body).
**Why it happens:** Developers reach for `new EventSource(url)` and discover they cannot send a request body.
**How to avoid:** Use `fetch()` with `ReadableStream` to manually parse SSE events from the response body. This is the standard pattern for AI chat streaming (used by ChatGPT, Claude web, etc.).
**Warning signs:** Seeing `new EventSource()` in chat code; GET endpoints for chat.

### Pitfall 2: SSE Connection Lifecycle
**What goes wrong:** SSE connections stay open after navigating away from chat, causing memory leaks.
**Why it happens:** The fetch stream isn't aborted when the component unmounts.
**How to avoid:** Create an `AbortController` per streaming request. Abort in useEffect cleanup. Also abort when user clicks "Stop generating".
**Warning signs:** Multiple SSE connections in DevTools Network tab; phantom responses.

### Pitfall 3: TanStack Router Plugin Order
**What goes wrong:** Routes aren't generated or HMR breaks.
**Why it happens:** The TanStack Router plugin MUST come before `@vitejs/plugin-react` in the Vite plugins array.
**How to avoid:** Order: `[TanStackRouterVite(), tailwindcss(), react(), VitePWA()]`.
**Warning signs:** Missing `routeTree.gen.ts`; type errors on route paths.

### Pitfall 4: Streaming Message Re-renders
**What goes wrong:** Each token delta causes the entire chat message list to re-render hundreds of times per second.
**Why it happens:** Streaming content is stored in shared state; React re-renders all consumers.
**How to avoid:** Isolate the streaming message in its own component with its own state. Use `React.memo` on the static message list. Consider buffering deltas with `requestAnimationFrame` and flushing at 60fps.
**Warning signs:** Visible jank during streaming; React DevTools shows excessive re-renders.

### Pitfall 5: Stale Closure in Streaming State
**What goes wrong:** `setStreamedContent(streamedContent + delta)` shows only the last token.
**Why it happens:** The closure captures a stale `streamedContent` value.
**How to avoid:** Always use functional updater: `setStreamedContent(prev => prev + delta)`.
**Warning signs:** Only last token visible; content resets on each delta.

### Pitfall 6: Tailwind v4 Configuration Confusion
**What goes wrong:** Developers create `tailwind.config.js` or use `@tailwind base/components/utilities` directives.
**Why it happens:** Most tutorials reference Tailwind v3 patterns.
**How to avoid:** Tailwind v4 uses `@import 'tailwindcss'` in CSS. No JS config file. Custom theme values go in CSS with `@theme`. ShadCN's `shadcn@latest init` handles this correctly for new projects.
**Warning signs:** Build errors about missing config; `@tailwind` directives not working.

### Pitfall 7: ShadCN Dark Mode with Tailwind v4
**What goes wrong:** Dark mode colors don't apply or look wrong.
**Why it happens:** Tailwind v4 changed how CSS variables and layers work. `:root` and `.dark` blocks must NOT be inside `@layer base`.
**How to avoid:** Follow ShadCN's Tailwind v4 documentation. Use `:root[class~="dark"]` selector. The `shadcn@latest init` handles this correctly.
**Warning signs:** Colors not changing on theme toggle; Tailwind warnings about layers.

### Pitfall 8: Monaco Editor Bundle Size
**What goes wrong:** Monaco is ~2MB+. Loading eagerly blocks initial page render.
**Why it happens:** Monaco includes all language grammars, completions, and workers.
**How to avoid:** Only load Monaco on the soul editor route (TanStack Router auto code-splits). Configure Monaco to only load markdown and yaml languages. The `@monaco-editor/react` wrapper lazy-loads from CDN by default.
**Warning signs:** Bundle > 500KB; slow first load; Lighthouse warnings.

### Pitfall 9: Missing Backend Endpoints
**What goes wrong:** Frontend calls endpoints that don't exist in the Axum API.
**Why it happens:** The current backend has bot and soul CRUD only. It is MISSING: chat session HTTP handlers, SSE streaming endpoint, IDENTITY.md/USER.md CRUD, dashboard stats.
**How to avoid:** Backend endpoint tasks MUST come before frontend feature tasks that depend on them.
**Warning signs:** 404 errors from the frontend; mocking endpoints that never get implemented.

### Pitfall 10: PWA Service Worker Caching API Responses
**What goes wrong:** Service worker caches API responses, showing stale data after mutations.
**Why it happens:** Workbox's default `generateSW` precaches everything.
**How to avoid:** Exclude `/api/` from precaching. Use `NetworkFirst` or `NetworkOnly` strategy for API routes. The SWR pattern belongs in TanStack Query, not the service worker.
**Warning signs:** Data not updating after create/edit/delete; works after hard refresh.

### Pitfall 11: SPA Client-Side Routing in Production
**What goes wrong:** Refreshing on `/bots/123/soul` returns 404 from Axum.
**Why it happens:** Axum doesn't know about client-side routes.
**How to avoid:** Use `tower_http::services::ServeDir` with `.fallback(ServeFile::new("dist/index.html"))` for production. Vite dev server handles this automatically during development.
**Warning signs:** 404 on page refresh; routes only work via client-side navigation.

## Backend Endpoints Inventory

### Existing Endpoints (from crates/boternity-api/src/http/router.rs)
```
POST   /api/v1/bots                              -- Create bot
GET    /api/v1/bots                              -- List bots (filter: status, category, sort, limit, offset)
GET    /api/v1/bots/{id}                         -- Get bot by ID or slug
PUT    /api/v1/bots/{id}                         -- Update bot
DELETE /api/v1/bots/{id}                         -- Delete bot
POST   /api/v1/bots/{id}/clone                   -- Clone bot

GET    /api/v1/bots/{id}/soul                    -- Get current soul content
PUT    /api/v1/bots/{id}/soul                    -- Update soul (creates new version)
GET    /api/v1/bots/{id}/soul/versions           -- List all soul versions
GET    /api/v1/bots/{id}/soul/versions/{version} -- Get specific version
POST   /api/v1/bots/{id}/soul/rollback           -- Rollback to version (body: {version})
GET    /api/v1/bots/{id}/soul/verify             -- Verify soul integrity

GET    /api/v1/secrets                            -- List secrets
PUT    /api/v1/secrets/{key}                      -- Set secret
DELETE /api/v1/secrets/{key}                      -- Delete secret

GET    /health                                    -- Health check
```

### Missing Endpoints (must add in this phase)
```
# Chat Streaming (SSE)
POST   /api/v1/bots/{id}/chat/stream     -- Send message + stream response via SSE
                                           Body: { session_id?: string, message: string }
                                           If session_id null, creates new session
                                           Returns: SSE stream of StreamEvent JSON

# Session Management (HTTP handlers for existing ChatService methods)
GET    /api/v1/bots/{id}/sessions        -- List sessions for bot (ChatService::list_sessions)
GET    /api/v1/sessions/{id}             -- Get session details (ChatService::get_session)
GET    /api/v1/sessions/{id}/messages    -- Get messages (ChatService::get_messages)
DELETE /api/v1/sessions/{id}             -- Delete session (ChatRepository::delete_session)
POST   /api/v1/sessions/{id}/clear       -- Clear messages but keep session

# Identity & User Files (file-based, not versioned)
GET    /api/v1/bots/{id}/identity        -- Read IDENTITY.md (parsed frontmatter)
PUT    /api/v1/bots/{id}/identity        -- Write IDENTITY.md
GET    /api/v1/bots/{id}/user            -- Read USER.md content
PUT    /api/v1/bots/{id}/user            -- Write USER.md

# Dashboard Stats (aggregate query)
GET    /api/v1/stats                      -- { total_bots, active_bots, total_sessions,
                                              active_sessions, total_conversations }
```

### Notes on Backend Implementation
- ChatService already has `create_session`, `list_sessions`, `get_session`, `get_messages`, `save_user_message`, `save_assistant_message`, `end_session` methods -- the HTTP handlers just need to wrap these.
- The SSE streaming endpoint follows the same pattern as the CLI chat loop (see `loop_runner.rs`): resolve bot -> parse identity -> build fallback chain -> build completion request -> forward LLM stream as SSE events.
- Axum provides `axum::response::sse::{Event, Sse}` for SSE responses with keep-alive support.
- The existing `CorsLayer::new().allow_origin(Any)` in router.rs already permits cross-origin requests.

## Code Examples

### Axum SSE Streaming Chat Endpoint (Backend Addition)

```rust
// Source: Axum SSE example + project's existing loop_runner.rs pattern
use axum::response::sse::{Event, Sse};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::time::Duration;

#[derive(Deserialize)]
pub struct ChatStreamRequest {
    session_id: Option<String>,
    message: String,
}

pub async fn stream_chat(
    State(state): State<AppState>,
    _auth: Authenticated,
    Path(id_or_slug): Path<String>,
    Json(body): Json<ChatStreamRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let bot = resolve_bot(&state, &id_or_slug).await?;

    // Parse identity for model config
    let identity_path = LocalFileSystem::identity_path(&state.data_dir, &bot.slug);
    let identity_content = tokio::fs::read_to_string(&identity_path).await.unwrap_or_default();
    let identity_fm = parse_identity_frontmatter(&identity_content);
    let model = identity_fm.as_ref().map(|fm| fm.model.clone())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    // Create or resume session
    let session = if let Some(sid) = &body.session_id {
        let id: Uuid = sid.parse().map_err(|_| AppError::Validation("invalid session_id".into()))?;
        state.chat_service.get_session(&id).await?
            .ok_or(AppError::NotFound("session not found".into()))?
    } else {
        state.chat_service.create_session(bot.id.0, model.clone()).await?
    };

    // Build completion request (similar to loop_runner.rs)
    // ...

    let stream = async_stream::stream! {
        // Emit session_id so frontend knows which session this is
        yield Ok(Event::default()
            .event("session")
            .data(serde_json::json!({"session_id": session.id}).to_string()));

        // Forward LLM stream events as SSE
        while let Some(event) = llm_stream.next().await {
            match event {
                Ok(StreamEvent::TextDelta { text, .. }) => {
                    yield Ok(Event::default()
                        .event("text_delta")
                        .data(serde_json::json!({"text": text}).to_string()));
                }
                Ok(StreamEvent::Usage(usage)) => {
                    yield Ok(Event::default()
                        .event("usage")
                        .data(serde_json::to_string(&usage).unwrap()));
                }
                Ok(StreamEvent::Done) => {
                    yield Ok(Event::default().event("done").data("{}"));
                    break;
                }
                Err(e) => {
                    yield Ok(Event::default()
                        .event("error")
                        .data(serde_json::json!({"message": e.to_string()}).to_string()));
                    break;
                }
                _ => {}
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}
```

### ShadCN Sidebar with Collapsible Rail (Root Layout)

```typescript
// src/routes/__root.tsx
import { Outlet, createRootRoute, Link } from '@tanstack/react-router';
import { SidebarProvider, Sidebar, SidebarContent, SidebarGroup,
  SidebarGroupLabel, SidebarMenu, SidebarMenuItem, SidebarMenuButton,
  SidebarHeader, SidebarTrigger
} from '@/components/ui/sidebar';
import { Toaster } from '@/components/ui/sonner';
import { CommandPalette } from '@/components/layout/command-palette';

export const Route = createRootRoute({
  component: RootLayout,
});

function RootLayout() {
  return (
    <SidebarProvider>
      <div className="flex min-h-screen w-full">
        <AppSidebar />
        <main className="flex-1 overflow-auto">
          <Outlet />
        </main>
      </div>
      <Toaster position="bottom-right" />
      <CommandPalette />
      <ThemeEffect />
    </SidebarProvider>
  );
}

function AppSidebar() {
  return (
    <Sidebar collapsible="icon"> {/* VS Code-style rail */}
      <SidebarHeader>
        <SidebarTrigger />
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>Navigation</SidebarGroupLabel>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton asChild tooltip="Dashboard">
                <Link to="/">Dashboard</Link>
              </SidebarMenuButton>
            </SidebarMenuItem>
            {/* Bots, Chat, Settings */}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  );
}
```

### Bot Card Component

```typescript
// src/components/dashboard/bot-card.tsx
import { Card, CardContent, CardFooter } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent,
  DropdownMenuItem } from '@/components/ui/dropdown-menu';
import { formatDistanceToNow } from 'date-fns';
import { MoreHorizontal, MessageCircle } from 'lucide-react';

const STATUS_COLORS = {
  active: 'bg-green-500',
  disabled: 'bg-yellow-500',
  archived: 'bg-red-500',
} as const;

function BotCard({ bot }: { bot: Bot }) {
  return (
    <Card className="group hover:border-primary/50 transition-colors">
      <CardContent className="pt-6">
        <div className="flex items-start gap-3">
          <Avatar>
            <AvatarFallback className="text-lg">
              {bot.emoji || bot.name.charAt(0)}
            </AvatarFallback>
          </Avatar>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <h3 className="font-semibold truncate">{bot.name}</h3>
              <Badge variant="outline" className="shrink-0">
                <span className={`w-2 h-2 rounded-full mr-1.5 ${STATUS_COLORS[bot.status]}`} />
                {bot.status}
              </Badge>
            </div>
            <p className="text-sm text-muted-foreground truncate mt-1">
              {bot.soulSnippet}
            </p>
          </div>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" className="opacity-0 group-hover:opacity-100">
                <MoreHorizontal className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent>
              <DropdownMenuItem>Edit</DropdownMenuItem>
              <DropdownMenuItem>Disable</DropdownMenuItem>
              <DropdownMenuItem className="text-destructive">Delete</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </CardContent>
      <CardFooter className="text-xs text-muted-foreground justify-between">
        <span>{bot.model}</span>
        <span>
          {bot.lastActiveAt
            ? formatDistanceToNow(new Date(bot.lastActiveAt), { addSuffix: true })
            : 'Never active'}
        </span>
      </CardFooter>
    </Card>
  );
}
```

### Markdown Renderer with Code Copy

```typescript
// src/components/chat/markdown-renderer.tsx
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Copy } from 'lucide-react';

function MarkdownRenderer({ content }: { content: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeHighlight]}
      components={{
        pre({ children }) {
          return (
            <div className="relative group my-2">
              <Button
                variant="ghost"
                size="icon"
                className="absolute top-2 right-2 h-7 w-7 opacity-0 group-hover:opacity-100 transition-opacity"
                onClick={() => {
                  // Extract text content from code element
                  const codeEl = (children as React.ReactElement);
                  const text = codeEl?.props?.children;
                  if (text) navigator.clipboard.writeText(text);
                  toast.success('Copied to clipboard');
                }}
              >
                <Copy className="h-3.5 w-3.5" />
              </Button>
              <pre className="overflow-x-auto rounded-lg bg-muted p-4 text-sm">
                {children}
              </pre>
            </div>
          );
        },
      }}
    />
  );
}
```

### PWA Configuration

```typescript
// In vite.config.ts VitePWA section
VitePWA({
  registerType: 'autoUpdate',
  includeAssets: ['icons/*.png'],
  manifest: {
    name: 'Boternity',
    short_name: 'Boternity',
    description: 'Bot fleet management dashboard',
    theme_color: '#0a0a0a',
    background_color: '#0a0a0a',
    display: 'standalone',
    icons: [
      { src: '/icons/icon-192.png', sizes: '192x192', type: 'image/png' },
      { src: '/icons/icon-512.png', sizes: '512x512', type: 'image/png' },
    ],
  },
  workbox: {
    // CRITICAL: Do NOT cache API routes in service worker
    // SWR belongs in TanStack Query, not the service worker
    navigateFallback: '/index.html',
    globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2}'],
    // Exclude API from precache
    navigateFallbackDenylist: [/^\/api\//],
  },
})
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Create React App | Vite 7 | CRA deprecated 2024-2025 | Use `pnpm create vite@latest -- --template react-ts` |
| Tailwind v3 + postcss + tailwind.config.js | Tailwind v4 + @tailwindcss/vite + CSS-only config | v4 GA Jan 2025 | No JS config, no PostCSS, use `@import 'tailwindcss'` |
| React Router v6 | TanStack Router 1.x | 2024-2025 | Full type safety, file-based routing, auto code-splitting |
| SWR or manual fetch | TanStack Query v5 | Ongoing | Built-in SWR, devtools, mutations, streamedQuery (experimental) |
| Individual @radix-ui/react-* packages | Unified `radix-ui` package | shadcn/ui 2025 | One import instead of many |
| React.forwardRef | ref as prop (React 19) | React 19, Dec 2024 | forwardRef deprecated, shadcn components updated |
| Zustand 4.x | Zustand 5.x | 2025 | React 19 compatible |
| ShadCN Toast | ShadCN Sonner | 2024+ | Toast component deprecated, use Sonner instead |

**Deprecated/outdated:**
- **Create React App:** Fully deprecated.
- **`tailwind.config.js`:** Replaced by CSS `@theme` in Tailwind v4.
- **`@tailwind base/components/utilities`:** Replaced by `@import 'tailwindcss'` in v4.
- **React.forwardRef:** Deprecated in React 19; ref is now a regular prop.
- **ShadCN Toast component:** Deprecated. Use Sonner instead.

## Claude's Discretion Recommendations

### Grid Pagination Strategy
**Recommendation:** Client-side pagination with 12 bots per page (4x3 grid on desktop). The bot list API already supports `limit` and `offset` query params. For most single-user instances, bot counts stay under 50. TanStack Query's `placeholderData: keepPreviousData` makes page transitions instant.

### Responsive Breakpoints
**Recommendation:** Align with Tailwind defaults:
- Mobile: `< 640px` (default) -- single column grid, hamburger sidebar
- Tablet: `640-1024px` (sm-lg) -- 2-column grid, collapsed sidebar rail
- Desktop: `> 1024px` (lg+) -- 3-4 column grid, full sidebar

### Loading Skeletons
**Recommendation:** Use ShadCN's `Skeleton` component. Show 6 skeleton bot cards matching card layout, 3 skeleton chat messages matching bubble layout, 5 skeleton sidebar items. Match skeleton count to expected content to avoid layout shift.

### Error State Handling
**Recommendation:** Use TanStack Query's built-in error states. Display inline error banners with retry buttons. Network errors: toast "Connection lost" with auto-retry (3 attempts, exponential backoff). 4xx errors: show message from API envelope's `errors[0].message`.

### Code Block Language Detection
**Recommendation:** rehype-highlight (highlight.js) auto-detects language when no hint provided. For explicit hints (```python), the `language-*` class is automatically applied. Bundle the 37 default languages (covers JS, TS, Python, Rust, etc.).

### Monaco Editor Configuration
**Recommendation:** Load only markdown and yaml language support. Theme: `vs-dark`. Config: `minimap: { enabled: false }`, `wordWrap: 'on'`, `lineNumbers: 'off'`. Use `@monaco-editor/react` which lazy-loads from CDN by default (avoids bundling the 5MB+ editor).

### PWA Manifest
**Recommendation:** `name: "Boternity"`, `short_name: "Boternity"`, `display: "standalone"`, `theme_color: "#0a0a0a"`, `background_color: "#0a0a0a"`. Icons: 192x192 and 512x512 PNG. Simple geometric icon placeholder. Service worker: `generateSW` with `NetworkOnly` for API routes, `CacheFirst` for static assets.

## Open Questions

1. **Authentication for Web UI**
   - What we know: CLI uses `Authenticated` extractor with API keys.
   - What's unclear: Should web UI use the same key, or no auth for localhost?
   - Recommendation: Skip auth for localhost connections in Phase 4. Add API key field in Settings page for remote backend support. Full auth is a later phase concern.

2. **Production SPA Serving**
   - What we know: Vite dev server handles routing in development. Production needs SPA fallback.
   - What's unclear: Should Axum serve the built SPA or deploy separately?
   - Recommendation: Axum serves `apps/web/dist/` using `tower_http::services::ServeDir` with `fallback(ServeFile::new("dist/index.html"))`. Single-binary deployment. The `bnity serve` command starts both API + SPA serving.

3. **IDENTITY.md and USER.md API Surface**
   - What we know: Soul API exists for SOUL.md with versioning. IDENTITY.md and USER.md are file-based.
   - What's unclear: Should these get versioned like SOUL.md?
   - Recommendation: Simple read/write (no versioning) for Phase 4. SOUL.md versioning is about its immutability contract; IDENTITY.md and USER.md are config files without that constraint.

4. **Monaco React 19 Compatibility**
   - What we know: @monaco-editor/react 4.7.0 is stable. React 19 changed ref handling.
   - What's unclear: Whether stable 4.7.0 works without peer dep issues.
   - Recommendation: Start with stable 4.7.0. Test early. If issues arise, use `--legacy-peer-deps` or check for newer release.

## Sources

### Primary (HIGH confidence)
- Project codebase: `crates/boternity-api/src/http/router.rs` -- existing API routes verified
- Project codebase: `crates/boternity-types/src/` -- domain types (Bot, ChatSession, Soul, Identity)
- Project codebase: `crates/boternity-api/src/cli/chat/loop_runner.rs` -- streaming chat pattern
- Project codebase: `crates/boternity-api/src/http/response.rs` -- API envelope format
- Project codebase: `crates/boternity-core/src/chat/service.rs` -- available ChatService methods
- Project codebase: `pnpm-workspace.yaml` -- existing monorepo with `apps/*` workspace
- [ShadCN UI Vite Installation](https://ui.shadcn.com/docs/installation/vite) -- verified setup steps
- [ShadCN Components](https://ui.shadcn.com/docs/components) -- full component list verified Feb 2026
- [ShadCN Sidebar](https://ui.shadcn.com/docs/components/radix/sidebar) -- collapsible icon rail
- [ShadCN Command](https://ui.shadcn.com/docs/components/radix/command) -- cmdk integration
- [ShadCN Sonner](https://ui.shadcn.com/docs/components/radix/sonner) -- toast replacement
- [Axum SSE Example](https://github.com/tokio-rs/axum/blob/main/examples/sse/src/main.rs) -- SSE handler pattern
- [Vite 7 release](https://vite.dev/blog/announcing-vite7) -- current stable version

### Secondary (MEDIUM confidence)
- [TanStack Router vs React Router](https://betterstack.com/community/comparisons/tanstack-router-vs-react-router/) -- architecture comparison
- [TanStack Router Vite Installation](https://tanstack.com/router/v1/docs/framework/react/installation/with-vite) -- file-based routing setup
- [TanStack Query SSE discussion](https://github.com/TanStack/query/discussions/418) -- streaming patterns
- [vite-plugin-pwa guide](https://vite-pwa-org.netlify.app/guide/) -- PWA setup
- [ShadCN Dark Mode](https://ui.shadcn.com/docs/dark-mode) -- theming strategy
- [ShadCN Tailwind v4](https://ui.shadcn.com/docs/tailwind-v4) -- CSS variable migration
- [rehype-highlight](https://github.com/rehypejs/rehype-highlight) -- code syntax highlighting
- [@monaco-editor/react](https://www.npmjs.com/package/@monaco-editor/react) -- Monaco integration

### Tertiary (LOW confidence)
- [Zustand vs Jotai comparison](https://betterstack.com/community/guides/scaling-nodejs/zustand-vs-redux-toolkit-vs-jotai/) -- community article
- React 19 compatibility of @monaco-editor/react -- not fully verified

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All libraries verified via official docs and npm. React 19 + Vite 7 + Tailwind v4 + TanStack ecosystem is the current standard.
- Architecture: HIGH -- Patterns derived from official docs + project codebase analysis. File structure follows TanStack Router conventions.
- Backend gaps: HIGH -- Verified by reading router.rs and comparing against frontend requirements. ChatService methods already exist; just need HTTP handlers.
- Pitfalls: HIGH -- SSE lifecycle, bundle size, stale closures, SPA fallback are well-documented problems with established solutions.
- Code examples: MEDIUM -- Synthesized from official docs and project-specific API shapes. Not from running code.

**Research date:** 2026-02-12
**Valid until:** 2026-03-12 (30 days -- ecosystem is stable, all libraries on current majors)
