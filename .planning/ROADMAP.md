# Roadmap: Boternity

## Overview

Boternity is built in 10 phases following the dependency chain: foundation types and traits first, then single-agent chat with one LLM provider, expanding to multi-provider with persistent memory, layering on the web UI and fleet dashboard, adding hierarchical agent orchestration with the event bus, building the skill system with WASM sandboxing, delivering the interactive builder agent, composing workflows and pipelines, integrating MCP bidirectionally, and finishing with full observability dashboards, cost controls, and platform polish. Each phase delivers a coherent, verifiable capability that unblocks the next. Security constraints (SOUL.md immutability, budget enforcement, memory poisoning prevention, MCP sanitization) are addressed in the phase where their attack surface first appears.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Foundation + Bot Identity** - Monorepo scaffold, crate structure, SQLite storage, bot CRUD with immutable SOUL.md, secrets vault, basic CLI and REST API
- [x] **Phase 2: Single-Agent Chat + LLM** - LLM provider abstraction, Anthropic Claude integration, streaming chat via CLI, session memory, chat persistence, structured logging
- [x] **Phase 3: Multi-Provider + Memory** - Additional LLM providers with fallback chains, long-term vector memory, shared memory with trust partitioning, per-bot storage
- [x] **Phase 4: Web UI Core + Fleet Dashboard** - React app scaffold, chat interface with streaming, fleet dashboard, soul editor with version history, PWA foundation
- [x] **Phase 5: Agent Hierarchy + Event System** - Sub-agent spawning (sequential + parallel), depth cap enforcement, message passing, event bus, WebSocket live updates, budget enforcement
- [x] **Phase 6: Skill System + WASM Sandbox** - Skill definition and execution, local skills, WASM sandbox for untrusted skills, registry discovery, permission model, trust tiers
- [x] **Phase 7: Builder System** - Universal builder agent, CLI wizard, web builder bot, adaptive question flow, skill creation and attachment via builder
- [x] **Phase 8: Workflows + Pipelines** - YAML workflow engine, visual builder, SDK, triggers (manual/cron/event), bot-to-bot communication, workflow composition
- [ ] **Phase 9: MCP Integration** - MCP tool consumption, bot-as-MCP-server exposure, MCP bot management interface, tool sanitization, MCP authentication
- [ ] **Phase 10: Observability + Cost + Polish** - Visual trace explorer, cost dashboards, budget alerts, gRPC + protocol multiplexing, memory browser, config export, bot templates, scriptable CLI, responsive PWA

## Phase Details

### Phase 1: Foundation + Bot Identity
**Goal**: A bot with a distinct, immutable identity exists in the system -- users can create bots with SOUL.md, store secrets securely, and manage bots via CLI and REST API, all backed by SQLite with a clean repository abstraction.
**Depends on**: Nothing (first phase)
**Requirements**: IDEN-01, IDEN-02, IDEN-03, IDEN-04, IDEN-05, IDEN-06, SECU-01, SECU-02, SECU-03, SECU-04, SECU-05, CLII-01, APIL-01, INFR-01, INFR-06, INFR-07, INFR-08
**Success Criteria** (what must be TRUE):
  1. User can create a bot via CLI and REST API, and the bot persists in SQLite with SOUL.md, IDENTITY.md, and USER.md files on disk
  2. SOUL.md is read-only at runtime -- no API endpoint or bot action can modify it; edits require explicit admin CLI/API command; SHA-256 hash is verified at bot startup
  3. User can store and retrieve API keys via the encrypted vault, OS keychain, or environment variables -- secrets never appear in logs or API responses
  4. User can list, configure, and delete bots via CLI commands and REST endpoints, with all operations reflected consistently in both interfaces
  5. The Turborepo + Cargo workspace builds successfully with boternity-types, boternity-core, boternity-infra, and boternity-api crates, and boternity-core has zero dependencies on boternity-infra
**Plans**: 6 plans

