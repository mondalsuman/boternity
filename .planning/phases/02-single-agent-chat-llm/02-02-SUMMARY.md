---
phase: 02-single-agent-chat-llm
plan: 02
subsystem: infra
tags: [opentelemetry, tracing, observability, workspace-deps, otel-genai]

# Dependency graph
requires:
  - phase: 01-foundation-bot-identity
    provides: workspace Cargo.toml structure, tracing and tracing-subscriber deps
provides:
  - boternity-observe crate with init_tracing() and shutdown_tracing()
  - GenAI semantic convention constants for LLM call instrumentation
  - All Phase 2 workspace dependencies (reqwest, termimad, syntect, rustyline-async, crossterm, secrecy, opentelemetry stack)
affects:
  - 02-single-agent-chat-llm (all downstream plans depend on workspace deps and observe crate)
  - 03-cross-session-memory (observe crate for memory extraction tracing)

# Tech tracking
tech-stack:
  added: [opentelemetry 0.31, opentelemetry_sdk 0.31, opentelemetry-stdout 0.31, tracing-opentelemetry 0.32, reqwest 0.12, reqwest-eventsource 0.6, eventsource-stream 0.2, async-stream 0.3, futures-util 0.3, termimad 0.34, syntect 5, rustyline-async 0.4, crossterm 0.28, secrecy 0.10, pin-project-lite 0.2]
  patterns: [OnceLock-based OTel provider storage for shutdown, GenAI semantic conventions for tracing spans]

key-files:
  created:
    - crates/boternity-observe/Cargo.toml
    - crates/boternity-observe/src/lib.rs
    - crates/boternity-observe/src/tracing_setup.rs
    - crates/boternity-observe/src/genai_attrs.rs
  modified:
    - Cargo.toml

key-decisions:
  - "OnceLock for OTel provider storage -- opentelemetry 0.31 removed global shutdown_tracer_provider(), store provider in OnceLock for clean shutdown"
  - "stdout exporter for dev -- opentelemetry-stdout for local development, swappable for OTLP in production"

patterns-established:
  - "OTel tracing init: init_tracing(enable_otel: bool) centralizes subscriber setup with optional OTel layer"
  - "GenAI attribute constants: pub const string slices following OTel GenAI semantic conventions for span fields"

# Metrics
duration: 3min
completed: 2026-02-11
---

# Phase 2 Plan 02: Observability Crate + Workspace Dependencies Summary

**boternity-observe crate with OTel-bridged tracing subscriber, GenAI semantic convention constants, and all Phase 2 workspace deps (15 new crates)**

## Performance

- **Duration:** 2m 46s
- **Started:** 2026-02-11T22:42:15Z
- **Completed:** 2026-02-11T22:45:01Z
- **Tasks:** 2
- **Files modified:** 5 (1 modified, 4 created)

## Accomplishments
- All Phase 2 workspace dependencies declared and resolving (reqwest, reqwest-eventsource, termimad, syntect, rustyline-async, crossterm, secrecy, opentelemetry stack, and more)
- boternity-observe crate with init_tracing() supporting structured logging + optional OTel trace export
- GenAI semantic convention constants for consistent LLM call instrumentation across entire codebase
- shutdown_tracing() with OnceLock-based provider storage for clean exit

## Task Commits

Each task was committed atomically:

1. **Task 1: Add all Phase 2 workspace dependencies** - `c166284` (chore)
2. **Task 2: Create boternity-observe crate with tracing setup and GenAI attributes** - `b059ed2` (feat)

## Files Created/Modified
- `Cargo.toml` - Added 15 new workspace dependencies + boternity-observe member/reference
- `crates/boternity-observe/Cargo.toml` - New crate with tracing, OTel, and subscriber deps
- `crates/boternity-observe/src/lib.rs` - Module re-exports for tracing_setup and genai_attrs
- `crates/boternity-observe/src/tracing_setup.rs` - init_tracing(enable_otel) with fmt + OTel layers, shutdown_tracing() with OnceLock provider
- `crates/boternity-observe/src/genai_attrs.rs` - OTel GenAI semantic convention constants (operation names, provider names, agent attributes)

## Decisions Made
- **OnceLock for provider storage:** opentelemetry 0.31 removed the global `shutdown_tracer_provider()` function. Used `std::sync::OnceLock<SdkTracerProvider>` to store the provider for clean shutdown via `provider.shutdown()`.
- **Clone provider before global set:** The SdkTracerProvider is cloned -- one copy goes to OnceLock for shutdown, one to the global registry for span creation.
- **stdout exporter for dev:** Using `opentelemetry_stdout::SpanExporter` as the default exporter for local development. Production deployments can swap to OTLP.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed opentelemetry 0.31 API change for shutdown**
- **Found during:** Task 2 (tracing_setup.rs implementation)
- **Issue:** Plan and RESEARCH.md referenced `opentelemetry::global::shutdown_tracer_provider()` which was removed in opentelemetry 0.31. Compilation failed with E0425.
- **Fix:** Used `std::sync::OnceLock<SdkTracerProvider>` to store the provider, then call `provider.shutdown()` directly in `shutdown_tracing()`.
- **Files modified:** crates/boternity-observe/src/tracing_setup.rs
- **Verification:** `cargo check -p boternity-observe` passes
- **Committed in:** b059ed2 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug: API change in opentelemetry 0.31)
**Impact on plan:** Necessary fix for correctness with current opentelemetry version. No scope creep.

## Issues Encountered
None beyond the API change documented in deviations.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- boternity-observe is ready to be wired into CLI and API entry points
- All workspace dependencies are available for downstream plans (LLM provider, chat UI, session persistence)
- GenAI attribute constants ready for use in LlmProvider tracing spans

## Self-Check: PASSED

---
*Phase: 02-single-agent-chat-llm*
*Completed: 2026-02-11*
