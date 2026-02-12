# Phase 5: Agent Hierarchy + Event System - Research

**Researched:** 2026-02-13
**Domain:** Sub-agent orchestration, event bus (tokio::sync::broadcast), WebSocket live updates, per-request token budget enforcement
**Confidence:** HIGH

## Summary

Phase 5 adds hierarchical agent orchestration to an existing single-agent system. The current `AgentEngine` + `AgentContext` in boternity-core executes one LLM call per user message. This phase extends it so an agent can spawn sub-agents (sequential or parallel) up to 3 levels deep, with message passing between parent and child agents, a typed event bus for real-time UI updates, WebSocket delivery to the React frontend, and per-request token budget enforcement to prevent runaway costs.

The architecture builds directly on existing patterns: `BoxLlmProvider` for dynamic dispatch, `AgentContext` for conversation state, `TokenBudget` for context window allocation, and `FallbackChain` for provider selection. The key new abstractions are: (1) `SubAgentSpawner` that creates child `AgentContext` instances with inherited configuration but fresh conversation history, (2) `RequestBudget` using `Arc<AtomicU64>` for thread-safe token accounting shared across an entire sub-agent tree, (3) `EventBus` wrapping `tokio::sync::broadcast` for typed system events, and (4) a WebSocket handler in boternity-api that subscribes to the event bus and fans out events to connected frontends.

The existing codebase is well-structured for this extension. `AgentContext` already carries `agent_config`, `token_budget`, and `system_prompt`. The `build_request` pattern in both `AgentEngine` and `loop_runner.rs` provides the template for sub-agent request construction. The `FallbackChain` already tracks per-call token usage via `StreamEvent::Usage`. The main architectural challenge is threading a shared `RequestBudget` through the agent tree without making `AgentContext` non-Clone (it is currently `#[derive(Clone)]`).

**Primary recommendation:** Build the sub-agent engine as a new module in boternity-core (`agent/spawner.rs`) that wraps `AgentEngine` with depth tracking, budget enforcement, and cycle detection. Use `Arc<AtomicU64>` for the shared token counter, `tokio::task::JoinSet` for parallel sub-agent execution, and `tokio::sync::broadcast` for the event bus. Axum's built-in WebSocket support (`axum::extract::ws`) handles the WebSocket layer with no additional dependencies.

## Standard Stack

### Core (already in workspace)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.x | Async runtime, `JoinSet`, `broadcast`, `mpsc` | Already in workspace, provides all concurrency primitives needed |
| tokio::sync::broadcast | (part of tokio) | Typed event bus | Specified in INFR-03 requirement; multi-producer multi-consumer |
| tokio::task::JoinSet | (part of tokio) | Parallel sub-agent execution | Dynamic task spawning with result collection |
| futures-util | 0.3 | Stream combinators | Already in workspace, needed for WebSocket stream handling |
| uuid | 1.20 (v7) | Agent and request IDs | Already in workspace, UUID v7 for time-sortable IDs |
| serde / serde_json | 1.x | Event serialization | Already in workspace, events serialized to JSON for WebSocket |
| tracing | 0.1 | Structured logging and spans | Already in workspace, OTel integration from Phase 2 |

### New Dependencies Required
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum `ws` feature | 0.8 | WebSocket upgrade handler | Already using axum 0.8, just need to enable `ws` feature |
| dashmap | 6.1 | Concurrent shared workspace | Sharded concurrent HashMap for lock-free sub-agent workspace access |
| tokio-util | latest | CancellationToken for hierarchical task cancellation | Provides child_token() for parent-child cancellation propagation |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tokio::sync::broadcast | tokio-bus, bus crate | broadcast is built into tokio, no extra dep; bus crate adds lock-free but bounded |
| JoinSet for parallel agents | tokio::join! macro | join! requires fixed count at compile time; JoinSet handles dynamic spawning |
| AtomicU64 for budget counter | Arc<Mutex<u64>> | Mutex adds contention; AtomicU64 is lock-free for simple add/check pattern |
| axum WebSocket | SSE (already planned for Phase 4) | WebSocket is bidirectional; SSE is sufficient for read-only events but req says OBSV-06 specifies WebSocket |
| DashMap for workspace | Arc<RwLock<HashMap>> | DashMap is sharded (one RwLock per shard), reducing contention vs single-lock HashMap |

**Installation:**
```toml
# In workspace Cargo.toml, update axum features and add new deps:
axum = { version = "0.8", features = ["macros", "ws"] }
dashmap = { version = "6.1", features = ["serde"] }
tokio-util = "0.7"
```

## Architecture Patterns

### Recommended Project Structure
```
crates/boternity-types/src/
  agent.rs               # ADD: SubAgentConfig, SubAgentResult, AgentSpawnRequest, SpawnMode
  event.rs               # NEW: SystemEvent enum, EventPayload variants

crates/boternity-core/src/
  agent/
    mod.rs               # UPDATE: export new modules
    context.rs           # UPDATE: add depth, parent_id, request_budget_ref fields
    engine.rs            # UNCHANGED (single LLM call stays as-is)
    spawner.rs           # NEW: SubAgentSpawner (sequential + parallel)
    budget.rs            # NEW: RequestBudget (Arc<AtomicU64> wrapper)
    cycle.rs             # NEW: CycleDetector (HashSet<AgentSignature>)
    workspace.rs         # NEW: SharedWorkspace (DashMap wrapper)
    prompt.rs            # UNCHANGED
    summarizer.rs        # UNCHANGED
    title.rs             # UNCHANGED
  event/
    mod.rs               # NEW: EventBus, typed broadcast wrapper
    bus.rs               # NEW: EventBus implementation

crates/boternity-api/src/
  http/
    handlers/
      ws.rs              # NEW: WebSocket upgrade handler, event fan-out
    router.rs            # UPDATE: add WebSocket route
  state.rs               # UPDATE: add EventBus to AppState
```

### Pattern 1: Sub-Agent Context Hierarchy

**What:** Each sub-agent gets a fresh `AgentContext` derived from its parent, with a reduced system prompt focused on the sub-task, inheriting the same `agent_config` but with an incremented depth counter and a shared request budget reference.

**When to use:** Every sub-agent spawn, whether sequential or parallel.

**Key design decisions:**
- Sub-agents get a FRESH conversation history (empty). They do NOT inherit the parent's conversation.
- Sub-agents DO inherit the parent's `agent_config` (model, temperature, max_tokens).
- Sub-agents get a FOCUSED system prompt: the parent's soul + a task-specific instruction.
- The `depth` field is checked before every spawn attempt.
- The `request_budget` is an `Arc<AtomicU64>` shared across the entire sub-agent tree for a single user request.

**Example:**
```rust
// crates/boternity-types/src/agent.rs -- new types

/// Mode of sub-agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpawnMode {
    Sequential,
    Parallel,
}

/// Request to spawn a sub-agent.
#[derive(Debug, Clone)]
pub struct SubAgentRequest {
    /// Unique ID for this sub-agent invocation.
    pub id: Uuid,
    /// ID of the parent agent that spawned this sub-agent.
    pub parent_id: Uuid,
    /// Human-readable task description for the sub-agent.
    pub task: String,
    /// Current depth (0 = root agent, 1 = first sub-agent, etc.).
    pub depth: u8,
    /// Maximum allowed depth.
    pub max_depth: u8,
}

/// Result returned by a completed sub-agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentResult {
    /// The sub-agent's ID.
    pub agent_id: Uuid,
    /// The task that was assigned.
    pub task: String,
    /// The sub-agent's response.
    pub result: String,
    /// Token usage for this sub-agent (input + output).
    pub tokens_used: u64,
    /// Whether the sub-agent completed successfully.
    pub success: bool,
    /// Error message if the sub-agent failed.
    pub error: Option<String>,
}
```

### Pattern 2: Request Budget with AtomicU64

**What:** A per-request token budget that is shared across the entire sub-agent tree using `Arc<AtomicU64>`. Each agent checks and updates the budget atomically before and after LLM calls.

**When to use:** Every LLM call within a request's sub-agent tree.

