# Architecture Research

**Domain:** Self-hosted AI bot management and orchestration platform
**Researched:** 2026-02-10
**Confidence:** MEDIUM-HIGH

## Standard Architecture

### System Overview

```
                                 CLIENTS
  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
  │  Web UI   │  │   CLI    │  │   SDK    │  │ MCP Host │
  │  (React)  │  │  (TS)    │  │  (TS)    │  │(External)│
  └─────┬─────┘  └─────┬────┘  └────┬─────┘  └────┬─────┘
        │               │           │              │
        │  GraphQL/WS   │  REST     │  REST/gRPC   │  MCP (JSON-RPC)
        ▼               ▼           ▼              ▼
  ┌──────────┐   ┌─────────────────────────────────────────────┐
  │ GraphQL  │   │              RUST BACKEND                    │
  │   BFF    │──▶│  ┌─────────────────────────────────────┐    │
  │(Yoga +   │   │  │          API Gateway Layer           │    │
  │ Pothos)  │   │  │  REST (Axum) + gRPC (Tonic) +       │    │
  └──────────┘   │  │  GraphQL (async-graphql) + MCP       │    │
                 │  └──────────────┬────────────────────────┘    │
                 │                 │                              │
                 │  ┌──────────────┴────────────────────────┐    │
                 │  │          Service Layer                 │    │
                 │  │                                        │    │
                 │  │  ┌──────────┐  ┌──────────────────┐   │    │
                 │  │  │   Bot    │  │     Agent         │   │    │
                 │  │  │ Manager  │  │  Orchestrator     │   │    │
                 │  │  └──────────┘  └──────────────────┘   │    │
                 │  │  ┌──────────┐  ┌──────────────────┐   │    │
                 │  │  │  Skill   │  │    Workflow       │   │    │
                 │  │  │ Registry │  │     Engine        │   │    │
                 │  │  └──────────┘  └──────────────────┘   │    │
                 │  │  ┌──────────┐  ┌──────────────────┐   │    │
                 │  │  │  Memory  │  │   LLM Provider   │   │    │
                 │  │  │  System  │  │   Abstraction     │   │    │
                 │  │  └──────────┘  └──────────────────┘   │    │
                 │  │  ┌──────────┐  ┌──────────────────┐   │    │
                 │  │  │  MCP     │  │  Observability   │   │    │
                 │  │  │ Manager  │  │     Engine        │   │    │
                 │  │  └──────────┘  └──────────────────┘   │    │
                 │  └───────────────────────────────────────┘    │
                 │                 │                              │
                 │  ┌──────────────┴────────────────────────┐    │
                 │  │         Event Bus (Pub/Sub)            │    │
                 │  │    tokio::sync::broadcast channels     │    │
                 │  └──────────────┬────────────────────────┘    │
                 │                 │                              │
                 │  ┌──────────────┴────────────────────────┐    │
                 │  │        Infrastructure Layer            │    │
                 │  │                                        │    │
                 │  │  ┌──────────┐  ┌──────────────────┐   │    │
                 │  │  │ Storage  │  │    WASM           │   │    │
                 │  │  │ (SQLite) │  │   Sandbox         │   │    │
                 │  │  └──────────┘  │  (Wasmtime)       │   │    │
                 │  │  ┌──────────┐  └──────────────────┘   │    │
                 │  │  │ Vector   │  ┌──────────────────┐   │    │
                 │  │  │  Store   │  │  Secret Vault    │   │    │
                 │  │  │(sqlite-  │  │ (OS Keychain +   │   │    │
                 │  │  │   vec)   │  │  SQLite encrypt) │   │    │
                 │  │  └──────────┘  └──────────────────┘   │    │
                 │  └───────────────────────────────────────┘    │
                 └──────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| **API Gateway Layer** | Protocol multiplexing (REST, gRPC, GraphQL, MCP), request routing, auth, rate limiting | Axum router with content-type based dispatch to Tonic (gRPC) vs Axum (REST) vs async-graphql; MCP via JSON-RPC over streamable HTTP |
| **Bot Manager** | Bot CRUD, soul versioning, config management, lifecycle (create/start/stop/delete) | Service crate with state machine per bot; soul versions stored as immutable snapshots in SQLite |
| **Agent Orchestrator** | Agent lifecycle, sub-agent spawning (max depth 3), message passing, shared workspace, execution tracking | Tokio task tree with parent-child tracking; `broadcast` channels for message passing; depth counter enforced at spawn |
| **Skill Registry** | Skill discovery, installation, validation, inheritance resolution, trust classification | Local skill scanner + HTTP clients to agentskills.io/skills.sh/ComposioHQ registries; WASM compilation for untrusted |
| **Workflow Engine** | DAG definition, execution scheduling (cron/event/manual), step orchestration, state tracking | Custom DAG executor with YAML/JSON intermediate representation; cron via `tokio-cron-scheduler` |
| **Memory System** | Short-term (session), long-term (per-bot), common (shared) memory; semantic recall via embeddings | SQLite for structured memory + sqlite-vec for vector search; embedding generation via LLM provider |
| **LLM Provider Abstraction** | Pluggable provider interface, streaming, fallback chains, rate limit handling | Trait-based provider system; inspired by `llm-connector`/`rig` patterns; streaming via `tokio::Stream` |
| **MCP Manager** | Consume external MCP servers + expose bots as MCP servers | JSON-RPC 2.0 over streamable HTTP transport; capability negotiation per MCP 2025-11-25 spec |
| **Observability Engine** | Distributed tracing, token/cost tracking, live trace explorer feed | `tracing` crate + `tracing-opentelemetry` + custom token counting layer; WebSocket feed for live UI |
| **Event Bus** | Internal pub/sub for decoupled communication between all components | `tokio::sync::broadcast` channels with typed event enums; fan-out to WebSocket for UI |
| **Storage Layer** | Abstracted persistence with SQLite v1, migration path to PostgreSQL | Trait-based repository pattern; SQLx for query execution; `refinery` for migrations |
| **WASM Sandbox** | Secure execution of untrusted registry skills | Wasmtime with WASI 0.2 component model; capability-based permissions; resource limits |
| **Secret Vault** | Encrypted API key storage, OS keychain integration | SQLite with `aes-gcm` encryption at rest; `security-framework` (macOS) / `secret-service` (Linux) |
| **GraphQL BFF** | Frontend-optimized GraphQL schema, real-time subscriptions | GraphQL Yoga + Pothos in Node.js; proxies to Rust backend REST/gRPC; WebSocket subscriptions |

## Recommended Project Structure

```
boternity/
├── Cargo.toml                    # Workspace root
├── turbo.json                    # Turborepo config
├── package.json                  # Root package.json (pnpm workspace)
├── pnpm-workspace.yaml           # pnpm workspace definition
│
├── crates/                       # Rust backend (Cargo workspace)
│   ├── boternity-api/            # API gateway — protocol multiplexing
│   │   ├── src/
│   │   │   ├── rest/             # Axum REST handlers
│   │   │   ├── grpc/             # Tonic gRPC service implementations
│   │   │   ├── graphql/          # async-graphql schema + resolvers
│   │   │   ├── mcp/              # MCP server/client endpoints
│   │   │   ├── ws/               # WebSocket handlers (streaming, events)
│   │   │   └── lib.rs            # Router composition, server startup
│   │   └── Cargo.toml
│   │
│   ├── boternity-core/           # Domain logic — bot, agent, skill, workflow
│   │   ├── src/
│   │   │   ├── bot/              # Bot entity, soul versioning, lifecycle
│   │   │   ├── agent/            # Agent orchestrator, sub-agent spawning
│   │   │   ├── skill/            # Skill registry, inheritance, validation
│   │   │   ├── workflow/         # Workflow engine, DAG executor, triggers
│   │   │   ├── memory/           # Memory system (short/long/common)
│   │   │   ├── llm/              # LLM provider abstraction, fallback chains
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── boternity-infra/          # Infrastructure — storage, events, secrets
│   │   ├── src/
│   │   │   ├── storage/          # Repository trait impls (SQLite, future PG)
│   │   │   ├── vector/           # Vector store abstraction (sqlite-vec)
│   │   │   ├── events/           # Event bus, typed events, pub/sub
│   │   │   ├── secrets/          # Secret vault, keychain integration
│   │   │   ├── wasm/             # WASM sandbox (Wasmtime runtime)
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── boternity-observe/        # Observability — tracing, metrics, cost
│   │   ├── src/
│   │   │   ├── tracing/          # Distributed tracing setup
│   │   │   ├── metrics/          # Token counting, cost tracking
│   │   │   ├── budget/           # Global budget enforcement, alerts
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   ├── boternity-mcp/            # MCP protocol — client + server
│   │   ├── src/
│   │   │   ├── client/           # Consume external MCP servers
│   │   │   ├── server/           # Expose bots as MCP servers
│   │   │   ├── transport/        # Streamable HTTP, stdio transports
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   │
│   └── boternity-types/          # Shared types, error definitions
│       ├── src/
│       │   ├── models/           # Domain models (Bot, Agent, Skill, etc.)
│       │   ├── events/           # Event type definitions
│       │   ├── errors/           # Error hierarchy
│       │   └── lib.rs
│       └── Cargo.toml
│
├── packages/                     # TypeScript packages (pnpm workspace)
│   ├── web/                      # React frontend (Vite + React 18+)
│   │   ├── src/
│   │   │   ├── features/         # Feature-sliced: chat, bots, workflows...
│   │   │   ├── shared/           # Shared UI components, hooks, utils
│   │   │   ├── app/              # App shell, routing, providers
│   │   │   └── main.tsx
│   │   └── package.json
│   │
│   ├── cli/                      # CLI application (TypeScript)
│   │   ├── src/
│   │   │   ├── commands/         # Command implementations
│   │   │   ├── client/           # API client (REST)
│   │   │   └── index.ts
│   │   └── package.json
│   │
│   ├── sdk/                      # TypeScript SDK for programmatic use
│   │   ├── src/
│   │   │   ├── client/           # API client with typed methods
│   │   │   ├── types/            # Generated types from Rust models
│   │   │   └── index.ts
│   │   └── package.json
│   │
│   ├── graphql-bff/              # GraphQL BFF layer (Yoga + Pothos)
│   │   ├── src/
│   │   │   ├── schema/           # Pothos schema definitions
│   │   │   ├── resolvers/        # Resolvers proxying to Rust backend
│   │   │   ├── subscriptions/    # WebSocket subscription handlers
│   │   │   └── server.ts
│   │   └── package.json
│   │
│   └── shared/                   # Shared TypeScript types and utilities
│       ├── src/
│       │   ├── types/            # Cross-package type definitions
│       │   ├── schemas/          # Zod validation schemas
│       │   └── index.ts
│       └── package.json
│
└── docs/                         # Documentation (not code)
```

### Structure Rationale

- **`crates/` (Cargo workspace):** Six crates following clean architecture. `boternity-types` is the leaf dependency (no external deps). `boternity-core` depends on `types` only. `boternity-infra` implements core traits. `boternity-api` depends on everything to compose the server. This mirrors n8n's layered monorepo but in Rust's crate system.
- **`packages/` (pnpm workspace):** TypeScript packages managed by Turborepo. The `graphql-bff` package exists because the frontend needs a Node.js GraphQL layer (Yoga + Pothos) that proxies to the Rust backend, providing frontend-optimized queries and real-time subscriptions.
- **Separation of `boternity-mcp`:** MCP is complex enough (bidirectional, transport negotiation, capability-based) to warrant its own crate rather than living inside `boternity-api`.
- **`boternity-observe` as separate crate:** Observability crosscuts everything. Isolating it allows any crate to instrument itself without circular dependencies. Inspired by how OpenTelemetry is layered in the Rust ecosystem.

### Dependency Flow (Crates)

```
boternity-api
  ├── boternity-core
  │     └── boternity-types
  ├── boternity-infra
  │     ├── boternity-core
  │     └── boternity-types
  ├── boternity-mcp
  │     ├── boternity-core
  │     └── boternity-types
  ├── boternity-observe
  │     └── boternity-types
  └── boternity-types
