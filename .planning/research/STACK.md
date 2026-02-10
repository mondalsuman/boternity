# Stack Research

**Domain:** Self-hosted AI bot management platform (Rust backend + TypeScript frontend)
**Researched:** 2026-02-10
**Confidence:** MEDIUM-HIGH (most crate versions verified via docs.rs; some ecosystem patterns from WebSearch only)

---

## Pre-Decided Stack (Given Constraints)

These technologies are already decided by the project owner. Research below focuses on validating fitness and filling gaps.

### Frontend (TypeScript)

| Technology | Version | Purpose | Validation |
|------------|---------|---------|------------|
| React | 18+ | UI framework | Solid choice; 19 is out but 18 is stable and well-supported |
| Vite | 6.x | Build tool | Standard for React projects in 2026 |
| ShadCN UI | latest | Component library | Good for self-hosted admin UIs; built on Radix primitives |
| Tailwind CSS | 4.x | Utility CSS | De facto standard |
| TanStack Router | latest | Type-safe routing | Best-in-class for React SPAs |
| TanStack Query | 5.x | Server state | Industry standard for data fetching |
| TanStack Virtual | latest | Virtual scrolling | Needed for bot logs, event streams |
| Zustand + Zundo | latest | Client state + undo | Lightweight, pairs well with React 18 |
| Framer Motion | latest | Animation | Best React animation library |
| React Hook Form + Zod | latest | Forms + validation | Type-safe form handling standard |
| Tiptap | 2.x | Rich text editor | For bot prompt/soul editing |
| Chart.js | 4.x | Charts | Lightweight charting for dashboards |
| date-fns | 4.x | Date utility | Tree-shakeable date library |
| dnd-kit | latest | Drag and drop | For workflow builder, bot arrangement |

### Backend Core (Rust)

| Technology | Version | Purpose | Validation |
|------------|---------|---------|------------|
| Tokio | 1.49.0 | Async runtime | The async runtime for Rust; LTS 1.47.x supported through Sep 2026 |
| Axum | 0.8.8 | HTTP framework | Latest stable; ergonomic, Tower-based, first-party Tokio support |
| Tonic | 0.14.3 | gRPC | Standard Rust gRPC; HTTP/2 native, async/await first-class |
| Turborepo | 2.x | Monorepo build | Orchestrates TypeScript packages; Rust side uses Cargo workspace |

### Storage (Decided)

| Technology | Purpose | Notes |
|------------|---------|-------|
| SQLite | Primary data store (v1) | Migration path to PostgreSQL |
| Embedded vector store | Bot memory/embeddings (v1) | Migration path to Qdrant |
| GraphQL Yoga + Pothos | Frontend BFF layer | TypeScript-side GraphQL |

---

## Recommended Stack: Rust Ecosystem

The research areas below fill the gaps in the decided stack with specific crate recommendations.

### 1. LLM Integration

**Recommendation: `llm` crate (graniet) as primary, `rig-core` as alternative**

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `llm` | 1.2.4 | Unified multi-provider LLM client | HIGH |
| `rig-core` | 0.30.0 | Agent framework with vector store integration | HIGH |

**Why `llm` as primary:**
- Unified API across OpenAI, Anthropic, Ollama, DeepSeek, xAI, Groq, Google, Cohere, Mistral, HuggingFace
- Built-in agent support with cooperative agents via shared memory -- directly maps to Boternity's bot fleet concept
- Multi-step chains connecting different providers sequentially
- Function calling, streaming, structured output (JSON schema), vision, reasoning
- Conversation history with sliding window memory management
- Resilience with exponential backoff retry
- REST API serving with OpenAI-compatible format (useful for MCP server exposure)
- Builder pattern API ("Stripe-like" experience)

**Why `rig-core` as complement (not replacement):**
- 20+ model providers under unified interface
- Built-in vector store integration (10+ stores including SQLite, LanceDB, Qdrant) via `VectorStoreIndex` trait
- Full OpenTelemetry / GenAI Semantic Convention compatibility for observability
- `#[tool]` procedural macro transforms functions into agent tools
- WASM compatible (core library)
- More opinionated about the agent abstraction layer

**Strategy:** Use `llm` for the LLM communication layer (provider abstraction, streaming, function calling). Use `rig-core`'s vector store traits and tool abstractions as patterns to inform Boternity's own agent orchestration layer. Do NOT use both simultaneously as the primary agent runtime -- pick one and build custom orchestration on top.

