---
phase: 05-agent-hierarchy-event-system
plan: 01
subsystem: types
tags: [serde, uuid, tokio-util, dashmap, toml, websocket, event-bus, agent-hierarchy]

# Dependency graph
requires:
  - phase: 01-foundation-bot-identity
    provides: "boternity-types crate, MemoryEntry, AgentConfig base types"
  - phase: 03-multi-provider-memory
    provides: "MemoryEntry with existing fields, workspace Cargo.toml layout"
provides:
  - "AgentEvent enum (13 variants) for event bus broadcast"
  - "GlobalConfig and ProviderPricing for config.toml parsing"
  - "SpawnMode, SpawnInstruction, AgentStatus, SubAgentResult, AgentNode for agent hierarchy"
  - "source_agent_id on MemoryEntry for sub-agent memory tagging"
  - "Workspace deps: dashmap, toml, tokio-util, axum ws feature"
affects:
  - 05-agent-hierarchy-event-system (plans 02-08 build on these types)
  - 06-sub-agent-ui-observability (UI will consume AgentEvent via WebSocket)

# Tech tracking
tech-stack:
  added: [dashmap 6.1, toml 0.8, tokio-util 0.7, axum ws feature]
  patterns:
    - "Serde tagged union for event bus (tag=type, rename_all=snake_case)"
    - "Optional source_agent_id for backward-compatible memory tagging"

key-files:
  created:
    - crates/boternity-types/src/event.rs
    - crates/boternity-types/src/config.rs
  modified:
    - Cargo.toml
    - crates/boternity-types/Cargo.toml
    - crates/boternity-types/src/agent.rs
    - crates/boternity-types/src/memory.rs
    - crates/boternity-types/src/lib.rs
    - crates/boternity-core/Cargo.toml
    - crates/boternity-api/Cargo.toml
    - crates/boternity-infra/Cargo.toml
    - crates/boternity-infra/src/sqlite/memory.rs
    - crates/boternity-core/src/memory/extractor.rs
    - crates/boternity-core/src/agent/prompt.rs
    - crates/boternity-api/src/cli/memory.rs
    - crates/boternity-api/src/cli/chat/loop_runner.rs

key-decisions:
  - "tokio-util without sync feature (CancellationToken available by default in 0.7)"
  - "toml as dev-dependency on boternity-types (only tests need parsing, runtime code uses serde)"
  - "source_agent_id: Option<Uuid> with None default for backward compatibility"

patterns-established:
  - "AgentEvent serde tagged union: all event bus messages use #[serde(tag = 'type', rename_all = 'snake_case')]"
  - "agent_id() helper on event enum for filtering agent-scoped events"

# Metrics
duration: 5min
completed: 2026-02-13
---

# Phase 5 Plan 1: Domain Types Summary

**AgentEvent 13-variant enum, GlobalConfig/ProviderPricing structs, SpawnMode/AgentStatus/SubAgentResult/AgentNode hierarchy types, and MemoryEntry source_agent_id field**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-13T21:37:38Z
- **Completed:** 2026-02-13T21:42:38Z
- **Tasks:** 2
- **Files modified:** 14

## Accomplishments

- Defined all 13 AgentEvent variants covering lifecycle, budget, safety, and provider events
- Created GlobalConfig with TOML deserialization support and sensible defaults (500k token budget)
- Extended agent.rs with SpawnMode, SpawnInstruction, AgentStatus, SubAgentResult, and AgentNode types
- Added source_agent_id to MemoryEntry and fixed all 6 construction sites across the workspace
- Added workspace deps (dashmap, toml, tokio-util) and axum ws feature for Phase 5 infrastructure

## Task Commits

Each task was committed atomically:

1. **Task 1: Add workspace dependencies and update crate Cargo.toml files** - `95d0ea8` (chore)
2. **Task 2: Create event.rs, config.rs, extend agent.rs and memory.rs** - `f5a79f1` (feat)

## Files Created/Modified

