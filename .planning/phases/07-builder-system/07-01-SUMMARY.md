---
phase: 07-builder-system
plan: 01
subsystem: types
tags: [schemars, json-schema, structured-output, builder, domain-types]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: boternity-types crate with serde derives and domain type patterns
provides:
  - BuilderTurn, BuilderPhase, PurposeCategory, BuilderConfig and related builder domain types
  - OutputConfig/OutputFormat/OutputJsonSchema for Claude structured output
  - add_additional_properties_false helper for Claude API compatibility
  - output_config field on CompletionRequest (backward compatible)
affects:
  - 07-02 through 07-10 (all builder plans depend on these types)
  - boternity-infra AnthropicRequest/BedrockRequest (will need output_config wiring in future plan)

# Tech tracking
tech-stack:
  added: [schemars v1]
  patterns: [schemars::JsonSchema derive on LLM output types, add_additional_properties_false post-processing]

key-files:
  created:
    - crates/boternity-types/src/builder.rs
  modified:
    - crates/boternity-types/src/llm.rs
    - crates/boternity-types/src/lib.rs
    - crates/boternity-types/Cargo.toml
    - Cargo.toml
    - crates/boternity-core/src/agent/engine.rs
    - crates/boternity-core/src/agent/orchestrator.rs
    - crates/boternity-core/src/agent/summarizer.rs
    - crates/boternity-core/src/agent/title.rs
    - crates/boternity-core/src/memory/extractor.rs
    - crates/boternity-core/src/llm/fallback.rs
    - crates/boternity-infra/src/llm/mod.rs
    - crates/boternity-infra/src/llm/anthropic/client.rs
    - crates/boternity-infra/src/llm/bedrock/client.rs
    - crates/boternity-infra/src/llm/openai_compat/mod.rs
    - crates/boternity-api/src/http/handlers/chat.rs
    - crates/boternity-api/src/cli/chat/loop_runner.rs

key-decisions:
  - "OutputFormat as flat struct with serde rename (not tagged enum) to match Claude API shape exactly"
  - "add_additional_properties_false checks for 'properties' key (not 'type: object') for robustness with anyOf schemas"
  - "schemars v1 (not 0.8) for latest JSON Schema draft support and serde attribute compatibility"

patterns-established:
  - "schemars::JsonSchema derive on all LLM-output types for structured output schema generation"
  - "add_additional_properties_false post-processing before sending schema to Claude API"
  - "output_config: None on all existing CompletionRequest constructions (backward compatible extension)"

# Metrics
duration: 5m 12s
completed: 2026-02-14
---

# Phase 7 Plan 01: Builder Domain Types and Structured Output Summary

**Builder domain types with schemars JsonSchema derives plus OutputConfig extension on CompletionRequest for Claude structured output**

## Performance

- **Duration:** 5m 12s
- **Started:** 2026-02-14T12:58:33Z
- **Completed:** 2026-02-14T13:03:45Z
- **Tasks:** 2
- **Files modified:** 17

## Accomplishments
- Created builder.rs with 14 domain types: BuilderTurn (tagged enum), BuilderPhase, PurposeCategory, BuilderConfig, PersonalityConfig, ModelConfig, SkillRequest, QuestionOption, BuilderPreview, BuilderAnswer, BuilderState, BuilderExchange, PartialBuilderConfig, and add_additional_properties_false helper
- Added OutputConfig/OutputFormat/OutputJsonSchema types to llm.rs enabling Claude structured output
- Updated all 16 existing CompletionRequest construction sites across 13 files with output_config: None for backward compatibility
- All 88 boternity-types tests pass including 8 new tests (6 builder + 2 output_config)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add schemars dependency and builder domain types** - `3f63465` (feat)
2. **Task 2: Add OutputConfig to CompletionRequest** - `d6f8c80` (feat)

## Files Created/Modified
- `crates/boternity-types/src/builder.rs` - All builder domain types with schemars::JsonSchema derives
- `crates/boternity-types/src/llm.rs` - OutputConfig, OutputFormat, OutputJsonSchema types; output_config field on CompletionRequest
- `crates/boternity-types/src/lib.rs` - Added pub mod builder
- `crates/boternity-types/Cargo.toml` - Added schemars workspace dependency
- `Cargo.toml` - Added schemars v1 to workspace dependencies
- `crates/boternity-core/src/agent/engine.rs` - output_config: None on CompletionRequest
- `crates/boternity-core/src/agent/orchestrator.rs` - output_config: None on CompletionRequest
- `crates/boternity-core/src/agent/summarizer.rs` - output_config: None on CompletionRequest
- `crates/boternity-core/src/agent/title.rs` - output_config: None on CompletionRequest
- `crates/boternity-core/src/memory/extractor.rs` - output_config: None on CompletionRequest
- `crates/boternity-core/src/llm/fallback.rs` - output_config: None on test CompletionRequest
- `crates/boternity-infra/src/llm/mod.rs` - output_config: None on test_provider_connection
- `crates/boternity-infra/src/llm/anthropic/client.rs` - output_config: None on 2 test CompletionRequests
- `crates/boternity-infra/src/llm/bedrock/client.rs` - output_config: None on 3 test CompletionRequests
- `crates/boternity-infra/src/llm/openai_compat/mod.rs` - output_config: None on 5 test CompletionRequests
- `crates/boternity-api/src/http/handlers/chat.rs` - output_config: None on CompletionRequest
- `crates/boternity-api/src/cli/chat/loop_runner.rs` - output_config: None on CompletionRequest

## Decisions Made
- Used flat struct OutputFormat with `#[serde(rename = "type")]` on type_field rather than tagged enum, to match Claude's exact API shape (`{"type": "json_schema", "json_schema": {...}}`)
- add_additional_properties_false checks for `"properties"` key presence rather than `"type": "object"` because schemars anyOf variants may have properties without explicit type annotation
- Added `"oneOf"` to the recursion list in add_additional_properties_false alongside anyOf/allOf for completeness

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Builder domain types ready for use in 07-02 (BuilderAgent trait) and all subsequent builder plans
- OutputConfig types ready for wiring through AnthropicRequest/BedrockRequest in a future plan (the provider-level types need the same field added)
- schemars v1 available across the workspace for any future JsonSchema derive needs

## Self-Check: PASSED

---
*Phase: 07-builder-system*
*Completed: 2026-02-14*
