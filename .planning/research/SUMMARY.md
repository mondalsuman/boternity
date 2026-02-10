# Project Research Summary

**Project:** Boternity
**Domain:** Self-hosted AI bot management and orchestration platform (Rust backend + TypeScript frontend)
**Researched:** 2026-02-10
**Confidence:** MEDIUM-HIGH

## Executive Summary

Boternity is a self-hosted platform for creating, managing, and orchestrating fleets of AI bots -- each with persistent identity (SOUL.md), memory, skills, and multi-provider LLM support. The market has single-agent platforms (OpenClaw, memU) and workflow-focused tools (n8n), but no self-hosted platform offers unified multi-bot fleet management with hierarchical agent orchestration. This is the gap Boternity fills. Experts build this type of platform using a layered architecture with clean separation between domain logic, infrastructure, and API layers -- the Rust ecosystem (Axum + Tokio + SeaORM) is well-suited for this, with the `llm` crate for provider abstraction, LanceDB for embedded vector storage, Wasmtime for WASM skill sandboxing, and `rmcp` for the MCP protocol.

The recommended approach is a trait-driven Rust backend (6 crates in a Cargo workspace) paired with a TypeScript frontend monorepo (Turborepo + pnpm). Build the foundation as a single-binary with SQLite + embedded vector storage, no external services required. The architecture is event-driven internally (Tokio broadcast channels) with protocol multiplexing on a single port (REST + gRPC + GraphQL + MCP). The critical design decision is making bot identity files (SOUL.md) **immutable at runtime** -- this prevents the most dangerous attack vector (persistent prompt injection via identity file manipulation, demonstrated by CVE-2026-25253 against OpenClaw).

The top risks are: (1) prompt injection compromising bot identity files, mitigated by read-only SOUL.md with admin-gated writes and hash verification; (2) agentic resource exhaustion from infinite loops or cascading sub-agent spawning, mitigated by hard budget caps at every level, cycle detection, and circuit breakers; (3) memory poisoning across the shared memory layer, mitigated by trust-leveled partitioning and write validation; and (4) WASM sandbox escape from runtime vulnerabilities, mitigated by defense-in-depth with OS-level sandboxing layered on top of Wasmtime. All four require security-first design from phase 1, not retrofitting.

## Key Findings

### Recommended Stack

The stack splits into a Rust backend (Cargo workspace) and TypeScript frontend (Turborepo + pnpm). The frontend stack is pre-decided (React 18+, Vite, ShadCN UI, TanStack Router/Query, Zustand). The backend research filled critical gaps with high-confidence crate recommendations verified via docs.rs and official repos.

**Core technologies:**
- **`llm` (graniet) 1.2.4:** Unified multi-provider LLM client -- use for all LLM communication (streaming, function calling, structured output); build custom orchestration on top rather than using an opinionated agent framework
- **SeaORM 1.1.19:** Async ORM with backend-generic entity API -- the only ORM that lets you write SQLite code in v1 and switch to PostgreSQL by changing a feature flag without rewriting queries
- **LanceDB 0.26.2:** Embedded persistent vector DB (Rust-native) -- for bot long-term memory; zero-copy, automatic versioning, clear migration path to hosted service
- **Wasmtime 41.0.3:** WASM runtime with component model -- for sandboxed execution of untrusted registry skills; strongest security track record, Bytecode Alliance backed
- **`rmcp` 0.15.0:** Official Rust MCP SDK -- for both consuming external MCP tools and exposing bots as MCP servers; the only official implementation
- **Tokio broadcast/mpsc/watch:** Event bus built on Tokio primitives -- zero additional dependencies, typed in-process pub/sub for all internal component communication
- **`tracing` + OpenTelemetry:** Structured observability from day one -- spans trace through agent hierarchies including LLM calls, tool use, and sub-agent spawning
- **`async-graphql` 7.2.1:** Rust-native GraphQL with Axum integration and WebSocket subscriptions -- the backend GraphQL layer, complementing the TypeScript BFF (Yoga + Pothos)

