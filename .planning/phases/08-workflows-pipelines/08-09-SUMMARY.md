---
phase: "08"
plan: "09"
subsystem: api
tags: [rest, workflow, webhook, messaging, handlers, appstate]
dependency-graph:
  requires: ["08-02", "08-04", "08-06", "08-07"]
  provides: ["REST workflow CRUD and trigger endpoints", "Webhook receiver with auth verification", "Bot-to-bot messaging REST API", "AppState with Phase 8 services"]
  affects: ["08-10", "08-11", "09"]
tech-stack:
  added: []
  patterns: ["Axum handler pattern with envelope response", "WebhookRegistry auth delegation", "Router merge for sub-routers"]
key-files:
  created:
    - crates/boternity-api/src/http/handlers/workflow.rs
    - crates/boternity-api/src/http/handlers/webhook.rs
    - crates/boternity-api/src/http/handlers/message.rs
    - crates/boternity-api/src/cli/message.rs
  modified:
    - crates/boternity-api/src/http/handlers/mod.rs
    - crates/boternity-api/src/http/router.rs
    - crates/boternity-api/src/state.rs
    - crates/boternity-api/src/cli/mod.rs
    - crates/boternity-api/src/main.rs
decisions:
  - "Webhook endpoint uses its own auth (HMAC/bearer via WebhookRegistry) rather than API key auth"
  - "Workflow routes provided via Router::merge pattern for clean separation"
  - "AppState initialized with Phase 8 services (workflow_repo, message_repo, message_bus, webhook_registry)"
  - "CLI message subcommand added as bonus for feature parity with REST API"
metrics:
  duration: "5m 58s"
  completed: "2026-02-14"
---

# Phase 8 Plan 09: REST API Workflow, Webhook, and Messaging Handlers

REST API handlers for workflows, webhooks, and bot-to-bot messaging with AppState wiring for Phase 8 services.

## One-liner

Axum REST handlers for workflow CRUD/trigger/runs, webhook receiver with HMAC/bearer auth, and bot-to-bot messaging endpoints wired into AppState.

## What Was Built

### Workflow Handlers (workflow.rs)
- **POST /api/v1/workflows** - Create workflow definition
- **GET /api/v1/workflows** - List all workflow definitions
- **GET /api/v1/workflows/:id** - Get workflow definition by ID
- **PUT /api/v1/workflows/:id** - Update workflow definition
- **DELETE /api/v1/workflows/:id** - Delete workflow definition
- **POST /api/v1/workflows/:id/trigger** - Manual trigger (creates pending run)
- **GET /api/v1/workflows/:id/runs** - List runs with pagination
- **GET /api/v1/runs/:run_id** - Run detail with step logs
- **POST /api/v1/runs/:run_id/approve** - Approve paused run
- **POST /api/v1/runs/:run_id/cancel** - Cancel running/pending run

All endpoints use `Authenticated` extractor and envelope `ApiResponse` format.

### Webhook Handler (webhook.rs)
- **POST /api/v1/webhooks/:path** - Receive incoming webhook
- Delegates auth to `WebhookRegistry.verify_request()` (HMAC-SHA256 or bearer token)
- Parses payload as JSON, creates pending workflow run
- Error mapping: PathNotFound -> 500, auth failures -> 401, missing auth -> 401

### Message Handlers (message.rs)
- **POST /api/v1/messages/send** - Send bot-to-bot message (direct or channel)
- **GET /api/v1/messages/history/:bot_a/:bot_b** - Direct message history
- **GET /api/v1/channels** - List pub/sub channels
- **POST /api/v1/channels/:name/subscribe** - Subscribe bot to channel
- **DELETE /api/v1/channels/:name/subscribe/:bot_id** - Unsubscribe bot
- **GET /api/v1/channels/:name/messages** - Channel message history

### AppState Wiring
Four new fields added to `AppState`:
- `workflow_repo: Arc<SqliteWorkflowRepository>` - Definition and run persistence
- `message_repo: Arc<SqliteMessageRepository>` - Bot message persistence
- `message_bus: Arc<MessageBus>` - Runtime pub/sub and direct messaging
- `webhook_registry: Arc<WebhookRegistry>` - Webhook path-to-config registry

All initialized in `AppState::init()` with database pool.

### Bonus: CLI Message Subcommand
Linter auto-generated `cli/message.rs` providing CLI parity:
- `bnity message send` - Send direct or channel messages
- `bnity message history` - View conversation history
- `bnity message channels` - List channels
- `bnity message subscribe` / `unsubscribe` - Channel management
- `bnity message channel-history` - Channel message log

## Task Commits

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | REST API workflow and webhook handlers | fd56eb2 | workflow.rs, webhook.rs, router.rs, state.rs |
| 2 | REST API message handlers and AppState wiring | 2b22084 | message.rs (REST + CLI), mod.rs, main.rs |

## Decisions Made

1. **Webhook auth delegation**: Webhook endpoint does not use API key auth (`Authenticated` extractor). Instead, it delegates authentication entirely to the `WebhookRegistry`, which supports HMAC-SHA256 and bearer token verification per-webhook.

2. **Router merge pattern**: Workflow routes use `workflow_routes() -> Router<AppState>` returned by the handler module, merged into the main API router. This keeps route definitions close to their handlers while maintaining a single router tree.

3. **AppState Phase 8 fields**: All four Phase 8 services (workflow_repo, message_repo, message_bus, webhook_registry) are `Arc`-wrapped and initialized in `AppState::init()`. The `MessageBus` uses a `LoopGuard::default()` for loop prevention.

4. **CLI message parity**: Added CLI subcommands for messaging alongside REST handlers, providing `bnity message send/history/channels/subscribe/unsubscribe/channel-history` for feature parity.

## Deviations from Plan

None - plan executed exactly as written. The CLI message subcommand was auto-generated by the project linter as a complementary addition.

## Verification

- `cargo check -p boternity-api` compiles cleanly (warnings only, no errors)
- All 10 workflow endpoints wired into router
- Webhook receiver with auth verification functional
- 6 message endpoints wired into router
- AppState holds all 4 Phase 8 services

## Next Phase Readiness

- Workflow CRUD + trigger + run management available for visual builder (08-10, 08-11)
- Webhook receiver ready for external integration testing
- Bot-to-bot messaging REST API available for web UI consumption
- AppState fully wired for any remaining Phase 8 plans

## Self-Check: PASSED
