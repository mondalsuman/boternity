# Boternity

## What This Is

Boternity is a self-hosted platform for creating, managing, and orchestrating AI bots. Users create many bots — each with a persistent soul (versioned identity), short-term and long-term memory, and configurable skills — via CLI, web UI, REST/gRPC/GraphQL APIs, or MCP servers. Bots are powered by agents that can dynamically spawn sub-agents up to 3 levels deep, discover skills from registries, and participate in orchestrated workflows. Think of it as the infrastructure layer for running a fleet of purposeful AI bots on your own machine.

## Core Value

A user can create a bot with a distinct identity, give it skills through an interactive builder, and have meaningful parallel conversations with it — all running locally with full observability into what every agent is doing and why.

## Requirements

### Validated

(None yet — ship to validate)

### Active

#### Bot System
- [ ] Create bots via CLI, web UI, REST/gRPC/GraphQL API, or MCP server
- [ ] Each bot has a SOUL.md defining personality, values, behavior, and goals
- [ ] Soul versioning — every update tracked, rollback to any previous version
- [ ] Short-term memory — key points extracted and saved per session
- [ ] Long-term memory — persistent memory per bot across all sessions
- [ ] Common long-term memory — shared memory layer accessible by all bots
- [ ] Per-bot persistent storage for files, knowledge bases, and structured data
- [ ] Parallel chat sessions — user can chat with multiple bots simultaneously
- [ ] Multiple parallel sessions with the same bot
- [ ] Streaming responses via SSE/WebSocket — token-by-token delivery
- [ ] Bot config export (soul + skill definitions, no memory) for sharing
- [ ] Bot-to-bot direct messaging for collaboration and task delegation
- [ ] Bot-to-bot structured communication via workflows

#### Agent Architecture
- [ ] Each bot has one default agent powering it
- [ ] Agents can dynamically spawn sequential or parallel sub-agents
- [ ] Sub-agent depth capped at 3 levels
- [ ] Sub-agent communication via message passing (default) or opt-in shared workspace
- [ ] Agent creation via interactive wizard (CLI) or builder bot (UI chat)
- [ ] Universal builder agent powers both wizard and builder bot
- [ ] Builder bot asks multi-choice questions based on agent purpose (5-10 questions, then offer skip)
- [ ] Adaptive question depth — complexity of purpose drives number of questions
- [ ] Builder assesses required skills, creates them, attaches to agent

#### Skill System
- [ ] Agents powered by one or more skills
- [ ] Skill inheritance hierarchy — child skill inherits parent features plus additions
- [ ] Skills follow agentskills.io specification and validation (via skill-creator skill)
- [ ] Discover and install skills from skills.sh registry (via find-skills skill or skills CLI)
- [ ] Discover skills from ComposioHQ/awesome-claude-skills
- [ ] Universal builder agent creates skills using same mechanism for all paths
- [ ] WASM sandbox for untrusted skills from registries
- [ ] Permission model — skills declare required capabilities, user approves on install
- [ ] Trust tiers: local skills run with permissions, registry skills run sandboxed

#### Workflows & Pipelines
- [ ] Define workflows using visual builder (drag-and-drop), YAML config, or TypeScript/Rust SDK
- [ ] All three workflow representations are interchangeable
- [ ] Trigger workflows manually, via cron schedule, or via events (webhooks, messages)
- [ ] Workflows compose agents and skills into execution chains
- [ ] Event-driven architecture throughout

#### LLM Provider Support
- [ ] Pluggable provider system — Anthropic Claude, OpenAI, Google, Mistral, local models (Ollama)
- [ ] AWS Bedrock models support
- [ ] Claude.ai subscription support (OpenClaw-style)
- [ ] GLM 4.7 from z.ai support
- [ ] Configurable fallback provider chain — user sets fallback sequence
- [ ] Automatic failover when provider is down or rate-limited

#### MCP Integration
- [ ] Bots consume external MCP tools (connect to MCP servers)
- [ ] Bots expose themselves as MCP servers (other tools like Claude Code can use bots as tools)
- [ ] Create and manage bots via MCP server interface

#### Observability & Debugging
- [ ] Full distributed tracing — every agent decision, timing, parent-child relationships
- [ ] Token usage tracking per agent and per bot
- [ ] Cost dashboards with provider breakdown
- [ ] Global token/cost budget with alerts and pause when exceeded
- [ ] Visual trace explorer in UI — real-time agent tree with status, timing, decisions
- [ ] WebSocket live updates for agent spawning, workflow progress, status changes
- [ ] Structured logging throughout

#### CLI
- [ ] Full bot lifecycle management (create, configure, list, delete)
- [ ] Skill management (create, install, list, remove)
- [ ] Interactive chat with bots from terminal
- [ ] Scriptable commands — pipe input/output for automation
- [ ] Workflow management (create, trigger, list, status)

#### Security & Secrets
- [ ] Encrypted local vault (SQLite-based) for API keys and credentials
- [ ] OS keychain integration (macOS Keychain, Linux Secret Service)
- [ ] Environment variable fallback for secrets
- [ ] Skill sandboxing via WASM for untrusted registry skills
- [ ] Permission grants for skill capabilities