**Critical version requirements:** Wasmtime 41.x must pair with wit-bindgen 0.51.x. OpenTelemetry 0.31.x must pair with tracing-opentelemetry 0.32.x. SeaORM 1.1.x pins its own SQLx version.

### Expected Features

**Must have (v1 table stakes):**
- T1: Bot identity system (SOUL.md/IDENTITY.md/USER.md) -- the atomic unit everything builds on
- T2: Multi-LLM provider support (OpenAI + Anthropic + Ollama minimum)
- T3: Session-scoped conversation memory
- T5: MCP tool consumption (interoperability baseline)
- T7: Web chat interface with streaming
- T8: CLI management tool (developer-first audience)
- T10: Basic observability (logs + traces)
- T11: Token/cost tracking per bot per provider
- T12: File-based configuration (GitOps-friendly)
- D1: Fleet dashboard (the differentiating feature from day 1)

**Should have (v1.x after core validation):**
- T4: Long-term memory (vector DB, cross-session persistence)
- T6: Skill/tool system with registry integration
- T9: Webhooks and trigger system
- T14: Provider fallback chains
- D6: Proactive behavior (heartbeat system)
- D10: Cost dashboard with budget controls
- D12: Sandboxed execution (must precede opening skill system to untrusted skills)
- T13: Multi-channel (Slack + Discord first)

**Defer (v2+):**
- D2: Hierarchical agent orchestration (max depth 3)
- D3: Bot-to-bot communication (A2A protocol)
- D4: Shared memory across bots
- D5: MCP server exposure (bidirectional)
- D7: Workflow orchestration (YAML-first, visual builder later)
- D8: Interactive agent builder wizard
- D9: Real-time trace explorer (AG-UI protocol)
- D11: Skill registry integration (ClawHub compatibility)

### Architecture Approach

The system uses a clean four-layer architecture: API Gateway (protocol multiplexing on single port), Service Layer (Bot Manager, Agent Orchestrator, Skill Registry, Memory System, LLM Abstraction, MCP Manager, Observability Engine), Event Bus (typed Tokio broadcast channels), and Infrastructure Layer (SQLite, vector store, WASM sandbox, secret vault). The dependency rule is strict: `boternity-core` defines traits, `boternity-infra` implements them, and `boternity-api` composes everything. Core never depends on infrastructure (dependency inversion).

**Major components:**
1. **Bot Manager** -- Bot CRUD, soul versioning (immutable snapshots), lifecycle state machine (create/start/stop/delete)
2. **Agent Orchestrator** -- Agent lifecycle, sub-agent spawning with depth-3 cap, message passing via mpsc channels, execution tracking
3. **LLM Provider Abstraction** -- Trait-based pluggable providers with streaming, fallback chains, circuit breakers, rate limiting
4. **Memory System** -- Short-term (session buffer), long-term (per-bot vector store), common (shared with ACL); embedding generation for semantic recall
5. **Event Bus** -- Typed broadcast channels for decoupled cross-component communication; fan-out to WebSocket for real-time UI
6. **MCP Manager** -- Bidirectional: consume external MCP servers (client) + expose bot capabilities as MCP tools (server)
7. **WASM Sandbox** -- Wasmtime with WASI 0.2 component model; capability-based permissions; trust-tiered execution (local vs registry vs unknown)

### Critical Pitfalls

1. **SOUL.md identity file manipulation (CVE-2026-25253)** -- Make SOUL.md immutable at runtime; all edits through admin API only; SHA-256 hash verification at startup; never let bots write their own identity files. *Address in Phase 1.*
2. **Agentic resource exhaustion (infinite loop cost explosion)** -- Hard limits at every level: max 15 iterations/agent, max 60s execution, max 50K tokens/request, global budget ceiling, total agent count cap per request (max 10), cycle detection on repeated tool calls. *Address in Phase 2.*
3. **Memory poisoning across shared memory** -- Trust-leveled memory partitioning (system core / per-bot / shared); block direct shared-memory writes; validation queue with injection detection; provenance tracking on every entry. *Address in Phase 3.*
4. **MCP tool poisoning and privilege escalation** -- Sanitize tool descriptions before LLM context; mandatory authentication on all MCP connections; least-privilege tool access; human-in-the-loop for destructive operations. *Address in Phase 5.*
5. **Context window overflow** -- Fixed token budgets per context segment (system prompt, soul, memory, tools, conversation); truncate tool outputs; sliding window history with summarization; structured summaries between agent levels. *Address from Phase 2 onward.*

