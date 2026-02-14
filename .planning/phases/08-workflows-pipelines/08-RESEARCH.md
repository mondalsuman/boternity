# Phase 8: Workflows + Pipelines - Research

**Researched:** 2026-02-14
**Domain:** Workflow orchestration, DAG execution, cron scheduling, visual node-graph builder, bot-to-bot communication
**Confidence:** MEDIUM-HIGH (most stack components verified via official docs; some patterns derived from ecosystem best practices)

## Summary

Phase 8 introduces a full workflow engine that composes agents and skills into multi-step execution chains with three representation formats (YAML, visual React Flow canvas, programmatic SDK), three trigger mechanisms (manual, cron, events), durable execution via SQLite checkpointing, and bot-to-bot communication. This is the most architecturally complex phase in the project.

The standard approach combines: `serde_yaml_ng` (already in stack) for YAML workflow definitions, `tokio-cron-scheduler` with `english-to-cron` for in-process cron scheduling, `jexl-eval` for `when` clause expression evaluation (supports dot-notation property access on JSON contexts), `notify` (v8) for filesystem watching, `petgraph` (already in stack) for DAG validation and topological execution ordering, `hmac` + `sha2` (sha2 already in stack) for webhook signature verification, and `@xyflow/react` (React Flow v12) with `dagre` for the visual workflow builder.

**Primary recommendation:** Build the workflow engine as a trait-based system in `boternity-core` with a `WorkflowExecutor` that walks a DAG of steps using `petgraph::algo::toposort`, checkpoints each step result to SQLite in the same transaction as the step output, and resumes from the last completed step on crash recovery. The visual builder uses `@xyflow/react` with custom node types per step kind, bidirectional YAML sync via a shared intermediate `WorkflowDefinition` struct, and the existing WebSocket infrastructure for live execution visualization.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Full step primitive set: sequential, parallel, conditional (if/else branching), and loops (repeat until condition)
- Step types: Agent, Skill, Code (TypeScript + WASM), and HTTP request steps
- Context object model for data flow: each step receives a workflow context with all prior step outputs, reads what it needs
- Fail-fast by default; LLM-driven self-correction available as option
- Max 3 LLM retry attempts before giving up
- Sub-workflow support: a step can invoke another workflow by name (needs depth cap)
- Bot-scoped workflows in `~/.boternity/bots/{slug}/workflows/`, cross-bot workflows in `~/.boternity/workflows/`
- Fully durable execution: state checkpointed to SQLite, workflow resumes from last completed step after crash/restart
- Configurable concurrency controls: workflow can declare `concurrency: N`
- Code-first SDK: TypeScript and Rust SDK with builder pattern
- In-process cron scheduler (runs inside boternity server process)
- Event sources: webhooks, internal EventBus events, and filesystem watch
- Webhook auth: both HMAC shared secret and bearer token
- Failure notifications via EventBus + WebSocket
- Trigger payload filtering with `when` clause expressions
- Human-readable schedule strings alongside standard cron
- Approval gate step type
- Missed cron runs caught up on restart
- Step-level timeouts by default; optional per-workflow timeout
- Configurable file watch paths
- Dual bot-to-bot communication: direct messaging (1:1) and pub/sub channels (one-to-many)
- Caller chooses sync or async: `send_and_wait()` / `send()`
- Fleet-wide visibility: any bot can send to any other active bot
- Typed envelope with flexible body: JSON envelope + structured JSON or free-form text body
- Full audit trail: all inter-bot messages persisted in SQLite
- Dynamic pub/sub channels: auto-created on first publish
- Autonomous communication: bots can send messages during normal conversation
- Default LLM-driven message processing; optional message handler skill intercepts first
- Bot-to-bot conversations create separate chat sessions tagged as 'bot-to-bot'
- Delegation supported: "Bot A asked Bot B for help" transparency
- React Flow node graph canvas
- Side panel for step configuration
- Toggle view between visual canvas and YAML editor (bidirectional sync)
- Both single-step testing and full workflow dry-run
- Categorized sidebar palette + search-based quick add
- Live execution visualization on canvas via WebSocket
- Full undo/redo history
- Rich node preview
- Built-in workflow templates
- Collapsible node groups
- Minimap
- Typed edges with color coding

### Claude's Discretion
- Exact YAML schema structure and field naming
- Workflow YAML vs separate trigger config file placement
- Sub-workflow depth cap number
- Canvas layout algorithm and auto-arrangement
- Template content and categories
- Edge color scheme for data types
- Minimap positioning and sizing

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Standard Stack

### Core (Rust Backend)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde_yaml_ng` | 0.10 | YAML workflow definition parsing/serialization | Already in workspace; serde_yaml successor; handles workflow YAML roundtrip |
| `tokio-cron-scheduler` | 0.15.1 | In-process async cron job scheduling | Most popular Tokio-native scheduler; supports English schedules, persistent state, async jobs |
| `english-to-cron` | 0.1.7 | Human-readable schedule string conversion | Used by tokio-cron-scheduler; converts "every 5 minutes" to cron syntax |
| `croner` | 3.0.1 | Cron expression parsing and next-occurrence calculation | Transitive dep of tokio-cron-scheduler; timezone-aware, DST-safe, POSIX/Vixie-compatible |
| `jexl-eval` | 0.4.0 | `when` clause expression evaluation for trigger filtering | Supports dot-notation property access on JSON (e.g., `event.type == 'push'`); serde_json context integration |
| `notify` | 8.2.0 | Cross-platform filesystem watching for event triggers | De facto Rust file watcher; inotify/FSEvents/ReadDirectoryChanges backends |
| `notify-debouncer-mini` | 0.5 | Debounced file system events | Prevents event storms from rapid file changes |
| `petgraph` | 0.7 | DAG validation, topological sort, cycle detection for workflow steps | Already in workspace; O(V+E) toposort, cycle detection built-in |
| `hmac` | 0.12 | HMAC computation for webhook signature verification | RustCrypto ecosystem; pairs with sha2 already in workspace |
| `sha2` | 0.10 | SHA-256 hashing for HMAC-SHA256 webhook signatures | Already in workspace dependencies |

