---
phase: 05-agent-hierarchy-event-system
plan: 04
subsystem: agent
tags: [orchestrator, joinset, parallel, sequential, budget, cancellation, synthesis, event-bus, memory-tagging]

# Dependency graph
requires:
  - phase: 05-agent-hierarchy-event-system
    provides: "RequestBudget, SharedWorkspace, CycleDetector, RequestContext, EventBus (05-02)"
  - phase: 05-agent-hierarchy-event-system
    provides: "parse_spawn_instructions, child_for_task, SystemPromptBuilder agent capabilities (05-03)"
  - phase: 02-single-agent-chat-llm
    provides: "AgentContext, AgentEngine, BoxLlmProvider, CompletionRequest, StreamEvent"
provides:
  - "AgentOrchestrator with execute(), execute_parallel(), execute_sequential(), execute_single_agent()"
  - "OrchestratorResult with pre_spawn_text, sub_agent_results, synthesis, final_response, memory_contexts"
  - "AgentMemoryContext for sub-agent memory extraction with source_agent_id tagging"
  - "build_synthesis_prompt() producing XML <sub_agent_results> block"
  - "OrchestratorError enum (LlmError, BudgetExhausted, Cancelled, Internal)"
affects:
  - 05-agent-hierarchy-event-system (plans 06-08 wire orchestrator into WebSocket, chat handler, CLI)
  - 06-sub-agent-ui-observability (UI consumes events published by orchestrator)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "JoinSet for parallel sub-agent execution with JoinError -> Failed SubAgentResult conversion"
    - "Retry-once pattern: first failure retries, second failure returns Failed status"
    - "Stream collection with inline budget tracking and event publishing"
    - "AgentMemoryContext for deferred memory extraction with source_agent_id tagging"

key-files:
  created:
    - crates/boternity-core/src/agent/orchestrator.rs
  modified:
    - crates/boternity-core/src/agent/mod.rs

key-decisions:
  - "Orchestrator is stateless coordinator: no fields beyond max_depth, all state via parameters"
  - "BoxLlmProvider streams created before JoinSet spawn (stream is 'static, provider is not Clone)"
  - "Token estimation via 4 chars/token heuristic for streaming budget, corrected by real Usage events"
  - "Sequential sub-agents see only immediately prior result (not full chain) per user decision"
  - "Memory extraction deferred to chat handler via AgentMemoryContext (orchestrator surfaces data, caller extracts)"

patterns-established:
  - "collect_stream_with_events(): reusable stream collection with budget + event publishing"
  - "build_completion_request(): free function replicating AgentEngine pattern for orchestrator use"
  - "Parallel execution via JoinSet with pre-created streams for 'static lifetime compatibility"

# Metrics
duration: 4min
completed: 2026-02-13
---

# Phase 5 Plan 4: Agent Orchestrator Summary

**AgentOrchestrator managing full request lifecycle: LLM call, spawn detection, parallel/sequential sub-agent execution via JoinSet, retry-once logic, budget enforcement with cancellation, synthesis, and 11 distinct AgentEvent types published at every lifecycle point**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-13T21:58:06Z
- **Completed:** 2026-02-13T22:02:11Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Built AgentOrchestrator as the central execution engine for multi-agent request lifecycle
- Implemented execute() with spawn detection, depth limiting, cycle detection, and synthesis
- Parallel execution via JoinSet with pre-created streams for 'static lifetime compatibility
- Sequential execution with prior result injection and cancellation/budget checks between tasks
- Retry-once logic: first failure retries with will_retry=true, second failure returns Failed
- 11 distinct AgentEvent types published via explicit event_bus.publish() calls at every lifecycle point
- OrchestratorResult includes AgentMemoryContext for deferred source_agent_id-tagged memory extraction
- 15 unit tests covering synthesis prompt generation, result construction, memory context population

## Task Commits

Each task was committed atomically:

1. **Task 1: AgentOrchestrator core structure and parallel execution** - `d805502` (feat)

## Files Created/Modified

- `crates/boternity-core/src/agent/orchestrator.rs` - Full orchestrator: execute, parallel/sequential execution, retry, budget, synthesis, 1224 lines
- `crates/boternity-core/src/agent/mod.rs` - Added `pub mod orchestrator` declaration

## Decisions Made

- **Stateless coordinator:** AgentOrchestrator has only `max_depth: u8` field. All state (provider, context, budget, event_bus) passed as parameters to execute(). No long-lived state between calls.
- **Pre-created streams for JoinSet:** Since BoxLlmProvider is not Clone, streams are created before spawning into JoinSet (streams are 'static). This enables true parallel token collection while the provider reference stays in the parent scope.
- **Token estimation heuristic:** Uses 4 chars/token estimate during streaming for budget tracking. When the provider sends a real Usage event, the budget is corrected with the differential to avoid double-counting.
- **Sequential prior result injection:** Per user decision, sequential sub-agents see only the immediately prior sub-agent's result injected as a user message, not the full result chain.
- **Deferred memory extraction:** Orchestrator surfaces AgentMemoryContext (agent_id + response_text + task_description) in OrchestratorResult. The actual memory extraction happens in the chat handler layer, not inside the orchestrator.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- AgentOrchestrator is ready to be wired into the chat handler (plan 07) and CLI integration (plan 08)
- EventBus publishing at all lifecycle points is ready for WebSocket subscribers (plan 06)
- OrchestratorResult.memory_contexts is ready for source_agent_id-tagged memory extraction
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 05-agent-hierarchy-event-system*
*Completed: 2026-02-13*
