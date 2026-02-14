# Requirements: Boternity

**Defined:** 2026-02-10
**Core Value:** A user can create a bot with a distinct identity, give it skills through an interactive builder, and have meaningful parallel conversations with it — all running locally with full observability.

## v1 Requirements

### Bot Identity

- [ ] **IDEN-01**: User can create a bot via CLI, web UI, REST API, gRPC API, or MCP server
- [ ] **IDEN-02**: Each bot has a SOUL.md defining personality, values, behavior, and goals
- [ ] **IDEN-03**: SOUL.md is immutable at runtime — edits only via admin UI/CLI, never by the bot itself
- [ ] **IDEN-04**: Soul versioning — every update tracked with full history, rollback to any previous version
- [ ] **IDEN-05**: Each bot has an IDENTITY.md defining display name, avatar, and description (presentation layer)
- [ ] **IDEN-06**: Each bot reads a USER.md providing user-specific context (preferences, name, communication style)
- [ ] **IDEN-07**: Pre-built bot templates for common use cases (assistant, researcher, coder, writer)
- [ ] **IDEN-08**: Bot config export (soul + skill definitions) for sharing — memory stays local

### Memory

- [ ] **MEMO-01**: Session memory — key points extracted and saved per conversation session (short-term)
- [ ] **MEMO-02**: Long-term vector memory — persistent per-bot embeddings for semantic recall across sessions
- [ ] **MEMO-03**: Common shared memory — shared memory layer accessible by all bots with trust-level partitioning
- [ ] **MEMO-04**: Write validation on shared memory — provenance tracking to prevent memory poisoning
- [ ] **MEMO-05**: Memory search and browse UI — view, search, and manage bot memories in the web interface
- [ ] **MEMO-06**: Per-bot persistent storage for files, knowledge bases, and structured data

### Agent Architecture

- [ ] **AGNT-01**: Each bot has one default agent powering it
- [ ] **AGNT-02**: Agents can dynamically spawn sequential sub-agents
- [ ] **AGNT-03**: Agents can dynamically spawn parallel sub-agents
- [ ] **AGNT-04**: Sub-agent depth hard-capped at 3 levels with enforcement at AgentContext level
- [ ] **AGNT-05**: Sub-agent communication via message passing (parent sends context, child returns result)
- [ ] **AGNT-06**: Opt-in shared workspace for agents that need shared state
- [x] **AGNT-07**: Agent creation via interactive builder bot in web UI chat
- [x] **AGNT-08**: Agent creation via CLI wizard with multi-choice questions
- [x] **AGNT-09**: Universal builder agent powers both wizard and builder bot
- [x] **AGNT-10**: Builder asks adaptive multi-choice questions based on agent purpose (5-10 questions, then offer skip)
- [x] **AGNT-11**: Builder assesses required skills, creates them, and attaches to agent
- [ ] **AGNT-12**: Per-request token budget enforcement to prevent runaway sub-agent costs
- [ ] **AGNT-13**: Cycle detection and circuit breakers for sub-agent spawning

### Skill System

- [x] **SKIL-01**: Agents are powered by one or more skills
- [x] **SKIL-02**: Local skill creation following agentskills.io specification (via skill-creator skill)
- [x] **SKIL-03**: Skill inheritance hierarchy — child skill inherits parent features plus additions
- [x] **SKIL-04**: Discover and install skills from skills.sh registry (via find-skills skill or CLI)
- [x] **SKIL-05**: Discover skills from ComposioHQ/awesome-claude-skills
- [x] **SKIL-06**: Universal builder agent creates skills using same mechanism for all paths
- [x] **SKIL-07**: Permission model — skills declare required capabilities, user approves on install
- [x] **SKIL-08**: WASM sandbox for untrusted registry skills (Wasmtime with WASI component model)
- [x] **SKIL-09**: Trust tiers — local skills run with permissions, registry skills run sandboxed
- [x] **SKIL-10**: Defense-in-depth — WASM + OS-level sandboxing + capability-based WASI

### Workflows & Pipelines

- [ ] **WKFL-01**: Define workflows in YAML config files
- [ ] **WKFL-02**: Visual workflow builder in web UI (drag-and-drop)
- [ ] **WKFL-03**: TypeScript/Rust SDK for programmatic workflow definition
- [ ] **WKFL-04**: All three workflow representations are interchangeable
- [ ] **WKFL-05**: Manual workflow trigger (user explicitly starts)
- [ ] **WKFL-06**: Scheduled workflow trigger (cron-based)
- [ ] **WKFL-07**: Event-driven workflow trigger (webhooks, messages, bot events)
- [ ] **WKFL-08**: Workflows compose agents and skills into execution chains
- [ ] **WKFL-09**: Bot-to-bot structured communication via workflows