## Implications for Roadmap

Based on combined research, the recommended structure is 6 phases following the dependency chain discovered in both Architecture and Features research.

### Phase 1: Foundation + Bot Identity + Single-Agent Chat
**Rationale:** Everything depends on `boternity-types` and `boternity-core`. Bot identity is the atomic unit. You need one bot talking to one LLM before anything else works. SQLite setup with WAL mode from day one prevents the single-writer contention pitfall.
**Delivers:** Create a bot with SOUL.md, chat with it via CLI (one LLM provider). Immutable soul file handling with hash verification. SQLite storage with repository trait abstraction.
**Addresses features:** T1 (Bot Identity), T3 (Session Memory), T8 (CLI basics), T12 (File-based Config)
**Avoids pitfalls:** SOUL.md manipulation (immutable from day one), SQLite contention (WAL mode + busy_timeout from day one), hardcoded provider (provider trait defined even if only one impl)
**Stack elements:** Axum, SeaORM (SQLite), `llm` crate, `clap`, `tracing`, `secrecy`

### Phase 2: Multi-Provider + Memory + Web UI
**Rationale:** Memory system requires the bot and LLM layers from Phase 1. Multiple providers require the LLM trait from Phase 1. The web UI needs a working backend to connect to. Feature research shows T2 (multi-provider) and T7 (chat UI) are both v1 table stakes.
**Delivers:** Bots remember conversations (session + long-term). Multiple LLM providers with fallback. Web UI for chatting and basic bot management. Token/cost tracking.
**Addresses features:** T2 (Multi-LLM), T4 (Long-term Memory), T7 (Chat Interface), T10 (Observability), T11 (Token Tracking), T14 (Fallback Chains)
**Avoids pitfalls:** Context window overflow (token budgeting built into agent engine), agentic resource exhaustion (budget caps and cycle detection), synchronous embedding blocking (async background embedding)
**Stack elements:** LanceDB, `vectorlite`, GraphQL BFF (Yoga + Pothos), React frontend, OpenTelemetry

### Phase 3: Fleet Management + Event System + Agent Hierarchy
**Rationale:** Fleet management (the core differentiator D1) requires multiple bots with identity and observability (Phase 1 + 2). The event bus is needed before real-time UI updates. Sub-agent spawning depends on the agent engine from Phase 2. Architecture research shows the event bus should exist before WebSocket streaming.
**Delivers:** Fleet dashboard showing all bots with status and metrics. Event-driven internal communication. Sub-agent spawning with depth-3 limit. WebSocket live updates.
**Addresses features:** D1 (Fleet Dashboard), D2 (Hierarchical Orchestration basics), D4 (Shared Memory foundations)
**Avoids pitfalls:** Memory poisoning (trust-leveled partitioning from initial memory architecture), agent infinite loops (depth cap + total agent cap + circuit breakers between levels), shared mutable state anti-pattern (message passing default, shared workspace opt-in)
**Stack elements:** Tokio broadcast/mpsc channels, WebSocket (Axum ws), `dashmap`

### Phase 4: Skill System + WASM Sandbox
**Rationale:** Skills extend bot capabilities but require sandboxed execution for safety. Architecture research says sandboxing must precede opening the skill system to untrusted code. Feature dependency graph shows D12 (Sandboxing) should precede T6 (Skills).
**Delivers:** Skill definition format, local skill execution, WASM sandbox for registry skills, skill installation CLI, trust-tiered execution.
**Addresses features:** T6 (Skill System), D12 (Sandboxed Execution), D11 (Skill Registry basics)
**Avoids pitfalls:** WASM sandbox escape (defense-in-depth: Wasmtime + OS-level sandbox), skill permission escalation (runtime enforcement at WASI level, not just declaration), WASM cold starts (pre-compile at install, cache compiled modules)
**Stack elements:** Wasmtime 41.x, wit-bindgen 0.51.x, wasmtime-wasi

