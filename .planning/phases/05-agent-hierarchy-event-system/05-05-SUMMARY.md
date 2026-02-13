---
phase: 05-agent-hierarchy-event-system
plan: 05
subsystem: infra
tags: [toml, config, pricing, cost-estimation, budget]

# Dependency graph
requires:
  - phase: 05-agent-hierarchy-event-system
    provides: "GlobalConfig and ProviderPricing types in boternity-types/src/config.rs"
  - phase: 01-foundation-bot-identity
    provides: "boternity-infra crate structure and toml workspace dependency"
provides:
  - "load_global_config() for reading ~/.boternity/config.toml with graceful defaults"
  - "resolve_request_budget() merging global default with per-bot IDENTITY.md override"
  - "estimate_cost() with hardcoded pricing for 5 providers and user override support"
  - "format_cost() labeling all estimates with ~ prefix"
affects:
  - 05-agent-hierarchy-event-system (plans 06-08 for budget enforcement and cost display)
  - 06-sub-agent-ui-observability (cost display in UI)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Graceful config fallback: missing/invalid file returns Default, never errors"
    - "Prefix-match pricing: model_pattern is prefix, not glob, for simplicity"
    - "Bedrock contains-match fallback for region-prefixed model IDs"

key-files:
  created:
    - crates/boternity-infra/src/config.rs
    - crates/boternity-infra/src/llm/pricing.rs
  modified:
    - crates/boternity-infra/src/lib.rs
    - crates/boternity-infra/src/llm/mod.rs

key-decisions:
  - "default_pricing_table() is private (not pub) since external callers use estimate_cost()"
  - "OpenAI gpt-4o-mini entry ordered before gpt-4o to ensure prefix match correctness"
  - "Bedrock uses contains() fallback for region-prefixed model IDs (eu.anthropic.claude-...)"
  - "Minimum budget floor of 10,000 tokens enforced in resolve_request_budget()"

patterns-established:
  - "Config loader pattern: async read -> parse -> fallback to Default on any error"
  - "Cost formatting: ~$X.XX for >= $0.01, ~$X.XXX for < $0.01"

# Metrics
duration: 4min
completed: 2026-02-13
---

# Phase 5 Plan 5: Config Loading and Cost Estimation Summary

**Config.toml loader with graceful defaults, per-bot budget resolution, and hardcoded pricing table covering 5 LLM providers with user override support**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-13T21:49:19Z
- **Completed:** 2026-02-13T21:53:19Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Created config.toml loader that reads GlobalConfig from ~/.boternity/ with graceful fallback to defaults on missing or invalid files
- Implemented resolve_request_budget() merging global default with per-bot IDENTITY.md override, enforcing 10,000 token minimum
- Built cost estimation with hardcoded pricing table covering Anthropic, Bedrock, OpenAI, Google, and Mistral
- Added user override capability from config.toml provider_pricing entries with priority over defaults
- Format function labels all costs as estimates with ~ prefix and adaptive decimal places

## Task Commits

Each task was committed atomically:

1. **Task 1: Config.toml loader** - `f85f58c` (feat)
2. **Task 2: Cost estimation and pricing table** - `f4d43f4` (feat)

## Files Created/Modified

- `crates/boternity-infra/src/config.rs` - load_global_config() and resolve_request_budget() with 6 tests
- `crates/boternity-infra/src/llm/pricing.rs` - estimate_cost(), format_cost(), default_pricing_table() with 9 tests
- `crates/boternity-infra/src/lib.rs` - Added config module declaration
- `crates/boternity-infra/src/llm/mod.rs` - Added pricing module declaration

## Decisions Made

- **default_pricing_table() is private:** Plan specified the struct as "private to this module" but left the function pub. Made function private too since external callers only need estimate_cost(). Eliminates private_interfaces warning.
- **OpenAI mini before regular:** gpt-4o-mini entry placed before gpt-4o in pricing table so prefix matching finds the more specific entry first.
- **Bedrock contains fallback:** Bedrock model IDs include region prefixes (e.g., `eu.anthropic.claude-sonnet-4-...`). After normal prefix match fails, a contains() check catches these patterns.
- **Minimum budget floor:** 10,000 tokens enforced regardless of source (global config or identity override) as a safety mechanism.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Made default_pricing_table() private to fix Rust warning**
- **Found during:** Task 2 (pricing module compilation)
- **Issue:** `default_pricing_table()` was `pub` but returned `Vec<PricingEntry>` where `PricingEntry` is private, triggering `private_interfaces` warning
- **Fix:** Changed function to `fn default_pricing_table()` (private). External callers use `estimate_cost()` which hides the internal type.
- **Files modified:** crates/boternity-infra/src/llm/pricing.rs
- **Verification:** `cargo test -p boternity-infra -- pricing` passes, no warnings
- **Committed in:** f4d43f4 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Visibility fix necessary for clean compilation. No scope creep.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Config loading infrastructure ready for budget enforcement in plans 06-08
- Cost estimation ready for display alongside token counts in chat responses
- resolve_request_budget() ready for integration with IDENTITY.md frontmatter parsing
- No blockers or concerns

---
*Phase: 05-agent-hierarchy-event-system*
*Completed: 2026-02-13*