### LLM Providers

- [ ] **LLMP-01**: Pluggable provider architecture with unified abstraction layer
- [ ] **LLMP-02**: Anthropic Claude support (API)
- [ ] **LLMP-03**: OpenAI support (API)
- [ ] **LLMP-04**: Google Gemini support (API)
- [ ] **LLMP-05**: Mistral support (API)
- [ ] **LLMP-06**: AWS Bedrock models support
- [ ] **LLMP-07**: Claude.ai subscription support (OpenClaw-style)
- [ ] **LLMP-08**: GLM 4.7 from z.ai support
- [ ] **LLMP-09**: Configurable fallback provider chain — user sets sequence
- [ ] **LLMP-10**: Automatic failover when provider is down or rate-limited
- [ ] **LLMP-11**: Streaming token delivery from all providers

### MCP Integration

- [ ] **MCPI-01**: Bots consume external MCP tools (connect to MCP servers)
- [ ] **MCPI-02**: Bots expose themselves as MCP servers (other tools can use bots as tools)
- [ ] **MCPI-03**: Create and manage bots via MCP server interface
- [ ] **MCPI-04**: MCP tool description sanitization to prevent injection attacks
- [ ] **MCPI-05**: Mandatory authentication on both consume and expose MCP sides

### Chat System

- [ ] **CHAT-01**: User can chat with any bot via web UI with streaming responses (SSE/WebSocket)
- [ ] **CHAT-02**: User can chat with multiple bots simultaneously (parallel sessions)
- [ ] **CHAT-03**: User can have multiple parallel sessions with the same bot
- [ ] **CHAT-04**: Chat history persistence and retrieval
- [ ] **CHAT-05**: User can chat with bots via CLI (interactive terminal chat)
- [ ] **CHAT-06**: Bot-to-bot direct messaging for collaboration and task delegation

### Observability & Debugging

- [ ] **OBSV-01**: Distributed tracing — every agent decision, timing, parent-child relationships (OpenTelemetry)
- [ ] **OBSV-02**: Token usage tracking per agent and per bot
- [ ] **OBSV-03**: Cost dashboards with provider breakdown
- [ ] **OBSV-04**: Global token/cost budget with alerts and automatic pause when exceeded
- [ ] **OBSV-05**: Visual trace explorer in web UI — real-time agent tree with status, timing, decisions
- [ ] **OBSV-06**: WebSocket live updates for agent spawning, workflow progress, status changes
- [ ] **OBSV-07**: Structured logging throughout the platform

### CLI

- [ ] **CLII-01**: Full bot lifecycle management (create, configure, list, start, stop, delete)
- [x] **CLII-02**: Skill management (create, install from registry, list, remove)
- [ ] **CLII-03**: Interactive chat with bots from terminal with streaming
- [ ] **CLII-04**: Scriptable commands — pipe input/output for automation
- [ ] **CLII-05**: Workflow management (create, trigger, list, status)
- [x] **CLII-06**: Agent creation wizard (interactive multi-choice)

### API Layer

- [ ] **APIL-01**: REST API for all platform operations
- [ ] **APIL-02**: gRPC API for high-performance programmatic access
- [ ] **APIL-03**: Protocol multiplexing — REST and gRPC on same port via content-type dispatch

### Security & Secrets

- [ ] **SECU-01**: Encrypted local vault (SQLite-based) for API keys and credentials
- [ ] **SECU-02**: OS keychain integration (macOS Keychain, Linux Secret Service)
- [ ] **SECU-03**: Environment variable fallback for secrets
- [ ] **SECU-04**: SOUL.md immutable at runtime — prevents persistent prompt injection (CVE-2026-25253 mitigation)
- [ ] **SECU-05**: SOUL.md hash verification at startup
- [x] **SECU-06**: Skill permission model with capability grants
- [x] **SECU-07**: WASM sandbox with defense-in-depth for untrusted skills

### Web UI

- [ ] **WEBU-01**: Fleet dashboard — overview of all bots, status, activity
- [ ] **WEBU-02**: Chat interface with streaming responses and parallel session support
- [ ] **WEBU-03**: Soul/config editor with version history and diff view
- [ ] **WEBU-04**: Skill browser — search, preview, install skills from registries
- [ ] **WEBU-05**: Visual workflow builder (drag-and-drop with dnd-kit)
- [ ] **WEBU-06**: Visual trace explorer — real-time agent chains, timing, decisions
- [ ] **WEBU-07**: Memory browser — search, view, manage bot memories
- [ ] **WEBU-08**: Cost dashboard — token usage, provider costs, budget status
- [ ] **WEBU-09**: Progressive Web Application (offline capable, installable)
- [ ] **WEBU-10**: Responsive design for mobile via PWA