Plans:
- [ ] 01-01-PLAN.md -- Monorepo scaffold, Cargo workspace, Turborepo config, domain types, repository traits
- [ ] 01-02-PLAN.md -- SQLite storage layer with WAL mode, split pools, migrations, repository implementations
- [ ] 01-03-PLAN.md -- Bot identity system (SOUL.md, IDENTITY.md, USER.md), BotService, SoulService, filesystem adapters
- [ ] 01-04-PLAN.md -- Secrets vault (AES-256-GCM), OS keychain integration, env var fallback, resolution chain
- [ ] 01-05-PLAN.md -- CLI (`bnity`) bot lifecycle commands, REST API with auth and envelope responses
- [ ] 01-06-PLAN.md -- Soul versioning (history, rollback, diff) and immutability enforcement (SHA-256 integrity)

### Phase 2: Single-Agent Chat + LLM
**Goal**: Users can have a streaming conversation with a bot powered by a single LLM provider -- the bot reads its soul, maintains session context, and delivers responses token-by-token via CLI.
**Depends on**: Phase 1
**Requirements**: LLMP-01, LLMP-02, LLMP-11, CHAT-04, CHAT-05, CLII-03, AGNT-01, MEMO-01, OBSV-01, OBSV-07
**Success Criteria** (what must be TRUE):
  1. User can start a CLI chat session with a bot and see streaming token-by-token responses from Anthropic Claude, with the bot's personality reflecting its SOUL.md
  2. Session memory extracts and persists key points from each conversation -- when the user starts a new session, the bot can reference previous session context
  3. Chat history is persisted and retrievable -- user can view past conversations for any bot
  4. Every LLM call produces a structured trace with timing, token count, and decision context visible in logs
**Plans**: 8 plans

Plans:
- [ ] 02-01-PLAN.md -- Domain types (LLM, chat, agent, memory) and trait abstractions (LlmProvider, BoxLlmProvider, ChatRepository, MemoryRepository)
- [ ] 02-02-PLAN.md -- Observability crate (boternity-observe) with OTel tracing setup and workspace dependency additions
- [ ] 02-03-PLAN.md -- Anthropic Claude provider with SSE streaming state machine
- [ ] 02-04-PLAN.md -- SQLite chat/memory persistence (migrations + repository implementations)
- [ ] 02-05-PLAN.md -- Agent engine, system prompt builder, and chat service
- [ ] 02-06-PLAN.md -- Memory extraction, context summarization, and session title generation
- [ ] 02-07-PLAN.md -- Interactive CLI streaming chat with markdown rendering and full session management
- [ ] 02-08-PLAN.md -- CLI session browser, memory browser, export, and management commands

### Phase 3: Multi-Provider + Memory
**Goal**: Bots can use any of multiple LLM providers with automatic failover, remember things long-term via vector embeddings, share knowledge across bots safely, and store files and structured data.
**Depends on**: Phase 2
**Requirements**: LLMP-03, LLMP-04, LLMP-05, LLMP-06, LLMP-07, LLMP-08, LLMP-09, LLMP-10, MEMO-02, MEMO-03, MEMO-04, MEMO-06, INFR-02
**Success Criteria** (what must be TRUE):
  1. User can configure any bot to use OpenAI, Google Gemini, Mistral, AWS Bedrock, Claude.ai subscription, or GLM 4.7 -- switching providers requires only a config change, not code changes
  2. User can set a fallback provider chain (e.g., Claude -> OpenAI -> Gemini) and the system automatically fails over when the primary provider is down or rate-limited
  3. Bot can semantically recall relevant information from past conversations -- user asks "what did we discuss about X?" and the bot retrieves related memories via vector search
  4. Shared memory works across bots with trust-level partitioning -- Bot A can write to shared memory and Bot B can read it, but provenance is tracked and write validation prevents poisoning
  5. User can upload files and structured data to a bot's persistent storage and the bot can reference them in conversation