**Example:**
```rust
// crates/boternity-core/src/agent/budget.rs
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Per-request token budget shared across a sub-agent tree.
///
/// Uses AtomicU64 for lock-free concurrent access from parallel sub-agents.
/// The budget tracks TOTAL tokens (input + output) consumed across ALL
/// agents in the tree for a single user request.
#[derive(Debug, Clone)]
pub struct RequestBudget {
    /// Maximum tokens allowed for this request.
    max_tokens: u64,
    /// Tokens consumed so far (atomically updated).
    consumed: Arc<AtomicU64>,
    /// Warning threshold (percentage of max_tokens, e.g., 80%).
    warn_threshold: u64,
}

impl RequestBudget {
    pub fn new(max_tokens: u64) -> Self {
        let warn_threshold = max_tokens * 80 / 100;
        Self {
            max_tokens,
            consumed: Arc::new(AtomicU64::new(0)),
            warn_threshold,
        }
    }

    /// Check if there is budget remaining for at least `estimated_tokens`.
    pub fn can_spend(&self, estimated_tokens: u64) -> bool {
        self.consumed.load(Ordering::Relaxed) + estimated_tokens <= self.max_tokens
    }

    /// Record tokens consumed. Returns the new total.
    pub fn record(&self, tokens: u64) -> u64 {
        self.consumed.fetch_add(tokens, Ordering::Relaxed) + tokens
    }

    /// Whether the budget is approaching the limit.
    pub fn is_warning(&self) -> bool {
        self.consumed.load(Ordering::Relaxed) >= self.warn_threshold
    }

    /// Whether the budget is exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.consumed.load(Ordering::Relaxed) >= self.max_tokens
    }

    /// Current consumption.
    pub fn consumed(&self) -> u64 {
        self.consumed.load(Ordering::Relaxed)
    }

    /// Maximum budget.
    pub fn max(&self) -> u64 {
        self.max_tokens
    }
}
```

### Pattern 3: Typed Event Bus with tokio::sync::broadcast

**What:** A typed event bus wrapping `tokio::sync::broadcast` that emits `SystemEvent` enums. Any component can publish events; WebSocket handlers subscribe and forward to connected frontends.

**When to use:** Agent spawning events, completion events, budget warnings, error events.

**Example:**
```rust
// crates/boternity-types/src/event.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Events emitted by the agent system for real-time UI updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SystemEvent {
    /// A sub-agent was spawned.
    AgentSpawned {
        agent_id: Uuid,
        parent_id: Option<Uuid>,
        task: String,
        depth: u8,
        timestamp: DateTime<Utc>,
    },
    /// An agent started executing (LLM call began).
    AgentExecuting {
        agent_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    /// Streaming text delta from an agent.
    AgentTextDelta {
        agent_id: Uuid,
        text: String,
    },
    /// An agent completed successfully.
    AgentCompleted {
        agent_id: Uuid,
        tokens_used: u64,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    /// An agent failed.
    AgentFailed {
        agent_id: Uuid,
        error: String,
        timestamp: DateTime<Utc>,
    },
    /// Token budget warning (approaching limit).
    BudgetWarning {
        request_id: Uuid,
        consumed: u64,
        max: u64,
        timestamp: DateTime<Utc>,
    },
    /// Token budget exhausted (execution paused).
    BudgetExhausted {
        request_id: Uuid,
        consumed: u64,
        max: u64,
        timestamp: DateTime<Utc>,
    },
    /// Cycle detected in sub-agent spawning.
    CycleDetected {
        agent_id: Uuid,
        task_signature: String,
        timestamp: DateTime<Utc>,
    },
    /// Depth limit reached.
    DepthLimitReached {
        agent_id: Uuid,
        depth: u8,
        max_depth: u8,
        timestamp: DateTime<Utc>,
    },
}
```

```rust
// crates/boternity-core/src/event/bus.rs
use tokio::sync::broadcast;
use boternity_types::event::SystemEvent;

/// Capacity of the event bus broadcast channel.
/// 256 is sufficient for real-time events; slow receivers will
/// receive RecvError::Lagged and can catch up.
const EVENT_BUS_CAPACITY: usize = 256;

/// Typed event bus for system-wide notifications.
///
/// Wraps tokio::sync::broadcast to provide a typed publish-subscribe
/// mechanism. Publishers clone the sender; subscribers call subscribe()
/// to get a receiver.
#[derive(Debug, Clone)]
pub struct EventBus {
    sender: broadcast::Sender<SystemEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_BUS_CAPACITY);
        Self { sender }
    }

    /// Publish an event to all subscribers.
    /// Returns the number of receivers that will see this event.
    /// If no subscribers exist, the event is silently dropped (not an error).
    pub fn publish(&self, event: SystemEvent) -> usize {
        // send() returns Err only when there are no receivers.
        // This is expected (no WebSocket clients connected), not an error.
        self.sender.send(event).unwrap_or(0)
    }

    /// Subscribe to events. Returns a receiver that yields all
    /// future events published after this call.
    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent> {
        self.sender.subscribe()
    }
}
```

### Pattern 4: Sub-Agent Spawner with JoinSet

**What:** The `SubAgentSpawner` orchestrates sub-agent execution. Sequential sub-agents run one after another with each result fed to the next. Parallel sub-agents run concurrently via `tokio::task::JoinSet`.

**When to use:** When the root agent determines a task should be decomposed.

**Example:**
```rust
// crates/boternity-core/src/agent/spawner.rs (simplified)
use tokio::task::JoinSet;

pub struct SubAgentSpawner {
    event_bus: EventBus,
}

impl SubAgentSpawner {
    /// Execute sub-agents sequentially. Each result is available to the next.
    pub async fn run_sequential(
        &self,
        requests: Vec<SubAgentRequest>,
        provider: &BoxLlmProvider,
        parent_context: &AgentContext,
        budget: &RequestBudget,
    ) -> Vec<SubAgentResult> {
        let mut results = Vec::with_capacity(requests.len());
        for request in requests {
            if budget.is_exhausted() {
                self.event_bus.publish(SystemEvent::BudgetExhausted { /* ... */ });
                break;
            }
            let result = self.execute_single(request, provider, parent_context, budget).await;
            results.push(result);
        }
        results
    }

    /// Execute sub-agents in parallel using JoinSet.
    pub async fn run_parallel(
        &self,
        requests: Vec<SubAgentRequest>,
        provider: &BoxLlmProvider,
        parent_context: &AgentContext,
        budget: &RequestBudget,
    ) -> Vec<SubAgentResult> {
        let mut join_set = JoinSet::new();

        for request in requests {
            // Clone what's needed for the spawned task
            let bus = self.event_bus.clone();
            let budget = budget.clone();
            // ... clone provider, context
            join_set.spawn(async move {
                // execute_single inside spawned task
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(sub_result) => results.push(sub_result),
                Err(e) => { /* JoinError -- task panicked */ }
            }
        }
        results
    }
}
```

### Pattern 5: WebSocket Event Fan-Out

**What:** An axum WebSocket handler that subscribes to the EventBus and forwards serialized events to connected clients. Each client gets its own broadcast::Receiver.

**When to use:** The `/ws/events` endpoint.

**Example:**
```rust
// crates/boternity-api/src/http/handlers/ws.rs
use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message};
use axum::extract::State;
use axum::response::Response;

pub async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.event_bus.subscribe();

    // Forward events from bus to WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            let json = serde_json::to_string(&event).unwrap();
            if sender.send(Message::Text(json.into())).await.is_err() {
                break; // Client disconnected
            }
        }
    });

    // Read from WebSocket (handle client messages or close)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(_msg)) = receiver.next().await {
            // Handle ping/pong or client commands
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
```

### Pattern 6: Cycle Detection via Task Signature Hashing

**What:** Before spawning a sub-agent, compute a "signature" from the task description (normalized) and check it against a `HashSet` carried through the agent tree. If the same signature appears at a different depth, it indicates a cycle (agent A spawns B which spawns a task equivalent to A).

**When to use:** Every spawn attempt.

