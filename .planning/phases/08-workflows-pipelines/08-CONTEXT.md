# Phase 8: Workflows + Pipelines - Context

**Gathered:** 2026-02-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Multi-step automations that compose agents and skills into execution chains. Workflows can be defined in YAML, built visually in a React Flow canvas, or written programmatically via TypeScript/Rust SDK. Triggers include manual, cron, event (webhooks, internal events, file watch). Bot-to-bot communication enables multi-bot collaboration. CLI management for workflow lifecycle.

</domain>

<decisions>
## Implementation Decisions

### Workflow definition & data flow
- Claude's discretion on step wiring approach (explicit vs implicit vs hybrid) — pick what's most robust for both static and dynamic workflows
- Full step primitive set: sequential, parallel, conditional (if/else branching), and loops (repeat until condition)
- Step types: Agent, Skill, Code (TypeScript + WASM), and HTTP request steps
- Context object model for data flow: each step receives a workflow context with all prior step outputs, reads what it needs
- Fail-fast by default; LLM-driven self-correction available as option — agent analyzes failure and retries with different approach/data instead of blind re-execution
- Max 3 LLM retry attempts before giving up
- Sub-workflow support: a step can invoke another workflow by name (needs depth cap like agent hierarchy)
- Bot-scoped workflows in `~/.boternity/bots/{slug}/workflows/`, cross-bot workflows in `~/.boternity/workflows/`
- Fully durable execution: state checkpointed to SQLite, workflow resumes from last completed step after crash/restart
- Configurable concurrency controls: workflow can declare `concurrency: N` (max parallel instances, default unlimited)
- Code-first SDK: TypeScript and Rust SDK with builder pattern for defining workflows programmatically (generates equivalent YAML)

### Trigger model & scheduling
- In-process cron scheduler (runs inside boternity server process)
- Event sources: webhooks, internal EventBus events (bot messages, agent completions), and filesystem watch
- Webhook auth: both HMAC shared secret and bearer token available, user chooses per webhook
- Failure notifications via EventBus + WebSocket (reuses Phase 5 infra) — failed runs visible in web UI dashboard
- Trigger payload filtering with `when` clause expressions (e.g., `when: event.type == 'push' && event.branch == 'main'`)
- Human-readable schedule strings accepted alongside standard cron (e.g., "every 5 minutes", "daily at 9am" parsed to cron internally)
- Approval gate step type available: pauses workflow until user confirms via CLI or web UI; workflows can be fully automated or include human checkpoints
- Missed cron runs caught up on restart (check for skipped scheduled runs and execute them)
- Step-level timeouts by default; optional per-workflow timeout that overrides when set
- Configurable file watch paths (not limited to bot storage)

### Bot-to-bot communication
- Dual communication model: direct messaging (1:1) and pub/sub channels (one-to-many)
- Caller chooses sync or async: `send_and_wait()` for synchronous (blocking), `send()` for asynchronous (fire-and-forget)
- Fleet-wide visibility: any bot can send to any other active bot, discovery via bot list API
- Typed envelope with flexible body: JSON envelope (sender, timestamp, type) + body that can be structured JSON or free-form text
- Full audit trail: all inter-bot messages persisted in SQLite, browsable per bot pair or channel
- Dynamic pub/sub channels: auto-created on first publish, no pre-definition required
- Autonomous communication: bots can send messages during normal conversation (via skills/agent actions), not just within workflows
- No rate limiting on inter-bot messages; budget enforcement (Phase 5) provides indirect cost control
- Default LLM-driven message processing; bots can optionally declare a message handler skill that intercepts first (cheaper, faster for programmatic responses)
- Bot-to-bot conversations create separate chat sessions tagged as 'bot-to-bot', visible in session browser
- Delegation supported: bot can forward user's question to another bot, user sees "Bot A asked Bot B for help" transparently

### Visual builder experience
- React Flow node graph canvas (free-form draggable nodes with connection lines)
- Side panel for step configuration (click node to open config panel on right)
- Toggle view between visual canvas and YAML editor (bidirectional sync, one active at a time)
- Both single-step testing and full workflow dry-run from the builder
- Categorized sidebar palette for browsing step types + search-based quick add (Notion-style / command)
- Live execution visualization on canvas: nodes light up green/yellow/red during execution, connections animate data flow via WebSocket
- Full undo/redo history for all canvas operations (Ctrl+Z/Ctrl+Y)
- Rich node preview: shows bot name, step type icon, first line of prompt, connection count
- Built-in workflow templates: common patterns (data pipeline, approval flow, multi-bot collaboration) for quick start
- Collapsible node groups: select nodes and group them, collapses to single node for overview
- Minimap for navigating large workflows
- Typed edges: connection lines show data type (text, JSON, file) with color coding

### Claude's Discretion
- Exact YAML schema structure and field naming
- Workflow YAML vs separate trigger config file placement
- Sub-workflow depth cap number
- Canvas layout algorithm and auto-arrangement
- Template content and categories
- Edge color scheme for data types
- Minimap positioning and sizing

</decisions>

<specifics>
## Specific Ideas

- n8n-like visual builder experience (node graph, side panel config, categorized palette)
- GitHub Actions-like `when` clause for trigger filtering
- Human-readable cron strings ("every 5 minutes") alongside standard cron syntax
- LLM self-correction on step failure as a differentiator: agent analyzes what went wrong and tries a different approach, not just retry
- Bot delegation visible to user: "Bot A asked Bot B for help" in conversation UI

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 08-workflows-pipelines*
*Context gathered: 2026-02-14*
