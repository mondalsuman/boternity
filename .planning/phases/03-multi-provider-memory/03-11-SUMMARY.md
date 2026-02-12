---
phase: 03-multi-provider-memory
plan: 11
subsystem: api, llm
tags: [cli, provider-management, fallback-chain, failover, verbose, comfy-table, circuit-breaker]

# Dependency graph
requires:
  - phase: 03-multi-provider-memory (plan 06)
    provides: FallbackChain, create_provider(), test_provider_connection(), failover warnings
  - phase: 03-multi-provider-memory (plan 03)
    provides: FallbackChain with health_status(), circuit breaker state tracking
  - phase: 03-multi-provider-memory (plan 08)
    provides: Vector memory search in ChatService, BoxVectorMemoryStore, BoxEmbedder
provides:
  - Provider CLI subcommands (bnity provider status/add/remove/list)
  - Provider config persistence in providers.json
  - Verbose chat mode (--verbose) showing memory recall and provider selection
  - Vector memory recall integrated into chat loop
  - Multi-provider fallback chain loading from providers.json
affects: [Phase 4 web UI (provider health dashboard), Phase 5 agent hierarchy]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Provider config persistence: providers.json in data_dir as JSON array of ProviderConfig"
    - "Verbose stderr pattern: [memory] and [provider] prefixed debug output to stderr"
    - "Graceful vector store fallback: Option<BoxVectorMemoryStore> for chat loop"

key-files:
  created:
    - crates/boternity-api/src/cli/provider.rs
  modified:
    - crates/boternity-api/src/cli/mod.rs
    - crates/boternity-api/src/main.rs
    - crates/boternity-api/src/state.rs
    - crates/boternity-api/src/cli/chat/loop_runner.rs

key-decisions:
  - "Provider configs persisted in ~/.boternity/providers.json (simple JSON array, not SQLite)"
  - "Circuit breaker state is session-scoped (resets each chat), not persisted in provider status"
  - "Verbose mode uses short flag -V (not -v which is taken by global verbosity counter)"
  - "Vector memory search integrated directly in chat loop with Option<BoxVectorMemoryStore> for graceful fallback"
  - "Provider add tests connection by default, --skip-test to bypass"

patterns-established:
  - "Provider persistence: JSON file in data_dir, loaded on demand by build_fallback_chain()"
  - "Verbose stderr output: [tag] prefix convention for debug-level chat loop info"
  - "Provider CLI: handle_provider_command() dispatch with json flag support"

# Metrics
duration: 9m 21s
completed: 2026-02-12
---

# Phase 3 Plan 11: Provider Management CLI and Failover Visibility Summary

**Provider CLI (status/add/remove/list) with providers.json persistence, verbose chat mode showing memory recall, and vector memory search integrated into chat loop**

## Performance

- **Duration:** 9m 21s
- **Started:** 2026-02-12T22:51:54Z
- **Completed:** 2026-02-12T23:01:15Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Created ProviderCommand CLI with status, add, remove, and list subcommands using comfy-table for display
- Provider configs persist to ~/.boternity/providers.json and are loaded into fallback chain on chat start
- Added --verbose/-V flag to bnity chat showing recalled memories (category, fact, relevance score) and provider selection details on stderr
- Integrated vector memory recall into the chat loop (search before each LLM request, inject into agent context)
- Enhanced "all providers down" error with clear messaging and bnity provider status suggestion
- Failover warnings appear on stderr in yellow with cost and capability degradation details
- Provider name shown in stats footer during failover (e.g., "model via provider_name")
- 5 new unit tests for capability inference across all provider types

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement provider CLI subcommands** - `25f75cc` (feat)
2. **Task 2: Finalize failover visibility in chat** - `6d1b479` (feat)

## Files Created/Modified

**Created:**
- `crates/boternity-api/src/cli/provider.rs` - ProviderCommand enum (Status/Add/Remove/List), handler functions, providers.json persistence, capability inference, connection testing

**Modified:**
- `crates/boternity-api/src/cli/mod.rs` - Added `pub mod provider`, Provider variant in Commands enum, --verbose flag on Chat command
- `crates/boternity-api/src/main.rs` - Dispatches Commands::Provider and passes verbose to chat loop
- `crates/boternity-api/src/state.rs` - build_fallback_chain() loads extra providers from providers.json, resolves API keys, creates BoxLlmProviders
- `crates/boternity-api/src/cli/chat/loop_runner.rs` - Added verbose param, print_verbose_memories(), print_verbose_provider_info(), vector memory recall integration, Option<BoxVectorMemoryStore> for graceful fallback

## Decisions Made
- Provider configurations stored in `~/.boternity/providers.json` as a simple JSON array rather than in SQLite -- avoids schema migration for config data that changes rarely
- Circuit breaker state shown as "closed" in `bnity provider status` CLI since state is session-scoped (tracked in-memory during chat, resets between sessions)
- Short flag `-V` used for verbose mode because `-v` is already taken by the global verbosity counter (`-v` = info, `-vv` = trace)
- Vector memory search uses a fresh LanceVectorStore connection per chat session, avoiding ownership issues with Arc<LanceVectorMemoryStore> in AppState
- Provider add requires `--experimental` flag for Claude subscription (enforces ToS awareness)
- Connection test runs by default on `bnity provider add`; `--skip-test` to bypass (per CONTEXT.md: "Always test connection when a new provider is configured")

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Vector memory store ownership for chat loop**
- **Found during:** Task 2 (vector memory integration)
- **Issue:** AppState holds `Arc<LanceVectorMemoryStore>` but `BoxVectorMemoryStore::new()` takes ownership of a concrete type. Cannot clone LanceVectorMemoryStore.
- **Fix:** Create a fresh `LanceVectorStore` connection (cheap: just opens existing DB) per chat session, wrapped in `Option<BoxVectorMemoryStore>` for graceful fallback if vector store unavailable.
- **Files modified:** crates/boternity-api/src/cli/chat/loop_runner.rs
- **Verification:** `cargo check --workspace` compiles, all 452 tests pass
- **Committed in:** 6d1b479 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minor ownership workaround. No scope creep.

## Issues Encountered
- Concurrent agent executions (03-12, 03-13) modified shared files (mod.rs, main.rs, memory.rs) between reads and writes, causing repeated linter-triggered reverts. Resolved by writing complete file contents and adapting to changes made by other agents.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Provider CLI is fully operational for configuring multi-provider fallback chains
- providers.json persistence ready for any number of providers with priority ordering
- Verbose mode ready for debugging memory recall and provider selection
- Vector memory recall integrated end-to-end in chat loop
- All 452 workspace tests pass

## Self-Check: PASSED

---
*Phase: 03-multi-provider-memory*
*Completed: 2026-02-12*
