# Phase 5: Agent Hierarchy + Event System - Context

**Gathered:** 2026-02-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Bots can decompose complex tasks by spawning sub-agents (sequential + parallel) up to 3 levels deep, communicating via message passing, with an event bus driving real-time UI updates and budget enforcement preventing runaway costs. This phase adds hierarchical agent orchestration, not new LLM capabilities or new UI pages.

</domain>

<decisions>
## Implementation Decisions

### Task Decomposition UX
- Sub-agent activity shown inline in chat as collapsible blocks (like Claude Code's tool use output)
- Each sub-agent block streams its response token-by-token (full streaming, not just status)
- Bot always produces a synthesis response after all sub-agents complete, integrating results into a cohesive answer
- Collapsed sub-agent blocks always show tokens used and duration (transparency into costs)

### Budget Controls UX
- Per-request token budgets have system defaults with per-bot override in IDENTITY.md frontmatter (`max_request_tokens` field)
- Global default budget configurable in `~/.boternity/config.toml` (`default_request_budget = 500000`)
- Warning threshold fixed at 80% (not configurable)
- At 80% warning: pause execution and ask user "Budget 80% used. Continue?" in both CLI and web
- At budget exhaustion: graceful stop -- present whatever sub-agent results are available, explain what wasn't completed
- Continue/stop only at pause prompt -- no budget increase mid-request
- CLI shows a live running budget counter during sub-agent execution (e.g., `[tokens: 12,450 / 500,000]`)
- Completed requests show estimated cost alongside token count (e.g., `~$0.12 estimated`)
- Cost estimation uses hardcoded per-provider pricing with user override capability in provider config

### Sub-agent Behavior
- Sub-agents have full memory access -- can both recall and create memories (tagged with which agent created them)
- No explicit cap on parallel sub-agents -- the token budget naturally limits how many can run
- Sequential sub-agents see only the immediately prior sub-agent's result (not the full chain)
- Recursive spawning allowed -- sub-agents can spawn their own sub-agents up to the 3-level depth cap
- Sub-agents inherit the parent bot's personality (SOUL.md) -- they respond in character
- Sub-agents always use the same model as the root agent (configured in IDENTITY.md)
- On sub-agent failure: retry once, then skip and continue with remaining sub-agents + partial results
- No per-agent timeout -- token budget is the only constraint

### Event Visibility
- Both CLI and web show sub-agent progress in real-time
- CLI uses tree indentation for sub-agent output (e.g., `├── agent-1: text...` with depth-based nesting)
- Parallel sub-agents display as interleaved output in CLI, each line prefixed by its position in the tree
- `--quiet` flag suppresses sub-agent detail, showing only the final synthesized response
- Cycle detection and depth limit events shown as visible warnings to the user
- WebSocket connection status indicator visible in web UI (Connected / Reconnecting)
- User can cancel individual sub-agents (granular cancel):
  - Web: agent tree panel (like a process manager) with per-agent stop buttons, always togglable via a button
  - CLI: numbered agents in tree output, user types `cancel 2` to stop agent #2
- Ctrl+C in CLI cancels the entire sub-agent tree

### Claude's Discretion
- Event bus scope: Claude decides whether to include non-agent events (memory extraction, provider failover) based on what provides best reactivity, control, and stability
- Exact format of the live budget counter in CLI
- WebSocket reconnection strategy (exponential backoff details)
- Tree indentation styling and colors

</decisions>

<specifics>
## Specific Ideas

- Sub-agent blocks in chat should feel like Claude Code's tool use output -- collapsible, showing work being done, with metadata
- The agent tree panel in the web UI should work like a process manager -- visual tree of running/completed agents with ability to stop individual ones
- CLI interleaved output should feel like `docker-compose logs` with tree structure instead of flat labels
- Budget pause prompt should be non-disruptive -- a clear question that doesn't lose context
- Cost estimates should be clearly labeled as estimates (providers may bill differently)

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope

</deferred>

---

*Phase: 05-agent-hierarchy-event-system*
*Context gathered: 2026-02-13*