**Plans**: 13 plans

Plans:
- [x] 03-01-PLAN.md -- Extended domain types (provider config, vector memory, shared memory, storage, KV) and core trait abstractions
- [x] 03-02-PLAN.md -- OpenAI-compatible provider (async-openai) supporting OpenAI, Gemini, Mistral, GLM 4.7
- [x] 03-03-PLAN.md -- Circuit breaker state machine, provider health tracking, and fallback chain logic
- [x] 03-04-PLAN.md -- LanceDB vector database + fastembed local embedding infrastructure
- [x] 03-05-PLAN.md -- SQLite migrations (KV store, audit log, provider health, file metadata) and repository implementations
- [x] 03-06-PLAN.md -- Claude subscription provider (experimental), provider factory, fallback chain wiring into chat
- [x] 03-07-PLAN.md -- LanceDB-backed vector memory store with time decay scoring and semantic dedup
- [x] 03-08-PLAN.md -- Memory recall integration into agent engine and system prompt injection
- [x] 03-09-PLAN.md -- Shared memory with trust-level partitioning, provenance tracking, and tamper detection
- [x] 03-10-PLAN.md -- Per-bot file storage with version history and semantic text indexing
- [x] 03-11-PLAN.md -- Provider CLI (status/add/remove/list) and failover visibility in chat
- [x] 03-12-PLAN.md -- Memory CLI enhancements (similarity scores, export) and shared memory CLI
- [x] 03-13-PLAN.md -- Storage CLI (upload/download/list/delete), KV CLI, and AppState Phase 3 wiring

### Phase 4: Web UI Core + Fleet Dashboard
**Goal**: Users can manage their bot fleet and chat with bots through a web interface -- the dashboard shows all bots at a glance, the chat interface streams responses in real-time, and the soul editor provides version-controlled identity management.
**Depends on**: Phase 3
**Requirements**: WEBU-01, WEBU-02, WEBU-03, WEBU-09, WEBU-10, CHAT-01, CHAT-02, CHAT-03, INFR-05
**Success Criteria** (what must be TRUE):
  1. User opens the web dashboard and sees all bots with their status, last activity, and key metrics in a fleet overview
  2. User can chat with any bot in the web UI and see streaming token-by-token responses, with support for multiple simultaneous chat sessions including multiple sessions with the same bot
  3. User can edit a bot's SOUL.md in the web editor, see version history with diffs, and roll back to any previous version
  4. The web app is installable as a PWA and works on mobile devices with responsive layout
**Plans**: 8 plans

Plans:
- [ ] 04-01-PLAN.md -- Backend API endpoints: SSE streaming chat, session CRUD, identity/user file endpoints, dashboard stats, SPA serving
- [ ] 04-02-PLAN.md -- React app scaffold with Vite, TanStack Router/Query, shadcn/ui, app shell (sidebar, command palette, theme, toaster)
- [ ] 04-03-PLAN.md -- Fleet dashboard: stats bar, bot card grid, search/sort, empty state, create bot dialog
- [ ] 04-04-PLAN.md -- Chat interface core: SSE streaming hook, session sidebar, message display, chat input, parallel sessions
- [ ] 04-05-PLAN.md -- Chat polish: markdown rendering with syntax highlighting, code copy, streaming markdown
- [ ] 04-06-PLAN.md -- Soul editor: Monaco editor, file tabs (SOUL/IDENTITY/USER), auto-save, identity form, split preview
- [ ] 04-07-PLAN.md -- Soul version history: timeline panel, side-by-side diff viewer, rollback with confirmation
- [ ] 04-08-PLAN.md -- PWA configuration and responsive layout polish across all pages

