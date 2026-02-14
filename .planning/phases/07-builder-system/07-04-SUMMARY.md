---
phase: "07-builder-system"
plan: "04"
subsystem: builder
tags: [smart-defaults, assembler, purpose-category, bot-creation]
depends_on:
  requires: ["07-02"]
  provides: ["SmartDefaults for purpose categories", "BotAssembler for creating bots from BuilderConfig"]
  affects: ["07-05", "07-06", "07-07"]
tech-stack:
  added: []
  patterns: ["stateless utility struct", "keyword-based classification heuristic"]
key-files:
  created:
    - crates/boternity-core/src/builder/defaults.rs
    - crates/boternity-core/src/builder/assembler.rs
  modified:
    - crates/boternity-core/src/builder/mod.rs
decisions:
  - id: "07-04-01"
    description: "All categories default to claude-sonnet-4-20250514 model (single model, temperature differentiates)"
  - id: "07-04-02"
    description: "classify_purpose uses first-match-wins priority order; Creative before Coding means 'write code' classifies as Creative"
  - id: "07-04-03"
    description: "Assembly overwrites default files from create_bot (write-then-overwrite acceptable per research pitfall 9)"
  - id: "07-04-04"
    description: "Skill attachment deferred to Plan 07-06 (skills_attached returns empty Vec)"
metrics:
  duration: "2m 50s"
  completed: "2026-02-14"
---

# Phase 7 Plan 4: Smart Defaults + BotAssembler Summary

Smart defaults for all 7 purpose categories with keyword-based classifier, plus BotAssembler that creates complete bots from BuilderConfig using existing BotService/SoulService APIs.

## What Was Done

### Task 1: Smart defaults for purpose categories

Created `defaults.rs` with two public functions:

- `smart_defaults_for_category(&PurposeCategory) -> SmartDefaults` -- returns model, temperature, max_tokens, suggested_tone, suggested_traits, and suggested_skills tuned per category. Coding gets 0.2 temperature, Creative gets 0.9, SimpleUtility/CustomerService get 2048 max_tokens, others get 4096.

- `classify_purpose(&str) -> PurposeCategory` -- keyword-based heuristic with case-insensitive matching and first-match-wins priority. Falls back to `Custom(first 50 chars)` when no keywords match.

### Task 2: BotAssembler

Created `assembler.rs` with stateless `BotAssembler` struct and content generators:

- `BotAssembler::assemble()` -- generic over BotService type params, executes the full assembly sequence: `create_bot` -> `write_and_save_soul` -> `write_identity` -> `write_user`. Returns `AssemblyResult` with bot, all content strings, and file paths.

- `generate_soul_content()` -- three-section template (Personality/Purpose/Boundaries) with YAML frontmatter containing name, traits list, and tone.

- `generate_identity_content()` -- YAML frontmatter with model, temperature, max_tokens.

- `generate_user_content()` -- seeded USER.md with bot name, description, and placeholder sections.

## Task Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Smart defaults for purpose categories | 36a7c79 | defaults.rs, mod.rs |
| 2 | BotAssembler | 78bca23 | assembler.rs, mod.rs |

## Decisions Made

1. **All categories use same model** -- claude-sonnet-4-20250514 across the board; temperature is the primary differentiator (0.2 for Coding, 0.9 for Creative).

2. **First-match-wins classification** -- Priority order: SimpleUtility > ComplexAnalyst > Creative > Coding > Research > CustomerService > Custom. This means "write code documentation" classifies as Creative (not Coding).

3. **Write-then-overwrite assembly** -- `create_bot` writes defaults, then assembler overwrites with builder content. Minor inefficiency accepted per research pitfall 9.

4. **Skills deferred** -- `skills_attached` returns empty Vec; skill attachment wired in Plan 07-06.

## Deviations from Plan

None -- plan executed exactly as written.

## Verification

- `cargo check --workspace` compiles (no new warnings)
- `cargo test -p boternity-core builder` -- 52 tests pass (31 new + 21 existing)
- SmartDefaults returns correct temperature/tokens for each category
- SOUL.md follows Personality/Purpose/Boundaries structure
- Assembly uses existing BotService::create_bot and SoulService methods

## Next Phase Readiness

Plan 07-05 (ForgeAgent LLM implementation) can proceed -- it will use `classify_purpose` and `smart_defaults_for_category` during the builder conversation flow. Plan 07-06 (Skill Builder) will wire skill attachment into the assembler.

## Self-Check: PASSED