### Core (Web Frontend)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@xyflow/react` | 12.x | Node-graph canvas for visual workflow builder | Industry standard for React node-based UIs; custom nodes, handles, minimap, controls built-in |
| `@xyflow/react` MiniMap | (included) | Bird's-eye navigation for large workflows | Built-in component, no extra dependency |
| `@xyflow/react` Controls | (included) | Zoom/fit viewport controls | Built-in component |
| `@xyflow/react` Background | (included) | Grid background for canvas orientation | Built-in component |
| `dagre` | 0.8.x | Auto-layout algorithm for workflow node arrangement | Standard tree/DAG layout engine; well-documented React Flow integration |
| `@dagrejs/dagre` | latest | TypeScript-friendly dagre package | Preferred npm package name for dagre |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `sqlx` | 0.8 | SQLite workflow execution log persistence | Already in workspace; workflow_runs, workflow_steps, bot_messages tables |
| `tokio::sync::broadcast` | (tokio 1.x) | Event distribution for workflow lifecycle events | Already used by EventBus; extend for workflow events |
| `tokio::sync::mpsc` | (tokio 1.x) | Async channel bridge for notify file watcher | Bridge sync notify callbacks to async tokio world |
| `axum` | 0.8 | REST API endpoints for workflow CRUD, trigger, webhook receiver | Already in workspace |
| `uuid` | 1.20 | Workflow run IDs, step execution IDs | Already in workspace; UUIDv7 time-sortable |
| `chrono` | 0.4 | Timestamps for execution log, schedule calculations | Already in workspace |
| `monaco-editor` | (in web) | YAML editor panel in workflow builder toggle view | Already in web app package.json |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `jexl-eval` | `evalexpr` | evalexpr has more operators but NO dot-notation property access on objects -- critical for `event.type == 'push'` patterns |
| `tokio-cron-scheduler` | raw `croner` + manual tokio loop | More control but must hand-build job management, missed-run tracking, scheduler lifecycle |
| `dagre` | `elkjs` | ELK is more powerful (supports ports, layers) but much heavier; dagre sufficient for workflow DAGs |
| `notify` | `inotify` directly | Platform-specific; notify abstracts cross-platform differences |

**Installation (Rust):**
```toml
# Add to workspace Cargo.toml [workspace.dependencies]
tokio-cron-scheduler = { version = "0.15", features = ["english"] }
jexl-eval = "0.4"
notify = "8.2"
notify-debouncer-mini = "0.5"
hmac = "0.12"
# Already present: serde_yaml_ng, petgraph, sha2, sqlx, chrono, uuid, tokio
```

**Installation (Web):**
```bash
cd apps/web && pnpm add @xyflow/react @dagrejs/dagre @types/dagre
```

## Architecture Patterns

### Recommended Project Structure (Rust)

```
crates/boternity-types/src/
  workflow.rs              # WorkflowDefinition, Step, Trigger, StepKind, etc.
  message.rs              # BotMessage envelope, Channel, MessageKind

crates/boternity-core/src/
  workflow/
    mod.rs
    definition.rs          # YAML <-> WorkflowDefinition parsing/validation
    executor.rs            # WorkflowExecutor trait + DagExecutor impl
    context.rs             # WorkflowContext (step outputs, variables)
    step_runner.rs         # Individual step execution (agent, skill, code, http)
    retry.rs               # LLM self-correction retry logic
    scheduler.rs           # CronScheduler wrapping tokio-cron-scheduler
    trigger.rs             # TriggerManager (cron, webhook, event, file watch)
    expression.rs          # JEXL expression evaluator for `when` clauses
    checkpoint.rs          # Durable execution checkpoint/resume logic
    concurrency.rs         # Semaphore-based concurrency limiter
  message/
    mod.rs
    bus.rs                 # MessageBus (direct + pub/sub)
    envelope.rs            # BotMessage envelope creation/validation
    handler.rs             # Message processing (LLM default + skill intercept)
    router.rs              # Route messages to correct bot/channel

crates/boternity-infra/src/
  sqlite/
    workflow.rs            # SqliteWorkflowRepository (definitions, runs, steps)
    message.rs             # SqliteMessageRepository (bot-to-bot message audit)
  workflow/
    mod.rs
    file_trigger.rs        # notify-based file watcher trigger
    webhook_handler.rs     # Axum handler for incoming webhooks + HMAC verification
    http_step.rs           # reqwest-based HTTP request step executor

crates/boternity-api/src/
  cli/
    workflow.rs            # CLI: workflow create/trigger/list/status/logs
  http/handlers/
    workflow.rs            # REST: workflow CRUD, trigger, run status
    webhook.rs             # POST /api/v1/webhooks/{id} receiver
    message.rs             # REST: bot-to-bot message send, channel list, history

apps/web/src/
  routes/workflows/
    index.tsx              # Workflow list page
    $workflowId.tsx        # Workflow detail/runs page
  routes/workflows/builder/
    $workflowId.tsx        # Visual builder page
  components/workflow/
    WorkflowCanvas.tsx     # React Flow canvas wrapper
    nodes/                 # Custom node components per step type
      AgentNode.tsx
      SkillNode.tsx
      CodeNode.tsx
      HttpNode.tsx
      ConditionalNode.tsx
      LoopNode.tsx
      ApprovalNode.tsx
      SubWorkflowNode.tsx
    edges/
      TypedEdge.tsx        # Color-coded edge by data type
    panels/
      StepConfigPanel.tsx  # Side panel for step configuration
      NodePalette.tsx      # Categorized sidebar for adding steps
    YamlEditor.tsx         # Monaco YAML editor with bidirectional sync
    ExecutionOverlay.tsx   # Live execution visualization layer
    WorkflowTemplates.tsx  # Template browser dialog
```

### Pattern 1: Canonical WorkflowDefinition as Intermediate Representation

**What:** All three workflow representations (YAML, visual, SDK) convert to/from a single canonical `WorkflowDefinition` struct. No direct YAML-to-visual or visual-to-SDK conversion.

**When to use:** Always. This is the core architectural pattern for WKFL-04 (interchangeable representations).

**Example:**
```rust
// boternity-types/src/workflow.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    #[serde(default)]
    pub concurrency: Option<u32>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    pub triggers: Vec<TriggerConfig>,
    pub steps: Vec<StepDefinition>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDefinition {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub step_type: StepType,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub condition: Option<String>,  // JEXL expression
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub retry: Option<RetryConfig>,
    pub config: StepConfig,
    // Visual builder metadata (position, group)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<StepUiMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Agent,
    Skill,
    Code,
    Http,
    Conditional,
    Loop,
    Approval,
    SubWorkflow,
}
```

### Pattern 2: DAG-Based Execution with Topological Ordering

**What:** Steps declare dependencies via `depends_on`. The executor builds a `petgraph::DiGraph`, validates it's a DAG (no cycles), topologically sorts it, then executes in waves -- all steps at the same topological level run in parallel.

**When to use:** For all workflow execution. Even sequential workflows are DAGs with linear dependency chains.