```

**Key rule:** `boternity-core` never depends on `boternity-infra`. Core defines traits; infra implements them. This is the dependency inversion principle that enables swapping SQLite for PostgreSQL later.

## Architectural Patterns

### Pattern 1: Protocol Multiplexing (Content-Type Dispatch)

**What:** Run REST (Axum), gRPC (Tonic), and GraphQL (async-graphql) on a single port by inspecting the `Content-Type` header. gRPC requests have `Content-Type: application/grpc`; everything else routes to Axum.

**When to use:** Always for Boternity -- the project requires REST + gRPC + GraphQL from a single backend process.

**Trade-offs:** Simplifies deployment (one port, one binary) but requires careful middleware ordering. Tower middleware works across both Axum and Tonic since they share the same service abstraction.

**Example (Rust):**
```rust
use axum::Router;
use tonic::transport::Server as TonicServer;
use hyper::Request;

// Build Axum routes (REST + GraphQL)
let axum_app = Router::new()
    .nest("/api", rest_routes)
    .nest("/graphql", graphql_routes)
    .layer(TraceLayer::new_for_http());

// Build Tonic gRPC services
let grpc_service = TonicServer::builder()
    .add_service(bot_service)
    .add_service(agent_service)
    .into_service();

// Dispatch based on content-type
let combined = tower::steer::Steer::new(
    vec![axum_app.into_service(), grpc_service],
    |req: &Request<_>, _services: &[_]| {
        if req.headers().get("content-type")
            .map(|v| v.as_bytes().starts_with(b"application/grpc"))
            .unwrap_or(false)
        { 1 } else { 0 }
    },
);
```

**Confidence:** HIGH -- Multiple production examples exist (FP Complete blog series, `http-grpc-cohosting` repo, `axum-tonic` crate).

### Pattern 2: Event-Driven Component Communication

**What:** All internal component communication flows through a typed event bus built on `tokio::sync::broadcast` channels. Components publish events (BotCreated, AgentSpawned, WorkflowTriggered) and subscribe to events they care about. No direct cross-component method calls for side effects.

**When to use:** For all cross-component communication (bot lifecycle -> observability, agent spawning -> UI updates, workflow triggers -> agent orchestration).

**Trade-offs:** Decouples components (a new observer does not require changing the publisher) at the cost of eventual consistency and harder debugging. Mitigated by the observability engine tracing all events.

**Example (Rust):**
```rust
// Typed event enum
#[derive(Clone, Debug)]
pub enum BotEvent {
    Created { bot_id: BotId, soul_version: u32 },
    Started { bot_id: BotId },
    Stopped { bot_id: BotId },
    SoulUpdated { bot_id: BotId, old_version: u32, new_version: u32 },
}