### Phase 5: Agent Hierarchy + Event System
**Goal**: Bots can decompose complex tasks by spawning sub-agents up to 3 levels deep, communicating via message passing, with an event bus driving real-time UI updates and budget enforcement preventing runaway costs.
**Depends on**: Phase 4
**Requirements**: AGNT-02, AGNT-03, AGNT-04, AGNT-05, AGNT-06, AGNT-12, AGNT-13, OBSV-02, OBSV-06, INFR-03
**Success Criteria** (what must be TRUE):
  1. A bot's agent can spawn sequential and parallel sub-agents to handle sub-tasks, with results flowing back to the parent via message passing -- user sees the task decomposed and completed
  2. Sub-agent depth is enforced at exactly 3 levels -- a 4th-level spawn attempt fails gracefully with an explanation, not a crash
  3. WebSocket live updates show agent spawning, execution progress, and completion in real-time in the web UI
  4. Per-request token budget is enforced -- when a sub-agent tree approaches the budget limit, execution pauses with an alert rather than silently running up costs
  5. Cycle detection catches and breaks infinite sub-agent spawning loops before they exhaust resources
**Plans**: 8 plans

Plans:
- [x] 05-01-PLAN.md -- Domain types (AgentEvent, SpawnInstruction, SubAgentResult, GlobalConfig) and new workspace dependencies
- [x] 05-02-PLAN.md -- Core primitives (RequestBudget, SharedWorkspace, CycleDetector, RequestContext, EventBus)
- [x] 05-03-PLAN.md -- Spawn instruction parser, AgentContext.child_for_task(), SystemPromptBuilder agent_capabilities
- [x] 05-04-PLAN.md -- AgentOrchestrator (parallel/sequential execution, retry, synthesis, budget/cancel integration)
- [x] 05-05-PLAN.md -- Config.toml loader, cost estimation, and pricing table
- [x] 05-06-PLAN.md -- WebSocket handler, EventBus on AppState, /ws/events route
- [x] 05-07-PLAN.md -- CLI tree renderer, budget display, and orchestrator integration into CLI + HTTP handlers
- [x] 05-08-PLAN.md -- Web UI: WebSocket hook, agent store, agent blocks, tree panel, budget indicator, WS status

### Phase 6: Skill System + WASM Sandbox
**Goal**: Agents can be extended with modular skills -- local skills run with permissions, untrusted registry skills run in a WASM sandbox, and users can discover, install, and manage skills from agentskills.io and community registries.
**Depends on**: Phase 5
**Requirements**: SKIL-01, SKIL-02, SKIL-03, SKIL-04, SKIL-05, SKIL-07, SKIL-08, SKIL-09, SKIL-10, SECU-06, SECU-07, CLII-02
**Success Criteria** (what must be TRUE):
  1. User can create a local skill following the agentskills.io spec and attach it to an agent -- the agent uses the skill in conversation
  2. User can search and install skills from skills.sh and ComposioHQ/awesome-claude-skills via CLI -- installed registry skills run inside a WASM sandbox with declared capabilities
  3. Skill permission model works -- skills declare required capabilities at install time, user approves or denies, and the runtime enforces those grants (a skill cannot access capabilities it was not granted)
  4. Skill inheritance works -- a child skill extends a parent skill's features and the agent sees the combined capabilities
  5. Defense-in-depth is observable -- untrusted skills are sandboxed at WASM level, WASI capabilities are restricted, and OS-level sandboxing provides a second barrier
**Plans**: 14 plans

