# Phase 5: Agent Hierarchy + Event System - Research

**Researched:** 2026-02-13
**Domain:** Hierarchical agent orchestration, event bus, WebSocket real-time updates, budget enforcement
**Confidence:** HIGH

## Summary

This phase adds sub-agent spawning (sequential + parallel), depth-capped hierarchical execution, an event bus for cross-cutting concerns, WebSocket infrastructure for real-time UI updates, and per-request token budget enforcement. The existing codebase provides a strong foundation: `AgentEngine` already handles single-agent LLM execution, `AgentContext` tracks conversation state, `StreamEvent` defines the streaming protocol, and `FallbackChain` handles provider selection. The phase extends these with an `AgentOrchestrator` that manages a tree of sub-agents, each with its own `AgentContext`, coordinated through tokio tasks and channels.

The core technology stack is entirely within the existing Rust ecosystem: tokio `JoinSet` for parallel sub-agent execution, tokio `broadcast` channels for the event bus, tokio-util `CancellationToken` for hierarchical cancellation (individual agent + tree-wide), and axum's built-in `ws` feature for WebSocket support. On the frontend, the native browser `WebSocket` API with a custom React hook handles reconnection with exponential backoff. No new major dependencies are needed beyond enabling axum's `ws` feature, adding `tokio-util` to `boternity-core`, and `dashmap` for the shared workspace.

The key architectural insight is that the event bus is the unifying abstraction: all agent lifecycle events (spawn, progress, completion, budget warnings, cancellation) flow through a single `tokio::sync::broadcast` channel. WebSocket connections subscribe to this channel and forward events to the UI. CLI rendering subscribes to the same channel. Budget enforcement subscribes and can trigger pause/cancellation. This makes the system composable and testable.