**Example:**
```rust
// crates/boternity-core/src/agent/cycle.rs
use std::collections::HashSet;

/// Detects cycles in sub-agent spawning chains.
///
/// A cycle occurs when a sub-agent attempts to spawn a task
/// that is semantically equivalent to one already in its ancestor chain.
/// Uses normalized task description hashing for detection.
#[derive(Debug, Clone)]
pub struct CycleDetector {
    /// Signatures of tasks in the current spawn chain.
    seen_signatures: HashSet<u64>,
    /// Maximum number of unique spawns allowed before circuit-breaking.
    max_spawns: u32,
    /// Current spawn count.
    spawn_count: u32,
}

impl CycleDetector {
    pub fn new(max_spawns: u32) -> Self {
        Self {
            seen_signatures: HashSet::new(),
            max_spawns,
            spawn_count: 0,
        }
    }

    /// Check if spawning a task with this description would create a cycle.
    /// Returns true if a cycle is detected or the circuit breaker triggers.
    pub fn would_cycle(&self, task: &str) -> bool {
        if self.spawn_count >= self.max_spawns {
            return true; // Circuit breaker
        }
        let sig = self.compute_signature(task);
        self.seen_signatures.contains(&sig)
    }

    /// Record a spawned task. Must be called after successful spawn.
    pub fn record_spawn(&mut self, task: &str) {
        let sig = self.compute_signature(task);
        self.seen_signatures.insert(sig);
        self.spawn_count += 1;
    }

    /// Create a child detector that inherits the parent's history.
    pub fn child(&self) -> Self {
        Self {
            seen_signatures: self.seen_signatures.clone(),
            max_spawns: self.max_spawns,
            spawn_count: self.spawn_count,
        }
    }

    fn compute_signature(&self, task: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let normalized = task.to_lowercase().trim().to_string();
        let mut hasher = DefaultHasher::new();
        normalized.hash(&mut hasher);
        hasher.finish()
    }
}
```

### Anti-Patterns to Avoid

- **Do NOT share conversation history between parent and child agents:** Child agents get fresh conversation history. Sharing parent history would blow the context window and cause the child to lose focus on its sub-task.
- **Do NOT use `Mutex` for the token budget counter:** `AtomicU64` with `Ordering::Relaxed` is sufficient for a monotonically increasing counter. Mutex adds unnecessary contention for parallel sub-agents.
- **Do NOT spawn sub-agents as detached tasks:** Always use `JoinSet` or structured concurrency so the parent can cancel children if the budget is exhausted or an error occurs. Detached tasks cannot be cancelled.
- **Do NOT put the EventBus sender in each AgentContext:** AgentContext is a data struct that should remain serializable/clonable. Pass the EventBus as a separate parameter to the spawner, not embedded in context.
- **Do NOT use unbounded channels for the event bus:** The broadcast channel MUST have a bounded capacity. Slow WebSocket receivers should lag gracefully (with `RecvError::Lagged`), not cause unbounded memory growth.
- **Do NOT make sub-agent depth configurable per-agent:** The requirement says "hard-capped at 3 levels" with "enforcement at AgentContext level". This is a system-wide constant, not a per-agent setting.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Parallel task management | Custom spawn + join logic | `tokio::task::JoinSet` | Handles cancellation, panics, dynamic task count |
| Event broadcast | Custom subscriber list with locks | `tokio::sync::broadcast` | Built into tokio, handles lagging, clone-on-demand |
| WebSocket handling | Custom TCP + upgrade | `axum::extract::ws` | Built into axum 0.8 with `ws` feature, handles upgrade handshake |
| Thread-safe counter | `Arc<Mutex<u64>>` | `Arc<AtomicU64>` | Lock-free, zero contention, standard library |
| Task cancellation | Custom flag + polling | `JoinSet::abort_all()` + `tokio::select!` | Cooperative cancellation built into tokio |
| JSON serialization for events | Manual string formatting | `serde_json::to_string` with `#[serde(tag = "type")]` | Tagged enum serialization matches existing `StreamEvent` pattern |
| WebSocket split read/write | Custom message queue | `futures_util::StreamExt::split()` on WebSocket | Standard pattern, concurrent read/write on same socket |
| Concurrent shared map | `Arc<RwLock<HashMap>>` | `DashMap` | Sharded locking, no global contention, serde support |
| Hierarchical cancellation | Manual flag propagation | `tokio_util::sync::CancellationToken` | child_token() for unidirectional cancellation propagation |

**Key insight:** This phase requires only two new external dependencies (dashmap 6.1 for shared workspace, tokio-util for CancellationToken). Everything else is built from tokio primitives (broadcast, JoinSet, AtomicU64), standard library types (HashSet, Arc), and existing workspace crates (axum with ws feature, serde_json, uuid). The complexity is in the orchestration logic, not in finding libraries.

## Common Pitfalls

### Pitfall 1: Broadcast Channel Lagging

**What goes wrong:** A slow WebSocket client falls behind on the event bus, causing `RecvError::Lagged`. If not handled, the receiver stops receiving events entirely.
**Why it happens:** `tokio::sync::broadcast` drops the oldest messages when the channel is at capacity. A receiver that hasn't consumed the dropped message gets `Lagged(n)` on next `recv()`.
**How to avoid:** Handle `RecvError::Lagged(n)` explicitly in the WebSocket forwarding task -- log it, skip the missed events, and continue. Do NOT treat it as a fatal error. Set broadcast capacity to 256 (generous for real-time UI events).
**Warning signs:** WebSocket clients stop receiving events after a burst; "lagged" errors in logs.

### Pitfall 2: Sub-Agent Context Window Blowup

**What goes wrong:** Child agents inherit the parent's full conversation history, causing context length errors on the LLM call.
**Why it happens:** Developers copy the parent `AgentContext` wholesale including its `conversation_history`.
**How to avoid:** Sub-agents MUST start with empty `conversation_history`. They receive their task via the system prompt or a single user message, not via inherited history. The sub-agent's system prompt should include the parent's soul/identity but focus on the specific sub-task.
**Warning signs:** `LlmError::ContextLengthExceeded` on sub-agent calls; sub-agents producing unfocused responses.

### Pitfall 3: AtomicU64 Ordering Semantics

**What goes wrong:** Using `Ordering::SeqCst` everywhere creates unnecessary memory barriers. Using `Ordering::Relaxed` for a check-then-act pattern allows TOCTOU race conditions.
**Why it happens:** Developers are either too cautious (SeqCst) or too relaxed.
**How to avoid:** For the token budget, `Ordering::Relaxed` is correct for both `fetch_add` and `load`. The budget is a soft limit -- it's acceptable if two parallel agents briefly exceed it by one call each before seeing the updated total. The hard enforcement happens before EACH call, not atomically across calls. If exact enforcement is needed, use `compare_exchange` in a CAS loop, but this is unnecessary for token budgets.
**Warning signs:** Performance bottleneck under parallel sub-agents; overly strict budget enforcement causing unnecessary pauses.

### Pitfall 4: JoinSet Task Panics

**What goes wrong:** A sub-agent task panics (e.g., serialization error) and the parent doesn't handle the `JoinError`.
**Why it happens:** `JoinSet::join_next()` returns `Result<T, JoinError>` where `JoinError` wraps a panic. If the parent unwraps blindly, the entire request fails.
**How to avoid:** Always match on `JoinError` -- convert it to a `SubAgentResult` with `success: false` and an error message. The parent agent should continue processing other sub-agents even if one panics.
**Warning signs:** Requests failing entirely when a single sub-agent encounters an edge case.

### Pitfall 5: WebSocket Connection Cleanup

**What goes wrong:** The broadcast::Receiver is not dropped when a WebSocket client disconnects, causing the broadcast channel to try sending to a dead receiver.
**Why it happens:** The event forwarding task keeps running after the WebSocket closes.
**How to avoid:** Use `tokio::select!` to race the send task and receive task. When either finishes (WebSocket closed or event bus closed), both tasks are cancelled. The broadcast::Receiver is dropped automatically when the task ends, which is correct behavior -- broadcast senders don't track individual receivers, they just clone on send.
**Warning signs:** Growing memory usage per disconnected client; "send to closed channel" errors.

### Pitfall 6: Depth Counting Off-By-One

**What goes wrong:** The root agent is counted as depth 0 or 1 inconsistently, causing the 3-level cap to actually allow 2 or 4 levels.
**Why it happens:** Ambiguity in whether "depth" means the current level or the number of parent-child hops.
**How to avoid:** Define explicitly: root agent = depth 0, first sub-agent = depth 1, second sub-agent = depth 2, third sub-agent = depth 3 (ALLOWED), fourth sub-agent = depth 4 (BLOCKED). The check is `if depth >= MAX_DEPTH { return Err(...) }` where `MAX_DEPTH = 4`. This gives exactly 3 levels of sub-agents below the root (depth 1, 2, 3).
**Warning signs:** Tests passing at the wrong boundary; users able to spawn 4 levels or only 2.

