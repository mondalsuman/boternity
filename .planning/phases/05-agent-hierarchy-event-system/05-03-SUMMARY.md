---
phase: 05-agent-hierarchy-event-system
plan: 03
subsystem: agent
tags: [xml-parsing, spawn-instructions, agent-context, system-prompt, sub-agent, hierarchy]

# Dependency graph
requires:
  - phase: 05-agent-hierarchy-event-system
    provides: "SpawnMode, SpawnInstruction types in boternity-types/src/agent.rs"
  - phase: 02-single-agent-chat-llm
    provides: "AgentContext, SystemPromptBuilder, AgentEngine base implementations"
provides:
  - "parse_spawn_instructions() and extract_text_before_spawn() in spawner.rs"
  - "AgentContext.child_for_task() for isolated sub-agent context creation"
  - "SystemPromptBuilder.build_with_capabilities() for root agent spawn awareness"
  - "SystemPromptBuilder.build_for_sub_agent() for task-focused sub-agent prompts"
affects:
  - 05-agent-hierarchy-event-system (orchestrator will use spawner + child_for_task)
  - 06-sub-agent-ui-observability (UI rendering of sub-agent tree)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "XML tag parsing for spawn instructions (matches existing <soul>/<identity> pattern)"
    - "child_for_task() clones personality but resets conversation (isolation pattern)"
    - "Depth-gated capabilities: <agent_capabilities> included only when depth < 3"

key-files:
  created:
    - crates/boternity-core/src/agent/spawner.rs
  modified:
    - crates/boternity-core/src/agent/mod.rs
    - crates/boternity-core/src/agent/context.rs
    - crates/boternity-core/src/agent/prompt.rs

key-decisions:
  - "Only first <spawn_agents> block parsed per response (single spawn per turn)"
  - "Default spawn mode is Parallel when no mode attribute present"
  - "Sub-agent prompts exclude user_context, session_memory, long_term_memory (fresh context)"
  - "Depth < 3 includes agent_capabilities for recursive spawning; depth 3 excludes it"

patterns-established:
  - "Spawn XML parsing: find tag boundaries, extract task attributes, handle escaped quotes"
  - "Sub-agent context isolation: child inherits soul+config, gets empty history/memories"
  - "agent_capabilities section as appendable prompt extension for capability-aware agents"

# Metrics
duration: 4min
completed: 2026-02-13
---

# Phase 5 Plan 3: Spawn Instruction Parser and Context Extensions Summary

**XML spawn parser extracts parallel/sequential sub-agent tasks from LLM responses; child_for_task creates isolated contexts with inherited personality; agent_capabilities prompt teaches LLM the spawn protocol with depth-gated recursion**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-13T21:48:44Z
- **Completed:** 2026-02-13T21:52:16Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Built spawn instruction parser that extracts SpawnInstruction from `<spawn_agents>` XML blocks in LLM responses
- Added AgentContext.child_for_task() that creates isolated sub-agent contexts with fresh conversation history but inherited personality
- Extended SystemPromptBuilder with build_with_capabilities() for root agents and build_for_sub_agent() for task-focused sub-agent prompts
- Depth-gated recursive spawning: agents at depth < 3 can spawn sub-agents, depth 3 cannot

## Task Commits

Each task was committed atomically:

1. **Task 1: Spawn instruction parser (spawner.rs)** - `eb9c32d` (feat)
2. **Task 2: AgentContext.child_for_task() and SystemPromptBuilder agent_capabilities** - `bee6a84` (feat)

## Files Created/Modified

- `crates/boternity-core/src/agent/spawner.rs` - XML parser for `<spawn_agents>` blocks with parallel/sequential mode extraction
- `crates/boternity-core/src/agent/mod.rs` - Added `pub mod spawner` declaration
- `crates/boternity-core/src/agent/context.rs` - Added child_for_task() method for sub-agent context creation
- `crates/boternity-core/src/agent/prompt.rs` - Added build_with_capabilities(), build_for_sub_agent(), and agent_capabilities_section()

## Decisions Made

- **Single spawn per response:** Only the first `<spawn_agents>` block is parsed. Multiple spawn blocks in one response would be ambiguous -- the orchestrator handles one spawn decision per turn.
- **Default mode is Parallel:** When no `mode` attribute is present on `<spawn_agents>`, defaults to parallel execution (more common use case).
- **Sub-agent context isolation:** child_for_task() creates completely fresh conversation history and memories, while inheriting soul_content, identity_content, and agent_config. This prevents context leaking (Research Pitfall 2).
- **Depth-gated capabilities:** `<agent_capabilities>` section is included at depth < 3 to allow recursive spawning. At depth 3, agents can execute but cannot spawn children (would be depth 4, exceeding the 3-level cap).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Spawner can parse LLM responses into SpawnInstruction structs ready for the orchestrator
- child_for_task() provides the isolation needed for parallel sub-agent execution
- System prompt extensions teach both root and sub-agents the spawn protocol
- Ready for orchestrator (05-04/05-05) to wire spawner + context into the execution loop

## Self-Check: PASSED

---
*Phase: 05-agent-hierarchy-event-system*
*Completed: 2026-02-13*
