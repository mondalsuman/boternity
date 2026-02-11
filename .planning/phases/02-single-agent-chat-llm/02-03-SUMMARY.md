---
phase: 02-single-agent-chat-llm
plan: 03
subsystem: infra, llm
tags: [anthropic, claude, sse, streaming, reqwest, reqwest-eventsource, secrecy, llm-provider]

# Dependency graph
requires:
  - phase: 02-single-agent-chat-llm
    plan: 01
    provides: "LlmProvider trait, StreamEvent enum, CompletionRequest/Response, LlmError, ProviderCapabilities, Usage, TokenCount"
provides:
  - "AnthropicProvider implementing LlmProvider for Anthropic Messages API"
  - "SSE streaming state machine with all 8 Anthropic event types"
  - "Tool use JSON fragment accumulation per content block index"
  - "Anthropic-specific request/response types (AnthropicRequest, AnthropicNonStreamResponse, etc.)"
affects:
  - 02-04 (Agent engine uses AnthropicProvider via BoxLlmProvider)
  - 02-05 (CLI chat uses stream() for token-by-token delivery)
  - 02-08 (Integration tests use AnthropicProvider)

# Tech tracking
tech-stack:
  added: [reqwest, reqwest-eventsource, async-stream, futures-util, secrecy, pin-project-lite (all workspace, added to boternity-infra)]
  patterns:
    - "SSE state machine: match on msg.event string -> deserialize msg.data into specific payload struct"
    - "Tool use accumulation: HashMap<u32, ToolUseAccumulator> with per-index JSON buffers parsed on block stop"
    - "SecretString for API keys: expose_secret() only for HTTP headers, never logged"
    - "Model-based capability detection: capabilities_for_model() derives ProviderCapabilities from model name"

key-files:
  created:
    - "crates/boternity-infra/src/llm/mod.rs"
    - "crates/boternity-infra/src/llm/anthropic/mod.rs"
    - "crates/boternity-infra/src/llm/anthropic/types.rs"
    - "crates/boternity-infra/src/llm/anthropic/client.rs"
    - "crates/boternity-infra/src/llm/anthropic/streaming.rs"
  modified:
    - "crates/boternity-infra/src/lib.rs"
    - "crates/boternity-infra/Cargo.toml"

key-decisions:
  - "SSE event dispatch via match on event type string, not serde tag on outer enum"
  - "Model capabilities derived from model name substring matching (sonnet/opus/haiku)"
  - "Empty tool use JSON buffer produces empty JSON object (not null or parse error)"
  - "5-minute reqwest timeout for long generation requests"
  - "AnthropicProvider does not derive Debug (defense-in-depth for API key)"

patterns-established:
  - "SSE state machine pattern: event string dispatch + typed payload deserialization"
  - "ToolUseAccumulator pattern: per-index buffer with String concat, parse once on block stop"
  - "Provider capability detection: model name substring matching with conservative defaults"

# Metrics
duration: 5min
completed: 2026-02-11
---

# Phase 2 Plan 03: Anthropic Claude Provider with SSE Streaming Summary

**AnthropicProvider implementing LlmProvider with reqwest+reqwest-eventsource SSE streaming, tool use JSON accumulation, and SecretString API key handling**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-11T22:50:58Z
- **Completed:** 2026-02-11T22:55:59Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- AnthropicProvider implements LlmProvider trait with complete(), stream(), and count_tokens() methods
- Full SSE state machine handling all 8 Anthropic event types with typed payload deserialization
- Tool use JSON fragment accumulation per content block index, parsed on content_block_stop
- API key wrapped in secrecy::SecretString, never logged or exposed in Debug output
- 23 unit tests covering type serialization, provider config, accumulator logic, and error mapping

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Anthropic-specific types and HTTP client** - `560309c` (feat)
2. **Task 2: Implement SSE streaming state machine** - `deff8eb` (feat)

## Files Created/Modified
- `crates/boternity-infra/src/llm/mod.rs` - LLM provider implementations module root
- `crates/boternity-infra/src/llm/anthropic/mod.rs` - Anthropic module re-exports
- `crates/boternity-infra/src/llm/anthropic/types.rs` - Anthropic-specific request/response types, SSE payload structs
- `crates/boternity-infra/src/llm/anthropic/client.rs` - AnthropicProvider implementing LlmProvider
- `crates/boternity-infra/src/llm/anthropic/streaming.rs` - SSE stream creation and state machine
- `crates/boternity-infra/src/lib.rs` - Added `pub mod llm`
- `crates/boternity-infra/Cargo.toml` - Added reqwest, reqwest-eventsource, async-stream, futures-util, secrecy, pin-project-lite
- `Cargo.lock` - Updated with new dependencies

## Decisions Made
- **SSE event dispatch via string match:** Instead of using serde tag on an outer enum, the streaming state machine matches on `msg.event.as_str()` and deserializes `msg.data` into specific payload structs. This matches how Anthropic's SSE protocol works (event type is in the SSE `event:` field, not in the JSON data).
- **Model capabilities from name substring:** `capabilities_for_model()` uses `model.contains("sonnet")` etc. to derive capabilities. Conservative defaults for unknown models.
- **Empty tool JSON buffer -> empty object:** When a tool use content block has no input_json_delta events, the accumulated buffer is empty. This produces `{}` (empty JSON object), not null or a parse error.
- **No Debug derive on AnthropicProvider:** Defense-in-depth to prevent accidental API key exposure, even though SecretString already redacts in Debug.

## Deviations from Plan

None -- plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None -- no external service configuration required for this plan. ANTHROPIC_API_KEY will be needed when the agent engine (02-04) or CLI (02-05) integrates this provider.

## Next Phase Readiness
- AnthropicProvider is ready for use via BoxLlmProvider in the agent engine (02-04)
- Streaming is ready for token-by-token CLI delivery (02-05)
- All 23 tests pass, workspace compiles cleanly
- No blockers or concerns

## Self-Check: PASSED

---
*Phase: 02-single-agent-chat-llm*
*Completed: 2026-02-11*