**Recommendation: Build custom orchestration on `llm`** because Boternity's hierarchical agent model (max depth 3, sub-agent spawning) is too specific for any off-the-shelf framework. The `llm` crate gives you the right abstraction level: LLM calls, not agent opinions.

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `rustformers/llm` | Unmaintained (README says so) | `graniet/llm` (different crate, same name) |
| `llm-connector` | Less mature, smaller ecosystem | `llm` (graniet) |
| `openai-api-rs` | Single-provider lock-in | `llm` for multi-provider |
| Rolling your own HTTP client per provider | Massive maintenance burden | `llm` abstracts this |

### 2. Embedded Vector Store

**Recommendation: `lancedb` for persistent vector storage, `vectorlite` for in-process hot cache**

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `lancedb` | 0.26.2 | Persistent embedded vector DB | HIGH |
| `vectorlite` | 0.1.5 | In-memory hot vector cache | MEDIUM |

**Why `lancedb`:**
- Native Rust SDK (not a binding)
- Persistent storage built-in (survives restarts) -- critical for bot memory
- Zero-copy operations, automatic versioning
- Vector similarity search + full-text search + SQL in one engine
- Automatic index creation (IVF-PQ for vector columns)
- Clear migration path: LanceDB can run embedded or as a service
- Active development: 0.26.2 released 2026-02-09 (yesterday)

**Why `vectorlite` as complement (NOT replacement):**
- In-memory, sub-millisecond semantic search
- Built-in embedding generation via Candle (all-MiniLM-L6-v2 locally)
- HNSW index for fast approximate nearest neighbor
- Ideal for session-scoped "hot" memory (current conversation context)
- Thread-safe concurrent access

**Strategy for v1:**
- Use `lancedb` as the primary persistent vector store for bot long-term memory
- Use `vectorlite` for session-scoped short-term memory (fast in-process search)
- Both are embedded, no external services needed

**Migration path to Qdrant:**
- LanceDB's query API is similar enough to Qdrant's that migration involves swapping the storage backend, not rewriting query logic
- Define a `VectorStore` trait in Boternity that both LanceDB and Qdrant implement
- Rig-core's `VectorStoreIndex` trait is a good reference for this abstraction

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `tinyvector` | Too simple, no persistence, no HNSW | `lancedb` |
| Qdrant embedded | Heavyweight for v1 self-hosted | `lancedb` (lighter) |
| `chromadb` | Python-first, Rust bindings are thin | `lancedb` (Rust-native) |

### 3. Embedded Pub/Sub for Event-Driven Architecture

**Recommendation: Tokio broadcast + custom event bus**

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `tokio::sync::broadcast` | (part of tokio 1.49.0) | Fan-out event channels | HIGH |
| `tokio::sync::mpsc` | (part of tokio 1.49.0) | Point-to-point channels | HIGH |
| `tokio::sync::watch` | (part of tokio 1.49.0) | State broadcast (latest value) | HIGH |

**Why Tokio built-ins over dedicated crates:**
- Zero additional dependencies -- already using Tokio
- `broadcast` channel: many-to-many fan-out, perfect for pub/sub ("every subscriber gets every event")
- `mpsc`: many-to-one, for command routing to specific handlers
- `watch`: one-to-many state broadcast, for configuration changes / bot status updates
- Battle-tested, maintained by the Tokio team
- No serialization overhead -- events stay as typed Rust values in-process

**Architecture pattern:**
```
EventBus {
    bot_events: broadcast::Sender<BotEvent>,        // Bot lifecycle events
    agent_events: broadcast::Sender<AgentEvent>,    // Agent execution events
    system_events: broadcast::Sender<SystemEvent>,  // System-wide events
    metrics: broadcast::Sender<MetricEvent>,        // Observability events
}
```

**Why NOT dedicated pub/sub crates:**

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `tokio-pubsub` | Thin wrapper over broadcast; adds dependency without value | `tokio::sync::broadcast` directly |
| `event_bus_rs` | Small community, uncertain maintenance | Custom on Tokio primitives |
| `ioevent` | Oriented toward IPC, not in-process events | Tokio channels |
| Redis/NATS | External service; violates self-hosted v1 constraint | Tokio channels |

**Future migration path:** If Boternity later needs cross-process events (multi-node deployment), add NATS or Redis pub/sub behind the same `EventBus` trait. The trait abstraction makes this a swap, not a rewrite.

### 4. MCP Protocol Implementation

**Recommendation: `rmcp` (official SDK)**

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `rmcp` | 0.15.0 | Official Rust MCP SDK (server + client) | HIGH |