// Event bus using tokio broadcast
pub struct EventBus {
    bot_events: broadcast::Sender<BotEvent>,
    agent_events: broadcast::Sender<AgentEvent>,
    workflow_events: broadcast::Sender<WorkflowEvent>,
    // ... per-domain channels
}

impl EventBus {
    pub fn subscribe_bot_events(&self) -> broadcast::Receiver<BotEvent> {
        self.bot_events.subscribe()
    }

    pub fn publish_bot_event(&self, event: BotEvent) {
        // Ignore error if no subscribers
        let _ = self.bot_events.send(event);
    }
}
```

**Confidence:** HIGH -- `tokio::sync::broadcast` is production-stable and the recommended approach for fan-out pub/sub in Tokio. Multiple community implementations confirm this pattern.

### Pattern 3: Trait-Based Storage Abstraction (Repository Pattern)

**What:** Define storage interfaces as Rust traits in `boternity-core`. Implement them for SQLite in `boternity-infra`. This allows swapping to PostgreSQL later by adding a new implementation without changing business logic.

**When to use:** All persistent storage access. This is the core pattern enabling the SQLite -> PostgreSQL migration path.

**Trade-offs:** Adds a layer of indirection; slightly more code. But absolutely necessary given the explicit requirement for storage migration. SQLx supports both SQLite and PostgreSQL, making the implementation switch manageable.

**Example (Rust):**
```rust
// In boternity-core (trait definition)
#[async_trait]
pub trait BotRepository: Send + Sync {
    async fn create(&self, bot: &CreateBot) -> Result<Bot, StorageError>;
    async fn get(&self, id: &BotId) -> Result<Option<Bot>, StorageError>;
    async fn list(&self, filter: &BotFilter) -> Result<Vec<Bot>, StorageError>;
    async fn update_soul(&self, id: &BotId, soul: &Soul) -> Result<SoulVersion, StorageError>;
    async fn delete(&self, id: &BotId) -> Result<(), StorageError>;
}