- `crates/boternity-types/src/event.rs` - AgentEvent enum with 13 variants and agent_id() helper
- `crates/boternity-types/src/config.rs` - GlobalConfig and ProviderPricing with TOML support
- `crates/boternity-types/src/agent.rs` - SpawnMode, SpawnInstruction, AgentStatus, SubAgentResult, AgentNode
- `crates/boternity-types/src/memory.rs` - Added source_agent_id field to MemoryEntry
- `crates/boternity-types/src/lib.rs` - Added event and config module declarations
- `crates/boternity-types/Cargo.toml` - Added toml dev-dependency
- `Cargo.toml` - Workspace deps: dashmap, toml, tokio-util; axum ws feature
- `crates/boternity-core/Cargo.toml` - tokio-util and dashmap dependencies
- `crates/boternity-api/Cargo.toml` - tokio-util dependency
- `crates/boternity-infra/Cargo.toml` - toml dependency
- `crates/boternity-infra/src/sqlite/memory.rs` - source_agent_id: None in MemoryEntry construction
- `crates/boternity-core/src/memory/extractor.rs` - source_agent_id: None in MemoryEntry construction
- `crates/boternity-core/src/agent/prompt.rs` - source_agent_id: None in test helper
- `crates/boternity-api/src/cli/memory.rs` - source_agent_id: None in remember command
- `crates/boternity-api/src/cli/chat/loop_runner.rs` - source_agent_id: None in /remember command

## Decisions Made

- **tokio-util without sync feature:** CancellationToken is available by default in tokio-util 0.7 (no `sync` feature exists). Plan specified `features = ["sync"]` but that feature does not exist in tokio-util.
- **toml as dev-dependency:** Only tests need TOML parsing; runtime GlobalConfig deserialization will happen in boternity-infra which already has the toml dependency.
- **source_agent_id backward-compatible:** Using `Option<Uuid>` with `None` default means existing MemoryEntry construction sites just add `source_agent_id: None` with no behavior change.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Removed non-existent `sync` feature from tokio-util**
- **Found during:** Task 1 (workspace dependency setup)
- **Issue:** Plan specified `tokio-util = { version = "0.7", features = ["sync"] }` but tokio-util 0.7 has no `sync` feature. Compilation failed.
- **Fix:** Changed to `tokio-util = "0.7"` (no features). CancellationToken is available without feature gates.
- **Files modified:** Cargo.toml
- **Verification:** `cargo check --workspace` passes
- **Committed in:** 95d0ea8 (Task 1 commit)

**2. [Rule 3 - Blocking] Added toml dev-dependency to boternity-types**
- **Found during:** Task 2 (config.rs tests)
- **Issue:** config.rs tests use `toml::from_str()` but boternity-types didn't have toml dependency
- **Fix:** Added `toml = { workspace = true }` as dev-dependency in boternity-types/Cargo.toml
- **Files modified:** crates/boternity-types/Cargo.toml
- **Verification:** `cargo test -p boternity-types` passes all config tests
- **Committed in:** f5a79f1 (Task 2 commit)

**3. [Rule 3 - Blocking] Fixed MemoryEntry construction sites across workspace**
- **Found during:** Task 2 (adding source_agent_id field)
- **Issue:** Adding new field to MemoryEntry broke 6 construction sites in infra, core, and api crates
- **Fix:** Added `source_agent_id: None` to all 6 sites (SQLite row mapping, extractor, CLI, chat loop, test helpers)
- **Files modified:** 5 files across 3 crates
- **Verification:** `cargo check --workspace` passes
- **Committed in:** f5a79f1 (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All auto-fixes necessary for compilation. No scope creep.

## Issues Encountered

None beyond the auto-fixed deviations above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- All domain types for Phase 5 are defined and compile
- Plans 02-08 can now import AgentEvent, GlobalConfig, SpawnMode, SubAgentResult, AgentNode
- Workspace deps (dashmap, tokio-util, toml, axum ws) are ready for event bus, config, and WebSocket work
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 05-agent-hierarchy-event-system*
*Completed: 2026-02-13*