### Phase 5: MCP Integration + Triggers + Proactive Behavior
**Rationale:** MCP consumption is a v1 table stake (T5) but depends on the skill system foundation from Phase 4 (MCP tools are a skill type). Bidirectional MCP (D5) extends consumption. Triggers (T9) enable proactive behavior (D6). The pitfall research warns MCP security must be designed before any external tool connectivity.
**Delivers:** MCP client (consume external tools), MCP server (expose bots as tools), webhook/trigger system, heartbeat/cron scheduler, proactive bot behavior.
**Addresses features:** T5 (MCP Consumption), D5 (MCP Server Exposure), T9 (Triggers), D6 (Proactive Behavior)
**Avoids pitfalls:** MCP tool poisoning (description sanitization, mandatory auth, tool signing), privilege escalation (least-privilege access, MCP firewall layer), confused deputy problem (user context always passed through)
**Stack elements:** `rmcp` 0.15.0, `tokio-cron-scheduler`

### Phase 6: Workflows + Multi-Channel + Polish
**Rationale:** Workflows require skills (Phase 4) and triggers (Phase 5) to be meaningful. Multi-channel deployment is high-cost and should come after the core platform is stable. Feature research explicitly recommends YAML workflows before visual builder.
**Delivers:** YAML-defined workflow engine, multi-channel deployment (Slack + Discord first), cost dashboard with budget controls, interactive agent builder, bot templates.
**Addresses features:** D7 (Workflow Orchestration), T13 (Multi-channel), D10 (Cost Dashboard), D8 (Interactive Builder), D13 (Bot Templates)
**Avoids pitfalls:** Visual builder complexity (YAML-first, visual later), builder bot question fatigue (3-question quick-create), workflow error recovery (idempotent steps, state tracking)
**Stack elements:** DAG executor, channel adapter pattern, `dnd-kit` (frontend)

### Phase Ordering Rationale

- **Phase 1 before everything:** Bot identity and single-agent chat are the atomic unit. Architecture research confirms `boternity-types` -> `boternity-core` -> `boternity-infra` is the correct build order. Pitfall research demands SOUL.md immutability from day one.
- **Phase 2 before Phase 3:** Memory and multi-provider are prerequisites for meaningful fleet management. You cannot manage a fleet without observability to monitor bots (feature dependency: D1 requires T10).
- **Phase 3 before Phase 4:** Fleet management and the event bus are needed before skills because skills generate events that need to flow through the system. Agent hierarchy is needed before skills because sub-agents use skills.
- **Phase 4 before Phase 5:** MCP tools are conceptually a skill type. The sandbox infrastructure from Phase 4 protects against MCP tool poisoning in Phase 5.
- **Phase 5 before Phase 6:** Workflows require triggers (Phase 5) and skills (Phase 4) to execute meaningful automations.
- **Security pitfalls are front-loaded:** The most dangerous pitfalls (SOUL.md injection, resource exhaustion, memory poisoning) map to Phases 1-3 and must be addressed there, not retrofitted later.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3 (Fleet + Events):** Shared memory architecture with trust levels is novel -- no off-the-shelf pattern exists. The memory partitioning and provenance tracking system needs design research.
- **Phase 4 (Skills + WASM):** WIT interface design for the skill system requires experimentation. The trust-tiering system (local vs registry vs unknown) needs detailed security modeling.
- **Phase 5 (MCP):** Bidirectional MCP is cutting-edge (Microsoft just added it Feb 2026). The MCP firewall/sanitization layer has no established patterns to follow.