**Example:**
```rust
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;

fn build_execution_plan(steps: &[StepDefinition]) -> Result<Vec<Vec<&StepDefinition>>, WorkflowError> {
    let mut graph = DiGraph::<&str, ()>::new();
    let mut node_map = HashMap::new();

    // Add nodes
    for step in steps {
        let idx = graph.add_node(step.id.as_str());
        node_map.insert(step.id.as_str(), idx);
    }

    // Add edges (dependency -> step)
    for step in steps {
        let step_idx = node_map[step.id.as_str()];
        for dep in &step.depends_on {
            let dep_idx = node_map.get(dep.as_str())
                .ok_or(WorkflowError::UnknownDependency(dep.clone()))?;
            graph.add_edge(*dep_idx, step_idx, ());
        }
    }

    // Validate DAG (detect cycles)
    let sorted = toposort(&graph, None)
        .map_err(|cycle| WorkflowError::CycleDetected(format!("{:?}", cycle)))?;

    // Group into parallel waves by topological level
    // Steps with no remaining unfinished dependencies go in the same wave
    let mut waves: Vec<Vec<&StepDefinition>> = Vec::new();
    // ... group by dependency depth ...
    Ok(waves)
}
```

### Pattern 3: Durable Execution via SQLite Checkpoint Log

**What:** Each step's execution is logged to an `execution_log` table BEFORE execution starts (status=PENDING), then updated to COMPLETE with the result in the SAME transaction as any side-effect persistence. On crash recovery, the executor queries for the last COMPLETE step and resumes from the next PENDING step.

**When to use:** For all workflow runs. This is the durable execution core.

**Example SQL schema:**
```sql
CREATE TABLE workflow_runs (
    id TEXT PRIMARY KEY,          -- UUIDv7
    workflow_id TEXT NOT NULL,
    workflow_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','running','completed','failed','cancelled','paused')),
    trigger_type TEXT NOT NULL,   -- 'manual','cron','webhook','event','file_watch'
    trigger_payload TEXT,         -- JSON of trigger context
    context TEXT NOT NULL DEFAULT '{}',  -- JSON workflow context
    started_at TEXT NOT NULL,
    completed_at TEXT,
    error TEXT,
    concurrency_key TEXT,         -- For concurrency limiting
    FOREIGN KEY (workflow_id) REFERENCES workflows(id)
);

CREATE TABLE workflow_steps (
    id TEXT PRIMARY KEY,          -- UUIDv7
    run_id TEXT NOT NULL,
    step_id TEXT NOT NULL,        -- Matches StepDefinition.id
    step_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','running','completed','failed','skipped','waiting_approval')),
    attempt INTEGER NOT NULL DEFAULT 1,
    input TEXT,                   -- JSON serialized input
    output TEXT,                  -- JSON serialized output
    error TEXT,
    started_at TEXT,
    completed_at TEXT,
    FOREIGN KEY (run_id) REFERENCES workflow_runs(id)
);
```

**Resume logic:**
```rust
async fn resume_workflow(run_id: &str, db: &DatabasePool) -> Result<(), WorkflowError> {
    // Find all completed steps
    let completed: HashSet<String> = sqlx::query_scalar(
        "SELECT step_id FROM workflow_steps WHERE run_id = ? AND status = 'completed'"
    )
    .bind(run_id)
    .fetch_all(&db.reader)
    .await?
    .into_iter()
    .collect();

    // Load workflow context from last checkpoint
    let context: WorkflowContext = load_context(run_id, db).await?;

    // Re-build execution plan, skip completed steps
    let plan = build_execution_plan(&definition.steps)?;
    for wave in plan {
        let remaining: Vec<_> = wave.iter()
            .filter(|s| !completed.contains(&s.id))
            .collect();
        execute_wave(remaining, &context, db).await?;
    }
    Ok(())
}
```

### Pattern 4: Bot-to-Bot Message Bus with Typed Envelopes

**What:** A `MessageBus` provides both direct (1:1) and pub/sub (channel) messaging. Messages use a typed envelope (`BotMessage`) with JSON-serializable body. The bus is in-memory (`tokio::sync::broadcast` per channel + `DashMap<BotId, mpsc::Sender>` for direct), with SQLite persistence for audit trail.

**When to use:** For all inter-bot communication within and outside workflows.

