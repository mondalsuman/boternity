# Phase 2: Single-Agent Chat + LLM - Context

**Gathered:** 2026-02-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Streaming conversational chat with a single LLM provider (Anthropic Claude) via CLI. The bot reads its SOUL.md, maintains session context, delivers token-by-token streaming responses, extracts and persists session memories, and provides structured observability. Users can have parallel sessions, browse session history, manage memories, and export conversations. All running locally via the `bnity` CLI.

</domain>

<decisions>
## Implementation Decisions

### Streaming chat feel
- **Rendering:** Character-by-character streaming — each token appears as it arrives, typewriter effect
- **Thinking indicator:** Animated spinner (e.g., ⠋⠙⠸⠴⠦⠇) with "thinking..." text while waiting for first token
- **Message formatting:** Full markdown rendering in terminal — bold, italic, code blocks with syntax highlighting, lists, headers
- **Bot identity in chat:** Bot emoji + colored name + accent color on every bot message — each bot visually distinct
- **Metadata per message:** Timestamps on each message + token count after bot responses — always visible
- **Stats footer:** After every bot response show: tokens used, response time, model — always visible (e.g., "│ 128 tokens · 1.2s · claude-sonnet-4-20250514")
- **Multiline input:** Shift+Enter for newlines AND paste-aware (pasted multiline stays as-is until sent) — both behaviors
- **In-chat commands:** Slash commands (/help, /clear, /exit, /new, /history) for discoverability + keyboard shortcuts (Ctrl+L, Ctrl+D) for power users — both
- **Long output:** Stream everything inline, user scrolls up — simple and predictable, no auto-paging
- **Error display:** Show error in chat, then offer choice: retry / switch model / abort — user stays in control
- **Welcome banner:** Full banner on session start — bot emoji + name + description + model + session info + hint about /help
- **User prompt:** Styled "You ›" in a distinct color — clear visual separation between user and bot

### Session lifecycle
- **Start:** `bnity chat <bot>` always starts a new session by default; user can opt to resume a previous session (e.g., `--resume` flag or session ID argument)
- **End:** Explicit exit only (/exit or Ctrl+D) — no auto-timeout, no surprises
- **Context window management:** Sliding window with LLM-generated summary — when approaching context limit, older messages are summarized and kept as context; bot stays coherent without user noticing
- **Parallel sessions:** Fully supported — multiple terminal tabs can have independent active sessions with the same bot simultaneously
- **Session browser:** `bnity sessions <bot>` lists past sessions with date, duration, title, and preview of first/last messages — pick one to resume
- **Session titles:** Auto-generated from first exchange by the LLM (like ChatGPT's conversation naming)
- **Session export:** Both Markdown (human-readable, default) and JSON (--json flag, full metadata) — consistent with Phase 1 CLI conventions
- **Session delete:** `bnity delete session <id>` with confirmation prompt, consistent with bot delete pattern from Phase 1

### Cross-session memory
- **Extraction timing:** Both periodic during session (every N messages) + final extraction at session end — resilient to crashes
- **Extraction logic:** LLM judges what's worth remembering — facts, preferences, decisions, relationships — no predefined categories
- **Memory recall:** Automatic and invisible — relevant memories injected into context silently; bot just "knows" things without announcing "I remember..."
- **Memory loading:** All memories loaded into system prompt at session start — full recall, works for early usage with modest memory counts
- **Memory browser:** `bnity memories <bot>` lists all extracted memories with full provenance: content, source session title, date extracted, and the message that triggered it — user can delete individual entries
- **Manual memory injection:** `bnity remember <bot> 'fact'` or /remember in chat — explicit knowledge injection supported
- **Memory wipe:** `bnity forget <bot>` wipes all memories with confirmation — clean slate
- **Crash recovery:** Best-effort — periodic extraction already captured some; on ungraceful exit, extract from whatever messages were saved to disk
- **Memory notification:** Silent extraction — no notification to user when memories are saved
- **Memory scope:** Shared across all sessions for the same bot — every session reads from and writes to the same memory pool
- **Cross-bot memory:** Own memories only by default in Phase 2, but data model should support cross-bot access for Phase 3
- **USER.md relationship:** USER.md and memory are separate systems — USER.md is curated standing instructions, memory is auto-extracted knowledge. No sync between them.
- **Memory limit:** No limit — keep everything forever, storage is local. User can manually prune via memory browser.

### Personality expression
- **Greeting:** Bot speaks first — sends a personality-driven greeting message when session opens; feels like the bot is waiting for you
- **Personality strength:** Strong personality — the bot's voice, tone, and personality should be unmistakable in every response; you should be able to tell which bot is talking without seeing the name
- **Context mapping:** All three files (SOUL.md + IDENTITY.md config + USER.md) compose into the system prompt — LLM sees everything as its identity/instructions
- **Bot distinctness:** Radically different — a creative writing bot and a research bot should feel like completely different beings; vocabulary, tone, response length, formatting all differ based on soul

### Claude's Discretion
- Exact markdown rendering library/approach for terminal
- Spinner animation style and timing
- System prompt template structure and ordering of SOUL/IDENTITY/USER sections
- Memory extraction prompt design
- Sliding window summary prompt and threshold
- Token counting approach (estimated vs exact)
- Session resume UX details
- Keyboard shortcut assignments
- Color scheme and theming details
- Auto-title generation prompt

</decisions>

<specifics>
## Specific Ideas

- Chat should feel alive — character-by-character streaming, animated spinners, styled output. Not a dead terminal.
- Bots are people, not tools — strong personality expression, greeting on start, radically distinct voices across different bots
- Memory should be invisible magic — user never notices extraction, bot just "knows" things. Full transparency available via memory browser for power users.
- Error handling keeps the user in control — never auto-retry silently, always offer choices
- Full provenance on memories — user can trace any memory back to the exact session and message that created it
- Session experience matches Phase 1 polish — same attention to styled output, emojis, rich tables, consistent patterns

</specifics>

<deferred>
## Deferred Ideas

- Vector-based semantic memory search (relevance-based recall instead of loading all) — Phase 3
- Cross-bot shared memory with trust partitioning — Phase 3
- Memory deduplication and merging — Phase 3
- Web UI chat interface — Phase 4

</deferred>

---

*Phase: 02-single-agent-chat-llm*
*Context gathered: 2026-02-10*