Phases with standard patterns (skip deep research):
- **Phase 1 (Foundation):** Axum + SeaORM + SQLite + clap are all well-documented with established patterns. Bot CRUD + REST API is standard.
- **Phase 2 (Multi-Provider + UI):** LLM provider abstraction, React frontend, GraphQL BFF are all well-trodden paths. Token counting and cost tracking have clear reference implementations (Langfuse, Portkey).
- **Phase 6 (Workflows):** DAG-based workflow execution is a well-known pattern (n8n, Dagu, dagrs all provide reference architectures).

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | MEDIUM-HIGH | Most crate versions verified via docs.rs with pinned versions. `llm` (graniet) and `rmcp` are newer crates -- watch for API churn. `ts-rs`/`specta` for Rust-to-TS type generation need validation. |
| Features | MEDIUM | Feature landscape verified across multiple platforms (OpenClaw, memU, n8n, Retool). MVP definition is solid. Competitor analysis is thorough but platforms evolve fast. |
| Architecture | MEDIUM-HIGH | Core patterns (protocol multiplexing, event bus, repository pattern, agent tree) all verified with production examples. The dual-GraphQL architecture (Rust async-graphql + TS Yoga/Pothos) adds complexity -- validate BFF necessity early. |
| Pitfalls | HIGH | All critical pitfalls sourced from real incidents (CVE-2026-25253, OWASP ASI06), security research (Penligent, Adversa AI), and Rust ecosystem experience. Recovery strategies included. |

**Overall confidence:** MEDIUM-HIGH

### Gaps to Address

- **Dual-GraphQL architecture complexity:** The BFF layer (Yoga + Pothos) between React and Rust adds a Node.js process. Validate early whether `async-graphql` alone (served from Rust) is sufficient for the frontend, eliminating the BFF. This decision impacts Phase 2.
- **`llm` crate maturity:** The `llm` (graniet) crate at v1.2.4 is relatively new. If its API proves unstable, fall back to building a thin wrapper over `reqwest` with provider-specific modules, or evaluate `rig-core` as the primary.
- **LanceDB vs sqlite-vec:** Architecture research references `sqlite-vec` while Stack research recommends `lancedb`. Both are viable. Decide in Phase 2 -- LanceDB is more capable (persistent, versioned, combined search) but adds a dependency; sqlite-vec is lighter and stays within the SQLite ecosystem.
- **TypeScript CLI vs Rust CLI:** Architecture research places CLI in `packages/cli` (TypeScript) while Stack research recommends `clap` (Rust). Decide: TS CLI is faster to build and shares types with the frontend; Rust CLI ships as a single binary and avoids Node.js dependency. For a self-hosted developer tool, **Rust CLI is the stronger choice**.
- **Tus protocol necessity:** Stack research suggests deferring tus in favor of simple multipart upload if upload volume is low. Validate actual upload requirements before implementing.
- **A2A protocol implementation:** Listed as v2+ but has no Rust SDK or reference implementation. Will require significant custom development when the time comes.

## Sources

### Primary (HIGH confidence)
- docs.rs: rmcp 0.15.0, sea-orm 1.1.19, wasmtime 41.0.3, lancedb 0.26.2, clap 4.5.57, axum 0.8.8, tokio 1.49.0, tracing 0.1.44, async-graphql 7.2.1, tonic 0.14.3
- GitHub: graniet/llm 1.2.4, modelcontextprotocol/rust-sdk, SeaQL/sea-orm, bytecodealliance/wasmtime
- Official docs: MCP Specification 2025-11-25, Wasmtime Security, SeaORM backend-agnostic design, OpenTelemetry Rust

### Secondary (MEDIUM confidence)
- Platform analysis: OpenClaw docs, memU GitHub, n8n architecture (DeepWiki), Retool agents overview
- Security research: Penligent (OpenClaw prompt injection), Adversa AI (OpenClaw security), OWASP ASI06 (memory poisoning), Practical DevSecOps (MCP vulnerabilities)
- Architecture patterns: Microsoft AI Agent Design Patterns, FP Complete (Axum + Tonic cohosting), Tokio channels documentation

### Tertiary (LOW confidence -- validate before depending on)
- `ts-rs` / `specta` for Rust-to-TypeScript type generation (need API stability check)
- LanceDB-to-Qdrant migration similarity (community reports, untested)
- A2A protocol implementation feasibility in Rust (no SDK exists)
- Turborepo + Cargo workspace bridge strategy (community patterns, not official)

---
*Research completed: 2026-02-10*
*Ready for roadmap: yes*
