---
phase: 08-workflows-pipelines
plan: 14
subsystem: workflow
tags: [dag-executor, step-execution, llm-provider, wasm, reqwest, tokio-spawn, background-task]

# Dependency graph
requires:
  - phase: 08-04
    provides: "DagExecutor, StepRunner, StepExecutionContext trait, PlaceholderExecutionContext"
  - phase: 08-09
    provides: "AppState Phase 8 fields (workflow_repo, webhook_registry), REST API handlers"
  - phase: 08-13
    provides: "Crash recovery, boxed-future StepExecutionContext, StepRunner::with_context()"
provides:
  - "LiveExecutionContext implementing StepExecutionContext with real Agent/Skill/HTTP execution"
  - "DagExecutor::with_execution_context() constructor for live service wiring"
  - "workflow_executor field on AppState initialized with LiveExecutionContext"
  - "Background executor spawning in trigger_workflow(), receive_webhook(), approve_run()"
affects: [09-observability, 10-polish]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Background executor spawning via tokio::spawn for async workflow execution"
    - "DagExecutor manages full run lifecycle (create_run inside execute())"
    - "LiveExecutionContext follows dependency inversion (trait in core, impl in infra)"

key-files:
  created:
    - "crates/boternity-infra/src/workflow/execution_context.rs"
  modified:
    - "crates/boternity-core/src/workflow/executor.rs"
    - "crates/boternity-infra/src/workflow/mod.rs"
    - "crates/boternity-api/src/state.rs"
    - "crates/boternity-api/src/http/handlers/workflow.rs"
    - "crates/boternity-api/src/http/handlers/webhook.rs"

key-decisions:
  - "DagExecutor::with_execution_context() added as separate constructor rather than modifying new() signature (preserves PlaceholderExecutionContext default for tests)"
  - "trigger_workflow and receive_webhook return 'submitted' status immediately; executor creates its own WorkflowRun record (async job submission pattern)"
  - "approve_run spawns executor.resume() which handles run status transition internally"
  - "LiveExecutionContext reuses AppState::create_single_provider pattern for auto-detecting Anthropic vs Bedrock keys"

patterns-established:
  - "Background execution: handlers spawn tokio::spawn(executor.execute()) and return immediately"
  - "Executor owns run lifecycle: creates WorkflowRun in execute(), manages Running/Completed/Failed transitions"
  - "LiveExecutionContext resolves bot model from IDENTITY.md frontmatter for agent steps"

# Metrics
duration: 6min
completed: 2026-02-14
---

# Phase 8 Plan 14: Workflow Execution Wiring Summary

**LiveExecutionContext wiring DagExecutor to real LLM, Skill, and HTTP services with background executor spawning from REST API triggers**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-14T16:44:18Z
- **Completed:** 2026-02-14T16:50:40Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- LiveExecutionContext implements StepExecutionContext with real Agent (LLM completion), Skill (WASM/prompt), and HTTP (reqwest) execution
- DagExecutor wired to AppState with LiveExecutionContext via new with_execution_context() constructor
- trigger_workflow() and receive_webhook() spawn background executor tasks for async workflow execution
- approve_run() spawns executor.resume() to continue paused workflows from their last checkpoint

## Task Commits

Each task was committed atomically:

1. **Task 1: LiveExecutionContext in infra layer + DagExecutor on AppState** - `6114a33` (feat)
2. **Task 2: Wire trigger_workflow() and webhook handler to spawn executor** - `69e1a09` (feat)

## Files Created/Modified
- `crates/boternity-infra/src/workflow/execution_context.rs` - LiveExecutionContext implementing StepExecutionContext with real Agent/Skill/HTTP services
- `crates/boternity-infra/src/workflow/mod.rs` - Added execution_context module export
- `crates/boternity-core/src/workflow/executor.rs` - Added DagExecutor::with_execution_context() constructor
- `crates/boternity-api/src/state.rs` - Added workflow_executor field on AppState with LiveExecutionContext wiring
- `crates/boternity-api/src/http/handlers/workflow.rs` - trigger_workflow() and approve_run() spawn background executor
- `crates/boternity-api/src/http/handlers/webhook.rs` - receive_webhook() spawns background executor

## Decisions Made
- **DagExecutor::with_execution_context()**: Added as separate constructor rather than modifying DagExecutor::new() signature, preserving PlaceholderExecutionContext as default for test compatibility
- **Async job submission pattern**: trigger_workflow and receive_webhook return "submitted" status immediately. The DagExecutor creates its own WorkflowRun record with Running status, avoiding duplicate run creation
- **Secret service early Arc wrapping**: Moved Arc::new(secret_service) before executor initialization to share the Arc with LiveExecutionContext, simplified struct initialization

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Workflow execution is fully wired end-to-end: REST trigger -> DagExecutor -> LiveExecutionContext -> real services
- Agent steps connect to LLM providers (Anthropic/Bedrock) via existing secret infrastructure
- Skill steps connect to SkillStore and WasmRuntime
- HTTP steps use reqwest with 30-second timeout
- All 134 core workflow tests and 64 infra workflow tests continue to pass

## Self-Check: PASSED

---
*Phase: 08-workflows-pipelines*
*Completed: 2026-02-14*
