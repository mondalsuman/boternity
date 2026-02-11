---
phase: 02-single-agent-chat-llm
plan: 06
subsystem: core, memory, agent
tags: [memory-extraction, context-summarization, title-generation, llm-utility, tracing]

# Dependency graph
requires:
  - phase: 02-single-agent-chat-llm
    plan: 01
    provides: "LlmProvider trait, BoxLlmProvider, CompletionRequest/Response, LlmError, Message, MemoryEntry, MemoryCategory"
  - phase: 02-single-agent-chat-llm
    plan: 03
    provides: "AnthropicProvider implementing LlmProvider (runtime provider for extraction/summarization/title calls)"
provides:
  - "SessionMemoryExtractor: LLM-based key fact extraction from conversations"
  - "ContextSummarizer: sliding window message condensation via LLM"
  - "generate_title(): auto-naming sessions from first user-assistant exchange"
affects:
  - 02-07 (Agent engine loop uses ContextSummarizer for sliding window management)
  - 02-07 (ChatService uses generate_title after first exchange)
  - 02-07 (Memory extraction pipeline uses SessionMemoryExtractor at session end and periodically)
  - 03 (Multi-provider phase may need extraction model selection logic)

# Tech tracking
tech-stack:
  added: [serde (workspace, added to boternity-core), serde_json (workspace, added to boternity-core)]
  patterns:
    - "Stateless LLM utility structs: no stored state, provider passed per-call"
    - "Graceful degradation: JSON parse failure logs warning + returns empty Vec (not error)"
    - "RawMemoryEntry intermediate deserialization with clamped importance conversion"
    - "select_messages_to_summarize splits conversation at keep_recent boundary"

key-files:
  created:
    - "crates/boternity-core/src/memory/extractor.rs"
    - "crates/boternity-core/src/agent/summarizer.rs"
    - "crates/boternity-core/src/agent/title.rs"
  modified:
    - "crates/boternity-core/src/memory/mod.rs"
    - "crates/boternity-core/src/agent/mod.rs"
    - "crates/boternity-core/Cargo.toml"

key-decisions:
  - "Extraction prompt returns JSON array with fact/category/importance fields; empty array for nothing worth extracting"
  - "Unknown memory categories from LLM are logged and skipped (not error)"
  - "Importance values clamped to 1-5 range (LLM may produce out-of-range integers)"
  - "ContextSummarizer formats conversation as role: content pairs for the summary LLM call"
  - "Title generation uses temperature 0.3, max_tokens 50 for short consistent titles"
  - "Title result trimmed of whitespace and surrounding quotes (both single and double)"
  - "serde/serde_json added to boternity-core for JSON parsing in extractor"

patterns-established:
  - "Stateless LLM utility pattern: struct with no fields, methods take &BoxLlmProvider as parameter"
  - "Graceful JSON parse degradation: log warning, return empty result, let caller queue retry"
  - "Message slice splitting: select_messages_to_summarize returns (&[to_summarize], &[to_keep])"

# Metrics
duration: 5min
completed: 2026-02-11
---

# Phase 2 Plan 06: Memory Extraction, Context Summarization, and Title Generation Summary

**SessionMemoryExtractor, ContextSummarizer, and generate_title -- three stateless LLM utility functions for persistent memory, sliding window context, and session auto-naming**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-11T23:00:26Z
- **Completed:** 2026-02-11T23:05:04Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- SessionMemoryExtractor uses LLM to identify facts, preferences, decisions, and corrections worth persisting across sessions
- ContextSummarizer condenses older messages into summaries for sliding window context management
- TitleGenerator creates 3-7 word session titles from the first user-assistant exchange
- All three utilities are stateless (no stored state), taking BoxLlmProvider as a parameter per-call
- Graceful degradation on JSON parse failures (logged, returns empty Vec, caller queues retry)
- 13 new unit tests covering JSON deserialization, importance clamping, message splitting, and title trimming

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement SessionMemoryExtractor** - `2b09608` (feat)
2. **Task 2: Implement ContextSummarizer and TitleGenerator** - `3a49cad` (feat)

## Files Created/Modified
- `crates/boternity-core/src/memory/extractor.rs` - SessionMemoryExtractor with extract() and extract_from_messages()
- `crates/boternity-core/src/agent/summarizer.rs` - ContextSummarizer with summarize() and select_messages_to_summarize()
- `crates/boternity-core/src/agent/title.rs` - generate_title() for session auto-naming
- `crates/boternity-core/src/memory/mod.rs` - Added pub mod extractor
- `crates/boternity-core/src/agent/mod.rs` - Added pub mod summarizer and pub mod title
- `crates/boternity-core/Cargo.toml` - Added serde and serde_json dependencies

## Decisions Made
- **Stateless utility pattern:** All three utilities (SessionMemoryExtractor, ContextSummarizer, generate_title) are stateless -- the BoxLlmProvider is passed per-call, not stored. This avoids lifetime complexity and enables different providers for different calls.
- **Graceful JSON parse degradation:** When the LLM returns malformed JSON from memory extraction, the extractor logs a warning and returns an empty Vec. The caller (ChatService or extraction pipeline) should queue the extraction for retry via pending_memory_extractions.
- **Importance clamping:** LLM-produced importance values are clamped to 1-5 range using i64::clamp() before u8 conversion. Out-of-range values are silently normalized.
- **Unknown categories skipped:** If the LLM returns an unrecognized memory category, the entry is logged and skipped rather than failing the entire extraction.
- **Title trimming:** generate_title trims whitespace, double quotes, and single quotes from the LLM response, since models often wrap titles in quotes.
- **serde/serde_json added to boternity-core:** Required for JSON parsing in the memory extractor. These were already workspace dependencies but not referenced from boternity-core.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added serde and serde_json dependencies to boternity-core**
- **Found during:** Task 1 (SessionMemoryExtractor)
- **Issue:** extractor.rs needs serde::Deserialize for RawMemoryEntry and serde_json for JSON parsing, but these were not in boternity-core's Cargo.toml
- **Fix:** Added `serde = { workspace = true }` and `serde_json = { workspace = true }` to boternity-core/Cargo.toml
- **Files modified:** crates/boternity-core/Cargo.toml
- **Verification:** `cargo check -p boternity-core` passes
- **Committed in:** 2b09608 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minimal -- serde/serde_json were already workspace dependencies, just needed reference from boternity-core. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All three LLM utility functions are ready for integration into the agent engine loop and ChatService
- SessionMemoryExtractor ready for the memory extraction pipeline (periodic + session-end extraction)
- ContextSummarizer ready for sliding window management in AgentContext
- TitleGenerator ready for auto-naming after first exchange
- Workspace compiles cleanly with 70 boternity-core tests passing
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 02-single-agent-chat-llm*
*Completed: 2026-02-11*
