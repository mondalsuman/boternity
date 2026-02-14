---
phase: 08-workflows-pipelines
plan: 03
subsystem: workflow-engine
tags: [petgraph, serde-yaml-ng, dag, topological-sort, workflow-context, template-resolution]

# Dependency graph
requires:
  - phase: 08-01
    provides: WorkflowDefinition, StepDefinition, and all workflow domain types
provides:
  - YAML parsing and serialization for WorkflowDefinition
  - Structural validation (unique IDs, valid deps, name format, orphan refs)
  - Filesystem load/save/discover for workflow YAML files
  - WorkflowError enum covering all workflow operation failures
  - DAG builder with petgraph-based cycle detection via toposort
  - Parallel wave computation from dependency depth grouping
  - Transitive dependency closure queries
  - WorkflowContext with 1MB per-step and 10MB total size limits
  - Template resolution for step outputs, trigger payload, and variables
  - JSON checkpoint/restore for context persistence
  - Expression context builder for JEXL evaluation
affects: [08-04 workflow engine, 08-06 trigger system, 08-07 visual builder, 08-08 sub-workflows]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "DAG-based wave computation: depth-first grouping for parallel step execution"
    - "Size-limited context: 1MB per step output, 10MB total context"
    - "Template resolution: {{ steps.X.output }} pattern for inter-step data flow"
    - "Unified WorkflowContext across expression, retry, and context modules"

key-files:
  created:
    - crates/boternity-core/src/workflow/definition.rs
    - crates/boternity-core/src/workflow/dag.rs
    - crates/boternity-core/src/workflow/context.rs
    - crates/boternity-core/src/workflow/mod.rs
  modified:
    - crates/boternity-core/src/lib.rs
    - crates/boternity-core/src/workflow/expression.rs
    - crates/boternity-core/src/workflow/retry.rs

key-decisions:
  - "WorkflowContext uses Uuid for run_id (not String) for type safety; to_expression_context converts to string"
  - "Oversized step outputs are truncated to a JSON object with _truncated metadata rather than returning an error"
  - "discover_workflows silently skips unparseable YAML files (logged as warnings)"
  - "Unified WorkflowContext in context.rs replaces minimal version in expression.rs to avoid type duplication"

patterns-established:
  - "Wave computation: O(V+E) depth-based grouping after topological sort"
  - "Template resolution: simple {{ }} pattern matching, not full template engine"
  - "Size enforcement: check-after-insert with truncation for graceful degradation"

# Metrics
duration: 8min
completed: 2026-02-14
---

# Phase 8 Plan 3: Workflow Core Modules Summary

**Petgraph DAG builder with parallel wave computation, YAML parser/validator, and size-limited WorkflowContext with template resolution**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-14T14:59:18Z
- **Completed:** 2026-02-14T15:07:43Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- YAML workflow definitions parse, validate, and roundtrip correctly via serde_yaml_ng
- Petgraph-based DAG validates dependency graphs and detects cycles via toposort
- Steps are grouped into parallel execution waves by dependency depth
- WorkflowContext tracks step outputs with 1MB per-step / 10MB total size limits
- Template resolution supports {{ steps.X.output }}, {{ trigger.Y }}, {{ variables.Z }}
- Unified WorkflowContext across all workflow modules (expression, retry, context)

## Task Commits

Each task was committed atomically:

1. **Task 1: Workflow definition parser and filesystem operations** - `9d42c7d` (feat)
2. **Task 2: DAG builder with topological sort and workflow context** - `d5d1eb6` (feat)

## Files Created/Modified
- `crates/boternity-core/src/workflow/mod.rs` - Module root exporting definition, dag, context, expression, retry
- `crates/boternity-core/src/workflow/definition.rs` - YAML parsing, validation, filesystem load/save/discover (512 lines)
- `crates/boternity-core/src/workflow/dag.rs` - DAG builder, cycle detection, wave computation, transitive deps (357 lines)
- `crates/boternity-core/src/workflow/context.rs` - WorkflowContext with size limits, template resolution, checkpointing (450 lines)
- `crates/boternity-core/src/lib.rs` - Added `pub mod workflow`
- `crates/boternity-core/src/workflow/expression.rs` - Updated to import WorkflowContext from context module
- `crates/boternity-core/src/workflow/retry.rs` - Updated to import WorkflowContext from context module

## Decisions Made
- WorkflowContext uses `Uuid` for `run_id` (not `String`) for type safety; `to_expression_context()` converts to string for JEXL evaluation
- Oversized step outputs are truncated to a JSON metadata object (`_truncated: true`) rather than returning a hard error, enabling graceful degradation
- `discover_workflows` silently skips unparseable YAML files (logged as tracing warnings) to handle mixed YAML directories
- Unified WorkflowContext in `context.rs` replaced the minimal version in `expression.rs` to avoid type duplication across the workflow module

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Unified WorkflowContext to resolve type duplication**
- **Found during:** Task 2 (DAG builder and workflow context)
- **Issue:** Parallel plan 08-05 had added `expression.rs` and `retry.rs` modules with a minimal `WorkflowContext` struct. Creating the full `WorkflowContext` in `context.rs` would result in two conflicting types.
- **Fix:** Defined the canonical `WorkflowContext` in `context.rs` with all required features (size limits, template resolution, checkpointing, expression context), then updated `expression.rs` and `retry.rs` to import from `context` instead of defining their own. Updated all test constructors from struct literals to `WorkflowContext::new()`.
- **Files modified:** `context.rs`, `expression.rs`, `retry.rs`
- **Verification:** All 77 workflow tests pass, workspace compiles clean
- **Committed in:** `d5d1eb6` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary to avoid type duplication from parallel plan execution. No scope creep.

## Issues Encountered
- Parallel plan execution (08-02, 08-05) overwrote Task 1 files with stubs after commit. Re-wrote files from committed content. All tests confirmed passing after restoration.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Workflow definition parsing, validation, and DAG computation are ready for the workflow engine (08-04)
- WorkflowContext is ready for trigger system (08-06) and step executors
- Expression context integration tested and working for JEXL evaluation
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 08-workflows-pipelines*
*Completed: 2026-02-14*
