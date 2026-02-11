---
phase: 01-foundation-bot-identity
plan: 05
status: complete
commits: ["e92a86b", "4d482be", "4b1479e", "3d4507d", "64e0c40", "56ff5df"]
tests_passing: 116
---

## Summary

Implemented the `bnity` CLI binary and REST API for Boternity.

### CLI (`bnity`)
- **Bot lifecycle**: create (interactive wizard + flags), list (colored table with slug column), show (full profile), delete (with confirmation), clone
- **Secrets**: set (hidden input prompt), list (masked values table)
- **System**: status (health summary), serve (starts REST API), completions (shell autocomplete)
- Styled output with emoji, colors, spinners (console, comfy-table, indicatif, dialoguer)

### REST API (Axum 0.8)
- Envelope response format: `{ data, meta: { request_id, timestamp, response_time_ms }, _links }`
- API key auth via `Authorization: Bearer` or `X-API-Key` header (SHA-256 hashed, stored in SQLite)
- Routes: bot CRUD, soul read + versions, secrets CRUD, health check
- CORS enabled, tracing middleware

### Bugs Fixed During Checkpoint
1. **KeychainProvider intercepting writes** — set() succeeded before VaultSecretProvider, so secrets went to macOS Keychain (which can't enumerate). Made keychain read-only.
2. **Repeated keychain password prompts** — Removed KeychainProvider from secret chain entirely.
3. **Vault key via keychain caused per-command prompts** — Switched to file-based vault key (`vault.key` with 0600 permissions).
4. **Clone bot soul version conflict** — `write_and_save_soul` hardcoded version 1; now queries max version and increments.
5. **Duplicate bot names indistinguishable** — Added Slug column to `list bots` table.