Plans:
- [x] 06-01-PLAN.md -- Skill domain types (SkillManifest, TrustTier, Capability, permissions, audit) and Phase 6 workspace dependencies
- [x] 06-02-PLAN.md -- SKILL.md manifest parser (agentskills.io format) and filesystem skill store (~/.boternity/skills/)
- [x] 06-03-PLAN.md -- Permission model (CapabilityEnforcer, granular grants/revocation) and SQLite audit logging
- [x] 06-04-PLAN.md -- Dependency resolution (petgraph DAG + toposort) and inheritance composition (mixin, max 3 levels)
- [x] 06-05-PLAN.md -- WIT interface definition (boternity:skill) and Wasmtime runtime configuration (dual engines per trust tier)
- [x] 06-06-PLAN.md -- SkillExecutor trait, prompt-based skill injection (progressive disclosure), and local skill executor
- [x] 06-07-PLAN.md -- WASM sandboxed executor (capability-gated host imports, ResourceLimiter, fresh Store per invocation)
- [x] 06-08-PLAN.md -- OS-level sandbox (macOS Seatbelt + Linux Landlock subprocess model) for defense-in-depth
- [x] 06-09-PLAN.md -- Registry discovery (GitHub API, skills.sh, ComposioHQ) with pluggable registry trait and local caching
- [x] 06-10-PLAN.md -- Agent integration (SystemPromptBuilder skills, skill chaining) and AppState wiring
- [x] 06-11-PLAN.md -- CLI skill commands (create, install, list, inspect, browse) and ratatui TUI skill browser
- [x] 06-12-PLAN.md -- REST API skill handlers and web UI skill management page (Skills tab in bot detail)
- [x] 06-13-PLAN.md -- [Gap closure] Wire OS sandbox into WASM executor for defense-in-depth (SECU-07, SKIL-10)
- [x] 06-14-PLAN.md -- [Gap closure] WASM compilation step in registry install flow (SKIL-08, SKIL-02)

### Phase 7: Builder System
**Goal**: Users can create fully-configured agents and skills through an interactive guided experience -- a universal builder agent powers both the CLI wizard and the web UI builder bot, asking adaptive questions and assembling the result.
**Depends on**: Phase 6
**Requirements**: AGNT-07, AGNT-08, AGNT-09, AGNT-10, AGNT-11, SKIL-06, CLII-06
**Success Criteria** (what must be TRUE):
  1. User can create an agent via CLI wizard -- the builder asks multi-choice questions adapted to the stated purpose, then creates the agent with appropriate skills attached
  2. User can create an agent via web UI chat with the builder bot -- same question flow, same result, powered by the same universal builder agent
  3. The builder adapts question depth to complexity -- a simple "email assistant" gets fewer questions than a "research analyst with multiple data sources"
  4. Builder-created skills follow the agentskills.io spec and are immediately usable by the new agent
**Plans**: 10 plans

Plans:
- [x] 07-01-PLAN.md -- Builder domain types (BuilderTurn, BuilderState, BuilderPhase, PurposeCategory) and OutputConfig extension to CompletionRequest
- [x] 07-02-PLAN.md -- Core BuilderAgent trait, BuilderState accumulator, and Forge system prompt builder
- [x] 07-03-PLAN.md -- SQLite draft persistence (BuilderDraftStore) and builder memory store for session recall
- [x] 07-04-PLAN.md -- Smart defaults per purpose category and BotAssembler for creating bots from BuilderConfig
- [x] 07-05-PLAN.md -- LlmBuilderAgent implementation with structured output and output_config provider wiring
- [x] 07-06-PLAN.md -- SkillBuilder for LLM-driven skill creation and skill attachment in BotAssembler
- [x] 07-07-PLAN.md -- CLI builder wizard (bnity create) with dialoguer multi-choice and standalone skill create
- [x] 07-08-PLAN.md -- REST API builder session endpoints and WebSocket handler for Forge chat
- [x] 07-09-PLAN.md -- Web UI step-by-step wizard with progress indicator, option cards, and live preview
- [x] 07-10-PLAN.md -- Web UI Forge chat bot interface with interactive option buttons and WebSocket conversation