### Infrastructure

- [ ] **INFR-01**: SQLite for structured data with abstraction layer for future PostgreSQL migration
- [ ] **INFR-02**: Embedded vector store (LanceDB) for memory embeddings
- [ ] **INFR-03**: Event-driven architecture with typed event bus (tokio::sync::broadcast)
- [ ] **INFR-04**: Tus protocol for resumable file uploads
- [ ] **INFR-05**: Stale-while-revalidate cache strategy
- [ ] **INFR-06**: Turborepo monorepo with Cargo workspace for Rust crates
- [ ] **INFR-07**: SQLite WAL mode for concurrent read/write safety
- [ ] **INFR-08**: Dedicated thread pools for blocking operations (SQLite, WASM)

## v2 Requirements

### LLM Providers

- **LLMP-V2-01**: Local models via Ollama integration
- **LLMP-V2-02**: Additional providers as ecosystem grows

### API Layer

- **APIL-V2-01**: GraphQL API from Rust backend (async-graphql)
- **APIL-V2-02**: GraphQL BFF layer (GraphQL Yoga + Pothos) for frontend-optimized queries

### Channels

- **CHAN-V2-01**: WhatsApp channel adapter
- **CHAN-V2-02**: Slack channel adapter
- **CHAN-V2-03**: Discord channel adapter
- **CHAN-V2-04**: Telegram channel adapter

### Deployment

- **DEPL-V2-01**: Cloud SaaS hosting option
- **DEPL-V2-02**: Multi-user auth and multi-tenancy
- **DEPL-V2-03**: Per-bot token/cost budgets (upgrade from global)

### Advanced Features

- **ADVN-V2-01**: Heartbeat mechanism — autonomous agent loops (OpenClaw-style)
- **ADVN-V2-02**: A2A (Agent-to-Agent) protocol support for cross-platform agent communication
- **ADVN-V2-03**: Bot marketplace for sharing bot templates
- **ADVN-V2-04**: Voice input/output for bot chat
- **ADVN-V2-05**: Memory export with privacy controls

## Out of Scope

