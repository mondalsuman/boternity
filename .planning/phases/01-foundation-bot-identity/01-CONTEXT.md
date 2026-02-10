# Phase 1: Foundation + Bot Identity - Context

**Gathered:** 2026-02-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Monorepo scaffold, crate structure, SQLite storage, bot CRUD with immutable SOUL.md, secrets vault, basic CLI (`bnity`) and REST API. Users can create bots with distinct identities, manage them, and store secrets securely. All running locally for a single user, with the data model prepared for multi-user later.

</domain>

<decisions>
## Implementation Decisions

### Bot identity model

**Three-file split:**
- SOUL.md = who the bot IS (personality, values, voice, boundaries) — **immutable at runtime**, SHA-256 verified at startup
- IDENTITY.md = system config (model, provider, temperature, max tokens, rate limits, response format, context window settings) — mutable, full config surface with best-practice defaults, all settings overridable at invocation time
- USER.md = user-authored briefing doc (preferences, standing instructions, important context) — curated, never auto-populated with session dumps; session memory is a separate system (Phase 2); promoting info from sessions to USER.md is an explicit user action

**SOUL.md format:**
- YAML frontmatter (name, traits, tone) + free-form markdown body
- Minimal frontmatter: structured data only (name, personality traits list, communication tone)
- Everything else (boundaries, expertise, backstory) lives in the markdown body

**Default soul:**
- When created with just a name, bot gets a pre-filled human-like personality (not a generic bot/assistant persona)
- Guided template with commented-out sections showing what to customize

**Visual identity:**
- Each bot has: avatar (image or generated), accent color, and emoji
- Stored in IDENTITY.md
- Displayed in CLI output and later in web dashboard

**Bot naming:**
- Display name is freeform (duplicates allowed)
- Auto-generated slug from name ("Research Assistant" -> "research-assistant"), enforced unique
- Short description is a required field (1-2 sentences for listings)

**Organization:**
- System categories (assistant, creative, research, utility) + freeform user tags
- Categories in IDENTITY.md, tags user-managed