### Phase 8: Workflows + Pipelines
**Goal**: Users can define multi-step automations that compose agents and skills into execution chains -- workflows can be defined in YAML, built visually, or written in code, and triggered manually, on schedule, or by events.
**Depends on**: Phase 7
**Requirements**: WKFL-01, WKFL-02, WKFL-03, WKFL-04, WKFL-05, WKFL-06, WKFL-07, WKFL-08, WKFL-09, CHAT-06, CLII-05, WEBU-05
**Success Criteria** (what must be TRUE):
  1. User can define a workflow in YAML that chains multiple agents and skills together, and execute it -- the workflow runs steps in the defined order with data flowing between them
  2. User can build the same workflow visually in the web UI drag-and-drop builder, and the visual representation converts to valid YAML (and vice versa)
  3. Workflows can be triggered manually, on a cron schedule, or by events (webhooks, bot messages) -- all three trigger types work reliably
  4. Bot-to-bot communication works -- one bot can send structured messages to another bot, and workflows can orchestrate multi-bot collaboration
  5. User can manage workflows via CLI (create, trigger, list, check status)
**Plans**: 15 plans

Plans:
- [x] 08-01-PLAN.md -- Domain types (workflow, message, triggers, steps) and Phase 8 workspace dependencies
- [x] 08-02-PLAN.md -- SQLite workflow + message persistence (migrations, repository traits, implementations)
- [x] 08-03-PLAN.md -- Workflow definition parser, DAG validator, topological wave computation, context model
- [x] 08-04-PLAN.md -- Workflow executor with durable checkpointing, step runners for all 8 step types
- [x] 08-05-PLAN.md -- JEXL expression evaluator with standard transforms and LLM self-correction retry handler
- [x] 08-06-PLAN.md -- Bot-to-bot message bus (direct + pub/sub), loop guard, message handler pipeline
- [x] 08-07-PLAN.md -- Trigger system (cron scheduler, webhook handler, EventBus listener, file watcher)
- [x] 08-08-PLAN.md -- CLI workflow management + bot-to-bot message commands
- [x] 08-09-PLAN.md -- REST API workflow/webhook/message handlers and AppState Phase 8 wiring
- [x] 08-10-PLAN.md -- Web UI workflow list/detail pages, React Flow canvas, 8 custom node types, typed edges
- [x] 08-11-PLAN.md -- Web UI step config panel, node palette, YAML editor toggle, undo/redo, templates, grouping
- [x] 08-12-PLAN.md -- TypeScript SDK package (@boternity/workflow-sdk) and Rust builder helpers
- [x] 08-13-PLAN.md -- Live execution visualization, workflow events, service wiring, crash recovery
- [x] 08-14-PLAN.md -- [Gap closure] Wire DagExecutor to AppState with real StepExecutionContext, spawn execution from triggers
- [x] 08-15-PLAN.md -- [Gap closure] Wire CronScheduler and EventBus listener to call DagExecutor for cron/event triggers

### Phase 9: MCP Integration
**Goal**: Bots participate in the MCP ecosystem bidirectionally -- they consume external MCP tools to extend their capabilities and expose themselves as MCP servers so external tools (like Claude Code) can use bots as tools.
**Depends on**: Phase 8
**Requirements**: MCPI-01, MCPI-02, MCPI-03, MCPI-04, MCPI-05
**Success Criteria** (what must be TRUE):
  1. User can connect a bot to external MCP servers and the bot can invoke those tools during conversation -- e.g., a bot connected to a filesystem MCP server can read/write files
  2. A bot exposes itself as an MCP server -- external tools like Claude Code can discover and invoke the bot as a tool
  3. User can create and manage bots via the MCP server interface -- an external MCP client can perform bot CRUD operations
  4. MCP tool descriptions are sanitized before reaching the LLM context -- injection attempts in tool descriptions are neutralized
  5. Both MCP consumption and exposure require authentication -- unauthenticated MCP connections are rejected
**Plans**: 13 plans