### Pitfall 7: Budget Enforcement Race with Parallel Agents

**What goes wrong:** Two parallel sub-agents both check the budget and see "enough remaining", then both execute LLM calls, exceeding the budget.
**Why it happens:** The check-then-act pattern is not atomic across "check budget" and "consume tokens".
**How to avoid:** This is acceptable behavior for token budgets. The budget is a soft limit. After each LLM call completes, the budget is updated with actual token usage. The next spawn attempt will see the updated budget. The overshoot is at most one LLM call per parallel agent -- bounded and predictable. Document this behavior explicitly.
**Warning signs:** Budget exceeded by exactly one parallel agent's token usage; budget exactly at limit after parallel execution.

### Pitfall 8: Cycle Detection False Positives

**What goes wrong:** Two legitimately different tasks hash to the same signature, blocking valid sub-agent spawning.
**Why it happens:** Hash collisions with simple string hashing, or over-normalization (e.g., stripping all numbers).
**How to avoid:** Use the full normalized (lowercased, trimmed) task string for hashing. Do NOT try to do semantic comparison. If false positives become a problem, combine the task string with the depth level in the hash. The circuit breaker (`max_spawns`) provides the ultimate safety net regardless of cycle detection accuracy.
**Warning signs:** Valid multi-step tasks being blocked; same task at different depths being flagged.

## Code Examples

### AgentContext Extension for Sub-Agents

```rust
// Extension to existing crates/boternity-core/src/agent/context.rs

impl AgentContext {
    /// Create a child agent context for a sub-agent.
    ///
    /// Inherits the parent's agent_config and soul content, but starts
    /// with empty conversation history and a task-focused system prompt.
    pub fn child_for_task(
        &self,
        task: &str,
        depth: u8,
        parent_id: Uuid,
    ) -> Self {
        // Build a focused system prompt for the sub-task
        let task_system_prompt = format!(
            "{}\n\n<task>\nYou are a sub-agent executing a specific task.\n\
            Task: {}\n\
            Respond with your result directly. Be concise and focused.\n</task>",
            self.system_prompt, task
        );

        Self {
            agent_config: self.agent_config.clone(),
            soul_content: self.soul_content.clone(),
            identity_content: self.identity_content.clone(),
            user_content: String::new(), // Sub-agents don't need USER.md
            memories: Vec::new(),        // Sub-agents don't recall memories
            recalled_memories: Vec::new(),
            conversation_history: Vec::new(), // FRESH history
            token_budget: self.token_budget.clone(),
            system_prompt: task_system_prompt,
            verbose: self.verbose,
        }
    }
}
```

### Axum WebSocket Route Registration

```rust
// Update to crates/boternity-api/src/http/router.rs
use crate::http::handlers::ws;

pub fn build_router(state: AppState) -> Router {
    // ... existing routes ...

    Router::new()
        .nest("/api/v1", api_routes)
        .route("/health", get(health_check))
        .route("/ws/events", get(ws::ws_events))  // WebSocket endpoint
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

### AppState Extension

```rust
// Update to crates/boternity-api/src/state.rs
use boternity_core::event::bus::EventBus;

pub struct AppState {
    // ... existing fields ...

    /// Event bus for system-wide notifications (WebSocket, UI updates).
    pub event_bus: Arc<EventBus>,
}