// In boternity-infra (SQLite implementation)
pub struct SqliteBotRepository {
    pool: SqlitePool,
}

#[async_trait]
impl BotRepository for SqliteBotRepository {
    async fn create(&self, bot: &CreateBot) -> Result<Bot, StorageError> {
        sqlx::query_as!(Bot, "INSERT INTO bots ...")
            .fetch_one(&self.pool)
            .await
            .map_err(StorageError::from)
    }
    // ...
}
```

**Confidence:** HIGH -- This is the standard Rust pattern for storage abstraction. Diesel, SQLx, and SeaORM all support this approach. The `async_trait` crate (or Rust's native async traits in recent editions) makes it ergonomic.

### Pattern 4: Agent Execution Tree with Depth-Limited Spawning

**What:** Each bot has a root agent that runs as a Tokio task. When the agent needs sub-agents, it spawns child tasks with a depth counter. Communication is via `mpsc` channels (message passing) or an optional `Arc<RwLock<Workspace>>` (shared workspace). Depth is hard-capped at 3.

**When to use:** Always for agent orchestration. This directly implements the requirement for hierarchical agents with max depth 3.

**Trade-offs:** Tokio tasks are lightweight (~500 bytes each), so spawning sub-agents is cheap. The depth cap prevents runaway spawning. Message passing is the default (clean isolation); shared workspace is opt-in (for when agents need to collaborate on artifacts).

**Example (Rust):**
```rust
pub struct AgentContext {
    pub agent_id: AgentId,
    pub bot_id: BotId,
    pub depth: u8,                             // 0 = root, max 3
    pub parent_tx: Option<mpsc::Sender<AgentMessage>>,  // Message to parent
    pub workspace: Option<Arc<RwLock<Workspace>>>,       // Opt-in shared state
    pub event_bus: Arc<EventBus>,
    pub llm: Arc<dyn LlmProvider>,
    pub trace_ctx: TraceContext,
}

impl AgentContext {
    pub async fn spawn_sub_agent(
        &self,
        config: SubAgentConfig,
    ) -> Result<AgentHandle, AgentError> {
        if self.depth >= 3 {
            return Err(AgentError::MaxDepthExceeded);
        }

        let (child_tx, child_rx) = mpsc::channel(32);
        let child_ctx = AgentContext {
            depth: self.depth + 1,
            parent_tx: Some(child_tx),
            workspace: if config.share_workspace {
                self.workspace.clone()
            } else {
                None
            },
            // ... inherit event_bus, llm, etc.
        };

        let handle = tokio::spawn(async move {
            run_agent_loop(child_ctx, child_rx).await
        });

        Ok(AgentHandle { task: handle, tx: child_tx })
    }
}
```

**Confidence:** HIGH -- Tokio's task spawning and channel primitives are well-proven. The depth-limiting pattern is straightforward. OpenClaw uses a similar isolated-agent-with-channels approach (in Node.js), validating the architectural concept.

### Pattern 5: LLM Provider Fallback Chain

**What:** An ordered list of LLM providers. On failure (rate limit, timeout, error), the system automatically falls through to the next provider. Each provider implements a common trait with streaming support via `tokio::Stream`.

**When to use:** All LLM interactions. This provides resilience against provider outages and rate limits.

**Trade-offs:** Adds latency on failure (must detect failure before falling through). Mitigated by aggressive timeouts and circuit breaker patterns. Different providers may have different capabilities (e.g., not all support tool use), so the chain must be capability-aware.

**Example (Rust):**
```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;

    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse, LlmError>;

    async fn stream(
        &self,
        req: &CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, LlmError>> + Send>>, LlmError>;
}

pub struct FallbackChain {
    providers: Vec<Arc<dyn LlmProvider>>,
    circuit_breakers: HashMap<String, CircuitBreaker>,
}