Plans:
- [ ] 09-01-PLAN.md -- MCP domain types (config, connection, audit, permissions) and Phase 9 workspace dependencies (rmcp, governor)
- [ ] 09-02-PLAN.md -- Core MCP traits (McpClientManager, McpServerExposer, McpConfigManager, McpAuditLogger, McpRateLimiter) and DefaultToolSanitizer
- [ ] 09-03-PLAN.md -- SQLite MCP persistence (connections + audit tables), JSON config store, MCP keystore, server presets
- [ ] 09-04-PLAN.md -- MCP client connection pool (RmcpClientManager), transport factory (stdio + HTTP), sampling handler
- [ ] 09-05-PLAN.md -- MCP server handler (BoternityMcpServer), dynamic tool registry from bot capabilities
- [ ] 09-06-PLAN.md -- MCP server transports (Streamable HTTP + stdio), bearer token auth middleware, governor rate limiter
- [ ] 09-07-PLAN.md -- Agent engine MCP integration: tool injection into system prompt, tool use loop, result sanitization
- [ ] 09-08-PLAN.md -- MCP prompt registry: bot skills and use cases as MCP prompt templates
- [ ] 09-09-PLAN.md -- CLI MCP commands (add/remove/list/status/connect/disconnect/serve/test-tool/set-credential/presets)
- [ ] 09-10-PLAN.md -- REST API MCP handlers and route wiring for web UI MCP tab
- [ ] 09-11-PLAN.md -- AppState Phase 9 wiring, health ping background task, chat handler MCP integration
- [ ] 09-12-PLAN.md -- Web UI MCP tab: server list, tool inventory, audit log components
- [ ] 09-13-PLAN.md -- Collapsible tool call blocks in chat (web UI + CLI) with syntax-highlighted JSON

### Phase 10: Observability + Cost + Polish
**Goal**: The platform provides full visibility into what every agent is doing and why, with cost tracking and budget controls, plus the remaining polish features that complete the v1 experience.
**Depends on**: Phase 9
**Requirements**: OBSV-03, OBSV-04, OBSV-05, APIL-02, APIL-03, WEBU-04, WEBU-06, WEBU-07, WEBU-08, MEMO-05, IDEN-07, IDEN-08, CLII-04, INFR-04
**Success Criteria** (what must be TRUE):
  1. User can open the visual trace explorer in the web UI and see a real-time tree of agent decisions, timing, parent-child relationships, and LLM calls for any active or completed request
  2. User can view cost dashboards showing token usage and provider costs broken down by bot, provider, and time period -- with a global budget that alerts and pauses execution when exceeded
  3. User can browse and manage bot memories in the web UI -- search across memories, view individual entries, and delete specific memories
  4. gRPC API is available alongside REST on the same port via protocol multiplexing -- programmatic clients can choose either protocol
  5. User can export a bot's config (soul + skills, no memory) and use pre-built bot templates to create new bots for common use cases
**Plans**: TBD

Plans:
- [ ] 10-01: Visual trace explorer
- [ ] 10-02: Cost dashboards and budget controls
- [ ] 10-03: gRPC API and protocol multiplexing
- [ ] 10-04: Memory browser UI
- [ ] 10-05: Skill browser UI polish
- [ ] 10-06: Bot templates and config export
- [ ] 10-07: Scriptable CLI and file upload (Tus)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9 -> 10

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation + Bot Identity | 6/6 | Complete | 2026-02-11 |
| 2. Single-Agent Chat + LLM | 8/8 | Complete | 2026-02-12 |
| 3. Multi-Provider + Memory | 13/13 | Complete | 2026-02-12 |
| 4. Web UI Core + Fleet Dashboard | 8/8 | Complete | 2026-02-13 |
| 5. Agent Hierarchy + Event System | 8/8 | Complete | 2026-02-13 |
| 6. Skill System + WASM Sandbox | 14/14 | Complete | 2026-02-14 |
| 7. Builder System | 10/10 | Complete | 2026-02-14 |
| 8. Workflows + Pipelines | 15/15 | Complete | 2026-02-14 |
| 9. MCP Integration | 0/13 | Not started | - |
| 10. Observability + Cost + Polish | 0/7 | Not started | - |