// In AppState::init():
let event_bus = Arc::new(EventBus::new());
```

### Default Request Budget Calculation

```rust
// Per-request budget defaults based on the existing TokenBudget pattern
impl RequestBudget {
    /// Create a default request budget based on model pricing.
    ///
    /// For Claude Sonnet: ~200K context, so a reasonable per-request
    /// budget for a sub-agent tree is 500K tokens total (allowing
    /// multiple sub-agents to each use a portion of the context window).
    pub fn default_for_model(model: &str) -> Self {
        let max_tokens = match model {
            m if m.contains("opus") => 1_000_000,
            m if m.contains("sonnet") => 500_000,
            m if m.contains("haiku") => 300_000,
            _ => 500_000,
        };
        Self::new(max_tokens)
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| async_trait for async trait methods | RPITIT (native async fn in traits) | Rust 2024 edition | Project already uses RPITIT; sub-agent traits should too |
| tokio::spawn + manual JoinHandle tracking | tokio::task::JoinSet | Stabilized tokio 1.20+ | Dynamic task management with cancellation; use for parallel sub-agents |
| Custom event emitter with Arc<Mutex<Vec<Callback>>> | tokio::sync::broadcast | Built-in to tokio | Type-safe pub-sub without custom infrastructure |
| SSE for real-time updates | WebSocket (bidirectional) | Always available | Requirement OBSV-06 specifies WebSocket; allows client-to-server messages (e.g., cancel request) |

**Deprecated/outdated:**
- **async_trait crate**: Not needed in this project (Rust 2024 edition with RPITIT).
- **Manual task tracking with JoinHandle Vec**: Use `JoinSet` instead for dynamic parallel tasks.

## Open Questions

1. **How does the LLM agent decide to spawn sub-agents?**
   - What we know: The agent needs to decide when a task is too complex for a single response and should be decomposed. This decision happens at the LLM level (the model itself decides to use a "spawn" tool).
   - What's unclear: Tool use integration is not yet built. The current `CompletionRequest` supports `stop_sequences` and the `StopReason::ToolUse` exists, but there's no tool execution framework.
   - Recommendation: For Phase 5, implement the sub-agent spawning infrastructure and test it with manually-constructed spawn requests. The "agent decides to spawn" behavior can be triggered by a special XML tag in the response (e.g., `<spawn_agents>`) that the parent agent parses, OR by a simple tool-use callback. Start with XML tag parsing since that aligns with the existing XML tag boundary pattern in SystemPromptBuilder. Full tool-use integration comes later.

2. **WebSocket authentication**
   - What we know: The REST API uses an `Authenticated` extractor. Phase 4 research recommends skipping auth for localhost.
   - What's unclear: Should WebSocket connections require authentication?
   - Recommendation: Follow Phase 4's precedent -- no auth for localhost WebSocket connections. Add optional API key query param (`/ws/events?token=...`) for remote connections in a future phase.

3. **Event filtering per WebSocket client**
   - What we know: The broadcast channel sends ALL events to ALL subscribers.
   - What's unclear: Should clients be able to filter events (e.g., only events for bot X)?
   - Recommendation: Send all events to all clients. The frontend filters by `agent_id` / `bot_id`. The event volume is low (agent lifecycle events, not per-token streaming). If volume becomes a concern, add server-side filtering with a subscription message from the client.

4. **Shared workspace implementation**
   - What we know: AGNT-06 requires "opt-in shared workspace for agents that need shared state."
   - What's unclear: What form this workspace takes (KV store? temporary files? in-memory map?).
   - Recommendation: Use `Arc<DashMap<String, serde_json::Value>>` as an in-memory shared workspace scoped to a single request. It lives on the `RequestBudget` or a new `RequestContext` struct. Parallel sub-agents can read/write concurrently. It's dropped when the request completes. No persistence needed -- it's ephemeral per-request state.

5. **Integration with Phase 4 SSE streaming**
   - What we know: Phase 4 plans SSE for chat streaming. Phase 5 adds WebSocket for system events.
   - What's unclear: Should chat streaming also move to WebSocket?
   - Recommendation: Keep SSE for chat streaming (POST-based, per-request). Use WebSocket for system events (persistent connection, multi-event). They serve different purposes. The WebSocket events include `AgentTextDelta` for sub-agent streaming, which the frontend can use to show sub-agent progress alongside the main chat.

## Deep Dive: Agent Spawn Decision Mechanism

**Confidence:** HIGH (codebase analysis) / MEDIUM (LLM orchestration patterns from community)

### Decision: Use XML Tag Parsing, Not Tool Use

The existing codebase already uses XML tags extensively for prompt structure (`<soul>`, `<identity>`, `<session_memory>`, `<long_term_memory>`, `<instructions>`, `<task>`). The `SystemPromptBuilder` in `prompt.rs` constructs the system prompt entirely from XML-tagged sections. The LLM is already trained to respect these boundaries. Use the same pattern for spawn instructions.

**Why NOT tool use:** The current `CompletionRequest` has no `tools` field. `StopReason::ToolUse` exists in the types, and `StreamEvent::ToolUseComplete` exists, but there is no tool execution loop in `AgentEngine.execute()` or `loop_runner.rs`. Building a full tool-use framework (tool registration, input validation, execution loop, result injection) is a separate phase. XML tag parsing achieves the same goal with far less infrastructure.

### Spawn Instruction Format

Add a `<spawn_agents>` XML tag to the system prompt instructions that teaches the LLM how to request sub-agents. The LLM outputs this tag in its response; the spawner parses it before delivering the final text to the user.

```xml
<spawn_agents mode="parallel">
  <agent task="Research the history of quantum computing" />
  <agent task="Summarize recent breakthroughs in quantum error correction" />
  <agent task="List the top 5 quantum computing companies" />
</spawn_agents>
```

The `mode` attribute determines sequential vs parallel:
- `mode="parallel"` -- all agents run concurrently (default if no shared state between tasks)
- `mode="sequential"` -- agents run one after another, each receiving the previous result

### Response Parsing Strategy

The spawner scans the LLM response for `<spawn_agents>` blocks. The parsing is simple string-based XML extraction (not a full XML parser) -- the same approach used in production LLM agent systems like Claude Code, which uses XML-formatted tool calls.

```rust
// crates/boternity-core/src/agent/spawner.rs

/// Parsed spawn instruction from LLM response.
#[derive(Debug, Clone)]
pub struct SpawnInstruction {
    pub mode: SpawnMode,
    pub tasks: Vec<String>,
}

/// Extract spawn instructions from an LLM response.
///
/// Looks for `<spawn_agents mode="...">` blocks containing
/// `<agent task="..." />` entries. Returns None if no spawn
/// instruction is found (normal single-agent response).
pub fn parse_spawn_instructions(response: &str) -> Option<SpawnInstruction> {
    let start_tag = "<spawn_agents";
    let end_tag = "</spawn_agents>";

    let start_idx = response.find(start_tag)?;
    let end_idx = response.find(end_tag)?;
    let block = &response[start_idx..end_idx + end_tag.len()];

    // Extract mode attribute
    let mode = if block.contains(r#"mode="sequential"#) {
        SpawnMode::Sequential
    } else {
        SpawnMode::Parallel // default
    };

    // Extract task attributes from <agent task="..." /> elements
    let mut tasks = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = block[search_from..].find(r#"task=""#) {
        let abs_pos = search_from + pos + 6; // skip past 'task="'
        if let Some(end_quote) = block[abs_pos..].find('"') {
            tasks.push(block[abs_pos..abs_pos + end_quote].to_string());
            search_from = abs_pos + end_quote + 1;
        } else {
            break;
        }
    }

    if tasks.is_empty() {
        return None;
    }

    Some(SpawnInstruction { mode, tasks })
}
```

### How the Agent Determines Sequential vs Parallel

The LLM decides, not the code. The system prompt teaches the agent when to use each mode:

```rust
// Addition to the <instructions> section in SystemPromptBuilder
const SPAWN_INSTRUCTIONS: &str = r#"
<agent_capabilities>
When a user's request is complex and would benefit from decomposition into
independent sub-tasks, you may request sub-agents by including a
<spawn_agents> block in your response.

Use mode="parallel" when tasks are independent (no shared data between them):
<spawn_agents mode="parallel">
  <agent task="Specific, focused task description 1" />
  <agent task="Specific, focused task description 2" />
</spawn_agents>

Use mode="sequential" when tasks have dependencies (output of one feeds into the next):
<spawn_agents mode="sequential">
  <agent task="First: gather the raw data on X" />
  <agent task="Second: analyze the data from the previous step" />
  <agent task="Third: write a summary based on the analysis" />
</spawn_agents>

Guidelines:
- Only spawn when the task genuinely benefits from decomposition (3+ independent parts, or a clear pipeline)
- Each task description must be self-contained -- the sub-agent does not see your conversation
- Keep task descriptions specific and include success criteria
- You may include text before or after the <spawn_agents> block
- Do NOT spawn for simple questions or single-topic responses
</agent_capabilities>
"#;
```

### Orchestration Loop

The spawn decision integrates into the existing `AgentEngine.execute()` flow. The spawner wraps the engine and adds a post-processing step:

1. Root agent receives user message and calls LLM
2. LLM response is accumulated (streaming or non-streaming)
3. After response completes, spawner checks for `<spawn_agents>` block
4. If found: parse tasks, execute sub-agents (parallel or sequential), collect results
5. Feed results back to the root agent as a follow-up message: `"Sub-agent results:\n<results>...</results>"`
6. Root agent produces final response incorporating sub-agent outputs
7. Strip any `<spawn_agents>` blocks from the final response before showing to user

This is a **two-LLM-call pattern** for the root agent: first call decides to spawn, second call synthesizes. Sub-agents each make one LLM call (unless they themselves spawn, up to depth limit).

### Codebase Integration Points

Based on codebase analysis, the spawn mechanism touches these files:

| File | Change |
|------|--------|
| `agent/prompt.rs` (SystemPromptBuilder::build) | Add `<agent_capabilities>` section to system prompt when spawning is enabled |
| `agent/engine.rs` (AgentEngine::execute) | No change -- stays as single LLM call |
| `agent/spawner.rs` (NEW) | Wraps AgentEngine with spawn detection + orchestration |
| `cli/chat/loop_runner.rs` | Replace direct `AgentEngine` usage with `SubAgentSpawner` |
| `http/handlers/chat.rs` | Replace direct `FallbackChain` streaming with spawner-aware flow |

The key insight is that `AgentEngine` stays unchanged as the primitive for "one LLM call." The `SubAgentSpawner` composes multiple `AgentEngine` calls into a hierarchical execution.

## Deep Dive: Shared Workspace Design

**Confidence:** HIGH

### Decision: DashMap with Per-Request Scoping

Use `DashMap<String, serde_json::Value>` wrapped in `Arc` and scoped to a single request tree. DashMap is a sharded concurrent HashMap (one RwLock per shard) that provides lock-free-like concurrency for disjoint key access -- exactly what parallel sub-agents need when writing to different keys.

### Why DashMap, Not Alternatives

| Option | Verdict | Reason |
|--------|---------|--------|
| `Arc<RwLock<HashMap>>` | Rejected | Single global lock; all sub-agents contend on every read/write |
| `Arc<Mutex<HashMap>>` | Rejected | Even worse than RwLock -- exclusive lock for reads too |
| `DashMap` | **Selected** | Sharded internally, concurrent reads/writes on disjoint keys are lock-free. 173M+ downloads, battle-tested |
| `tokio::sync::RwLock<HashMap>` | Rejected | Tokio docs recommend std::sync::Mutex for short-hold locks; async RwLock overhead unnecessary for in-memory map |
| Channels (`mpsc`) | Rejected | Sub-agents need random-access reads, not just message passing |

### DashMap Version and Features

Use dashmap 6.1 with `serde` feature. Version 7.0 is in RC and not yet stable. Version 6.1 is the latest stable release with 173M+ total downloads.

```toml
dashmap = { version = "6.1", features = ["serde"] }
```

### Workspace Scoping: Per-Request-Tree

Each user request creates one `SharedWorkspace` instance. All sub-agents within that request share the same workspace. When the request completes, the workspace is dropped (no persistence).

```rust
// crates/boternity-core/src/agent/workspace.rs
use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;

/// Shared workspace for sub-agents within a single request tree.
///
/// Provides a concurrent key-value store where parallel sub-agents
/// can read and write without contention (DashMap uses sharded locks).
/// The workspace is ephemeral -- it lives for the duration of one
/// user request and is dropped when the request completes.
#[derive(Debug, Clone)]
pub struct SharedWorkspace {
    inner: Arc<DashMap<String, Value>>,
}

impl SharedWorkspace {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    /// Write a value to the workspace.
    pub fn set(&self, key: impl Into<String>, value: Value) {
        self.inner.insert(key.into(), value);
    }

    /// Read a value from the workspace. Returns None if key doesn't exist.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.inner.get(key).map(|r| r.value().clone())
    }

    /// Remove a value from the workspace. Returns the removed value.
    pub fn remove(&self, key: &str) -> Option<Value> {
        self.inner.remove(key).map(|(_, v)| v)
    }

    /// Check if a key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }

    /// List all keys currently in the workspace.
    /// Used by sub-agents to discover what's available.
    pub fn keys(&self) -> Vec<String> {
        self.inner.iter().map(|r| r.key().clone()).collect()
    }

    /// Get the number of entries in the workspace.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the workspace is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Snapshot the entire workspace as a JSON object.
    /// Useful for injecting workspace state into a sub-agent's context.
    pub fn snapshot(&self) -> Value {
        let mut map = serde_json::Map::new();
        for entry in self.inner.iter() {
            map.insert(entry.key().clone(), entry.value().clone());
        }
        Value::Object(map)
    }
}

impl Default for SharedWorkspace {
    fn default() -> Self {
        Self::new()
    }
}
```

### Data Format: serde_json::Value

Use `serde_json::Value` as the value type, not raw `String` or strongly typed generics. Reasons:

1. **Flexibility:** Sub-agents produce heterogeneous data (strings, numbers, arrays, objects). `Value` handles all of these without type parameters.
2. **No deserialization cost on write:** Sub-agents can write `json!({"key": "value"})` directly.
3. **Easy injection into prompts:** `Value.to_string()` produces valid JSON that can be included in the system prompt for the synthesizer call.
4. **Existing pattern:** The project already uses `serde_json::Value` extensively (e.g., `StreamEvent::ToolUseComplete { input: serde_json::Value }`).

### How Sub-Agents Discover Workspace Contents

Sequential sub-agents automatically receive the workspace snapshot in their system prompt. For parallel sub-agents (who write but may not need to read each other's output), the workspace is available but not injected into the prompt by default.

```rust
// When building the sub-agent system prompt for sequential tasks:
fn build_sequential_task_prompt(
    parent_prompt: &str,
    task: &str,
    workspace: &SharedWorkspace,
    previous_results: &[SubAgentResult],
) -> String {
    let mut prompt = format!(
        "{parent_prompt}\n\n<task>\n{task}\n</task>"
    );

    // Inject previous results for sequential agents
    if !previous_results.is_empty() {
        let results_text: Vec<String> = previous_results.iter()
            .map(|r| format!("- {}: {}", r.task, r.result))
            .collect();
        prompt.push_str(&format!(
            "\n\n<previous_results>\n{}\n</previous_results>",
            results_text.join("\n")
        ));
    }

    // Inject workspace snapshot if non-empty
    if !workspace.is_empty() {
        prompt.push_str(&format!(
            "\n\n<workspace>\n{}\n</workspace>",
            serde_json::to_string_pretty(&workspace.snapshot()).unwrap_or_default()
        ));
    }

    prompt
}
```

### Cleanup Strategy

The workspace requires no explicit cleanup. Since `SharedWorkspace` wraps `Arc<DashMap<...>>`, when the last `Arc` reference is dropped (the request tree completes), the DashMap is deallocated. This happens automatically when `RequestContext` goes out of scope. No background tasks, no timers, no GC.

### DashMap Gotcha: Never Hold Refs Across .await

DashMap's `Ref` and `RefMut` hold shard locks. Holding them across an `.await` point would block other sub-agents trying to access the same shard. The workspace API avoids this by always cloning values out:

```rust
// CORRECT: clone immediately, no lock held across await
let value = workspace.get("key"); // Returns Option<Value>, lock released
do_something_async(value).await;

// WRONG: would hold lock across await (DashMap API prevents this by
// design -- get() returns Ref which must be used immediately)
// The SharedWorkspace API avoids this by returning cloned Values.
```

### RequestContext: Combining Budget + Workspace + Cancellation

Introduce a `RequestContext` struct that groups the per-request shared state:

```rust
// crates/boternity-core/src/agent/request_context.rs
use tokio_util::sync::CancellationToken;

/// Per-request shared context for an agent tree.
///
/// Created once per user request and shared (via Arc/Clone) across
/// all agents in the tree. Contains the token budget, shared workspace,
/// cancellation token, and request metadata.
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Unique ID for this request.
    pub request_id: Uuid,
    /// Token budget shared across the agent tree.
    pub budget: RequestBudget,
    /// Shared workspace for inter-agent data passing.
    pub workspace: SharedWorkspace,
    /// Cancellation token for the entire request tree.
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
    /// Shares the same budget, workspace, and cancellation token.
    pub fn child(&self) -> Self {
        Self {
            request_id: self.request_id,
            budget: self.budget.clone(), // Arc clone -- same underlying AtomicU64
            workspace: self.workspace.clone(), // Arc clone -- same DashMap
            cancellation: self.cancellation.child_token(), // child cancellation
        }
    }
}
```

## Deep Dive: WebSocket + SSE Coexistence

**Confidence:** HIGH

### Decision: SSE for Chat Streaming, WebSocket for System Events

Keep the existing SSE endpoint (`POST /api/v1/bots/{id}/chat/stream`) for per-request chat streaming. Add a new WebSocket endpoint (`GET /ws/events`) for persistent system event delivery. They serve fundamentally different purposes and should not be merged.

### Why Two Protocols

| Aspect | SSE (Chat Streaming) | WebSocket (System Events) |
|--------|---------------------|--------------------------|
| Lifetime | Per-request (one POST, one stream) | Persistent (one connection for the session) |
| Direction | Server-to-client only | Bidirectional (enables client cancel requests) |
| Trigger | User sends message, server streams response | Server pushes events whenever they occur |
| Data | Text deltas, usage stats for ONE LLM call | Agent lifecycle events across ALL active requests |
| Volume | High (every token) | Low (spawn/complete/budget events) |
| Client model | React `fetch()` with ReadableStream | React `new WebSocket()` or reconnecting-websocket |

### How They Work Together in the Same Axum App

Both protocols share the same `AppState` via axum's `.with_state()`. The existing router already serves both API routes and health checks. Adding a WebSocket route is simply another `.route()` call.

```rust
// The router already handles mixed protocol endpoints:
pub fn build_router(state: AppState) -> Router {
    let api_routes = Router::new()
        // SSE endpoint (already exists)
        .route("/bots/{id}/chat/stream", post(handlers::chat::stream_chat))
        // ... other REST routes ...
        ;

    Router::new()
        .nest("/api/v1", api_routes)
        .route("/health", get(health_check))
        // WebSocket endpoint (new) -- lives OUTSIDE /api/v1 since it's not REST
        .route("/ws/events", get(handlers::ws::ws_events))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

Key architectural point: The WebSocket route lives OUTSIDE the `/api/v1` nest because it is not a REST endpoint. WebSocket and SSE routes are both `GET` handlers to axum -- the protocol upgrade happens inside the handler via the `WebSocketUpgrade` extractor.

### Event Routing: How Sub-Agent Streaming Relates to Main Chat SSE

When a root agent spawns sub-agents, the chat is effectively "expanded" into multiple parallel streams. Here is how events flow:

```
User sends message via POST /api/v1/bots/{id}/chat/stream
  |
  v
SSE stream opened (per-request)
  |
  v
Root agent calls LLM --> SSE: text_delta events (root agent's response)
  |
  v
Root agent response contains <spawn_agents> block
  |
  v
SubAgentSpawner kicks in:
  - EventBus <-- AgentSpawned { agent_id: A1, task: "..." }  --> WebSocket clients
  - EventBus <-- AgentSpawned { agent_id: A2, task: "..." }  --> WebSocket clients
  |
  v
Sub-agent A1 calls LLM:
  - EventBus <-- AgentExecuting { agent_id: A1 }              --> WebSocket clients
  - EventBus <-- AgentTextDelta { agent_id: A1, text: "..." } --> WebSocket clients
  - EventBus <-- AgentCompleted { agent_id: A1, ... }         --> WebSocket clients
  |
  v (parallel)
Sub-agent A2 calls LLM:
  - EventBus <-- AgentExecuting { agent_id: A2 }              --> WebSocket clients
  - EventBus <-- AgentTextDelta { agent_id: A2, text: "..." } --> WebSocket clients
  - EventBus <-- AgentCompleted { agent_id: A2, ... }         --> WebSocket clients
  |
  v
Results fed back to root agent for synthesis
  |
  v
Root agent produces final response --> SSE: text_delta events
  |
  v
SSE: done event
```

The SSE stream carries the root agent's final text. The WebSocket carries sub-agent lifecycle events. The frontend can render both: the chat message from SSE, and a "sub-agent activity" panel from WebSocket events.

### Frontend Integration Strategy

The React frontend connects to both:

1. **SSE (per chat message):** On each user message send, open a `fetch()` to the SSE endpoint. Process `text_delta` events into the chat bubble. Close when `done` event arrives.

2. **WebSocket (persistent):** On app mount, connect `new WebSocket('/ws/events')`. Maintain connection for the session. Reconnect on disconnect. Process incoming `SystemEvent` JSON messages to update:
   - Sub-agent activity indicators
   - Budget consumption bars
   - Error/warning notifications

```typescript
// Frontend pseudo-code for dual-protocol handling

// SSE: per-message
async function sendMessage(botId: string, message: string) {
  const response = await fetch(`/api/v1/bots/${botId}/chat/stream`, {
    method: 'POST',
    body: JSON.stringify({ message }),
  });
  const reader = response.body.getReader();
  // Process SSE text_delta events into chat UI
}

// WebSocket: persistent
const ws = new WebSocket(`ws://${host}/ws/events`);
ws.onmessage = (event) => {
  const systemEvent = JSON.parse(event.data);
  switch (systemEvent.type) {
    case 'agent_spawned':
      showSubAgentIndicator(systemEvent.agent_id, systemEvent.task);
      break;
    case 'agent_completed':
      markSubAgentDone(systemEvent.agent_id);
      break;
    case 'budget_warning':
      showBudgetWarning(systemEvent.consumed, systemEvent.max);
      break;
    case 'budget_exhausted':
      showBudgetExhaustedAlert(systemEvent.consumed, systemEvent.max);
      break;
  }
};
```

### Connection Lifecycle Management

**WebSocket:**
- Client connects on app mount
- Server spawns two tasks per connection (send + receive)
- `tokio::select!` races both tasks; when either ends, connection is cleaned up
- Handle `RecvError::Lagged` gracefully -- log and continue
- No heartbeat needed from server side (axum handles WebSocket ping/pong automatically)
- Client should implement reconnection with exponential backoff

**SSE:**
- Opened per chat message (POST request)
- `KeepAlive::new().interval(Duration::from_secs(15))` already configured in existing `stream_chat`
- Closes when `done` event is sent or client disconnects
- No session affinity needed (each SSE stream is independent)

### CORS Consideration

The existing CORS configuration uses `Any` for origins, methods, and headers. WebSocket upgrade requests go through the normal HTTP handshake, so CORS applies. The current permissive configuration is sufficient for development. For production, restrict to the specific frontend origin.

## Deep Dive: Budget Enforcement Edge Cases

**Confidence:** HIGH

### Decision: Soft Limits with Hard Ceiling and Cancellation

Use a three-tier enforcement model: (1) soft warning at 80%, (2) soft block on new spawns at 100%, (3) hard cancellation via `CancellationToken` for runaway streams.

### Soft vs Hard Limit Trade-offs

| Approach | Pros | Cons | Verdict |
|----------|------|------|---------|
| Strict atomic (CAS loop) | Exact enforcement | Contention under parallel agents; complex code; agents get spuriously rejected | Rejected |
| Soft limit (check-then-act) | Simple; no contention; bounded overshoot | Can exceed budget by up to N parallel calls | **Selected** |
| Hard limit (abort mid-stream) | Prevents any overshoot | Wastes tokens already consumed; disrupts user experience | Used only as emergency ceiling |

**The selected model works in layers:**

```
0%                    80%                   100%              120% (hard ceiling)
|------ normal -------|---- warning zone ---|-- soft block ----|-- CANCEL ---|
                      ^                     ^                  ^
                      BudgetWarning event   Stop spawning      CancellationToken::cancel()
                                            new sub-agents     abort in-flight LLM calls
```

### Parallel Overshoot Scenarios with Exact Bounds

When N sub-agents run in parallel, the maximum overshoot is bounded by N * max_output_tokens_per_call:

**Example:** Budget = 500K tokens. Three parallel sub-agents each estimate 50K tokens. Budget shows 400K consumed. All three check `can_spend(50K)` simultaneously and see "100K remaining >= 50K" -- all proceed. Each uses 50K tokens. Result: 550K consumed (50K over budget).

**Quantified overshoot formula:** `max_overshoot = (parallel_count - 1) * max_tokens_per_call`

For the boternity system with `max_tokens: 4096` (the `agent_config.max_tokens` field seen in the codebase), the maximum overshoot with 3 parallel agents is: `(3-1) * 4096 = 8192 tokens` -- negligible relative to a 500K budget.

The hard ceiling at 120% catches pathological cases. If consumed exceeds 120% of budget, the `CancellationToken` is cancelled, aborting all in-flight tasks.

### Cancellation Strategies: Can You Cancel In-Flight LLM Calls?

**Yes, with tokio cancellation semantics.** The key insight is that the LLM provider's `stream()` method returns a `Pin<Box<dyn Stream>>`. When the tokio task running the stream consumer is cancelled (via `JoinSet::abort_all()` or `CancellationToken`), the stream is dropped, which closes the underlying HTTP connection (reqwest). The LLM provider stops sending tokens. No tokens are billed for unsent output.

```rust
// In SubAgentSpawner::run_parallel -- budget-aware cancellation
pub async fn run_parallel(
    &self,
    requests: Vec<SubAgentRequest>,
    request_ctx: &RequestContext,
    provider: &BoxLlmProvider,
    parent_context: &AgentContext,
) -> Vec<SubAgentResult> {
    let mut join_set = JoinSet::new();

    for request in requests {
        let ctx = request_ctx.child();
        let bus = self.event_bus.clone();
        // ... clone provider, parent_context ...

        join_set.spawn(async move {
            tokio::select! {
                // Normal execution path
                result = execute_single_agent(request, provider, parent_context, &ctx) => {
                    result
                }
                // Cancellation path -- budget exhausted or parent cancelled
                _ = ctx.cancellation.cancelled() => {
                    SubAgentResult {
                        agent_id: request.id,
                        task: request.task.clone(),
                        result: String::new(),
                        tokens_used: 0,
                        success: false,
                        error: Some("Cancelled: budget exhausted".to_string()),
                    }
                }
            }
        });
    }

    // Monitor budget while collecting results
    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(sub_result) => {
                // Record token usage
                let new_total = request_ctx.budget.record(sub_result.tokens_used);

                // Check budget thresholds after each completion
                if new_total >= request_ctx.budget.max() * 120 / 100 {
                    // Hard ceiling: cancel remaining tasks
                    request_ctx.cancellation.cancel();
                    self.event_bus.publish(SystemEvent::BudgetExhausted { /* ... */ });
                    join_set.abort_all(); // Force-cancel remaining tasks
                } else if request_ctx.budget.is_warning() {
                    self.event_bus.publish(SystemEvent::BudgetWarning { /* ... */ });
                }

                results.push(sub_result);
            }
            Err(join_error) => {
                // Task panicked or was cancelled
                results.push(SubAgentResult {
                    agent_id: Uuid::now_v7(),
                    task: "unknown".to_string(),
                    result: String::new(),
                    tokens_used: 0,
                    success: false,
                    error: Some(format!("Task failed: {join_error}")),
                });
            }
        }
    }
    results
}
```

### Pre-Call Cost Estimation vs Post-Call Accounting

Use **both**, but for different purposes:

**Pre-call estimation (gating):** Before spawning a sub-agent, estimate whether there is enough budget remaining. The estimate uses the `agent_config.max_tokens` (output) + a rough input estimate (system prompt + task description length / 4). This is conservative -- it prevents spawning agents that would certainly exceed the budget.

```rust
impl RequestBudget {
    /// Estimate whether there is enough budget to run a sub-agent.
    ///
    /// Uses a conservative estimate: system_prompt_tokens + max_output_tokens.
    /// This prevents spawning agents that would certainly exceed budget,
    /// while allowing agents that might use less than their maximum.
    pub fn can_afford_agent(&self, estimated_input_tokens: u64, max_output_tokens: u64) -> bool {
        let estimated_total = estimated_input_tokens + max_output_tokens;
        self.can_spend(estimated_total)
    }
}
```

**Post-call accounting (tracking):** After each LLM call completes, the actual token usage from `StreamEvent::Usage` is recorded via `budget.record()`. This is the source of truth for budget consumption.

The asymmetry is intentional: pre-call estimates are pessimistic (use max_tokens), post-call accounting is exact (use actual usage). This means the budget may show "not enough" when there actually is, but never shows "enough" when there isn't (except for the parallel overshoot case, which is bounded).

### What Happens When Budget Is Exhausted Mid-Stream

When the budget is exhausted while a streaming LLM response is in progress:

1. **The stream is NOT interrupted mid-token.** The current LLM call completes (Anthropic bills for the full response regardless of client-side disconnection).
2. **The budget is updated with actual usage** via the `StreamEvent::Usage` event at stream end.
3. **No new sub-agents are spawned.** The `can_spend()` check prevents new spawns.
4. **A `BudgetExhausted` event is published** to the EventBus, notifying WebSocket clients.
5. **The root agent's synthesizer call still proceeds** -- it needs to present results even if some sub-agents were not spawned.

Interrupting an in-flight stream wastes tokens (the provider bills for output generated before disconnection) and produces an incomplete result. It is better to let the current call finish and prevent future calls.

**Exception:** If the hard ceiling (120%) is breached, indicating a runaway situation, the `CancellationToken` is cancelled and `JoinSet::abort_all()` is called. This drops the streams, closing HTTP connections. Some tokens may be wasted, but the runaway is stopped.

### Handling the "Pause with Alert" Requirement

When the budget reaches the warning threshold (80%), the system emits a `BudgetWarning` event but does NOT pause execution. The frontend receives this via WebSocket and displays a warning to the user.

When the budget reaches 100%:
- **No pause** for the currently executing agent (let it finish)
- **Block new spawns** -- `can_spend()` returns false
- **Emit `BudgetExhausted` event** -- frontend shows an alert
- **The synthesizer call proceeds** with whatever results were collected

A true "pause and ask user" flow would require bidirectional communication (WebSocket message from client: "continue" or "stop"). This is architecturally possible (the WebSocket receive task can listen for client commands and relay them via a channel to the spawner), but adds complexity. For Phase 5, implement the simpler "stop spawning + alert" behavior. A "pause and ask" flow can be added later using the WebSocket bidirectional channel.

```rust
// Future enhancement: WebSocket client commands for budget control
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientCommand {
    /// User authorizes additional budget after BudgetExhausted alert
    ExtendBudget { additional_tokens: u64 },
    /// User cancels the current request
    CancelRequest { request_id: Uuid },
}
```

### Summary of Budget Enforcement Layers

| Layer | Trigger | Action | Tokens Wasted |
|-------|---------|--------|---------------|
| Warning (80%) | `is_warning()` after `record()` | `BudgetWarning` event to frontend | 0 |
| Soft block (100%) | `can_spend()` returns false | Stop spawning new sub-agents | 0 |
| Hard cancel (120%) | Post-record check | `CancellationToken::cancel()` + `JoinSet::abort_all()` | Up to 1 call per parallel agent |

## Sources

### Primary (HIGH confidence)
- Project codebase: `crates/boternity-core/src/agent/` -- AgentEngine, AgentContext, SystemPromptBuilder patterns
- Project codebase: `crates/boternity-core/src/llm/` -- LlmProvider trait, BoxLlmProvider, TokenBudget, FallbackChain
- Project codebase: `crates/boternity-api/src/cli/chat/loop_runner.rs` -- Full chat loop pattern with streaming
- Project codebase: `crates/boternity-api/src/http/handlers/chat.rs` -- SSE streaming endpoint with per-request lifecycle
- Project codebase: `crates/boternity-api/src/state.rs` -- AppState structure, service wiring
- Project codebase: `crates/boternity-api/src/http/router.rs` -- Existing Axum router pattern (SSE + REST coexistence)
- Project codebase: `crates/boternity-types/src/llm.rs` -- StreamEvent, StopReason::ToolUse, CompletionRequest structure
- [tokio::sync::broadcast documentation](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html) -- Channel semantics, lagging, capacity
- [tokio::task::JoinSet documentation](https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html) -- Dynamic task management, abort_all()
- [axum::extract::ws documentation](https://docs.rs/axum/latest/axum/extract/ws/index.html) -- WebSocket upgrade, split, message types
- [std::sync::atomic::AtomicU64](https://doc.rust-lang.org/std/sync/atomic/struct.AtomicU64.html) -- Lock-free atomic operations
- [tokio Graceful Shutdown](https://tokio.rs/tokio/topics/shutdown) -- CancellationToken pattern with code examples
- [tokio_util::sync::CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html) -- child_token() for hierarchical cancellation
- [DashMap official docs](https://docs.rs/dashmap/6.1.0/dashmap/struct.DashMap.html) -- API, sharding, serde support, deadlock warnings

### Secondary (MEDIUM confidence)
- [Axum WebSocket broadcast pattern](https://medium.com/@mikecode/axum-websocket-468736a5e1c7) -- Real-world WebSocket fan-out with tokio broadcast
- [Claude Code Sub-Agent Best Practices](https://claudefa.st/blog/guide/agents/sub-agent-best-practices) -- Parallel vs sequential routing decisions, invocation quality
- [Tokio Task Cancellation Patterns](https://cybernetist.com/2024/04/19/rust-tokio-task-cancellation-patterns/) -- CancellationToken, JoinHandle abort, AbortOnDropHandle
- [DashMap vs HashMap discussion](https://users.rust-lang.org/t/dashmap-vs-hashmap/122953) -- Community analysis of DashMap vs RwLock<HashMap> tradeoffs
- [stream-cancel crate](https://github.com/jonhoo/stream-cancel) -- Tripwire/Valved patterns for stream cancellation
- [Sub-Agent Spawning pattern](https://agentic-patterns.com/patterns/sub-agent-spawning/) -- Architectural pattern for hierarchical agents
- [Managing LLM Agent Costs](https://apxml.com/courses/multi-agent-llm-systems-design-implementation/chapter-6-system-evaluation-debugging-tuning/managing-llm-agent-costs) -- Token budget and cost tracking strategies

### Tertiary (LOW confidence)
- [Swarms-rs multi-agent framework](https://medium.com/@kyeg/the-comprehensive-guide-to-swarms-rs-building-powerful-multi-agent-systems-in-rust-a3f3a5d974fe) -- Rust agent framework patterns (community)
- [Ractor actor framework](https://docs.rs/ractor) -- Alternative actor-based pattern (not recommended for this project due to existing architecture)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- Two new deps (dashmap, tokio-util) are stable, battle-tested crates. All other primitives are tokio built-ins or stdlib.
- Architecture: HIGH -- Patterns derived directly from existing codebase analysis (AgentEngine, AgentContext, FallbackChain, BoxLlmProvider). Extension points are clear.
- Sub-agent spawning: HIGH -- JoinSet for parallel, sequential loop for sequential. Well-established tokio patterns.
- Event bus: HIGH -- tokio::sync::broadcast is the specified technology (INFR-03). Usage patterns verified from official docs.
- WebSocket: HIGH -- axum 0.8 built-in support verified from official docs. The `ws` feature flag is the only change needed.
- Spawn decision mechanism: HIGH -- XML tag parsing aligns with existing SystemPromptBuilder pattern. Community practices confirm this approach.
- Shared workspace: HIGH -- DashMap is the de facto standard for concurrent HashMaps in Rust (173M+ downloads). API verified from official docs.
- WebSocket + SSE coexistence: HIGH -- Both are standard axum handler patterns sharing AppState. Verified from codebase (SSE already works) and axum docs.
- Budget enforcement: HIGH -- Three-tier model (warn/block/cancel) with bounded overshoot. CancellationToken + JoinSet patterns verified from tokio docs.
- Cycle detection: MEDIUM -- Hash-based signature detection is straightforward, but semantic equivalence of tasks is inherently fuzzy. Circuit breaker provides the hard safety net.
- Integration with Phase 4: MEDIUM -- Phase 4 is not yet built, so exact integration points for the React WebSocket client are speculative. The backend WebSocket endpoint pattern is solid.

**Research date:** 2026-02-13
**Valid until:** 2026-03-13 (30 days -- tokio and axum are stable, no major version changes expected)