impl FallbackChain {
    pub async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse, LlmError> {
        for provider in &self.providers {
            if self.circuit_breakers[provider.name()].is_open() {
                continue;
            }
            match provider.complete(req).await {
                Ok(resp) => return Ok(resp),
                Err(e) if e.is_retriable() => {
                    self.circuit_breakers[provider.name()].record_failure();
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Err(LlmError::AllProvidersFailed)
    }
}
```

**Confidence:** HIGH -- The Rust ecosystem has multiple crates validating this pattern (`flyllm`, `llm-connector`, `rig`). Circuit breaker patterns are well-established in distributed systems.

### Pattern 6: WASM Skill Sandboxing with Capability-Based Permissions

**What:** Untrusted skills from registries are compiled to WASM components and executed in a Wasmtime sandbox. The host defines a WIT interface exposing only the capabilities the user has approved. Local trusted skills run natively with OS-level permissions.

**When to use:** For all registry skills (agentskills.io, skills.sh, ComposioHQ). Local skills skip the sandbox for performance.

**Trade-offs:** WASM sandboxing adds ~1-5ms overhead per skill invocation but provides strong isolation guarantees. The WASI 0.2 component model is still pre-1.0, so the WIT interface may need updates as the spec stabilizes. However, Wasmtime is production-ready and widely used.

**Confidence:** MEDIUM-HIGH -- Wasmtime is mature and the component model is usable. Microsoft's Wassette project specifically validates WASM sandboxing for AI agent tools. The WIT interface design requires experimentation during implementation.

## Data Flow

### Request Flow (Chat Message)

```
User sends message
    |
    v
[Web UI] --GraphQL mutation--> [GraphQL BFF]
    |                               |
    |                     Translates to REST/gRPC call
    |                               |
    v                               v
[Rust API Gateway] --routes--> [Bot Manager]
    |                               |
    |                     Loads bot config + soul
    |                               |
    v                               v
[Agent Orchestrator] --creates--> [Root Agent Task]
    |                               |
    |                     1. Loads SOUL.md + MEMORY.md
    |                     2. Builds system prompt
    |                     3. Calls LLM provider
    |                               |
    v                               v
[LLM Provider Chain] --streams--> [Token stream]
    |                               |
    |                     May invoke tools/skills
    |                               |
    v                               v
[Skill Executor] --if untrusted--> [WASM Sandbox]
    |                               |
    |                     Skill returns result
    |                               |
    v                               v
[Agent] --continues LLM loop or--> [Spawns sub-agent]
    |                               |
    |                     Sub-agent depth + 1
    |                               |
    v                               v
[Agent completes] --emits events--> [Event Bus]
    |                               |
    |     ┌─────────────┬───────────┼────────────────┐
    |     |             |           |                |
    v     v             v           v                v
[Memory  [Observability  [WebSocket   [Workflow
 System]  Engine]         feed to     Engine
                          UI]         (if triggered)]
```

### Event Flow (Internal Communication)

```
[Any Component] --publishes--> [Event Bus (broadcast channels)]
       |
       |-- BotEvent channel -----> [Bot Manager, Observability, UI Feed]
       |-- AgentEvent channel ---> [Orchestrator, Observability, UI Feed]
       |-- WorkflowEvent channel -> [Workflow Engine, Observability]
       |-- SkillEvent channel ----> [Skill Registry, Observability]
       |-- SystemEvent channel ---> [Budget Manager, Observability]
       |
[WebSocket Hub] <--subscribes-- [Event Bus]
       |
       |-- Filters events per client session
       |-- Sends to connected Web UI clients
```

### Memory Flow (Per-Bot)

```
[Chat Session]
    |
    |-- Extracts key points --> [Short-term Memory] (session-scoped)
    |                               |
    |                     Session ends
    |                               |
    v                               v
[Memory Consolidator] --summarizes--> [Long-term Memory] (per-bot, persistent)
    |                                       |
    |                             Embedding generated
    |                                       |
    v                                       v
[LLM Provider] --embeds--> [Vector Store (sqlite-vec)]
    |
    |-- Semantic search on next session start
    |
    v
[Common Memory] (shared read across bots, write by privileged operations)
```

### Key Data Flows

1. **Chat message to response:** Web UI -> BFF -> Rust API -> Bot Manager -> Agent Orchestrator -> LLM Provider -> (optional skill execution) -> streaming response back via WebSocket/SSE. Round-trip latency target: <200ms to first token (dominated by LLM provider latency).

2. **Agent spawning cascade:** Root agent decides to spawn sub-agent -> Orchestrator creates child Tokio task with depth+1 -> Child communicates results to parent via mpsc channel -> Parent integrates results and continues. Each spawn emits AgentSpawned event to Event Bus for real-time UI tracking.

3. **Skill execution (untrusted):** Agent requests skill -> Skill Registry resolves skill -> if trust tier = untrusted: compile to WASM component -> Wasmtime instantiates with approved capabilities only -> Execute in sandbox -> Return result to agent. ~5-10ms overhead per WASM invocation.

4. **MCP bidirectional flow:**
   - **Consuming:** Bot agent discovers external MCP server -> `boternity-mcp` client connects via streamable HTTP -> Negotiates capabilities -> Bot can call external tools/resources.
   - **Exposing:** External MCP host (e.g., Claude Code) connects to Boternity's MCP server -> Server maps bot capabilities to MCP tools -> External host can invoke bot skills as MCP tools.

5. **Observability pipeline:** Every LLM call instrumented with `tracing` spans -> Token count + cost calculated per span -> Spans exported via OpenTelemetry to internal collector -> Budget manager checks against global token limit -> If over budget, emit SystemEvent::BudgetExceeded -> Agent pauses, UI notified.

## Scaling Considerations

Since Boternity is self-hosted and single-user for v1, the scaling table is adjusted for single-machine capacity rather than multi-user horizontal scaling.

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 1-5 bots, casual use | SQLite + embedded sqlite-vec. Single process. Tokio's async handles all concurrency. No bottlenecks. |
| 5-20 bots, active workflows | SQLite may slow under write contention from concurrent agents. Enable WAL mode. Consider connection pooling. sqlite-vec stays fine (vector searches are read-heavy). |
| 20-50+ bots, heavy automation | SQLite WAL is the first bottleneck. This is where PostgreSQL migration pays off. Vector search may need Qdrant for better indexing. Event bus still fine (broadcast channels are lightweight). |
| Future multi-user (out of scope for v1) | Requires auth layer, per-user isolation, possibly separate processes per user. The trait-based storage abstraction and event bus design support this transition. |

### Scaling Priorities

1. **First bottleneck (SQLite write contention):** When many agents write memory/traces simultaneously, SQLite's single-writer lock becomes a problem. Mitigation: WAL mode (default in v1), batched writes via write-behind queue, and the PostgreSQL migration path for future.
2. **Second bottleneck (LLM API rate limits):** Multiple bots making concurrent LLM calls will hit provider rate limits. Mitigation: Global rate limiter in the LLM provider abstraction, automatic fallback chain, and request queuing with priority.
3. **Third bottleneck (WASM cold starts):** First invocation of a WASM skill has compilation overhead. Mitigation: Pre-compile WASM modules at install time, cache compiled modules, use Wasmtime's `Module::deserialize` for instant loading from cache.

## Anti-Patterns

### Anti-Pattern 1: God Service / Monolithic Core

**What people do:** Put all business logic in a single `core` crate with direct cross-domain method calls (e.g., bot manager directly calls workflow engine which directly calls memory system).

**Why it's wrong:** Creates tight coupling that makes testing hard, changes cascade unpredictably, and future feature additions require touching many files. n8n's early architecture suffered from this -- their refactor to controllers/services/repositories was partly to address monolithic coupling.

**Do this instead:** Use the event bus for cross-domain side effects. Bot Manager emits `BotCreated` event; Workflow Engine subscribes to `BotCreated` to set up default workflows. Direct calls only happen within a single domain (Bot Manager calls BotRepository, not WorkflowEngine).

### Anti-Pattern 2: Shared Mutable State Between Agents

**What people do:** Default to shared mutable state (Arc<RwLock<HashMap>>) for inter-agent communication because it seems simpler than message passing.

**Why it's wrong:** Leads to deadlocks, race conditions, and makes agent behavior non-deterministic. Microsoft's AI agent design patterns documentation explicitly warns against "sharing mutable state between concurrent agents."

**Do this instead:** Default to message passing via channels. Shared workspace is opt-in and should use a structured API (not raw HashMap). The workspace should be append-only or use CRDT-like merge semantics for conflict resolution.

### Anti-Pattern 3: Blocking the Tokio Runtime

**What people do:** Call synchronous operations (SQLite queries without async, CPU-heavy WASM compilation, file I/O) directly in async contexts.

**Why it's wrong:** Blocks the Tokio executor thread, causing latency spikes for all concurrent operations. Particularly dangerous in a platform where many agents run simultaneously.

**Do this instead:** Use `tokio::task::spawn_blocking` for synchronous operations. SQLx is natively async. For WASM compilation, offload to a blocking thread and cache the result. For CPU-heavy work, use `rayon` with a bridge to Tokio.

### Anti-Pattern 4: Tight Coupling to SQLite Specifics

**What people do:** Use SQLite-specific features (e.g., `json_extract`, `GROUP_CONCAT`, `datetime('now')`) directly in business logic.

**Why it's wrong:** Makes the PostgreSQL migration path a rewrite rather than a swap. Different SQL dialects handle types (UUID, boolean, timestamp) differently.

**Do this instead:** Use SQLx's `query_as!` with standard SQL. Where dialect-specific features are needed, isolate them in the `boternity-infra` crate behind the repository trait. Use application-level UUID generation (not database-level). Store timestamps as ISO 8601 strings or Unix timestamps.

### Anti-Pattern 5: Exposing Internal Events to External Clients

**What people do:** Send raw internal events (from the event bus) directly to WebSocket clients.

**Why it's wrong:** Internal events contain implementation details (task IDs, memory addresses, internal state). Changing internal event structure breaks the frontend. Also a security risk if internal data leaks.

**Do this instead:** Define a separate set of "client events" that the WebSocket hub translates from internal events. Internal `AgentSpawned { task_id, depth, parent_task_id }` becomes client `AgentUpdate { agent_id, status: "spawned", depth, parent_agent_id }`. The translation layer acts as a stable contract.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| LLM Providers (Anthropic, OpenAI, Google, Ollama) | HTTP client with streaming (SSE/chunked transfer) via `reqwest` | Each provider is a trait implementation. Streaming uses `tokio::Stream`. Ollama is localhost HTTP. |
| MCP Servers (external) | JSON-RPC 2.0 over streamable HTTP transport | Per MCP 2025-11-25 spec. Client maintains stateful session. Capability negotiation on connect. |
| Skill Registries (agentskills.io, skills.sh, ComposioHQ) | HTTP REST clients for discovery; git clone or HTTP download for installation | Rate-limit aware. Cache registry metadata locally. Validate skill manifest before installation. |
| OS Keychain (macOS Keychain, Linux Secret Service) | Platform-specific crates (`security-framework`, `secret-service`) | Graceful fallback to encrypted SQLite vault if keychain unavailable. |
| Embedding Providers | Same LLM provider trait, specialized for embedding models | OpenAI `text-embedding-3-small`, Anthropic embeddings, or local via Ollama. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| API Gateway <-> Core Services | Direct method calls via injected trait objects | API layer holds `Arc<dyn BotService>`, etc. No serialization overhead. |
| Core Services <-> Storage | Async trait calls (Repository pattern) | `BotRepository`, `MemoryRepository`, etc. Implementation swappable. |
| Cross-domain side effects | Event Bus (broadcast channels) | Bot Manager does not call Workflow Engine directly. Emits events. |
| Rust Backend <-> GraphQL BFF | REST/gRPC over localhost | BFF is a separate Node.js process. Communicates via local HTTP/gRPC. Low latency (<1ms). |
| Backend <-> Web UI (real-time) | WebSocket from Rust backend directly | Streaming responses, agent tree updates, workflow progress. Not proxied through BFF. |
| Backend <-> CLI | REST API over localhost | CLI uses same REST endpoints as any other client. |
| Agent <-> Sub-Agent | `mpsc` channels (message passing) or shared workspace (`Arc<RwLock>`) | Message passing is default. Shared workspace is opt-in per sub-agent config. |
| Agent <-> WASM Sandbox | Wasmtime host-guest function calls via WIT interface | Defined capability boundary. Guest cannot escape sandbox. |

## Build Order (Dependency-Driven)

Understanding what depends on what determines the optimal build order for implementation phases.

### Layer 0: Foundation (No dependencies on other Boternity crates)
- `boternity-types` — Domain models, error types, event definitions
- `packages/shared` — TypeScript shared types

### Layer 1: Core Domain (Depends on types only)
- `boternity-core` — Bot, Agent, Skill, Memory, LLM trait definitions
- `boternity-observe` — Tracing setup, metric definitions

### Layer 2: Infrastructure (Implements core traits)
- `boternity-infra` — SQLite storage, sqlite-vec, event bus, secret vault, WASM sandbox
- `boternity-mcp` — MCP client and server

### Layer 3: API and Clients (Composes everything)
- `boternity-api` — REST + gRPC + GraphQL server, WebSocket hub
- `packages/graphql-bff` — GraphQL BFF (needs Rust API running)
- `packages/cli` — CLI (needs REST API)
- `packages/sdk` — SDK (needs REST/gRPC API)
- `packages/web` — Web UI (needs BFF + WebSocket)

### Recommended Implementation Phases (Architectural Perspective)

**Phase 1: Skeleton + Bot CRUD + Single Agent Chat**
Build: `boternity-types` -> `boternity-core` (bot + agent only) -> `boternity-infra` (SQLite storage only) -> `boternity-api` (REST only) -> `packages/cli` (basic commands)

This gives you: Create a bot, give it a soul, chat with it via CLI. One LLM provider (Anthropic). No skills, no workflows, no sub-agents yet.

**Phase 2: Memory + Multiple Providers + Web UI**
Build: Memory system in `boternity-core` -> Vector store in `boternity-infra` -> LLM fallback chain -> `boternity-observe` (basic tracing) -> `packages/graphql-bff` -> `packages/web` (chat + bot management)

This gives you: Bots remember conversations. Multiple LLM providers with fallback. Basic web UI for chatting.

**Phase 3: Agent Hierarchy + Skills + Event System**
Build: Sub-agent spawning in `boternity-core` -> Event bus in `boternity-infra` -> Skill system (local skills first) -> WASM sandbox for registry skills -> WebSocket live updates

This gives you: Agents can spawn sub-agents. Bots can use skills. Internal events drive real-time UI updates.

**Phase 4: MCP + Workflows + Full Observability**
Build: `boternity-mcp` (consume then expose) -> Workflow engine -> Full observability (cost tracking, trace explorer) -> gRPC + GraphQL on backend

This gives you: Full platform capabilities. MCP both directions. Visual workflow builder. Complete observability.

**Phase ordering rationale:**
- You cannot build agent orchestration without bot management (Phase 1 before Phase 3)
- Memory and LLM abstraction are needed before skills can be meaningful (Phase 2 before Phase 3)
- MCP and workflows are the most complex and least-dependency features (Phase 4 last)
- The event bus is needed before WebSocket UI updates, but after basic REST API works (Phase 3)

## Sources

### Architecture Patterns (HIGH confidence)
- [Microsoft AI Agent Design Patterns](https://learn.microsoft.com/en-us/azure/architecture/ai-ml/guide/ai-agent-design-patterns) -- Sequential, concurrent, group chat, handoff, magentic orchestration patterns
- [n8n Architecture (DeepWiki)](https://deepwiki.com/n8n-io/n8n) -- Monorepo structure, execution engine, node system, layered architecture
- [n8n Architecture Overview](https://docs.n8n.io/hosting/architecture/overview/) -- Frontend-backend separation, execution modes, database structure

### OpenClaw Architecture (MEDIUM confidence)
- [OpenClaw Identity Architecture (MMNTM)](https://www.mmntm.net/articles/openclaw-identity-architecture) -- SOUL.md structure, agent isolation, memory system, identity cascade
- [OpenClaw Architecture for Beginners (Cyber Strategy Institute)](https://cyberstrategyinstitute.com/openclaw-architecture-for-beginners-jan-2026/) -- 4-layer model, gateway/runtime/memory/connectors
- [OpenClaw and the Programmable Soul (Barnacle.ai)](https://www.barnacle.ai/blog/2026-02-02-openclaw-and-the-programmable-soul)

### Rust Backend (HIGH confidence)
- [Axum GitHub Repository](https://github.com/tokio-rs/axum) -- Axum architecture, Tower integration, routing
- [FP Complete: Combining Axum, Hyper, Tonic, and Tower](https://academy.fpblock.com/blog/axum-hyper-tonic-tower-part1/) -- Multi-protocol server pattern
- [http-grpc-cohosting repo](https://github.com/sunsided/http-grpc-cohosting) -- Axum + Tonic on same Hyper server
- [async-graphql](https://github.com/async-graphql/async-graphql) -- GraphQL server library for Rust with Axum integration

### MCP Protocol (HIGH confidence)
- [MCP Specification 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25) -- JSON-RPC 2.0, streamable HTTP, tools/resources/prompts/sampling primitives
- [MCP Architecture (modelcontextprotocol.info)](https://modelcontextprotocol.info/docs/concepts/architecture/) -- Bidirectional client-server model

### WASM Sandboxing (MEDIUM-HIGH confidence)
- [Wasmtime Security](https://docs.wasmtime.dev/security.html) -- Sandbox security model, linear memory guards, CFI
- [WASI and Component Model Status (eunomia)](https://eunomia.dev/blog/2025/02/16/wasi-and-the-webassembly-component-model-current-status/) -- WASI 0.2 + Component Model maturity
- [Wassette: WASM tools for AI agents (Microsoft)](https://opensource.microsoft.com/blog/2025/08/06/introducing-wassette-webassembly-based-tools-for-ai-agents/) -- WASM sandboxing specifically for AI agent tools via MCP
- [Plugins with Rust and WASI Preview 2](https://benw.is/posts/plugins-with-rust-and-wasi) -- Practical plugin system implementation

### Storage & Vector Search (MEDIUM-HIGH confidence)
- [sqlite-vec (GitHub)](https://github.com/asg017/sqlite-vec) -- Vector search SQLite extension, Rust support via `cargo add sqlite-vec`
- [Qdrant Documentation](https://qdrant.tech/documentation/overview/) -- Rust-native vector database for future scaling
- [Diesel Dual Backends](https://colliery.io/blog/dual_backends/) -- Trait-based multi-database abstraction in Rust

### Observability (HIGH confidence)
- [axum-tracing-opentelemetry](https://crates.io/crates/axum-tracing-opentelemetry) -- Axum + tracing + OpenTelemetry integration
- [tracing-opentelemetry](https://crates.io/crates/tracing-opentelemetry) -- OpenTelemetry bridge for the tracing crate

### LLM Provider Abstraction (MEDIUM confidence)
- [FlyLLM](https://github.com/rodmarkun/flyllm) -- Rust multi-provider LLM client with load balancing
- [llm-connector](https://crates.io/crates/llm-connector) -- Multi-provider abstraction with streaming
- [Rig](https://rig.rs/) -- LLM application framework in Rust

### Event-Driven Architecture (HIGH confidence)
- [Tokio Channels Tutorial](https://tokio.rs/tokio/tutorial/channels) -- Official Tokio channel documentation
- [tokio::sync module](https://docs.rs/tokio/latest/tokio/sync/index.html) -- broadcast, mpsc, watch channel documentation
- [Designing Event-Driven Systems in Rust (Medium)](https://medium.com/@kanishks772/designing-an-event-driven-system-in-rust-a-step-by-step-architecture-guide-18c0e8013e86)

### Workflow Engines (MEDIUM confidence)
- [dagrs (Rust DAG engine)](https://github.com/open-rust-initiative/dagrs) -- Rust-native DAG execution
- [Dagu](https://github.com/dagu-org/dagu) -- YAML-defined workflows with built-in web UI (reference architecture)
- [n8n Workflow Execution](https://deepwiki.com/n8n-io/n8n) -- Execution engine internals, queue modes

---
*Architecture research for: Self-hosted AI bot management and orchestration platform*
*Researched: 2026-02-10*