#### Web UI
- [ ] Dashboard with bot fleet overview
- [ ] Chat interface with streaming responses
- [ ] Visual workflow builder (drag-and-drop)
- [ ] Visual trace explorer — real-time agent chains, timing, decisions
- [ ] Bot configuration and soul editor with version history
- [ ] Skill browser and installer
- [ ] Progressive Web Application

### Out of Scope

- Multi-user auth / multi-tenancy — single-user for v1, multi-user deferred
- Cloud SaaS hosting — self-hosted only for v1
- Messaging channels (WhatsApp, Slack, Discord, Telegram) — web UI + API only for v1
- Mobile native apps — PWA covers mobile use cases for v1
- Bot marketplace / community sharing — config export only, no centralized marketplace
- Memory export/sharing — memory stays local for privacy
- Video/audio streaming in chat — text and file-based for v1

## Context

### Inspiration
OpenClaw (formerly Clawdbot/Moltbot) pioneered the "programmable soul" concept with SOUL.md — a markdown file that defines an agent's identity, read at every session start. OpenClaw is a personal AI assistant (one agent, many channels). Boternity extends this philosophy to a multi-bot platform where users manage fleets of purposeful bots, each with their own soul, memory, and capabilities.

Key OpenClaw concepts adopted:
- **SOUL.md** — persistent identity file per bot (versioned in Boternity)
- **MEMORY.md** — persistent memory across sessions (split into short-term/long-term in Boternity)
- **MCP integration** — universal tool protocol for agent capabilities
- **Heartbeat mechanism** — autonomous agent loops (adapted for scheduled workflows)

Key differentiators from OpenClaw:
- Multi-bot management vs single assistant
- Agent hierarchy with sub-agent spawning (max depth 3)
- Interactive builder bot for agent/skill creation
- Skill inheritance and registry integration
- Visual workflow builder and pipeline orchestration
- Full observability with distributed tracing and cost tracking
- Rust backend for performance vs OpenClaw's TypeScript

### Agent Skills Ecosystem
The Agent Skills specification (agentskills.io) defines the standard format for modular AI capabilities. Skills are self-contained folders with a SKILL.md file. The skills.sh registry (by Vercel) provides discovery and installation via `npx skills add <name>`. ComposioHQ/awesome-claude-skills curates community skills. Boternity integrates with all three — local skill creation following the spec, plus discovery from registries.

### Technical Environment
- Self-hosted, single-binary deployment target
- SQLite for structured data (migration path to PostgreSQL for future scale)
- Embedded vector store for memory (migration path to Qdrant/pgvector)
- Redis-compatible embedded pub/sub for event-driven architecture
- WASM runtime for skill sandboxing

## Constraints

- **Tech Stack (Frontend)**: TypeScript, Vite, React 18+, ShadCN UI, Tailwind CSS, Tanstack Router/Query/Virtual, Zustand + Zundo, Framer Motion, React Hook Form + Zod, Tiptap, Chart.js, date-fns, dnd-kit, ESLint + Prettier, Vitest + Playwright — non-negotiable
- **Tech Stack (Backend)**: Rust, Tokio + Axum (HTTP), Tonic (gRPC), async-graphql or Juniper (GraphQL) — non-negotiable
- **Tech Stack (Build)**: Turborepo monorepo — non-negotiable
- **Deployment**: Self-hosted, single-user for v1 — must work as near-single-binary
- **Sub-agent Depth**: Maximum 3 levels — hard architectural limit
- **Storage**: SQLite + embedded for v1 — must be abstractable for future PostgreSQL migration
- **Protocol**: Tus protocol for resumable file uploads
- **Caching**: Stale-while-revalidate cache strategy throughout
- **GraphQL (Frontend)**: GraphQL Yoga + Pothos GraphQL for frontend GraphQL layer — Note: backend exposes REST + gRPC + GraphQL natively in Rust; the Pothos/Yoga layer may serve as a BFF or be used for frontend-specific schema needs

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust backend, TypeScript frontend | Performance for agent orchestration + rich UI experience | — Pending |
| SQLite for v1 with migration path | Single-binary deployment, lower ops burden. Abstract storage layer for future PostgreSQL | — Pending |
| WASM sandboxing for untrusted skills | Security without sacrificing performance. Permission model for trusted local skills | — Pending |
| Versioned souls (not immutable, not freely mutable) | Identity should evolve intentionally with full history. Rollback as safety net | — Pending |
| Message passing + opt-in shared workspace | Clean defaults (isolated agents) with escape hatch (shared state when needed) | — Pending |
| Global token budget (not per-bot) | Simpler for single-user v1. Per-bot budgets can layer on later | — Pending |
| Streaming responses via SSE/WebSocket | Real-time feel is essential for chat UX. Non-negotiable | — Pending |
| Three workflow definition formats | Visual builder for accessibility, YAML for version control, SDK for programmatic use | — Pending |
| Config export only (no memory export) | Privacy-first. Soul + skills are shareable identity. Memory is private experience | — Pending |

---
*Last updated: 2026-02-10 after initialization*