**Why `rmcp`:**
- THE official Rust implementation of the Model Context Protocol, maintained by the MCP organization
- Released 2026-02-10 (today) at version 0.15.0
- Supports both MCP server (expose bot capabilities to AI assistants) AND MCP client (bots consuming external MCP tools)
- `#[tool]` macro for defining tools with minimal boilerplate
- Supports MCP spec versions: 2024-11-05, 2025-03-26, 2025-06-18, 2025-11-25
- Multiple transports: stdio, child process, HTTP streaming
- OAuth2 authentication support
- Structured tool output (JSON schema via `schemars`)
- Task lifecycle management (SEP-1686) for long-running operations
- Async/await on Tokio

**Bidirectional MCP (both directions):**
- **Boternity as MCP Server:** Expose bot fleet management, skill execution, memory access as MCP tools that Claude/GPT can invoke
- **Boternity as MCP Client:** Bots consume external MCP servers (filesystem, databases, APIs) as skills

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `prism-mcp-sdk` | Community implementation; official SDK exists | `rmcp` |
| `mcp-rust-sdk` (Derek-X-Wang) | Pre-dates official SDK; smaller community | `rmcp` |
| `rust-mcp-schema` | Schema only, no runtime | `rmcp` (includes schema) |
| Building from scratch | MCP spec is complex; official SDK handles edge cases | `rmcp` |

### 5. SQLite Abstraction (with PostgreSQL Migration Path)

**Recommendation: SeaORM 1.1.x stable (evaluate 2.0 when released)**

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `sea-orm` | 1.1.19 | Async ORM with backend-agnostic API | HIGH |
| `sea-orm-migration` | 1.1.x | Database migrations | HIGH |
| `sea-query` | (transitive) | Dynamic SQL query builder | HIGH |

**Why SeaORM over alternatives:**
- **Backend-generic Entity API:** Does not require a database backend at compile time. Design applications that use SQLite in dev/v1 and PostgreSQL in production -- same entity code, different feature flag
- **Migration independence:** Migrations are independent of the backend database, allowing reuse across Postgres, MySQL, SQLite
- **Async-first:** Built on SQLx, full async/await compatibility
- **Code generation:** `sea-orm-cli` generates entity files from existing database
- **Mature:** 250K+ weekly downloads, production use at startups and enterprises
- **GraphQL friendly:** Designed for building REST, GraphQL, and gRPC APIs with joining, filtering, sorting, pagination
- **2.0 preview:** SeaORM 2.0 (currently at rc.31) adds `sea-orm-sync` for lightweight CLI programs with SQLite -- useful for Boternity's CLI tool

