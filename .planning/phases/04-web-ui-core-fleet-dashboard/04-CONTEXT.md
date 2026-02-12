# Phase 4: Web UI Core + Fleet Dashboard - Context

**Gathered:** 2026-02-12
**Status:** Ready for planning

<domain>
## Phase Boundary

A React single-page application for managing bots and chatting with them through a web interface. Delivers: fleet dashboard with bot cards and stats, streaming chat interface with parallel sessions, soul/config editor with version history and diffs, installable as a PWA. The backend already exists (Axum REST API from Phases 1-3) — this phase adds the frontend and any missing API endpoints (SSE chat streaming, session CRUD).

</domain>

<decisions>
## Implementation Decisions

### Fleet Dashboard
- Card grid layout (responsive, like Linear's project cards)
- Rich cards showing: bot name, emoji avatar (user-assignable or auto-generated from name), model, color-coded status badge (green/yellow/red for Active/Disabled/Archived), last activity time, session count, first line of SOUL.md as personality snippet
- Stats bar at the top: total bots, active sessions, total conversations — stats are clickable to filter the grid (clicking "3 Active" filters to active bots)
- Quick actions on each card: "Chat" button + overflow menu (edit, disable, delete)
- Search box to filter by name + sort dropdown (name, last activity, status)
- CTA-focused empty state: illustration + "Create your first bot" with prominent button
- "Create Bot" accessible via FAB on mobile + header button on desktop

### Chat Experience
- Session sidebar list (left panel, like Slack channels) — sessions grouped under bot name headers
- Auto-expanding textarea for message input (grows up to ~6 lines, then scrolls)
- Streaming feedback: brief "Bot is thinking..." indicator, then tokens appear live as they stream
- "New chat" button for same bot + "+" to pick a different bot — covers both use cases
- Full markdown rendering: headers, bold, lists, code blocks with syntax highlighting and copy button, tables
- "Stop generating" button during streaming responses
- Relative timestamps on messages ("2m ago", "yesterday")
- Delete session (removes entirely) + clear session (empties messages, keeps session)
- Chat header shows bot name, emoji, and model badge (e.g., "Claude Sonnet")
- Empty chat state: grid of available bots to start chatting with (functional empty state)

### Soul Editor
- Lives as a tab on the bot detail page (alongside Chat, Overview, Settings)
- Supports editing all three files: SOUL.md, IDENTITY.md, USER.md (tabs or dropdown to switch)
- IDENTITY.md editing: form view by default (model dropdown, temperature slider, max_tokens input) with toggle to raw text editor
- Auto-save with debounce (saves after 2s of inactivity)
- Auto-generated version labels ("Edited Feb 12, 2026 at 3:45 PM") — no commit message prompt
- Version history in a collapsible right side panel with visual timeline (vertical dots + connecting lines)
- Side-by-side diff view for comparing versions (read-only)
- Split preview: editor left, rendered markdown preview right
- Rollback via confirmation dialog: select version → "Restore this version?" with preview → confirm

### App Shell & Navigation
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

</decisions>

<specifics>
## Specific Ideas

- Linear-inspired aesthetic: clean, dark, minimal — Linear's dashboard as the visual reference
- Chat session sidebar organized like Slack channels (grouped by bot, recency within groups)
- Command palette should feel like Linear's Cmd+K — search everything, navigate everywhere, trigger actions
- Soul editor split preview similar to VS Code's markdown preview
- Bot emoji avatars for personality at a glance (not generic initials)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 04-web-ui-core-fleet-dashboard*
*Context gathered: 2026-02-12*
