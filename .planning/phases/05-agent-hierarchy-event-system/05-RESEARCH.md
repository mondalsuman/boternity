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

### No New External Dependencies Needed

Everything required is already in the workspace or is a feature flag on existing dependencies. Specifically:
- `tokio::sync::broadcast` -- part of tokio 1.x (already in workspace)
- `tokio::task::JoinSet` -- part of tokio 1.x (already in workspace)
- `std::sync::atomic::AtomicU64` -- standard library
- `axum::extract::ws` -- axum 0.8 with `ws` feature (axum already in workspace)

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tokio::sync::broadcast | tokio-bus, bus crate | broadcast is built into tokio, no extra dep; bus crate adds lock-free but bounded |
| JoinSet for parallel agents | tokio::join! macro | join! requires fixed count at compile time; JoinSet handles dynamic spawning |
| AtomicU64 for budget counter | Arc<Mutex<u64>> | Mutex adds contention; AtomicU64 is lock-free for simple add/check pattern |
| axum WebSocket | SSE (already planned for Phase 4) | WebSocket is bidirectional; SSE is sufficient for read-only events but req says OBSV-06 specifies WebSocket |

**Installation:**
```toml
# In workspace Cargo.toml, update axum features:
axum = { version = "0.8", features = ["macros", "ws"] }

# No new dependencies needed. Everything else is already in workspace.
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

**Key insight:** This phase requires zero new external dependencies. Everything is built from tokio primitives (broadcast, JoinSet, AtomicU64), standard library types (HashSet, Arc), and existing workspace crates (axum with ws feature, serde_json, uuid). The complexity is in the orchestration logic, not in finding libraries.

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

## Sources

### Primary (HIGH confidence)
- Project codebase: `crates/boternity-core/src/agent/` -- AgentEngine, AgentContext, SystemPromptBuilder patterns
- Project codebase: `crates/boternity-core/src/llm/` -- LlmProvider trait, BoxLlmProvider, TokenBudget, FallbackChain
- Project codebase: `crates/boternity-api/src/cli/chat/loop_runner.rs` -- Full chat loop pattern with streaming
- Project codebase: `crates/boternity-api/src/state.rs` -- AppState structure, service wiring
- Project codebase: `crates/boternity-api/src/http/router.rs` -- Existing Axum router pattern
- [tokio::sync::broadcast documentation](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html) -- Channel semantics, lagging, capacity
- [tokio::task::JoinSet documentation](https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html) -- Dynamic task management
- [axum::extract::ws documentation](https://docs.rs/axum/latest/axum/extract/ws/index.html) -- WebSocket upgrade, split, message types
- [std::sync::atomic::AtomicU64](https://doc.rust-lang.org/std/sync/atomic/struct.AtomicU64.html) -- Lock-free atomic operations

### Secondary (MEDIUM confidence)
- [Axum WebSocket broadcast pattern](https://medium.com/@mikecode/axum-websocket-468736a5e1c7) -- Real-world WebSocket fan-out with tokio broadcast
- [Building real-time WebSockets with Axum](https://medium.com/rustaceans/beyond-rest-building-real-time-websockets-with-rust-and-axum-in-2025-91af7c45b5df) -- Connection management patterns
- [Sub-Agent Spawning pattern](https://agentic-patterns.com/patterns/sub-agent-spawning/) -- Architectural pattern for hierarchical agents
- [Managing LLM Agent Costs](https://apxml.com/courses/multi-agent-llm-systems-design-implementation/chapter-6-system-evaluation-debugging-tuning/managing-llm-agent-costs) -- Token budget and cost tracking strategies
- [LLM Cost Tracking](https://www.traceloop.com/blog/from-bills-to-budgets-how-to-track-llm-token-usage-and-cost-per-user) -- Per-request budget enforcement patterns

### Tertiary (LOW confidence)
- [Swarms-rs multi-agent framework](https://medium.com/@kyeg/the-comprehensive-guide-to-swarms-rs-building-powerful-multi-agent-systems-in-rust-a3f3a5d974fe) -- Rust agent framework patterns (community)
- [Ractor actor framework](https://docs.rs/ractor) -- Alternative actor-based pattern (not recommended for this project due to existing architecture)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- No new dependencies required. All primitives are tokio built-ins or stdlib.
- Architecture: HIGH -- Patterns derived directly from existing codebase analysis (AgentEngine, AgentContext, FallbackChain, BoxLlmProvider). Extension points are clear.
- Sub-agent spawning: HIGH -- JoinSet for parallel, sequential loop for sequential. Well-established tokio patterns.
- Event bus: HIGH -- tokio::sync::broadcast is the specified technology (INFR-03). Usage patterns verified from official docs.
- WebSocket: HIGH -- axum 0.8 built-in support verified from official docs. The `ws` feature flag is the only change needed.
- Cycle detection: MEDIUM -- Hash-based signature detection is straightforward, but semantic equivalence of tasks is inherently fuzzy. Circuit breaker provides the hard safety net.
- Token budget enforcement: HIGH -- AtomicU64 pattern is well-established. Soft-limit semantics appropriate for the use case.
- Integration with Phase 4: MEDIUM -- Phase 4 is not yet built, so exact integration points for the React WebSocket client are speculative. The backend WebSocket endpoint pattern is solid.

**Research date:** 2026-02-13
**Valid until:** 2026-03-13 (30 days -- tokio and axum are stable, no major version changes expected)