**Migration strategy: SQLite -> PostgreSQL:**
1. Use `sea-orm` with `sqlx-sqlite` feature for v1
2. Write all migrations using SeaORM's migration framework
3. Avoid SQLite-specific SQL (use SeaORM's query builder exclusively)
4. To migrate: change feature flag to `sqlx-postgres`, run migrations against PostgreSQL
5. Test with both backends in CI from day one

| Alternative | Why Not |
|-------------|---------|
| **Diesel** | Compile-time backend selection is less flexible for runtime switching; heavier proc-macro compile times |
| **SQLx (raw)** | No ORM abstractions; you'd write raw SQL that may not be portable between SQLite and PostgreSQL |
| **SurrealDB** | Interesting but too different from relational model; harder PostgreSQL migration |

### 6. WebSocket / SSE Streaming in Axum

**Recommendation: Axum built-in WebSocket + SSE support**

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `axum` (ws feature) | 0.8.8 | WebSocket support | HIGH |
| `axum` (sse feature) | 0.8.8 | Server-Sent Events | HIGH |
| `axum-extra` | 0.10.x | Additional extractors and utilities | HIGH |
| `tokio-tungstenite` | (transitive via axum) | WebSocket protocol implementation | HIGH |
| `futures-util` | 0.3.x | Stream splitting for WebSocket sender/receiver | HIGH |

**WebSocket use cases in Boternity:**
- Bot status live updates
- Agent execution streaming (real-time output as agent processes)
- Interactive builder bot conversation
- Workflow execution progress

**SSE use cases in Boternity:**
- LLM token streaming to frontend (one-way server-to-client)
- Event log tailing
- Dashboard metric updates
- Notification streams

**Architecture guidance:**
- Use **SSE** for LLM token streaming and one-way updates (simpler, works through proxies, auto-reconnect built into browsers)
- Use **WebSocket** for bidirectional communication (interactive builder bot, collaborative editing)
- Axum 0.8 handles Close/Ping/Pong automatically for WebSocket
- SSE in Axum 0.8.5+ no longer requires `tokio` feature for basic functionality (only for keep-alive)

**Pattern:**
```rust
// SSE for LLM streaming
async fn stream_completion(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = state.llm.stream_completion(/*...*/).await;
    Sse::new(stream.map(|token| Ok(Event::default().data(token))))
        .keep_alive(KeepAlive::default())
}

// WebSocket for interactive bot
async fn bot_chat(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_bot_socket)
}
```

### 7. CLI Framework

**Recommendation: `clap` with derive macros**

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `clap` | 4.5.57 | CLI argument parsing with derive | HIGH |
| `clap_complete` | 4.x | Shell completions generation | HIGH |
| `indicatif` | 0.17.x | Progress bars and spinners | MEDIUM |
| `dialoguer` | 0.11.x | Interactive prompts | MEDIUM |
| `console` | 0.15.x | Terminal styling | MEDIUM |
| `anyhow` | 1.x | Error handling | HIGH |

**Why `clap`:**
- De facto standard for Rust CLIs (most popular by far)
- Derive macro approach: define CLI as Rust struct, get parsing + validation + help for free
- Subcommand support maps directly to Boternity's CLI needs:
  - `boternity bot create/list/delete/start/stop`
  - `boternity skill install/list/remove`
  - `boternity agent run/inspect`
  - `boternity server start/stop/status`
- Auto-generated help always in sync with code
- Environment variable support built-in
- Shell completion generation via `clap_complete`

**Supporting CLI crates:**
- `indicatif` for progress bars during long operations (bot creation, skill installation)
- `dialoguer` for interactive prompts (bot configuration wizard)
- `console` for colored output and terminal detection

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `structopt` | Merged into clap 3+; deprecated | `clap` with derive |
| `argh` | Google's, minimalist but less ecosystem | `clap` |
| `bpaf` | Newer, smaller community | `clap` |

### 8. Turborepo + Rust Monorepo Integration

**Recommendation: Hybrid Turborepo + Cargo workspace**

| Tool | Purpose | Confidence |
|------|---------|------------|
| Turborepo 2.x | Orchestrates TypeScript packages (frontend, BFF, shared types) | HIGH |
| Cargo workspace | Orchestrates Rust crates (backend, CLI, shared types) | HIGH |
| `pnpm` | Package manager for TypeScript side | HIGH |
| Custom `turbo.json` tasks | Bridge between Turborepo and Cargo | MEDIUM |

**Critical insight:** Turborepo only supports JavaScript/TypeScript projects natively. It cannot manage Rust/Cargo builds. The solution is a hybrid approach.

**Recommended monorepo structure:**
```
boternity/
  turbo.json                    # Turborepo config (TypeScript tasks)
  Cargo.toml                   # Cargo workspace root (Rust tasks)
  pnpm-workspace.yaml          # pnpm workspace config

  # TypeScript packages (managed by Turborepo)
  apps/
    web/                       # React frontend (Vite)
    bff/                       # GraphQL BFF (Yoga + Pothos)
  packages/
    ui/                        # Shared UI components
    types/                     # Shared TypeScript types
    config/                    # Shared configs (ESLint, TS, Tailwind)

  # Rust crates (managed by Cargo workspace)
  crates/
    boternity-core/            # Core domain types and traits
    boternity-server/          # Axum HTTP + gRPC server
    boternity-agent/           # Agent orchestration engine
    boternity-llm/             # LLM provider abstraction
    boternity-mcp/             # MCP server + client
    boternity-storage/         # SQLite + vector store
    boternity-cli/             # CLI binary
    boternity-events/          # Event bus
    boternity-skills/          # Skill registry + WASM sandbox
```

**Bridge strategy:**
- In `turbo.json`, define a `build:rust` task that shells out to `cargo build`
- Use Turborepo's caching for TypeScript artifacts only
- Use `cargo`'s incremental compilation for Rust artifacts
- Shared types between Rust and TypeScript: generate TypeScript types from Rust structs using `ts-rs` or `specta`

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `ts-rs` | 10.x | Generate TypeScript types from Rust structs | MEDIUM |
| `specta` | 2.x | Type-safe TypeScript bindings from Rust | MEDIUM |

**Best practices:**
- Run `cargo check` and TypeScript type checking in parallel via Turborepo
- Use `pnpm` (not npm/yarn) for faster installs and strict dependency resolution
- Shared `.env` files at root; Turborepo handles env variable passthrough
- CI: Run `turbo run build test lint` which cascades to both TS and Rust

### 9. Tus Protocol (Resumable Uploads)

**Recommendation: Custom implementation on Axum (rustus is stale)**

| Approach | Confidence |
|----------|------------|
| Custom tus server implementation in Axum | MEDIUM |
| `rustus` as reference (not dependency) | LOW |

**Problem:** The `rustus` crate (v0.5.10) was last updated August 2022 -- nearly 4 years ago. It is a standalone binary, not a library you embed into your Axum server. The `tus-rust` crate is similarly old.

**Recommendation:**
- Implement tus protocol v1.0 directly as Axum middleware/handlers
- The tus protocol is simple: it is HTTP-based with specific headers (`Upload-Length`, `Upload-Offset`, `Tus-Resumable`, `Tus-Version`)
- Core operations: creation, head (offset check), patch (upload chunk), termination
- Reference `rustus` source code for protocol compliance details
- Store upload metadata in SQLite alongside bot/skill data
- Store upload chunks on local filesystem (v1)

**Why custom is acceptable here:**
- Tus protocol is well-specified and not complex (unlike MCP where an SDK is warranted)
- Embedding into existing Axum server avoids running a separate upload service
- Full control over storage backend (integrate with Boternity's file management)
- Only needed for large file uploads (skill packages, training data) -- not a core hot path

**Alternative:** If upload volume is low in v1, defer tus entirely and use simple multipart upload in Axum. Add tus when file sizes/reliability requirements demand it.

### 10. Secret Management

**Recommendation: `keyring` + `secrecy` + encrypted local file**

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `secrecy` | 0.10.3 | In-memory secret protection (zeroize on drop) | HIGH |
| `keyring` | 3.6.3 | OS keychain integration (macOS Keychain, Windows Credential Manager, Linux Secret Service) | HIGH |
| `securestore` | 0.100.0 | Encrypted file-based secret storage | MEDIUM |
| `aes-gcm` | 0.10.x | AES-GCM encryption for custom vault | MEDIUM |

**Layered approach for self-hosted platform:**

1. **In-memory protection:** Wrap all secrets in `secrecy::SecretString` / `Secret<T>`. Automatically zeroized on drop, redacted in Debug output, prevents accidental logging.

2. **OS keychain (interactive use):** Use `keyring` for CLI-managed secrets (API keys entered by user). Cross-platform: macOS Keychain, Windows Credential Manager, Linux Secret Service. Version 3.6.3 supports binary secret data.

3. **Encrypted file vault (server use):** Use `securestore` for secrets that need to persist across server restarts without interactive keychain access. Encrypted JSON file that can be committed to git (encrypted). Password or keyfile for decryption.

4. **Runtime secret management:** At server startup, load secrets from keychain or encrypted vault into `Secret<T>` wrappers. Pass through `AppState` in Axum. Never log, never serialize unencrypted.

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `secret-vault` (abdolence) | Designed for cloud secret managers (GCP/AWS), not self-hosted | `securestore` + `keyring` |
| HashiCorp Vault | External service; violates self-hosted v1 simplicity | Local encrypted file |
| `.env` files with plaintext secrets | Insecure, easily leaked to logs/git | `securestore` encrypted vault |
| `cryptex` | Uses SQLCipher, additional C dependency | `securestore` (pure Rust-friendly) |

---

## Recommended Stack: Observability

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `tracing` | 0.1.44 | Structured logging and span instrumentation | HIGH |
| `tracing-subscriber` | 0.3.x | Subscriber for formatting, filtering | HIGH |
| `tracing-opentelemetry` | 0.32.1 | Bridge tracing spans to OpenTelemetry | HIGH |
| `opentelemetry` | 0.31.0 | OpenTelemetry API (traces, metrics, logs) | HIGH |
| `opentelemetry_sdk` | 0.31.x | OTel SDK implementation | HIGH |
| `opentelemetry-otlp` | 0.31.x | OTLP exporter (to Jaeger, Grafana, etc.) | HIGH |

**Why this stack:**
- `tracing` is THE instrumentation framework for async Rust (maintained by Tokio team)
- Structured spans with typed fields -- perfect for agent execution tracing
- `tracing-opentelemetry` connects Rust spans to distributed traces
- Full OTel support: traces, metrics, logs via OTLP protocol
- Compatible with Jaeger, Grafana Tempo, Datadog, Honeycomb, etc.
- `rig-core` already uses OpenTelemetry GenAI Semantic Conventions, so instrumentation patterns are well-established in the Rust AI ecosystem

**Agent tracing pattern:**
```
Bot Execution Span
  |-- Agent Span (depth 0)
  |     |-- LLM Call Span (provider, model, tokens)
  |     |-- Tool Call Span (tool name, input, output)
  |     |-- Sub-Agent Span (depth 1)
  |     |     |-- LLM Call Span
  |     |     |-- Sub-Agent Span (depth 2, max)
  |     |           |-- LLM Call Span
```

---

## Recommended Stack: Rust GraphQL (Backend Native)

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `async-graphql` | 7.2.1 | GraphQL server library | HIGH |
| `async-graphql-axum` | 7.0.17 | Axum integration | HIGH |

**Why `async-graphql` over `juniper`:**
- Async-first (native Tokio support)
- Subscriptions via WebSocket (critical for real-time bot updates)
- Apollo Federation support (future-proofing)
- More feature-rich: custom scalars, dataloader, query complexity limiting
- Active development (8.0 RC in progress)
- First-class Axum integration

**Note on the dual-GraphQL architecture:**
- Frontend BFF: GraphQL Yoga + Pothos (TypeScript) -- aggregates Rust backend APIs for the frontend
- Backend native: `async-graphql` (Rust) -- exposes bot/agent/skill operations as GraphQL
- The BFF calls the Rust GraphQL backend (or REST/gRPC, depending on the operation)

---

## Recommended Stack: WASM Sandboxing for Skills

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `wasmtime` | 41.0.3 | WASM runtime (component model) | HIGH |
| `wit-bindgen` | 0.51.0 | WIT binding generator for plugins | HIGH |
| `wasmtime-wasi` | 41.0.x | WASI support for sandboxed I/O | HIGH |

**Why Wasmtime (not Wasmer, not Extism):**
- Bytecode Alliance project, strongest community and corporate backing
- Component Model support is native and mature (bindgen! macro)
- WIT (WebAssembly Interface Types) for defining skill interfaces
- Sandboxed by default: memory limits, CPU limits, filesystem restrictions via WASI
- Future/Stream support for async skill execution (added Jan 2026)

**Why NOT Extism:**
- Extism (v1.13.0) uses wasmtime internally (v27-30 range), so it is an abstraction over wasmtime
- For Boternity, we need fine-grained control over the sandbox (custom resource limits per bot, custom host functions for memory access)
- Extism's abstraction would fight against Boternity's custom skill interface
- Using wasmtime directly gives full control without the abstraction tax

**Skill plugin architecture:**
- Define skill interfaces in WIT (inputs, outputs, capabilities)
- Skills compiled to WASM components by skill authors
- Boternity loads WASM components via wasmtime, sandboxed per execution
- Host functions expose bot memory, event bus, and HTTP client to skills
- Resource limits (memory, CPU time) configurable per bot

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Extism | Abstraction over wasmtime; less control | `wasmtime` directly |
| Wasmer | Less component model support; focused on standalone execution | `wasmtime` |
| `wasm-sandbox` | Thin wrapper; not actively maintained | `wasmtime` directly |
| Native plugins (dylib) | No sandboxing; security nightmare | WASM |

---

## Recommended Stack: Supporting Crates

| Crate | Version | Purpose | Confidence |
|-------|---------|---------|------------|
| `serde` | 1.0.x | Serialization framework | HIGH |
| `serde_json` | 1.0.x | JSON serialization | HIGH |
| `tower` | 0.5.x | Middleware framework (used by Axum) | HIGH |
| `tower-http` | 0.6.8 | HTTP middleware (CORS, compression, tracing, auth) | HIGH |
| `uuid` | 1.x | UUID generation (bot IDs, agent IDs) | HIGH |
| `chrono` | 0.4.x | Date/time handling | HIGH |
| `anyhow` | 1.x | Application error handling | HIGH |
| `thiserror` | 2.x | Library error types with derive | HIGH |
| `reqwest` | 0.12.x | HTTP client (for external API calls) | HIGH |
| `jsonwebtoken` | 9.x | JWT encoding/decoding | MEDIUM |
| `argon2` | 0.5.x | Password hashing | MEDIUM |
| `base64` | 0.22.x | Base64 encoding | HIGH |
| `rand` | 0.9.x | Random number generation | HIGH |
| `regex` | 1.x | Regular expressions | HIGH |
| `semver` | 1.x | Semantic versioning (skill versions, bot soul versions) | HIGH |
| `tempfile` | 3.x | Temporary files | HIGH |
| `walkdir` | 2.x | Directory traversal | HIGH |
| `schemars` | 1.x | JSON Schema generation (for MCP tools) | HIGH |
| `dashmap` | 6.x | Concurrent hash map | HIGH |
| `bytes` | 1.x | Byte buffer utilities | HIGH |

---

## Alternatives Considered (Full Matrix)

| Category | Recommended | Alternative | When to Use Alternative |
|----------|-------------|-------------|-------------------------|
| LLM Client | `llm` (graniet) | `rig-core` | When you want opinionated agent abstractions, not just LLM calls |
| Vector Store | `lancedb` | Qdrant (embedded) | When you need distributed vector search from day 1 |
| ORM | `sea-orm` | `sqlx` (raw) | When you want full SQL control and don't need backend portability |
| ORM | `sea-orm` | Diesel | When compile-time SQL checking is more important than runtime flexibility |
| GraphQL | `async-graphql` | Juniper | When you need sync execution or prefer code-first without macros |
| WASM Runtime | Wasmtime | Wasmer | When targeting standalone WASM execution, not embedded plugins |
| WASM Runtime | Wasmtime | Extism | When you want a higher-level plugin framework with less control |
| CLI | `clap` | `argh` | When binary size matters more than features |
| Secrets | `keyring` + `securestore` | HashiCorp Vault | When running in enterprise with existing Vault infrastructure |
| Pub/Sub | Tokio channels | NATS | When you need cross-process or multi-node pub/sub |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `rustformers/llm` | Explicitly unmaintained; README says to look elsewhere | `graniet/llm` |
| `diesel` for this project | Compile-time backend selection makes SQLite->PostgreSQL migration harder | `sea-orm` |
| `actix-web` | Axum is already decided; mixing frameworks adds confusion | `axum` |
| `warp` | Predecessor pattern; Axum supersedes it in the Tokio ecosystem | `axum` |
| `rocket` | Opinionated, own runtime; doesn't compose well with Tokio ecosystem | `axum` |
| `native plugins (cdylib)` | No sandboxing, memory safety risks from third-party code | WASM via wasmtime |
| `sqlx` alone (no ORM) | Raw SQL won't be portable between SQLite and PostgreSQL dialects | `sea-orm` (wraps sqlx) |
| Cloud secret managers (Vault, AWS SM) | External dependency; violates self-hosted v1 constraint | `keyring` + `securestore` |
| `.env` plaintext secrets | Security risk; easily leaked | Encrypted vault |

---

## Version Compatibility Matrix

| Crate A | Compatible With | Notes |
|---------|-----------------|-------|
| `axum` 0.8.x | `tokio` 1.49.x | First-party compatibility |
| `axum` 0.8.x | `tower` 0.5.x, `tower-http` 0.6.x | Required middleware layer |
| `axum` 0.8.x | `tonic` 0.14.x | Both use `hyper` 1.x and `tokio` 1.x |
| `sea-orm` 1.1.x | `sqlx` 0.8.x (transitive) | SeaORM pins its sqlx version |
| `tracing` 0.1.x | `tracing-opentelemetry` 0.32.x | Check opentelemetry version alignment |
| `opentelemetry` 0.31.x | `tracing-opentelemetry` 0.32.x | Must be version-aligned |
| `wasmtime` 41.x | `wit-bindgen` 0.51.x | Must use compatible versions; check wasmtime release notes |
| `rmcp` 0.15.x | `tokio` 1.x, `serde` 1.x | Standard Tokio ecosystem |
| `llm` 1.2.x | `tokio` 1.x, `reqwest` 0.12.x | Uses reqwest for HTTP calls |

---

## Cargo.toml Dependency Sketch

```toml
[workspace]
resolver = "2"
members = [
    "crates/boternity-core",
    "crates/boternity-server",
    "crates/boternity-agent",
    "crates/boternity-llm",
    "crates/boternity-mcp",
    "crates/boternity-storage",
    "crates/boternity-cli",
    "crates/boternity-events",
    "crates/boternity-skills",
]

[workspace.dependencies]
# Async runtime
tokio = { version = "1.49", features = ["full"] }

# Web framework
axum = { version = "0.8", features = ["ws"] }
axum-extra = { version = "0.10", features = ["typed-header"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "compression-gzip", "trace", "timeout"] }

# gRPC
tonic = "0.14"
prost = "0.13"

# Database
sea-orm = { version = "1.1", features = ["sqlx-sqlite", "runtime-tokio-rustls", "macros"] }
sea-orm-migration = "1.1"

# LLM
llm = "1.2"

# MCP
rmcp = "0.15"

# Vector store
lancedb = "0.26"

# WASM
wasmtime = { version = "41", features = ["component-model"] }
wasmtime-wasi = "41"

# GraphQL
async-graphql = "7.2"
async-graphql-axum = "7.0"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-opentelemetry = "0.32"
opentelemetry = "0.31"
opentelemetry_sdk = { version = "0.31", features = ["rt-tokio"] }
opentelemetry-otlp = "0.31"

# CLI
clap = { version = "4.5", features = ["derive", "env"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Security
secrecy = { version = "0.10", features = ["serde"] }
keyring = { version = "3.6", features = ["apple-native", "windows-native", "linux-native"] }

# Utilities
uuid = { version = "1", features = ["v4", "v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1"
thiserror = "2"
reqwest = { version = "0.12", features = ["json", "stream"] }
semver = "1"
schemars = "1"
dashmap = "6"
bytes = "1"
futures-util = "0.3"
rand = "0.9"
```

---

## Sources

### Verified via docs.rs (HIGH confidence)
- [rmcp 0.15.0](https://docs.rs/crate/rmcp/latest) -- Official MCP Rust SDK, released 2026-02-10
- [rig-core 0.30.0](https://docs.rs/crate/rig-core/latest) -- LLM agent framework, released 2026-02-03
- [sea-orm 1.1.19](https://docs.rs/crate/sea-orm/latest) -- Async ORM, stable release; 2.0.0-rc.31 in preview
- [wasmtime 41.0.3](https://docs.rs/crate/wasmtime/latest) -- WASM runtime, released 2026-02-04
- [clap 4.5.57](https://docs.rs/crate/clap/latest) -- CLI framework, released 2026-02-03
- [vectorlite 0.1.5](https://docs.rs/crate/vectorlite/latest) -- In-memory vector DB, released 2025-10-24
- [lancedb 0.26.2](https://docs.rs/crate/lancedb/latest) -- Embedded vector DB, released 2026-02-09
- [tracing 0.1.44](https://docs.rs/crate/tracing/latest) -- Structured logging, released 2025-12-18
- [tracing-opentelemetry 0.32.1](https://docs.rs/crate/tracing-opentelemetry/latest) -- OTel bridge, released 2026-01-12
- [opentelemetry 0.31.0](https://docs.rs/crate/opentelemetry/latest) -- OTel API, released 2025-09-25
- [async-graphql 7.2.1](https://docs.rs/crate/async-graphql/latest) -- GraphQL server, released 2026-01-20
- [tower-http 0.6.8](https://docs.rs/crate/tower-http/latest) -- HTTP middleware, released 2025-12-08
- [tokio 1.49.0](https://docs.rs/crate/tokio/latest) -- Async runtime, released 2026-01-03
- [tonic 0.14.3](https://docs.rs/crate/tonic/latest) -- gRPC, released 2026-01-28
- [extism 1.13.0](https://docs.rs/crate/extism/latest) -- WASM plugin framework, released 2025-11-25
- [secrecy 0.10.3](https://docs.rs/crate/secrecy/latest) -- Secret management, released 2024-10-09
- [keyring 3.6.3](https://docs.rs/crate/keyring/latest) -- OS keychain access
- [axum 0.8.8](https://docs.rs/axum/latest/axum/) -- HTTP framework

### Verified via GitHub README (HIGH confidence)
- [graniet/llm 1.2.4](https://github.com/graniet/llm) -- Multi-provider LLM crate with agent support
- [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) -- Official MCP Rust SDK repo
- [SeaQL/sea-orm](https://github.com/SeaQL/sea-orm) -- SeaORM repository and migration features
- [bytecodealliance/wasmtime](https://github.com/bytecodealliance/wasmtime/releases) -- Wasmtime releases
- [bytecodealliance/wit-bindgen](https://github.com/bytecodealliance/wit-bindgen) -- WIT binding generator

### Verified via official documentation (HIGH confidence)
- [SeaORM docs](https://www.sea-ql.org/SeaORM/docs/internal-design/diesel/) -- Backend-agnostic design
- [SeaORM 2.0 blog](https://www.sea-ql.org/blog/2025-12-12-sea-orm-2.0/) -- sea-orm-sync and 2.0 features
- [Axum 0.8 announcement](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0) -- Breaking changes and new features
- [Rig docs](https://docs.rig.rs/) -- Agent framework documentation
- [OpenTelemetry Rust](https://opentelemetry.io/docs/languages/rust/) -- OTel Rust docs
- [LanceDB docs](https://lancedb.com/documentation/overview/index.html) -- Embedded vector DB

### WebSearch verified with multiple sources (MEDIUM confidence)
- Turborepo + Rust hybrid monorepo pattern -- multiple community projects confirm approach
- `rustus` stale status (last release 2022-08-25) -- verified via docs.rs
- `securestore` for encrypted file vault -- verified via docs.rs and GitHub

### WebSearch only (LOW confidence -- validate before depending on)
- `ts-rs` / `specta` for Rust-to-TypeScript type generation -- need to verify current API stability
- `indicatif` / `dialoguer` / `console` versions -- need to verify latest versions
- LanceDB to Qdrant migration similarity -- based on community reports, not tested

---
*Stack research for: Boternity -- Self-hosted AI bot management platform*
*Researched: 2026-02-10*