| Feature | Reason |
|---------|--------|
| Real-time voice/video chat | High complexity, not core to bot management. Text-first for v1 |
| Mobile native apps | PWA covers mobile use cases adequately for v1 |
| Multi-user / multi-tenancy | Single-user self-hosted for v1. Complexity doesn't justify until SaaS |
| Cloud SaaS deployment | Self-hosted only for v1. Cloud offering is v2+ |
| Messaging channels (WhatsApp, Slack, Discord, Telegram) | Web UI + API is sufficient for v1. Channel adapters are v2 |
| Local LLM models (Ollama) | Focus on cloud providers first. Local model support deferred to v2 |
| GraphQL (backend or BFF) | REST + gRPC is sufficient for v1. GraphQL adds maintenance burden |
| Bot marketplace | Config export covers sharing. Centralized marketplace is v2+ |
| Memory export/sharing | Memory stays local for privacy in v1 |
| Agent-to-Agent protocol (A2A) | Emerging standard, Rust SDK doesn't exist yet. Defer to v2 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| IDEN-01 | Phase 1 | Complete |
| IDEN-02 | Phase 1 | Complete |
| IDEN-03 | Phase 1 | Complete |
| IDEN-04 | Phase 1 | Complete |
| IDEN-05 | Phase 1 | Complete |
| IDEN-06 | Phase 1 | Complete |
| IDEN-07 | Phase 10 | Pending |
| IDEN-08 | Phase 10 | Pending |
| MEMO-01 | Phase 2 | Complete |
| MEMO-02 | Phase 3 | Complete |
| MEMO-03 | Phase 3 | Complete |
| MEMO-04 | Phase 3 | Complete |
| MEMO-05 | Phase 10 | Pending |
| MEMO-06 | Phase 3 | Complete |
| AGNT-01 | Phase 2 | Complete |
| AGNT-02 | Phase 5 | Complete |
| AGNT-03 | Phase 5 | Complete |
| AGNT-04 | Phase 5 | Complete |
| AGNT-05 | Phase 5 | Complete |
| AGNT-06 | Phase 5 | Complete |
| AGNT-07 | Phase 7 | Complete |
| AGNT-08 | Phase 7 | Complete |
| AGNT-09 | Phase 7 | Complete |
| AGNT-10 | Phase 7 | Complete |
| AGNT-11 | Phase 7 | Complete |
| AGNT-12 | Phase 5 | Complete |
| AGNT-13 | Phase 5 | Complete |
| SKIL-01 | Phase 6 | Pending |
| SKIL-02 | Phase 6 | Pending |
| SKIL-03 | Phase 6 | Pending |
| SKIL-04 | Phase 6 | Pending |
| SKIL-05 | Phase 6 | Pending |
| SKIL-06 | Phase 7 | Complete |
| SKIL-07 | Phase 6 | Pending |
| SKIL-08 | Phase 6 | Pending |
| SKIL-09 | Phase 6 | Pending |
| SKIL-10 | Phase 6 | Pending |
| WKFL-01 | Phase 8 | Pending |
| WKFL-02 | Phase 8 | Pending |
| WKFL-03 | Phase 8 | Pending |
| WKFL-04 | Phase 8 | Pending |
| WKFL-05 | Phase 8 | Pending |
| WKFL-06 | Phase 8 | Pending |
| WKFL-07 | Phase 8 | Pending |
| WKFL-08 | Phase 8 | Pending |
| WKFL-09 | Phase 8 | Pending |
| LLMP-01 | Phase 2 | Complete |
| LLMP-02 | Phase 2 | Complete |
| LLMP-03 | Phase 3 | Complete |
| LLMP-04 | Phase 3 | Complete |
| LLMP-05 | Phase 3 | Complete |
| LLMP-06 | Phase 3 | Complete |
| LLMP-07 | Phase 3 | Complete |
| LLMP-08 | Phase 3 | Complete |
| LLMP-09 | Phase 3 | Complete |
| LLMP-10 | Phase 3 | Complete |
| LLMP-11 | Phase 2 | Complete |
| MCPI-01 | Phase 9 | Pending |
| MCPI-02 | Phase 9 | Pending |
| MCPI-03 | Phase 9 | Pending |
| MCPI-04 | Phase 9 | Pending |
| MCPI-05 | Phase 9 | Pending |
| CHAT-01 | Phase 4 | Complete |
| CHAT-02 | Phase 4 | Complete |
| CHAT-03 | Phase 4 | Complete |
| CHAT-04 | Phase 2 | Complete |
| CHAT-05 | Phase 2 | Complete |
| CHAT-06 | Phase 8 | Pending |
| OBSV-01 | Phase 2 | Complete |
| OBSV-02 | Phase 5 | Complete |
| OBSV-03 | Phase 10 | Pending |
| OBSV-04 | Phase 10 | Pending |
| OBSV-05 | Phase 10 | Pending |
| OBSV-06 | Phase 5 | Complete |
| OBSV-07 | Phase 2 | Complete |
| CLII-01 | Phase 1 | Complete |
| CLII-02 | Phase 6 | Pending |
| CLII-03 | Phase 2 | Complete |
| CLII-04 | Phase 10 | Pending |
| CLII-05 | Phase 8 | Pending |
| CLII-06 | Phase 7 | Complete |
| APIL-01 | Phase 1 | Complete |
| APIL-02 | Phase 10 | Pending |
| APIL-03 | Phase 10 | Pending |
| SECU-01 | Phase 1 | Complete |
| SECU-02 | Phase 1 | Complete |
| SECU-03 | Phase 1 | Complete |
| SECU-04 | Phase 1 | Complete |
| SECU-05 | Phase 1 | Complete |
| SECU-06 | Phase 6 | Pending |
| SECU-07 | Phase 6 | Pending |
| WEBU-01 | Phase 4 | Complete |
| WEBU-02 | Phase 4 | Complete |
| WEBU-03 | Phase 4 | Complete |
| WEBU-04 | Phase 10 | Pending |
| WEBU-05 | Phase 8 | Pending |
| WEBU-06 | Phase 10 | Pending |
| WEBU-07 | Phase 10 | Pending |
| WEBU-08 | Phase 10 | Pending |
| WEBU-09 | Phase 4 | Complete |
| WEBU-10 | Phase 4 | Complete |
| INFR-01 | Phase 1 | Complete |
| INFR-02 | Phase 3 | Complete |
| INFR-03 | Phase 5 | Complete |
| INFR-04 | Phase 10 | Pending |
| INFR-05 | Phase 4 | Complete |
| INFR-06 | Phase 1 | Complete |
| INFR-07 | Phase 1 | Complete |
| INFR-08 | Phase 1 | Complete |

**Coverage:**
- v1 requirements: 109 total (corrected from initial count of 89)
- Mapped to phases: 109
- Unmapped: 0

---
*Requirements defined: 2026-02-10*
*Last updated: 2026-02-13 after Phase 5 completion*