**Primary recommendation:** Build an `AgentOrchestrator` in `boternity-core` that manages a tree of sub-agents via `RequestContext` (shared budget + workspace + cancellation token), all publishing to a shared `broadcast::Sender<AgentEvent>`. Keep SSE for basic chat streaming; add WebSocket as a parallel channel for agent hierarchy events and bidirectional commands (cancel, budget continue).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Task Decomposition UX
- Sub-agent activity shown inline in chat as collapsible blocks (like Claude Code's tool use output)
- Each sub-agent block streams its response token-by-token (full streaming, not just status)
- Bot always produces a synthesis response after all sub-agents complete, integrating results into a cohesive answer
- Collapsed sub-agent blocks always show tokens used and duration (transparency into costs)

#### Budget Controls UX
- Per-request token budgets have system defaults with per-bot override in IDENTITY.md frontmatter (`max_request_tokens` field)
- Global default budget configurable in `~/.boternity/config.toml` (`default_request_budget = 500000`)
- Warning threshold fixed at 80% (not configurable)
- At 80% warning: pause execution and ask user "Budget 80% used. Continue?" in both CLI and web
- At budget exhaustion: graceful stop -- present whatever sub-agent results are available, explain what wasn't completed
- Continue/stop only at pause prompt -- no budget increase mid-request
- CLI shows a live running budget counter during sub-agent execution (e.g., `[tokens: 12,450 / 500,000]`)
- Completed requests show estimated cost alongside token count (e.g., `~$0.12 estimated`)
- Cost estimation uses hardcoded per-provider pricing with user override capability in provider config

#### Sub-agent Behavior
- Sub-agents have full memory access -- can both recall and create memories (tagged with which agent created them)
- No explicit cap on parallel sub-agents -- the token budget naturally limits how many can run
- Sequential sub-agents see only the immediately prior sub-agent's result (not the full chain)
- Recursive spawning allowed -- sub-agents can spawn their own sub-agents up to the 3-level depth cap
- Sub-agents inherit the parent bot's personality (SOUL.md) -- they respond in character
- Sub-agents always use the same model as the root agent (configured in IDENTITY.md)
- On sub-agent failure: retry once, then skip and continue with remaining sub-agents + partial results
- No per-agent timeout -- token budget is the only constraint

#### Event Visibility
- Both CLI and web show sub-agent progress in real-time
- CLI uses tree indentation for sub-agent output (e.g., tree characters with depth-based nesting)
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

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Standard Stack

### Core (Rust Backend -- already in workspace)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.49 | Async runtime, JoinSet for parallel tasks, broadcast for event bus | Already in workspace; JoinSet is the standard for managing parallel task groups |
| tokio::sync::broadcast | (part of tokio 1.49) | Event bus channel | Multi-producer, multi-consumer; every subscriber sees every event |
| tokio::task::JoinSet | (part of tokio 1.49) | Parallel sub-agent execution + result collection | Dynamic task spawning with abort_all(), panic recovery, type-safe results |
| futures-util | 0.3 | Stream combinators (split for WebSocket, StreamExt) | Already in workspace; needed for WebSocket sender/receiver split |
| async-stream | 0.3 | Stream construction helpers | Already in workspace; needed for event-enriched SSE streams |
| uuid | 1.20 (v7) | Agent and request IDs | Already in workspace; UUID v7 for time-sortable agent identifiers |
| serde / serde_json | 1.x | Event and command serialization for WebSocket | Already in workspace; tagged enum serialization matches existing StreamEvent pattern |
| console | 0.15 | CLI tree styling (colors, bold) | Already in workspace; used by existing ChatRenderer |

### New Dependencies Required
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum `ws` feature | 0.8.8 | WebSocket upgrade handler in axum | Just a feature flag on existing axum dep; uses tokio-tungstenite 0.28 internally |
| tokio-util | 0.7.18 | CancellationToken for hierarchical task cancellation | Already in dep tree transitively; provides child_token() for parent-child cancel hierarchy |
| dashmap | 6.1 | Concurrent shared workspace (AGNT-06) | Sharded concurrent HashMap; 173M+ downloads; avoids single-lock contention for parallel sub-agents |
| toml | 0.8 | Parse `~/.boternity/config.toml` for global defaults | De facto standard TOML parser in Rust; needed for `default_request_budget` setting |

### Core (React Frontend)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Native WebSocket API | (browser built-in) | WebSocket client for agent events | No external dependency needed; custom hook wraps with reconnection |
| zustand | 5.x | Agent tree + budget state management | Already in web app; new store for agent tree state |
| React 19 | 19.x | UI components | Already in web app |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tokio broadcast | tokio mpsc + manual fan-out | broadcast is simpler for multi-consumer event bus; mpsc would need per-subscriber forwarding |
| Native WebSocket | react-use-websocket npm | custom hook is lighter (no dep); the reconnection logic is ~30 lines |
| JoinSet | tokio::join! macro | join! requires fixed number of tasks at compile time; JoinSet handles dynamic parallel spawning |
| Keep SSE + add WS | Replace SSE with WS entirely | SSE works well for per-request streaming; WS adds bidirectional (cancel, budget continue); coexistence is better |
| DashMap | Arc<RwLock<HashMap>> | DashMap is sharded (one RwLock per shard), far less contention with parallel sub-agents writing disjoint keys |
| XML tag parsing for spawn | Full tool-use framework | Tool-use would require tool registration, validation, execution loop -- XML parsing leverages existing pattern in SystemPromptBuilder |

### Installation

**Rust (Cargo.toml changes):**
```toml
# workspace Cargo.toml
axum = { version = "0.8", features = ["macros", "ws"] }  # add "ws"
dashmap = { version = "6.1", features = ["serde"] }      # new
toml = "0.8"                                              # new

# boternity-core Cargo.toml
tokio-util = { version = "0.7", features = ["sync"] }    # new (for CancellationToken)
dashmap = { workspace = true }                            # new
```

**Frontend:** No new npm packages needed. Native WebSocket API is sufficient.

## Architecture Patterns

### Recommended Project Structure

```
crates/boternity-types/src/
    agent.rs              # EXTEND: SubAgentRequest, SubAgentResult, SpawnMode, AgentStatus
    event.rs              # NEW: AgentEvent enum (all event bus event types)
    config.rs             # NEW: GlobalConfig, RequestBudgetConfig (for config.toml)

crates/boternity-core/src/
    agent/
        context.rs        # EXTEND: depth tracking, child_for_task() method
        engine.rs         # UNCHANGED: stays as single LLM call primitive
        orchestrator.rs   # NEW: AgentOrchestrator (top-level spawn/manage/synthesize)
        spawner.rs        # NEW: SubAgentSpawner (parse spawn instructions, execute)
        budget.rs         # NEW: RequestBudget (Arc<AtomicU32> per-request token tracker)
        cycle_detector.rs # NEW: CycleDetector (HashSet<u64> task signature tracking)
        workspace.rs      # NEW: SharedWorkspace (Arc<DashMap<String, Value>>)
        request_context.rs # NEW: RequestContext (budget + workspace + cancellation)
        prompt.rs         # EXTEND: add <agent_capabilities> section
        mod.rs            # EXTEND: export new modules
    event/
        bus.rs            # NEW: EventBus (tokio::sync::broadcast wrapper)
        mod.rs            # NEW: module exports

crates/boternity-infra/src/
    config.rs             # NEW: config.toml loader (toml crate)

crates/boternity-api/src/
    http/
        handlers/
            ws.rs         # NEW: WebSocket upgrade handler + event forwarding + command receiving
            chat.rs       # EXTEND: orchestrator-aware streaming with sub-agent events
        router.rs         # EXTEND: add /ws/events route
    cli/
        chat/
            tree_renderer.rs  # NEW: CLI tree indentation + box-drawing chars
            budget_display.rs # NEW: Live budget counter rendering
            loop_runner.rs    # EXTEND: use orchestrator, handle cancel commands during execution
    state.rs              # EXTEND: add EventBus to AppState

apps/web/src/
    hooks/
        use-websocket.ts      # NEW: WebSocket with reconnection + exponential backoff
        use-agent-tree.ts     # NEW: agent tree state derived from WS events
    components/
        chat/
            agent-block.tsx       # NEW: Collapsible sub-agent block (inline in chat)
            agent-tree-panel.tsx  # NEW: Process-manager style tree panel with cancel buttons
            budget-indicator.tsx  # NEW: Budget usage bar + cost estimate
            ws-status.tsx         # NEW: Connected / Reconnecting indicator
    stores/
        agent-store.ts    # NEW: Agent tree + budget state (zustand)
    types/
        agent.ts          # NEW: AgentEvent types matching Rust AgentEvent enum
```

### Pattern 1: Event Bus with Broadcast Channel

**What:** A centralized event bus using `tokio::sync::broadcast` that all components subscribe to.
**When to use:** Every agent lifecycle event, budget update, and UI notification flows through this bus.

```rust
// Source: tokio 1.49 broadcast channel docs
use tokio::sync::broadcast;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    AgentSpawned {
        agent_id: Uuid,
        parent_id: Option<Uuid>,
        task_description: String,
        depth: u8,
    },
    AgentTextDelta {
        agent_id: Uuid,
        text: String,
    },
    AgentCompleted {
        agent_id: Uuid,
        result_summary: String,
        tokens_used: u32,
        duration_ms: u64,
    },
    AgentFailed {
        agent_id: Uuid,
        error: String,
        will_retry: bool,
    },
    AgentCancelled {
        agent_id: Uuid,
        reason: String,
    },
    BudgetUpdate {
        tokens_used: u32,
        budget_total: u32,
        percentage: f32,
    },
    BudgetWarning {
        tokens_used: u32,
        budget_total: u32,
    },
    BudgetExhausted {
        tokens_used: u32,
        budget_total: u32,
        completed_agents: Vec<Uuid>,
        incomplete_agents: Vec<Uuid>,
    },
    DepthLimitReached {
        agent_id: Uuid,
        attempted_depth: u8,
        max_depth: u8,
    },
    CycleDetected {
        agent_id: Uuid,
        cycle_description: String,
    },
    SynthesisStarted,
    MemoryCreated {
        agent_id: Uuid,
        fact: String,
    },
    ProviderFailover {
        from_provider: String,
        to_provider: String,
        warning: String,
    },
}

pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: AgentEvent) {
        let _ = self.sender.send(event); // Ignore "no receivers" error
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self { sender: self.sender.clone() }
    }
}
```

### Pattern 2: Hierarchical Cancellation with CancellationToken

**What:** Each sub-agent gets a child token from its parent. Cancelling a parent cancels all children. Individual agents can be cancelled without affecting siblings.
**When to use:** Ctrl+C cancels root token (all agents stop). Web UI stop button cancels individual agent's token.

```rust
// Source: tokio-util 0.7.18 CancellationToken docs
use tokio_util::sync::CancellationToken;

pub struct SubAgentHandle {
    pub agent_id: Uuid,
    pub cancel_token: CancellationToken,
    pub task_handle: tokio::task::JoinHandle<SubAgentResult>,
}

// Root token for the entire request tree
let root_token = CancellationToken::new();

// Each sub-agent gets a child token
let agent_token = root_token.child_token();

// Sub-agent execution respects cancellation via tokio::select!
let handle = tokio::spawn(async move {
    tokio::select! {
        result = execute_sub_agent(context) => result,
        _ = agent_token.cancelled() => SubAgentResult::cancelled(agent_id),
    }
});

// Cancel one agent (children of this agent also cancel)
agent_token.cancel();

// Cancel ALL agents (Ctrl+C)
root_token.cancel();
```

### Pattern 3: Parallel Sub-Agent Execution with JoinSet

**What:** Use `tokio::task::JoinSet` to spawn and collect parallel sub-agent results.
**When to use:** When the root agent decides to run multiple sub-tasks concurrently.

```rust
// Source: tokio 1.49 JoinSet docs
use tokio::task::JoinSet;

async fn execute_parallel_agents(
    tasks: Vec<SubAgentTask>,
    parent_token: &CancellationToken,
    event_bus: &EventBus,
    budget: &RequestBudget,
) -> Vec<SubAgentResult> {
    let mut set = JoinSet::new();

    for task in tasks {
        let token = parent_token.child_token();
        let bus = event_bus.clone();
        let budget = budget.clone();

        set.spawn(async move {
            tokio::select! {
                result = run_single_agent(task, &bus, &budget) => result,
                _ = token.cancelled() => SubAgentResult::cancelled(task.agent_id),
            }
        });
    }

    let mut results = Vec::new();
    while let Some(join_result) = set.join_next().await {
        match join_result {
            Ok(agent_result) => results.push(agent_result),
            Err(e) => {
                // Task panicked -- treat as failure, do NOT propagate
                results.push(SubAgentResult::failed(format!("Task panicked: {e}")));
            }
        }
    }
    results
}
```

### Pattern 4: SSE + WebSocket Coexistence

**What:** Keep existing SSE endpoint for per-request chat streaming. Add WebSocket endpoint for persistent system event delivery and bidirectional commands (cancel, budget continue).
**When to use:** SSE for simple one-agent chat. WebSocket for agent hierarchy events alongside chat.

```rust
// Source: axum 0.8.8 ws module docs
use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message};
use futures_util::{SinkExt, StreamExt};

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.event_bus.subscribe();

    let send_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    let json = serde_json::to_string(&event).unwrap_or_default();
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break; // Client disconnected
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(missed = n, "WebSocket client lagged, continuing");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            // Parse cancel/continue commands from client
            // e.g., {"type": "cancel_agent", "agent_id": "..."}
            // e.g., {"type": "budget_continue"}
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
```

### Pattern 5: Shared Budget Tracker (Arc<AtomicU32>)

**What:** A thread-safe, lock-free budget tracker that all sub-agents increment atomically.
**When to use:** Every sub-agent reports token usage; the orchestrator checks against the budget limit.

```rust
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct RequestBudget {
    total_budget: u32,
    tokens_used: Arc<AtomicU32>,
}

impl RequestBudget {
    pub fn new(total_budget: u32) -> Self {
        Self {
            total_budget,
            tokens_used: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Add tokens and return (new_total, crossed_warning, exceeded_budget).
    pub fn add_tokens(&self, tokens: u32) -> (u32, bool, bool) {
        let threshold = self.total_budget * 80 / 100;
        let prev = self.tokens_used.fetch_add(tokens, Ordering::SeqCst);
        let new_total = prev + tokens;
        let crossed_warning = prev < threshold && new_total >= threshold;
        let exceeded = new_total >= self.total_budget;
        (new_total, crossed_warning, exceeded)
    }

    pub fn tokens_used(&self) -> u32 {
        self.tokens_used.load(Ordering::SeqCst)
    }

    pub fn remaining(&self) -> u32 {
        self.total_budget.saturating_sub(self.tokens_used())
    }
}
```

### Pattern 6: RequestContext (Budget + Workspace + Cancellation)

**What:** A per-request shared context struct grouping the budget, workspace, and cancellation token. Created once per user request, cloned (Arc-based) for all sub-agents.

```rust
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct RequestContext {
    pub request_id: Uuid,
    pub budget: RequestBudget,
    pub workspace: SharedWorkspace, // Arc<DashMap<String, Value>>
    pub cancellation: CancellationToken,
}

impl RequestContext {
    pub fn new(request_id: Uuid, budget: RequestBudget) -> Self {
        Self {
            request_id,
            budget,
            workspace: SharedWorkspace::new(),
            cancellation: CancellationToken::new(),
        }
    }

    /// Create a child context for a sub-agent.
    /// Shares budget + workspace (Arc clones), gets child cancellation token.
    pub fn child(&self) -> Self {
        Self {
            request_id: self.request_id,
            budget: self.budget.clone(),
            workspace: self.workspace.clone(),
            cancellation: self.cancellation.child_token(),
        }
    }
}
```

### Pattern 7: Spawn Decision via XML Tag Parsing

**What:** The LLM agent's response is scanned for `<spawn_agents>` blocks. If found, the orchestrator parses tasks and execution mode, then spawns sub-agents. This leverages the existing XML tag pattern in SystemPromptBuilder.
**When to use:** After each root agent LLM call completes.

```xml
<!-- LLM response contains spawn instructions -->
<spawn_agents mode="parallel">
  <agent task="Research the history of quantum computing" />
  <agent task="Summarize recent breakthroughs in quantum error correction" />
</spawn_agents>
```

```rust
pub fn parse_spawn_instructions(response: &str) -> Option<SpawnInstruction> {
    let start_idx = response.find("<spawn_agents")?;
    let end_tag = "</spawn_agents>";
    let end_idx = response.find(end_tag)?;
    let block = &response[start_idx..end_idx + end_tag.len()];

    let mode = if block.contains(r#"mode="sequential"#) {
        SpawnMode::Sequential
    } else {
        SpawnMode::Parallel
    };

    // Extract task="..." attributes from <agent> elements
    let mut tasks = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = block[search_from..].find(r#"task=""#) {
        let abs_pos = search_from + pos + 6;
        if let Some(end_quote) = block[abs_pos..].find('"') {
            tasks.push(block[abs_pos..abs_pos + end_quote].to_string());
            search_from = abs_pos + end_quote + 1;
        } else { break; }
    }

    if tasks.is_empty() { return None; }
    Some(SpawnInstruction { mode, tasks })
}
```

### Anti-Patterns to Avoid

- **Shared mutable AgentContext across sub-agents:** Each sub-agent MUST get its own `AgentContext` clone with fresh `conversation_history`. Sharing a single context causes data races and confused conversation.
- **Using mpsc instead of broadcast for events:** The event bus must be multi-consumer (WebSocket, CLI renderer, budget tracker all listen). Use broadcast.
- **Blocking on budget pause in the event bus:** The pause prompt is a user interaction. The orchestrator handles the pause by stopping new spawns and waiting for user input through CLI stdin or WebSocket message, not the event bus.
- **Spawning sub-agents as detached tasks:** Always use `JoinSet` so the parent can cancel children. Detached tasks via `tokio::spawn` cannot be cancelled or tracked.
- **Putting EventBus in AgentContext:** `AgentContext` is `#[derive(Clone)]` data struct. Pass EventBus as a separate parameter to the orchestrator/spawner.
- **Unbounded broadcast channel:** Always set bounded capacity (1024). Slow receivers get `RecvError::Lagged`, which is handled gracefully.
- **Holding DashMap Ref across .await:** DashMap's `Ref` holds a shard lock. The `SharedWorkspace` API returns cloned `Value`s to prevent this.

## Event Bus Scope Recommendation (Claude's Discretion)

**Recommendation:** Include memory and provider failover events in the event bus.

1. **Memory events** (`MemoryCreated` by sub-agents) provide transparency -- users see what sub-agents learned. Memory tagging (which agent created it) is a locked decision.
2. **Provider failover events** are already emitted via `print_failover_warning` in CLI. Routing through the event bus lets the web UI display them too.
3. Marginal cost is near-zero (just additional enum variants on the broadcast channel).
4. **Exclude:** Internal tracing/debug events belong in the tracing subscriber, not the user-facing event bus.

## WebSocket Reconnection Strategy (Claude's Discretion)

**Recommendation:** Exponential backoff starting at 1s, doubling to max 30s, with 30% jitter. Max 10 attempts before showing "Disconnected".

```typescript
const INITIAL_DELAY_MS = 1000;
const MAX_DELAY_MS = 30000;
const MAX_ATTEMPTS = 10;
const JITTER_FACTOR = 0.3;

function getReconnectDelay(attempt: number): number {
    const base = Math.min(INITIAL_DELAY_MS * 2 ** attempt, MAX_DELAY_MS);
    const jitter = base * JITTER_FACTOR * (Math.random() * 2 - 1);
    return Math.max(0, base + jitter);
}
```

Reset attempt counter on successful connection. Status indicator states: "Connected" (green dot), "Reconnecting..." (yellow, with attempt count), "Disconnected" (red, after max attempts).

## CLI Tree Indentation (Claude's Discretion)

**Recommendation:** Use Unicode box-drawing characters matching standard tree output.

```
  You > Explain quantum computing in depth

  Bot > I'll break this down into sub-tasks...

  [tokens: 1,200 / 500,000]
  ├── agent-1: Researching quantum computing history...
  │   The history of quantum computing begins in the 1980s when Richard
  │   Feynman proposed that quantum systems could simulate...
  │   | 2,450 tokens · 3.2s
  ├── agent-2: Analyzing recent breakthroughs...
  │   Recent breakthroughs in quantum error correction include...
  │   | 1,890 tokens · 2.8s
  └── agent-3: Listing top companies...
      The leading quantum computing companies are...
      | 1,200 tokens · 2.1s
  [tokens: 6,740 / 500,000 · ~$0.04 estimated]

  Bot > Here's a comprehensive overview of quantum computing...
```

Characters: `├──` (U+251C U+2500 U+2500), `└──` (U+2514 U+2500 U+2500), `│` (U+2502). Colors: agent labels in cyan, budget counter in dim (yellow when >80%).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Parallel task management | Manual `Vec<JoinHandle>` with polling | `tokio::task::JoinSet` | Handles completion ordering, abort_all(), panic recovery |
| Event fan-out to multiple consumers | Custom subscriber list with mpsc | `tokio::sync::broadcast` | Lock-free, handles lagged receivers, built for this |
| WebSocket server | Raw TCP with tungstenite | `axum::extract::ws::WebSocketUpgrade` | Integrates with existing router, handles upgrade protocol |
| Hierarchical task cancellation | Custom boolean flags | `tokio_util::sync::CancellationToken` | child_token() creates parent-child chains, async cancelled() future |
| Atomic token counting | `Arc<Mutex<u32>>` | `Arc<AtomicU32>` | Lock-free, no contention, perfect for single counter |
| Concurrent shared map | `Arc<RwLock<HashMap>>` | `DashMap` | Sharded locking, disjoint key access is lock-free |
| TOML config parsing | Manual string parsing | `toml` crate with serde | De facto standard, 350M+ downloads, derives work with existing serde types |
| Tree indentation in CLI | Manual string padding | `console` crate (already in deps) + box-drawing chars | Already used for styling; box-drawing chars give standard tree structure |

**Key insight:** This phase is primarily orchestration glue. Every individual piece (channels, cancellation, task groups, WebSocket) is a solved problem in the tokio ecosystem. The value is in composing them with the existing `AgentEngine`/`AgentContext` architecture.

## Common Pitfalls

### Pitfall 1: Broadcast Channel Lagging
**What goes wrong:** A slow WebSocket client falls behind, gets `RecvError::Lagged`, and misses events.
**Why it happens:** Sub-agents produce many `TextDelta` events rapidly. A slow client can't keep up.
**How to avoid:** Use generous capacity (1024). Handle `RecvError::Lagged` by logging and continuing (receiver catches up). Optionally batch rapid text deltas before WebSocket send (collect for 50ms, send as one message).
**Warning signs:** Users report missing sub-agent output; "lagged receiver" in logs.

### Pitfall 2: Sub-Agent Context Leaking into Parent
**What goes wrong:** A sub-agent's conversation history leaks into the parent's context, corrupting subsequent LLM calls.
**Why it happens:** `AgentContext` is cloned but mutation occurs on the wrong reference.
**How to avoid:** Sub-agents get a `child_for_task()` clone with empty `conversation_history`. Sub-agent results return to parent as a `String`, never merged into parent's history directly.
**Warning signs:** Parent produces responses referencing sub-agent internal dialogue.

### Pitfall 3: Budget Race on 80% Threshold
**What goes wrong:** Multiple parallel sub-agents cross the 80% threshold simultaneously, each triggering a pause prompt.
**Why it happens:** `AtomicU32::fetch_add` is atomic but the crossed-warning check is per-agent.
**How to avoid:** The `add_tokens()` method returns `crossed_warning` only when `prev < threshold && new_total >= threshold`. Since `fetch_add` is atomic, exactly ONE call will see the transition. The orchestrator (not agents) acts on the warning.
**Warning signs:** Multiple simultaneous budget warning prompts.

### Pitfall 4: WebSocket Disconnect During Agent Execution
**What goes wrong:** User closes tab; agents keep running, consuming tokens with nobody watching.
**Why it happens:** WebSocket disconnection doesn't automatically cancel the agent tree.
**How to avoid:** On WebSocket disconnect, start a 30-second grace period. If no reconnection, cancel the associated request's `CancellationToken`. On reconnect, re-subscribe to event bus and send a state snapshot.
**Warning signs:** Token usage accumulating after user navigates away.

### Pitfall 5: Depth Counting Off-By-One
**What goes wrong:** The 3-level cap actually allows 2 or 4 levels.
**Why it happens:** Ambiguity in whether root agent is depth 0 or 1.
**How to avoid:** Define explicitly: root = depth 0, sub-agents = depth 1/2/3. The check is `if depth >= 3 { reject }`. Sub-agents at depth 3 can execute but cannot spawn children (would be depth 4).
**Warning signs:** Tests passing at wrong boundary; 4 levels or only 2 allowed.

### Pitfall 6: CLI Cancel Command Parsing During Streaming
**What goes wrong:** User types `cancel 2` but it's ignored because stdin isn't read during execution.
**Why it happens:** Current input loop (`rustyline-async`) waits for complete input. During streaming, no polling.
**How to avoid:** During sub-agent execution, spawn a separate tokio task reading stdin for cancel commands. Use `tokio::select!` to listen for both stdin and agent completion.
**Warning signs:** Cancel commands only work after all agents complete.

### Pitfall 7: Memory Tagging Without Agent Identity
**What goes wrong:** Memories from sub-agents are indistinguishable from root agent memories.
**Why it happens:** `MemoryEntry` doesn't have an agent identity field.
**How to avoid:** Add `source_agent_id: Option<Uuid>` to `MemoryEntry`. Root agent memories have `None`; sub-agent memories have `Some(agent_id)`. Backward-compatible (existing memories remain `None`).
**Warning signs:** Can't trace memory provenance; debugging sub-agent behavior impossible.

### Pitfall 8: JoinSet Task Panics Crashing Entire Request
**What goes wrong:** One sub-agent panics; the parent doesn't handle `JoinError` and the request fails.
**Why it happens:** `join_next()` returns `Result<T, JoinError>` -- unwrapping blindly propagates the panic.
**How to avoid:** Always match on `JoinError`. Convert to `SubAgentResult { success: false, error: Some(...) }`. Parent continues with remaining sub-agents.
**Warning signs:** Entire requests failing when one sub-agent hits an edge case.

## Code Examples

### AgentContext Extension for Sub-Agents

```rust
// Source: Extension to existing crates/boternity-core/src/agent/context.rs
impl AgentContext {
    /// Create a child context for a sub-agent task.
    /// Inherits personality (SOUL.md) but gets fresh conversation history.
    pub fn child_for_task(&self, task: &str) -> Self {
        let task_prompt = format!(
            "{}\n\n<task>\nYou are executing a focused sub-task.\n\
            Task: {}\n\
            Respond with your result directly. Be concise.\n</task>",
            self.system_prompt, task
        );

        Self {
            agent_config: self.agent_config.clone(),
            soul_content: self.soul_content.clone(),
            identity_content: self.identity_content.clone(),
            user_content: String::new(),
            memories: Vec::new(),
            recalled_memories: Vec::new(),
            conversation_history: Vec::new(), // FRESH
            token_budget: self.token_budget.clone(),
            system_prompt: task_prompt,
            verbose: self.verbose,
        }
    }
}
```

### React WebSocket Hook with Reconnection

```typescript
// Source: Native WebSocket API + exponential backoff pattern
import { useState, useEffect, useRef, useCallback } from 'react';

export function useAgentWebSocket(url: string) {
  const [status, setStatus] = useState<'connected' | 'reconnecting' | 'disconnected'>('disconnected');
  const wsRef = useRef<WebSocket | null>(null);
  const attemptRef = useRef(0);
  const listenersRef = useRef<((event: AgentEvent) => void)[]>([]);

  const connect = useCallback(() => {
    const ws = new WebSocket(url);
    ws.onopen = () => { attemptRef.current = 0; setStatus('connected'); };
    ws.onmessage = (e) => {
      try {
        const event = JSON.parse(e.data);
        listenersRef.current.forEach(fn => fn(event));
      } catch { /* ignore */ }
    };
    ws.onclose = () => {
      setStatus('reconnecting');
      const base = Math.min(1000 * 2 ** attemptRef.current, 30000);
      const jitter = base * 0.3 * (Math.random() * 2 - 1);
      if (attemptRef.current < 10) {
        setTimeout(() => { attemptRef.current++; connect(); }, base + jitter);
      } else {
        setStatus('disconnected');
      }
    };
    wsRef.current = ws;
  }, [url]);

  const sendCommand = useCallback((cmd: object) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(cmd));
    }
  }, []);

  const onEvent = useCallback((fn: (event: AgentEvent) => void) => {
    listenersRef.current.push(fn);
    return () => { listenersRef.current = listenersRef.current.filter(l => l !== fn); };
  }, []);

  useEffect(() => { connect(); return () => wsRef.current?.close(); }, [connect]);

  return { status, sendCommand, onEvent };
}
```

### CLI Tree Renderer

```rust
// Source: console crate (already in deps) + Unicode box-drawing
const BRANCH: &str = "\u{251C}\u{2500}\u{2500}"; // ├──
const LAST:   &str = "\u{2514}\u{2500}\u{2500}"; // └──
const PIPE:   &str = "\u{2502}   ";               // │
const SPACE:  &str = "    ";                       // (blank indent)

pub fn render_agent_header(depth: u8, index: usize, total: usize, task: &str) -> String {
    let indent = "  ".repeat(depth as usize);
    let branch = if index == total - 1 { LAST } else { BRANCH };
    let label = console::style(format!("agent-{}", index + 1)).cyan();
    format!("{indent}{branch} {label}: {task}")
}

pub fn render_budget_counter(used: u32, total: u32) -> String {
    let pct = (used as f64 / total as f64 * 100.0) as u32;
    let text = format!("[tokens: {:>6} / {:>6}]",
        format_tokens(used), format_tokens(total));
    if pct >= 80 {
        format!("  {}", console::style(text).yellow())
    } else {
        format!("  {}", console::style(text).dim())
    }
}

fn format_tokens(n: u32) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1_000_000.0) }
    else if n >= 1_000 { format!("{:.1}K", n as f64 / 1_000.0) }
    else { n.to_string() }
}
```

### Cost Estimation

```rust
// Source: Existing ProviderCostInfo in boternity-types + locked decision on hardcoded pricing
pub fn estimate_cost(
    input_tokens: u32,
    output_tokens: u32,
    cost_info: &ProviderCostInfo,
) -> f64 {
    let input_cost = (input_tokens as f64 / 1_000_000.0) * cost_info.input_cost_per_million;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * cost_info.output_cost_per_million;
    input_cost + output_cost
}

pub fn format_cost(cost: f64) -> String {
    if cost < 0.01 { format!("~${:.4}", cost) }
    else { format!("~${:.2}", cost) }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual `Vec<JoinHandle>` + loop | `tokio::task::JoinSet` | tokio 1.20 (2022) | Type-safe, handles panics, abort_all(), cancel-safe |
| Custom cancel flags `Arc<AtomicBool>` | `tokio_util::sync::CancellationToken` | tokio-util 0.7 (2022) | Hierarchical via child_token(), async cancelled() future, drop guards |
| External WS library (tungstenite) | axum built-in `ws` feature | axum 0.7+ (2023) | Zero additional deps, integrated with extractors/state |
| async_trait crate | RPITIT (native async fn in traits) | Rust 2024 edition | Project already uses RPITIT; no async_trait needed |
| Arc<Mutex<HashMap>> | DashMap | dashmap 5+ (2022) | Sharded locking, disjoint key access is effectively lock-free |

**Deprecated/outdated:**
- **async_trait crate**: Not needed (Rust 2024 edition with RPITIT).
- **Manual JoinHandle Vec tracking**: Use JoinSet for dynamic parallel tasks.
- **Global mutex for shared state**: Use DashMap or atomics.

## Open Questions

1. **How does the LLM decide to spawn sub-agents?**
   - What we know: The LLM response needs a signal to decompose. Tool use infrastructure doesn't exist yet. XML tags are the existing pattern.
   - What's unclear: Prompt engineering reliability for producing well-formed `<spawn_agents>` blocks.
   - Recommendation: Use XML tag parsing (matches existing `<soul>`, `<identity>` pattern). Add `<agent_capabilities>` to system prompt. Start with this; evolve to tool-use later. Test with 2-3 bot personas for reliability.

2. **Config.toml parsing library**
   - What we know: `~/.boternity/config.toml` is needed for `default_request_budget`. No TOML parsing exists yet.
   - What's unclear: Config structure beyond budget.
   - Recommendation: Use `toml = "0.8"` (350M+ downloads, de facto standard). Define a small `GlobalConfig` struct with `default_request_budget: u32`. Expand as needed.

3. **WebSocket authentication**
   - What we know: REST API uses `Authenticated` extractor. Current setup is localhost-focused.
   - What's unclear: Whether WS needs auth.
   - Recommendation: No auth for localhost WebSocket (matches existing REST API pattern). Add optional token query param for future remote access.

4. **Budget pause prompt implementation**
   - What we know: User decision says "pause execution and ask user" at 80%.
   - What's unclear: How to implement the pause/resume flow for both CLI and web.
   - Recommendation: CLI uses a `tokio::sync::oneshot` channel -- orchestrator pauses and sends prompt via event bus, CLI renders prompt and sends response on the channel. Web uses the WebSocket bidirectional channel -- server sends `BudgetWarning` event, client sends `{"type": "budget_continue"}` or `{"type": "budget_stop"}` command.

5. **Shared workspace (AGNT-06) scope**
   - What we know: Requirement says "opt-in shared workspace." `LanceSharedMemoryStore` exists for cross-bot memory.
   - What's unclear: Whether this needs new persistent infrastructure or ephemeral per-request state.
   - Recommendation: Per-request ephemeral `SharedWorkspace` using `DashMap`. Dropped when request completes. For persistent cross-bot sharing, the existing `LanceSharedMemoryStore` already covers that.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `AgentEngine`, `AgentContext`, `SystemPromptBuilder`, `StreamEvent`, `FallbackChain`, `BoxLlmProvider`, `TokenBudget`, `ChatService`, `AppState` -- all thoroughly analyzed
- [tokio 1.49 broadcast channel](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html) -- multi-consumer semantics, capacity, lagged handling
- [tokio 1.49 JoinSet](https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html) -- spawn, join_next, join_all, abort_all, len
- [tokio-util 0.7.18 CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html) -- child_token(), cancelled(), drop_guard(), run_until_cancelled()
- [axum 0.8.8 WebSocket](https://docs.rs/axum/latest/axum/extract/ws/index.html) -- WebSocketUpgrade, split pattern, Message types, `ws` feature flag

### Secondary (MEDIUM confidence)
- [Axum WebSocket example](https://github.com/tokio-rs/axum/blob/main/examples/websockets/src/main.rs) -- broadcast + WebSocket integration pattern
- [Event bus with tokio broadcast](https://blog.digital-horror.com/blog/event-bus-in-tokio/) -- EventBus wrapper pattern
- [WebSocket reconnection 2026](https://oneuptime.com/blog/post/2026-01-24-websocket-reconnection-logic/view) -- Exponential backoff with jitter best practices
- [Tokio task cancellation patterns](https://cybernetist.com/2024/04/19/rust-tokio-task-cancellation-patterns/) -- CancellationToken, JoinHandle abort patterns

### Tertiary (LOW confidence)
- [AutoAgents Rust framework](https://github.com/liquidos-ai/AutoAgents) -- Multi-agent patterns in Rust (reference only)
- [Sub-agent orchestration pattern (Spring AI)](https://gaetanopiazzolla.github.io/java/ai/2026/02/09/sub-agent-pattern.html) -- Cross-platform pattern validation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All deps verified against current dependency tree and official docs. Only 3 new deps (dashmap, toml, tokio-util feature).
- Architecture: HIGH -- Patterns derived from thorough codebase analysis. Extension points clear (AgentContext, AgentEngine, router, AppState).
- Event bus: HIGH -- tokio::sync::broadcast semantics verified from official docs. Capacity and lagging behavior understood.
- WebSocket: HIGH -- axum 0.8.8 ws feature verified. Handler pattern from official example.
- Budget enforcement: HIGH -- Three-tier model (warn/block/cancel) with bounded overshoot analysis. AtomicU32 ordering verified.
- Cancellation: HIGH -- CancellationToken child_token() hierarchy verified from tokio-util 0.7.18 docs.
- Spawn decision: MEDIUM -- XML parsing approach aligns with existing patterns but LLM reliability for structured output is inherently uncertain.
- Cycle detection: MEDIUM -- Hash-based approach is sound but semantic task equivalence is fuzzy. Circuit breaker provides safety net.

**Research date:** 2026-02-13
**Valid until:** 2026-03-15 (stable ecosystem; tokio, axum, dashmap are mature with no expected breaking changes)
