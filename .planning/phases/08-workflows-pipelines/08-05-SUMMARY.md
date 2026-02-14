---
phase: 08-workflows-pipelines
plan: 05
subsystem: workflow
tags: [jexl, expression-evaluation, retry, self-correction, llm]

# Dependency graph
requires:
  - phase: 08-01
    provides: "RetryConfig, RetryStrategy, StepDefinition, StepConfig types in boternity-types"
provides:
  - "WorkflowEvaluator with JEXL expression evaluation and 10 standard transforms"
  - "RetryHandler with Simple and LlmSelfCorrect strategies"
  - "WorkflowContext struct for expression evaluation surface"
affects: [08-04, 08-07, 08-13]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Stateless handler pattern (RetryHandler matches MemoryExtractor)"
    - "JEXL context passed as JSON object, never interpolated into expressions"
    - "JavaScript-like truthiness coercion for boolean evaluation"

key-files:
  created:
    - "crates/boternity-core/src/workflow/expression.rs"
    - "crates/boternity-core/src/workflow/retry.rs"
  modified:
    - "crates/boternity-core/src/workflow/mod.rs"

key-decisions:
  - "Defined minimal WorkflowContext in expression.rs for evaluation surface (steps, trigger, variables, workflow metadata)"
  - "Used JavaScript-like truthiness for bool coercion (empty string/null/zero = false)"
  - "match transform uses substring match, not regex (security/simplicity trade-off)"
  - "RetryHandler is stateless -- callers track attempt counts externally"

patterns-established:
  - "Expression context shape: { steps: { <id>: { output: <v> } }, trigger: {...}, variables: {...}, workflow: {...} }"
  - "Transforms: subject is first arg, additional args follow"

# Metrics
duration: 4min
completed: 2026-02-14
---

# Phase 8 Plan 5: Expression Evaluator + Retry Handler Summary

**JEXL expression evaluator with 10 standard transforms and retry handler supporting Simple re-execution and LLM self-correction analysis**

## Performance

- **Duration:** 4 min 26s
- **Started:** 2026-02-14T15:00:12Z
- **Completed:** 2026-02-14T15:04:38Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- WorkflowEvaluator wrapping jexl_eval with 10 pre-registered transforms (lower, upper, trim, split, not, contains, startsWith, endsWith, match, length)
- evaluate_bool, evaluate_value, evaluate_in_workflow_context methods with proper context validation
- RetryHandler with should_retry (attempt limit enforcement) and prepare_retry (Simple/LlmSelfCorrect)
- LLM self-correction prompts include step name, config summary, error details, and remaining attempts
- 42 tests total (32 expression + 10 retry)

## Task Commits

Each task was committed atomically:

1. **Task 1: JEXL expression evaluator with standard transforms** - `627c7d3` (feat)
2. **Task 2: Retry handler with Simple and LLM self-correction** - `a159213` (feat)

## Files Created/Modified
- `crates/boternity-core/src/workflow/expression.rs` - WorkflowEvaluator with JEXL eval, transforms, WorkflowContext (692 lines)
- `crates/boternity-core/src/workflow/retry.rs` - RetryHandler with Simple/LlmSelfCorrect strategies (352 lines)
- `crates/boternity-core/src/workflow/mod.rs` - Added expression and retry module declarations

## Decisions Made
- Defined a minimal `WorkflowContext` struct in expression.rs rather than depending on 08-03's context module (which runs in parallel). The struct captures the expression evaluation surface: step_outputs, trigger_payload, variables, workflow_name, run_id.
- Used JavaScript-like truthiness for `evaluate_bool` coercion: null/false/0/empty-string are falsy, everything else truthy.
- The `match` transform uses simple substring matching rather than regex to avoid regex injection attacks and maintain simplicity.
- RetryHandler is fully stateless (same pattern as MemoryExtractor from phase 2) -- attempt tracking is the caller's responsibility.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Created stub files for parallel plan modules**
- **Found during:** Task 1 (compilation)
- **Issue:** mod.rs declared context, dag, definition modules (from 08-03) that don't exist yet
- **Fix:** Verified they already had placeholder stubs from 08-03's initial setup, no changes needed
- **Files modified:** none (stubs already existed)
- **Verification:** cargo check --workspace passes

---

**Total deviations:** 1 (blocking, auto-resolved -- stubs already existed)
**Impact on plan:** No scope creep. Plan executed as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Expression evaluator ready for trigger `when` clause filtering (08-07)
- Expression evaluator ready for conditional step branching (08-04)
- Retry handler ready for step execution engine retry logic (08-04)
- WorkflowContext struct may need reconciliation with 08-03's full context implementation

## Self-Check: PASSED

---
*Phase: 08-workflows-pipelines*
*Completed: 2026-02-14*
