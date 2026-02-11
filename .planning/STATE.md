# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-10)

**Core value:** A user can create a bot with a distinct identity, give it skills through an interactive builder, and have meaningful parallel conversations with it -- all running locally with full observability.
**Current focus:** Phase 2 (Single-Agent Chat + LLM) - In progress

## Current Position

Phase: 2 of 10 (Single-Agent Chat + LLM)
Plan: 4 of 8 in current phase (02-01, 02-02, 02-03, 02-04 complete)
Status: In progress
Last activity: 2026-02-11 -- Completed 02-04-PLAN.md (SQLite chat and memory persistence)

Progress: [██████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 10/53 (~19%)

## Performance Metrics

**Velocity:**
- Total plans completed: 10
- Average duration: 6m 18s
- Total execution time: 66m 0s

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1. Foundation + Bot Identity | 6/6 | 49m 14s | 8m 12s |
| 2. Single-Agent Chat + LLM | 4/8 | 16m 46s | 4m 12s |

**Recent Trend:**
- Last 5 plans: 02-02 (2m 46s), 02-01 (5m 0s), 02-03 (5m 0s), 02-04 (4m 0s)
- Trend: Phase 2 plans executing faster than Phase 1

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Roadmap]: 10 phases derived from 109 requirements following dependency chain: types -> core -> infra -> api
- [Roadmap]: SOUL.md immutability enforced from Phase 1 (CVE-2026-25253 mitigation)
- [Roadmap]: boternity-core must never depend on boternity-infra (dependency inversion)
- [Roadmap]: Security concerns front-loaded into the phase where their attack surface first appears
- [01-01]: Rust 2024 edition with resolver 3 and native async fn in traits (RPITIT, no async_trait)
- [01-01]: UUID v7 for all entity IDs (time-sortable, process-local ordering)
- [01-01]: BotStatus: Active/Disabled/Archived (lifecycle states from CONTEXT.md)
- [01-01]: Identity defaults: claude-sonnet-4-20250514, temperature 0.7, max_tokens 4096
- [01-01]: Redacted wrapper pattern for secret values (custom Debug/Display)
- [01-01]: Repository traits return impl Future (RPITIT) not Box<dyn Future>
- [01-02]: Split read/write SQLite pools (8 readers, 1 writer) with WAL mode on both
- [01-02]: Private BotRow struct for SQLite-to-domain mapping (no sqlx derives on domain types)
- [01-02]: Secrets scope stored as string not FK (allows pre-provisioned keys)
- [01-02]: Sort field whitelist in list() to prevent SQL injection
- [01-02]: Transaction for soul save (INSERT + UPDATE version_count atomically)
- [01-03]: Generic services (BotService<B, S, F, H>) over trait objects -- RPITIT traits not object-safe
- [01-03]: Free functions for content generation (generate_default_soul, etc.) -- no trait bounds needed for static calls
- [01-03]: Simple line-based YAML frontmatter parser -- avoids serde_yaml dep for narrow use case
- [01-03]: LocalFileSystem auto-creates parent dirs on write -- prevents missing dir errors
- [01-04]: BoxSecretProvider with blanket impl for object-safe dynamic dispatch of RPITIT traits
- [01-04]: Fixed Argon2id salt "boternity-vault-v1" for password KDF (password provides entropy)
- [01-04]: Auto-generated master key in OS keychain as zero-friction default
- [01-04]: Secret<T> generic wrapper alongside existing Redacted(String)
- [01-06]: LCS-based line diff in pure Rust (no external diff library)
- [01-06]: Message field on Soul struct for version commit messages
- [01-06]: update_soul saves DB first then file (DB failure leaves disk unchanged)
- [01-06]: bnity check enhanced with soul integrity verification
- [02-01]: MessageRole defined in llm.rs, re-exported from chat.rs (single source of truth)
- [02-01]: stream() returns Pin<Box<dyn Stream>> not RPITIT (needs object safety for BoxLlmProvider)
- [02-01]: BoxLlmProvider follows same LlmProviderDyn blanket impl pattern as BoxSecretProvider
- [02-01]: ContextSummary on ChatRepository not MemoryRepository (session-scoped)
- [02-01]: TokenBudget allocation: soul 15%, memory 10%, user_context 5%, conversation 70%
- [02-01]: Summarization triggers at 80% of conversation budget
- [02-02]: OnceLock for OTel provider storage -- opentelemetry 0.31 removed global shutdown, store in OnceLock
- [02-02]: stdout exporter for dev -- opentelemetry-stdout for local development, swappable for OTLP
- [02-03]: SSE event dispatch via match on event type string, not serde tag on outer enum
- [02-03]: Model capabilities derived from model name substring matching (sonnet/opus/haiku)
- [02-03]: Empty tool use JSON buffer produces empty JSON object (not null or parse error)
- [02-03]: AnthropicProvider does not derive Debug (defense-in-depth for API key)
- [02-04]: save_message atomically increments session message_count (prevents drift)
- [02-04]: get_pending_extractions filters attempt_count < 3 (max retry policy in query)
- [02-04]: ON DELETE CASCADE on chat_sessions cascades to messages and summaries (not memories)

### Pending Todos

None yet.

### Blockers/Concerns

- [Research]: Dual-GraphQL architecture (Yoga+Pothos BFF vs async-graphql alone) needs validation in Phase 4
- [Research]: `llm` crate (graniet) v1.2.4 is newer -- may need fallback to thin reqwest wrapper if API unstable
- [Research]: LanceDB vs sqlite-vec decision deferred to Phase 3 planning

## Session Continuity

Last session: 2026-02-11T22:56:08Z
Stopped at: Completed 02-04-PLAN.md (SQLite chat and memory persistence)
Resume file: None