**Example:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotMessage {
    pub id: Uuid,
    pub sender_bot_id: Uuid,
    pub sender_bot_name: String,
    pub recipient: MessageRecipient,
    pub message_type: String,      // User-defined type tag
    pub body: serde_json::Value,   // Flexible JSON body
    pub timestamp: DateTime<Utc>,
    pub reply_to: Option<Uuid>,    // For conversation threading
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRecipient {
    Direct { bot_id: Uuid },
    Channel { name: String },
}
```

### Pattern 5: React Flow Canonical Node-to-Step Mapping

**What:** Each React Flow node maps 1:1 to a `StepDefinition`. Edges map to `depends_on` entries. The visual builder maintains nodes/edges state that syncs bidirectionally with the `WorkflowDefinition` struct (serialized as YAML for the toggle editor view).

**When to use:** For the visual workflow builder.

```typescript
// Convert React Flow nodes/edges to WorkflowDefinition
function flowToDefinition(nodes: Node[], edges: Edge[]): WorkflowDefinition {
  const steps: StepDefinition[] = nodes
    .filter(n => n.type !== 'group')
    .map(node => ({
      id: node.id,
      name: node.data.label,
      type: node.data.stepType,
      depends_on: edges
        .filter(e => e.target === node.id)
        .map(e => e.source),
      config: node.data.config,
      ui: { position: node.position, group: node.parentId },
    }));
  return { name: '...', steps, triggers: [...] };
}

// Convert WorkflowDefinition to React Flow nodes/edges
function definitionToFlow(def: WorkflowDefinition): { nodes: Node[], edges: Edge[] } {
  const nodes = def.steps.map(step => ({
    id: step.id,
    type: stepTypeToNodeType(step.step_type),
    position: step.ui?.position ?? { x: 0, y: 0 },
    parentId: step.ui?.group,
    data: { label: step.name, stepType: step.step_type, config: step.config },
  }));
  const edges = def.steps.flatMap(step =>
    step.depends_on.map(dep => ({
      id: `${dep}-${step.id}`,
      source: dep,
      target: step.id,
      type: 'typed',
      data: { dataType: inferDataType(step) },
    }))
  );
  return { nodes, edges };
}
```

### Anti-Patterns to Avoid

- **Direct YAML-to-visual conversion:** Always go through the canonical `WorkflowDefinition` struct. Never parse YAML directly into React Flow nodes.
- **Step execution without checkpoint:** Every step MUST write to `workflow_steps` before AND after execution. No "fire and forget" step execution.
- **Shared mutable workflow context:** The `WorkflowContext` must be passed immutably through the DAG. Each step receives a snapshot; its output is merged into the context for downstream steps only.
- **Blocking the tokio runtime with notify:** The `notify` crate's `recommended_watcher` uses synchronous callbacks. Bridge to async via `tokio::sync::mpsc::channel` and `tokio::task::spawn_blocking`.
- **Polling for cron in a tight loop:** Use `tokio-cron-scheduler`'s built-in tick mechanism (500ms poll interval) rather than hand-rolling a polling loop.
- **Storing workflow state only in memory:** All workflow run state MUST be in SQLite for durability. In-memory structures are caches, not sources of truth.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cron expression parsing | Custom cron parser | `croner` 3.0 (via `tokio-cron-scheduler`) | POSIX compliance, timezone handling, DST transitions, extended specifiers (L, #, W) |
| Job scheduling loop | Custom tokio::time::interval scheduler | `tokio-cron-scheduler` 0.15 | Handles job lifecycle, missed runs (with custom logic), graceful shutdown, English parsing |
| Human-readable schedule parsing | Regex-based string parsing | `english-to-cron` 0.1.7 | Handles "every 5 minutes", "daily at 9am", "midnight on Tuesdays", etc. |
| Expression evaluation | Custom predicate parser for `when` clauses | `jexl-eval` 0.4 | Dot-notation property access on JSON objects, comparison operators, string ops, well-tested |
| DAG cycle detection | Manual graph traversal | `petgraph::algo::toposort` returns Err on cycles | O(V+E), battle-tested, already in workspace |
| DAG topological ordering | Custom BFS/DFS | `petgraph::algo::toposort` | Returns deterministic ordering, handles disconnected components |
| File system watching | Custom polling / inotify | `notify` 8.2 + `notify-debouncer-mini` | Cross-platform, event debouncing, recursive watch |
| HMAC-SHA256 verification | Manual byte manipulation | `hmac` 0.12 + `sha2` 0.10 | Constant-time comparison, RustCrypto audit, sha2 already in workspace |
| Node-graph UI canvas | Custom SVG/Canvas rendering | `@xyflow/react` v12 | Panning, zooming, selection, drag-and-drop, minimap, handles, custom nodes/edges |
| DAG auto-layout | Custom force-directed layout | `dagre` via `@dagrejs/dagre` | Directed graph layout with configurable rank direction, node separation |
| YAML roundtrip | Custom YAML formatter | `serde_yaml_ng` 0.10 with `#[serde(skip_serializing_if)]` | Preserves structure through serialize/deserialize cycle |

**Key insight:** This phase has more "don't hand-roll" items than any previous phase. The workflow engine's value is in composing existing primitives (cron, expression eval, DAG traversal, file watch, HMAC) into a coherent orchestration layer -- not in reimplementing those primitives.

## Common Pitfalls

### Pitfall 1: Non-Atomic Checkpoint Writes
**What goes wrong:** Step executes successfully, but crash occurs before checkpoint is written to SQLite. On resume, step re-executes (potentially with side effects like sending duplicate HTTP requests or double-invoking agents).
**Why it happens:** Separating step execution from checkpoint persistence into different transactions.
**How to avoid:** Write the step's PENDING status before execution. Write COMPLETE status + output in the same transaction as any persistent side effect. For non-idempotent steps (HTTP, agent calls), implement idempotency keys stored in the step log.
**Warning signs:** Duplicate agent invocations after restart; HTTP webhooks sent twice.

### Pitfall 2: tokio-cron-scheduler Does NOT Auto-Catch-Up Missed Runs
**What goes wrong:** Server restarts after being down for 2 hours. Cron jobs scheduled during downtime are silently skipped.
**Why it happens:** `tokio-cron-scheduler` schedules based on "next occurrence from now" -- it has no built-in awareness of missed runs.
**How to avoid:** On startup, query SQLite for all active cron-triggered workflows. For each, compare `last_run_at` with what `croner` says the next occurrence should have been. If `last_run_at + expected_interval < now`, trigger a catch-up run. This is custom logic wrapping the scheduler.
**Warning signs:** Scheduled workflows silently missing runs after restarts.

### Pitfall 3: Deadlock in Parallel Step Execution with Concurrency Limits
**What goes wrong:** Workflow has 10 parallel steps but concurrency limit is 3. Steps acquire semaphore permits and then wait on each other's outputs, causing deadlock.
**Why it happens:** Concurrency limits applied at the wrong granularity (per-step instead of per-wave/per-workflow-instance).
**How to avoid:** Apply concurrency limits at the workflow INSTANCE level (max N concurrent runs of the same workflow), not at the step level within a single run. Within a run, parallel waves execute freely.
**Warning signs:** Workflow runs hang indefinitely; semaphore permits never released.

### Pitfall 4: notify Crate Blocking Tokio Runtime
**What goes wrong:** File watcher callback runs on the tokio runtime thread, blocking async operations.
**Why it happens:** `notify::recommended_watcher` uses sync callbacks on a dedicated OS thread, but if the callback sends to a `std::sync::mpsc` channel that's polled from the tokio runtime, backpressure can stall.
**How to avoid:** Use `tokio::sync::mpsc::channel` for the bridge. Create the watcher callback as: `move |res| { let _ = tx.blocking_send(res); }`. The `blocking_send` will not block the notify thread. Consume with `rx.recv().await` in a tokio task.
**Warning signs:** SSE/WebSocket connections stall when many file changes occur simultaneously.

### Pitfall 5: React Flow State Sync Loop Between Canvas and YAML Editor
**What goes wrong:** Editing YAML triggers flow update, which triggers YAML regeneration, which triggers flow update... infinite loop.
**Why it happens:** Bidirectional sync without a "source of truth" flag.
**How to avoid:** Use a `activeEditor: 'canvas' | 'yaml'` state flag. Only sync FROM the active editor TO the inactive one. When user switches editors, convert once from the previously active format. Never have both editors live-syncing simultaneously.
**Warning signs:** Browser tab becomes unresponsive; React re-render count explodes.

### Pitfall 6: Sub-Workflow Infinite Recursion
**What goes wrong:** Workflow A invokes workflow B as a sub-workflow, which invokes workflow A.
**Why it happens:** No depth tracking across sub-workflow boundaries.
**How to avoid:** Pass a `depth` counter through the execution context. Increment on each sub-workflow invocation. Enforce a hard cap (recommend 5). This mirrors the existing `max_depth` pattern in `AgentOrchestrator`.
**Warning signs:** Stack overflow or OOM from infinite recursion.

### Pitfall 7: JEXL Expression Injection in `when` Clauses
**What goes wrong:** User-provided webhook payload data is evaluated as part of a JEXL expression, allowing unintended code execution.
**Why it happens:** Concatenating user data into expression strings rather than passing it as context.
**How to avoid:** ALWAYS pass trigger payloads as the JEXL context object, never interpolate them into the expression string. The expression is `event.type == 'push'` and `event` is the JSON payload passed as context to `eval_in_context()`.
**Warning signs:** Unexpected expression evaluation results with crafted webhook payloads.

### Pitfall 8: React Flow v12 Package Name Change
**What goes wrong:** Installing `reactflow` (old package name) instead of `@xyflow/react` (current v12 package name). Imports and APIs differ.
**Why it happens:** Outdated blog posts and Stack Overflow answers reference the old package name.
**How to avoid:** Always use `@xyflow/react` for v12. Import from `@xyflow/react`, not `reactflow`. CSS import is `@xyflow/react/dist/style.css`.
**Warning signs:** Module not found errors; API incompatibilities.

### Pitfall 9: Losing Workflow Context on Large Payloads
**What goes wrong:** Workflow context grows unboundedly as each step appends its output. After many steps, the context JSON becomes too large for SQLite TEXT columns or causes OOM.
**Why it happens:** No size limits on step output stored in context.
**How to avoid:** Implement a max output size per step (e.g., 1MB). For large outputs, store in file storage and put a reference URI in the context. Truncate LLM agent outputs to the relevant portion.
**Warning signs:** SQLite write performance degrades; workflow runs slow down after many steps.

## Code Examples

### YAML Workflow Definition (Recommended Schema)

```yaml
# ~/.boternity/bots/researcher/workflows/daily-digest.yaml
name: daily-digest
description: Gather news, analyze trends, generate summary
version: "1.0"
concurrency: 1

triggers:
  - type: cron
    schedule: "0 9 * * *"          # Standard cron
    # OR: schedule: "daily at 9am"  # Human-readable
  - type: webhook
    path: /trigger/daily-digest
    auth:
      type: hmac_sha256
      secret_name: DIGEST_WEBHOOK_SECRET
    when: "event.source == 'github'"

steps:
  - id: gather-news
    name: Gather News
    type: agent
    config:
      bot: researcher
      prompt: "Find the top 5 AI news stories from today"
    timeout_secs: 120

  - id: gather-papers
    name: Gather Papers
    type: agent
    config:
      bot: researcher
      prompt: "Find relevant new arxiv papers on LLMs"
    timeout_secs: 120

  - id: analyze
    name: Analyze Trends
    type: agent
    depends_on: [gather-news, gather-papers]
    config:
      bot: analyst
      prompt: "Analyze the news and papers for emerging trends"
    retry:
      max_attempts: 3
      strategy: llm_self_correct

  - id: format
    name: Format Digest
    type: skill
    depends_on: [analyze]
    config:
      skill: markdown-formatter
      input: "{{ steps.analyze.output }}"

  - id: notify
    name: Send Notification
    type: http
    depends_on: [format]
    config:
      method: POST
      url: "https://hooks.slack.com/..."
      headers:
        Content-Type: application/json
      body: |
        {"text": "{{ steps.format.output }}"}
```

### tokio-cron-scheduler Setup with English + Missed Run Catch-Up

```rust
use tokio_cron_scheduler::{Job, JobScheduler};

async fn start_cron_scheduler(
    workflows: Vec<WorkflowDefinition>,
    db: DatabasePool,
    event_bus: EventBus,
) -> anyhow::Result<JobScheduler> {
    let scheduler = JobScheduler::new().await?;

    for workflow in &workflows {
        for trigger in &workflow.triggers {
            if let TriggerConfig::Cron { schedule, .. } = trigger {
                let workflow_id = workflow.name.clone();
                let db = db.clone();
                let bus = event_bus.clone();

                let job = Job::new_async(schedule.as_str(), move |_uuid, _lock| {
                    let wf_id = workflow_id.clone();
                    let db = db.clone();
                    let bus = bus.clone();
                    Box::pin(async move {
                        if let Err(e) = trigger_workflow_run(&wf_id, "cron", &db, &bus).await {
                            tracing::error!(workflow = %wf_id, error = %e, "Cron trigger failed");
                            // Publish failure to event bus
                        }
                    })
                })?;
                scheduler.add(job).await?;
            }
        }
    }

    scheduler.start().await?;
    Ok(scheduler)
}
```

### JEXL Expression Evaluation for Trigger Filtering

```rust
use jexl_eval::Evaluator;
use serde_json::json;

fn evaluate_when_clause(
    expression: &str,
    trigger_payload: &serde_json::Value,
) -> Result<bool, WorkflowError> {
    let evaluator = Evaluator::new();
    // Wrap payload in an "event" context object
    let context = json!({ "event": trigger_payload });
    let result = evaluator
        .eval_in_context(expression, context)
        .map_err(|e| WorkflowError::ExpressionError(e.to_string()))?;

    match result {
        serde_json::Value::Bool(b) => Ok(b),
        _ => Err(WorkflowError::ExpressionError(
            format!("when clause must evaluate to boolean, got: {result}")
        )),
    }
}

// Usage:
// evaluate_when_clause("event.type == 'push' && event.branch == 'main'", &payload)
```

### HMAC-SHA256 Webhook Verification

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn verify_webhook_signature(
    payload: &[u8],
    signature_header: &str,
    secret: &[u8],
) -> Result<(), WebhookError> {
    // Strip "sha256=" prefix if present (GitHub convention)
    let sig_hex = signature_header
        .strip_prefix("sha256=")
        .unwrap_or(signature_header);

    let sig_bytes = hex::decode(sig_hex)
        .map_err(|_| WebhookError::InvalidSignature)?;

    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|_| WebhookError::InvalidSecret)?;
    mac.update(payload);

    mac.verify_slice(&sig_bytes)
        .map_err(|_| WebhookError::SignatureMismatch)
}
```

### File Watcher to Tokio Bridge

```rust
use notify::{recommended_watcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

async fn start_file_watcher(
    paths: Vec<PathBuf>,
) -> anyhow::Result<mpsc::Receiver<notify::Event>> {
    let (tx, rx) = mpsc::channel(256);

    let mut watcher = recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            // blocking_send won't block the notify thread
            let _ = tx.blocking_send(event);
        }
    })?;

    for path in &paths {
        watcher.watch(path, RecursiveMode::Recursive)?;
    }

    // Keep watcher alive by moving it into a tokio task
    tokio::task::spawn(async move {
        let _watcher = watcher; // prevent drop
        // Hold forever (or until cancellation)
        std::future::pending::<()>().await;
    });

    Ok(rx)
}
```

### React Flow Custom Node with Typed Handles

```typescript
import { Handle, Position, type NodeProps } from '@xyflow/react';