**Lifecycle states:**
- Active (running, fully functional)
- Disabled (paused — visible but can't chat)
- Archived (hidden from default views, all data preserved, restorable)
- Delete = permanent purge of all data after confirmation

**Versioning:**
- All SOUL.md versions kept forever — full history with diffs
- Integrity check at bot startup: SHA-256 hash mismatch = **hard block, bot refuses to start**, clear error message

**Cloning:**
- Users can clone a bot: copies soul + config, not history/memories
- Templates deferred to Phase 10 per roadmap

**Metadata tracked per bot (system-managed):**
- created_at, updated_at, last_active_at
- conversation_count, total_tokens_used, version_count

**Ownership:**
- Single user locally (no auth), but data model includes user_id fields for future multi-user support

**File layout:**
- One directory per bot: ~/.boternity/bots/{slug}/
- SOUL.md, IDENTITY.md, USER.md inside each bot directory

**Minimum to create a bot:**
- Just a name — everything else gets sensible defaults

### CLI interaction style

**Command name:** `bnity` (short form of boternity)

**Command structure:** Verb-noun (like Docker)
- `bnity create bot`, `bnity list bots`, `bnity show <bot>`, `bnity delete bot <slug>`
- `bnity set secret`, `bnity list secrets`
- `bnity check <bot>` — health check (secrets, config, API keys)
- `bnity status` — system dashboard (bot counts, storage, API health, version)

**Aliases:** Short aliases for common operations
- `bnity ls` (list bots), `bnity rm` (delete), `bnity new` (create)

**Output style:**
- Styled human output: colored text, tables, emojis, progress spinners
- `--json` flag for machine-readable output
- `--quiet` for errors-only (scripts/CI)
- Auto-detect TTY vs pipe: colors when interactive, plain when piped
- Bot emoji displayed next to bot name in all output

**Bot listing:** Rich colored table showing name, status, description, last active, model

**Bot detail (`bnity show`):** Full profile view — name, description, status, soul preview, config summary, stats, timestamps

**Creation flow:**
- Bare `bnity create bot` launches interactive wizard (step-by-step prompts)
- Providing flags skips the wizard (one-shot)
- After wizard basics, offers: continue inline OR open SOUL.md in $EDITOR
- Wizard default, flags override — best of both worlds

**Confirmations:** Always confirm destructive actions. `--force` to skip.

**Verbosity:** Four levels
- `--quiet` (errors only)
- Default (normal output)
- `--verbose` / `-v` (detailed info)
- `--debug` / `-vv` (trace-level for development)

**Help system:** Examples in every command's --help (2-3 usage examples per command)

**Shell completions:** bash, zsh, fish from day one. Auto-complete commands, bot names, flags.

**Global config:** `~/.boternity/config.toml` for user preferences (default model, output format, color theme)

### Secrets handling

**Scope:** Global secrets as default + per-bot overrides
- Environment variables override everything
- Precedence: env vars > per-bot keys > global vault

**Storage backends (all three supported):**
- Encrypted vault file (~/.boternity/vault.enc) — machine-derived key, zero friction
- OS keychain (macOS Keychain / Linux Secret Service)
- Environment variables
- User picks preferred backend in config

**UX:**
- `bnity set secret KEY` prompts with hidden input (secure, never in shell history)
- `--value` flag available for scripts/automation
- `bnity list secrets` shows key names + masked last 4 chars: "ANTHROPIC_API_KEY: ****r3xk"
- Validation on set: lightweight API call to verify key works, immediate feedback

**Rotation:** Simple overwrite — `bnity set secret KEY` replaces the old value

**Health check:** `bnity check <bot>` validates required secrets present, API keys valid, config complete

### API design conventions

**Style:** Standard REST, versioned
- `/api/v1/bots`, `/api/v1/bots/:id`, `/api/v1/bots/:id/soul`, etc.

**Authentication:** API key auth required on every request from day one
- Key generated on first run

**Response format:** Envelope on every response
```json
{
  "data": { ... },
  "meta": { "request_id": "...", "timestamp": "..." },
  "errors": [],
  "_links": { "self": "...", "soul": "...", "secrets": "..." }
}
```

**Error format:** Structured with machine-readable codes
```json
{
  "error": {
    "code": "BOT_NOT_FOUND",
    "message": "Bot 'luna' not found",
    "details": { ... }
  }
}
```

**Content types:** JSON (default) + CBOR for performance-sensitive clients. Content negotiation via Accept header.

**Features:**
- HATEOAS-style `_links` in every response for discoverability
- Sparse fieldsets: `?fields=name,status,created_at`
- Filtering + sorting on list endpoints: `?status=active&sort=created_at&order=desc`
- Full tracing headers: X-Request-Id, X-Response-Time on every response
- Auto-generated OpenAPI spec with Swagger UI at `/api/docs`
- No rate limiting in Phase 1 (local-only tool)

### Claude's Discretion
- Pagination style (cursor vs offset) for list endpoints
- Secret export/import for migration (whether to include in Phase 1)
- Secret reference syntax in config files (template vs convention)
- Configurable data directory (BOTERNITY_HOME env var)
- Exact avatar generation approach
- Default category assignments for new bots
- Internal crate architecture and module boundaries
- SQLite schema design
- Repository trait abstraction patterns

</decisions>

<specifics>
## Specific Ideas

- Bots should feel like people, not tools — default personality is human-like, never generic bot/assistant
- CLI should feel polished: styled output, emojis, spinners, rich tables — a joy to use in the terminal
- `bnity status` as a terminal dashboard — quick system health overview
- `bnity check <bot>` as a doctor command — validates everything is configured correctly
- SOUL.md integrity is non-negotiable: hash mismatch = hard block at startup, no exceptions
- USER.md stays clean — never auto-polluted with session history; session memory is a separate concern
- Shell completions from day one — power user experience matters

</specifics>

<deferred>
## Deferred Ideas

- Bot templates (pre-built personalities for common use cases) — Phase 10
- Multi-user auth and bot ownership/permissions — future milestone
- Rate limiting — add when multi-user comes
- Scheduled secret rotation — future phase

</deferred>

---

*Phase: 01-foundation-bot-identity*
*Context gathered: 2026-02-10*