interface AgentNodeData {
  label: string;
  botName: string;
  prompt: string;
  status?: 'idle' | 'running' | 'completed' | 'failed';
}

export function AgentNode({ data, selected }: NodeProps<AgentNodeData>) {
  const statusColor = {
    idle: 'bg-muted',
    running: 'bg-yellow-500/20 animate-pulse',
    completed: 'bg-green-500/20',
    failed: 'bg-red-500/20',
  }[data.status ?? 'idle'];

  return (
    <div className={cn(
      'rounded-lg border-2 p-3 min-w-[200px]',
      selected ? 'border-primary' : 'border-border',
      statusColor,
    )}>
      <Handle type="target" position={Position.Top} />
      <div className="flex items-center gap-2">
        <Bot className="h-4 w-4" />
        <span className="font-medium text-sm">{data.label}</span>
      </div>
      <div className="text-xs text-muted-foreground mt-1">
        Bot: {data.botName}
      </div>
      <div className="text-xs text-muted-foreground truncate">
        {data.prompt}
      </div>
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}
```

### Undo/Redo with Snapshot History

```typescript
import { useCallback, useRef } from 'react';
import type { Node, Edge } from '@xyflow/react';

interface HistoryEntry {
  nodes: Node[];
  edges: Edge[];
}

export function useUndoRedo(maxHistory = 50) {
  const past = useRef<HistoryEntry[]>([]);
  const future = useRef<HistoryEntry[]>([]);

  const takeSnapshot = useCallback((nodes: Node[], edges: Edge[]) => {
    past.current.push({ nodes: structuredClone(nodes), edges: structuredClone(edges) });
    if (past.current.length > maxHistory) past.current.shift();
    future.current = []; // Clear redo stack on new action
  }, [maxHistory]);

  const undo = useCallback((currentNodes: Node[], currentEdges: Edge[]) => {
    const prev = past.current.pop();
    if (!prev) return null;
    future.current.push({ nodes: structuredClone(currentNodes), edges: structuredClone(currentEdges) });
    return prev;
  }, []);

  const redo = useCallback((currentNodes: Node[], currentEdges: Edge[]) => {
    const next = future.current.pop();
    if (!next) return null;
    past.current.push({ nodes: structuredClone(currentNodes), edges: structuredClone(currentEdges) });
    return next;
  }, []);

  return { takeSnapshot, undo, redo, canUndo: () => past.current.length > 0, canRedo: () => future.current.length > 0 };
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `reactflow` npm package | `@xyflow/react` (v12) | 2024 | New package name, different imports, server-side rendering support |
| `serde_yaml` (dtolnay, unmaintained) | `serde_yaml_ng` 0.10 | 2024 | Active maintenance fork; already in workspace |
| Custom cron loop with `tokio::time::interval` | `tokio-cron-scheduler` 0.15 with `english` feature | 2025 | Built-in English schedule parsing, job lifecycle management |
| `evalexpr` for all expression evaluation | `jexl-eval` 0.4 for JSON-context expressions | 2025 | JEXL supports dot-notation on JSON objects, which evalexpr lacks |
| Polling-based file watching | `notify` 8.2 event-driven with debouncer | 2024 | Event-driven rather than polling; per-platform native APIs |

**Deprecated/outdated:**
- `reactflow` npm package: Renamed to `@xyflow/react`. The old package still works but receives no updates.
- `serde_yaml` by dtolnay: Explicitly unmaintained. Use `serde_yaml_ng` (already in workspace).
- `cron` crate (zslayton): Less actively maintained than `croner`. `tokio-cron-scheduler` uses `croner` internally.

## Open Questions

1. **tokio-cron-scheduler persistence for missed-run catch-up**
   - What we know: The crate supports PostgreSQL/NATS storage backends for persisting job metadata, but its default in-memory store loses all state on restart.
   - What's unclear: Whether the persistent backends automatically detect and catch up missed runs, or if this requires custom logic regardless.
   - Recommendation: Implement custom missed-run detection in our `CronScheduler` wrapper. On startup, compare `last_run_at` from SQLite with expected next occurrence from `croner`. This is a few dozen lines of code and gives full control.

2. **jexl-eval completeness for complex expressions**
   - What we know: Version 0.4.0 has only 13% documentation coverage. It handles dot notation, basic comparisons, and string operations.
   - What's unclear: Whether it supports array indexing (`event.tags[0]`), conditional ternary (`condition ? a : b`), and regex matching.
   - Recommendation: Write a comprehensive test suite for the expression patterns needed by `when` clauses early in implementation. If jexl-eval falls short on specific operators, consider adding custom JEXL transforms via its transform API.

3. **React Flow Pro features vs. open-source**
   - What we know: The undo/redo example and some grouping features are marked as "Pro examples" requiring a paid subscription for source code.
   - What's unclear: Whether the underlying APIs to implement these features are available in the open-source library, just without example code.
   - Recommendation: The snapshot-based undo/redo pattern documented above uses only open-source React Flow APIs. For node grouping, React Flow's open-source `parentId` + `type: 'group'` API is sufficient. No Pro subscription needed.

4. **TypeScript SDK distribution**
   - What we know: The locked decision specifies a TypeScript SDK with builder pattern for programmatic workflow definition.
   - What's unclear: How the TypeScript SDK is packaged and consumed -- as an npm package, bundled with the web app, or a standalone CLI tool.
   - Recommendation: Start with a `@boternity/workflow-sdk` package in the monorepo that generates YAML. Distribution can be refined later.

## Deep Dive 1: jexl-eval Expression Completeness

Key findings from empirical testing (cargo run with actual test project):

**Pattern Verification (10/10 patterns tested):**
| # | Pattern | Status | Notes |
|---|---------|--------|-------|
| 1 | Dot-notation (`event.payload.user.name`) | PASS | Works natively |
| 2 | Array indexing (`event.tags[0]`) | PASS | Works natively |
| 3 | Boolean operators (`&&`, `||`) | PASS | Works natively |
| 4 | Comparison (`>`, `<`, `>=`, `<=`, `!=`, `==`) | PASS | Works natively |
| 5 | String operations (`|lower`, `|upper`) | PASS | Via custom transforms |
| 6 | Ternary (`condition ? a : b`) | PASS with caveat | Condition MUST be parenthesized: `(x > 5) ? a : b`. Without parens, `?` has higher precedence than comparison operators |
| 7 | Regex matching | FAIL natively | Use custom `|match('pattern')` transform |
| 8 | `in` operator | PASS | Works for both array membership and substring |
| 9 | Null handling | PASS | Undefined fields resolve to null |
| 10 | Nested boolean with parens | PASS | Works natively |

**Critical limitations:**
- **No `!` (NOT/negation) operator** -- PR #30 open since May 2023, still unmerged. Workaround: `(expr) == false`
- **Ternary precedence bug** -- `a > 5 ? x : y` parsed as `a > (5 ? x : y)`. Always parenthesize: `(a > 5) ? x : y`

**Recommendation: Stick with jexl-eval 0.4.0.** Build a `WorkflowEvaluator` wrapper that pre-registers standard transforms (lower, upper, startsWith, endsWith, contains, match, length, trim, split, not). ~50-80 lines of code. Far simpler than switching to cel-interpreter which would require serde_json-to-CEL-Value conversion.

**Standard transforms to register:**
| Transform | Syntax | Purpose |
|-----------|--------|---------|
| `lower` | `value|lower` | Lowercase |
| `upper` | `value|upper` | Uppercase |
| `startsWith` | `value|startsWith('prefix')` | Prefix check |
| `endsWith` | `value|endsWith('suffix')` | Suffix check |
| `contains` | `value|contains('substr')` | Substring check |
| `match` | `value|match('^pattern$')` | Regex matching |
| `length` | `value|length` | String/array length |
| `not` | `value|not` | Boolean negation workaround |

Confidence: HIGH (empirically verified)

## Deep Dive 2: Durable Execution Patterns

Researched Temporal, Restate, Prefect, Airflow, Dapr, LangGraph, and Persistasaurus.

**Recommended State Machine:**

Workflow Run states (7): `Pending → Running → Paused → Completed | Failed | Crashed | Cancelled`
- `Paused`: approval gates, human checkpoints
- `Crashed`: infrastructure failures (distinguished from `Failed` which is application errors)
- `Cancelled`: user-initiated abort

Step states (6): `Pending → Running → Completed | Failed | Skipped | WaitingApproval`

**SQLite Schema for Checkpointing:**
- `workflow_runs`: id, workflow_id, status, trigger_payload, context (JSON), started_at, completed_at, error
- `workflow_steps`: id, run_id, step_id, step_name, step_type, status, attempt, idempotency_key, input, output, started_at, completed_at, error

**Parallel Branch Strategy:** No separate `branch_id` needed. DAG-based execution with `depends_on` relationships. Steps are organized into waves (topological sort). On resume, query completed steps, rebuild context from their outputs, skip them, continue with pending/running steps.

**Crash Recovery Protocol:**
1. On startup: find `workflow_runs WHERE status = 'running'`
2. For each: find `workflow_steps WHERE status = 'running'` (stuck during crash)
3. For idempotent steps (conditional, loop): reset to pending
4. For side-effecting steps (HTTP, agent): use idempotency key to check completion
5. Resume from last completed step

**Critical finding: Set `synchronous=FULL` on SQLite** for workflow checkpoint operations. The default `synchronous=NORMAL` in WAL mode is NOT durable against power failures.

**Context Serialization:** JSON blob in SQLite with:
- 1MB per-step output limit (spill to file for larger)
- 10MB total context limit
- JSON blob over structured columns (universal pattern across all engines studied)

**Approval Gates:** Use Paused pattern -- mark run as `paused`, stop executor, resume when approval arrives. No blocking threads.

Confidence: HIGH (verified across 7 workflow engines)

## Deep Dive 3: TypeScript SDK Design

Researched CircleCI Config SDK, Serverless Workflow SDK, Hatchet v1, Inngest v3, Trigger.dev v3, rustyscript.

**Architecture: Build-to-YAML (primary) with optional API push**
- User writes `.workflow.ts` → runs `npx @boternity/workflow build` → produces `.yaml`
- Secondary: `npx @boternity/workflow push` calls Boternity REST API directly
- CircleCI Config SDK proves this exact pattern at scale

**Builder Pattern:** Factory functions (not classes), typed step references for DAG validation
```typescript
const gatherNews = workflow.agent('gather-news', { bot: 'researcher', prompt: '...' });
const analyze = workflow.agent('analyze', { bot: 'analyst', parents: [gatherNews] });
```

**Type Safety:** Three layers:
1. Zod schemas for step config validation (generated from Rust JSON Schema via schemars)
2. Typed parent refs for DAG dependency validation at compile time
3. ts-rs generates TypeScript interfaces from Rust types

**Rust SDK:** NOT a separate crate. Use `boternity-types` directly with a builder module. Users construct `WorkflowDefinition` structs and serialize to YAML.

**Code Step Execution: Use `rustyscript`** (deno_core/V8 wrapper):
- TypeScript transpilation built-in
- Sandboxed by default (no filesystem/network)
- Clean Rust↔JS data exchange via serde JSON
- Fresh Runtime per step execution (isolation)
- Reserve WASM for skill system (Phase 6), use rustyscript for workflow code steps

**Package:** `@boternity/workflow-sdk` in `packages/workflow-sdk/`

Confidence: MEDIUM-HIGH

## Deep Dive 4: Bot-to-Bot Communication Architecture

Researched AutoGen messaging, Tokio actor patterns (Alice Ryhl), existing codebase EventBus/DashMap usage.

**Message Routing: Actor handle pattern (separate from EventBus)**
- `MessageBus` with per-bot `mpsc::Sender` in `DashMap<Uuid, mpsc::Sender>`
- Each bot has its own mailbox (mpsc receiver) processed in a dedicated tokio task
- Separate from EventBus (different semantics: targeted delivery vs broadcast notifications)
- Bounded channels with backpressure (buffer: 256 messages)

**Sync vs Async Delivery:**
- `send_and_wait()`: Creates `oneshot::channel`, includes sender in message envelope, waits with 30s default timeout
- `send()`: Fire-and-forget, no reply channel
- Bot B processes messages in a SEPARATE task from its active chat (no interruption)

**Pub/Sub Implementation:**
- Per-topic `broadcast::channel` in `DashMap<String, broadcast::Sender>`
- Lazy creation (auto-create on first publish)
- Subscriptions persisted to SQLite for restart recovery
- At-most-once delivery in-memory + at-least-once via SQLite audit trail

**Message Processing Pipeline:**
1. Check bot's message_handler skill → if exists, run skill first
2. If skill returns "handled" → send reply, done
3. If skill returns "pass_through" or no handler → forward to LLM
4. Create/reuse bot-to-bot session (tagged `session_type = 'bot-to-bot'`)
5. Execute via AgentEngine (non-streaming)
6. Persist exchange to bot-to-bot session

**Loop Prevention (3 layers):**
1. Delegation depth cap (default: 5)
2. Exchange rate per bot pair (default: 10 messages per 60 seconds)
3. Time window reset (prevents permanent blocks)

**Delegation:** Special `send_and_wait()` with `message_type: "delegation"`. Not a separate mechanism. UI shows "Bot A asked Bot B for help" inline. Bot A triggers via a `delegate_to_bot` tool/function call registered in agent capabilities.

**SQLite Schema:** Three tables:
- `bot_messages`: full audit trail with indexes for pair queries and channel queries
- `bot_channels`: channel registry
- `bot_subscriptions`: persisted subscriptions (UNIQUE(bot_id, channel_name))
- `chat_sessions`: add `session_type` and `peer_bot_id` columns

**New components:** MessageBus, LoopGuard, MessageHandler, MessageRepository trait + SQLite impl, BotMessage type, REST handlers, CLI commands

Confidence: MEDIUM-HIGH

## Sources

### Primary (HIGH confidence)
- [tokio-cron-scheduler 0.15.1 - docs.rs](https://docs.rs/crate/tokio-cron-scheduler/latest) - Version, features, API, dependencies
- [tokio-cron-scheduler - GitHub](https://github.com/mvniekerk/tokio-cron-scheduler) - Architecture, job types, storage backends
- [croner 3.0.1 - docs.rs](https://docs.rs/croner/latest/croner/) - Cron parsing, timezone support, DST handling
- [evalexpr 13.1.0 - docs.rs](https://docs.rs/evalexpr/latest/evalexpr/) - Expression evaluation API, context, operators
- [jexl-eval 0.4.0 - docs.rs](https://docs.rs/jexl-eval/latest/jexl_eval/) - JEXL evaluator, dot-notation, context API
- [notify 8.2.0 - docs.rs](https://docs.rs/notify/latest/notify/) - File watcher API, event kinds, platform backends
- [english-to-cron 0.1.7 - GitHub](https://github.com/kaplanelad/english-to-cron) - Supported phrases, API
- [@xyflow/react - reactflow.dev](https://reactflow.dev/learn) - React Flow v12 quick start, custom nodes, handles
- [React Flow Custom Nodes](https://reactflow.dev/learn/customization/custom-nodes) - Custom node creation, nodeTypes registration
- [React Flow Drag and Drop](https://reactflow.dev/examples/interaction/drag-and-drop) - Sidebar drag-and-drop implementation
- [React Flow Sub Flows](https://reactflow.dev/learn/layouting/sub-flows) - Node grouping, parentId, extent
- [React Flow Built-in Components](https://reactflow.dev/learn/concepts/built-in-components) - MiniMap, Controls, Background, Panel
- [React Flow Dagre Layout](https://reactflow.dev/examples/layout/dagre) - Auto-layout integration
- [petgraph toposort - docs.rs](https://docs.rs/petgraph/latest/petgraph/algo/fn.toposort.html) - DAG topological sort API

### Secondary (MEDIUM confidence)
- [Building a Durable Execution Engine with SQLite](https://www.morling.dev/blog/building-durable-execution-engine-with-sqlite/) - Checkpoint table schema, resume logic, idempotency patterns
- [React Flow Undo/Redo](https://reactflow.dev/examples/interaction/undo-redo) - Snapshot-based undo/redo pattern description (full code is Pro-only)
- [React Flow Selection Grouping](https://reactflow.dev/examples/grouping/selection-grouping) - Group/ungroup interaction pattern
- [HMAC Webhook Verification (axum example)](https://pg3.dev/post/github_webhooks_rust) - Practical HMAC-SHA256 in axum handler

### Tertiary (LOW confidence)
- Workflow engine common pitfalls derived from Apache Airflow and Temporal documentation patterns -- generally applicable but not Rust-specific
- dagre version and API based on training data -- verify npm package version at install time

### Deep Dive Sources
- [jexl-eval 0.4.0 empirical testing](https://github.com/TomFrost/jexl) - Expression pattern verification via cargo test suite
- [jexl-eval PR #30](https://github.com/TomFrost/jexl/pull/30) - Negation operator discussion (unmerged)
- [Temporal Workflows](https://docs.temporal.io/workflows) - Durable execution patterns, state machines
- [Restate Durable Execution](https://docs.restate.dev/) - Checkpoint strategies, crash recovery
- [Prefect Architecture](https://docs.prefect.io/latest/concepts/flows/) - Flow state machines, task states
- [Apache Airflow](https://airflow.apache.org/docs/apache-airflow/stable/core-concepts/dags.html) - DAG execution, state transitions
- [Dapr Workflows](https://docs.dapr.io/developing-applications/building-blocks/workflow/) - Workflow orchestration patterns
- [LangGraph](https://python.langchain.com/docs/langgraph) - Graph-based agent orchestration
- [Persistasaurus](https://github.com/ssube/persistasaurus) - SQLite checkpoint implementation
- [CircleCI Config SDK](https://github.com/CircleCI-Public/circleci-config-sdk-ts) - TypeScript-to-YAML builder pattern
- [Serverless Workflow SDK](https://github.com/serverlessworkflow/sdk-typescript) - Workflow definition builders
- [Hatchet v1 SDK](https://docs.hatchet.run/sdks/typescript-sdk) - TypeScript workflow SDK patterns
- [Inngest v3](https://www.inngest.com/docs/typescript) - TypeScript function builders
- [Trigger.dev v3](https://trigger.dev/docs) - Workflow SDK architecture
- [rustyscript](https://docs.rs/rustyscript/latest/rustyscript/) - V8 integration, TypeScript execution
- [AutoGen Messaging](https://microsoft.github.io/autogen/docs/tutorial/conversation-patterns) - Multi-agent communication patterns
- [Tokio Actor Pattern (Alice Ryhl)](https://ryhl.io/blog/actors-with-tokio/) - Message passing, mailbox pattern
- [SQLite synchronous=FULL](https://www.sqlite.org/pragma.html#pragma_synchronous) - Durability guarantees in WAL mode

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All Rust crates verified via docs.rs with current versions; React Flow verified via official docs
- Architecture: MEDIUM-HIGH - Patterns derived from official docs + established durable execution literature; specific Boternity integration is novel
- Pitfalls: MEDIUM - Based on ecosystem knowledge and official docs; some derived from general workflow engine experience
- Bot-to-bot communication: MEDIUM - Architecture pattern is sound but novel to this project; no direct library equivalent
- Visual builder: HIGH - React Flow v12 APIs well-documented with official examples for all required features
- Deep dives: HIGH for jexl-eval (empirical), HIGH for durable execution (7 engines studied), MEDIUM-HIGH for SDK design (5 SDKs + rustyscript), MEDIUM-HIGH for bot-to-bot (actor pattern + existing patterns)

**Research date:** 2026-02-14
**Valid until:** 2026-03-14 (30 days -- stack is stable; React Flow and crate versions may get minor updates)
