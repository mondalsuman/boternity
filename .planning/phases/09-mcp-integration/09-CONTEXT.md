# Phase 9: MCP Integration - Context

**Gathered:** 2026-02-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Bots participate in the MCP ecosystem bidirectionally: they consume external MCP tools/resources/prompts to extend their capabilities and expose themselves as MCP servers so external tools (like Claude Code) can use bots as tools. Includes full MCP spec compliance (tools, resources, prompts, sampling), security sanitization, and management interface. Workflow-level MCP orchestration belongs in Phase 8; observability dashboards belong in Phase 10.

</domain>

<decisions>
## Implementation Decisions

### Tool consumption UX
- Auto-discover tools/resources/prompts on MCP server connect (no manual approval step)
- Full MCP spec compliance: tools, resources, prompts, and sampling all supported
- Multiple MCP servers per bot simultaneously (tools from all servers available together)
- Transport support: stdio + SSE + Streamable HTTP for connecting to external servers
- Global MCP server pool with per-bot overrides (`bnity mcp add` without --bot for global, `--bot slug` for per-bot)
- Managed lifecycle for stdio servers: bot starts/stops server processes automatically
- Graceful degradation on disconnect: bot continues without tools, mentions unavailability if attempted
- Full tool results persisted in chat history for context continuity
- Inline collapsible tool call blocks in chat (both CLI and web UI), with syntax highlighting for JSON input/output
- Sampling supported: MCP servers can request LLM completions from the bot

### Bot-as-server exposure
- Full capability surface exposed via MCP: chat, management, memory access, skill invocation, workflow triggering
- Single MCP server process exposes all bots, tools namespaced by bot
- Server transport: stdio + Streamable HTTP
- Everything via tool calls (fine-grained tools), no MCP resource exposure from server side
- MCP prompts exposed: bot's skills and common use cases surfaced as MCP prompt templates
- Dedicated command: `bnity mcp serve` to start the MCP server
- Push notifications: server pushes events (bot status changes, new messages, workflow completions) to MCP clients
- Streaming responses supported for chat tool calls via MCP
- Tool annotations (readOnlyHint, destructiveHint, etc.) applied to all exposed tools

### MCP management interface
- Hybrid storage: connection metadata in SQLite, server configs in JSON file
- Full CLI command surface: `bnity mcp add/remove/list/status/connect/disconnect`
- Same `add` command for global (no --bot) and per-bot (--bot slug) connections
- Dedicated MCP tab per bot in web UI showing connected servers, available tools, connection status
- Browsable tool inventory: show all tools/resources from each connected server with descriptions and input schemas
- `bnity mcp test-tool` command for testing MCP tool calls outside of chat sessions
- Periodic background health pings on connected servers, status surfaced in UI and CLI
- Tool usage audit log visible in MCP tab
- Server presets for common MCP servers (filesystem, GitHub, Slack, etc.) with pre-filled config for quick-add
- Hot connect/disconnect: add or remove MCP servers while bot is running, changes take effect immediately

### Security & authentication
- API key / bearer token auth for incoming MCP server connections (bot-as-server)
- Reject unauthenticated connections entirely: zero anonymous access
- Per-server credentials for outgoing MCP client connections, stored in separate MCP keystore (not shared vault)
- Tool description sanitization: strip HTML/markdown injection vectors, escape special characters, truncate overly long descriptions
- Tool result sanitization: same rigor applied to results before entering bot context
- Sanitization produces logged warnings when content is modified (before/after for debugging)
- Permission scopes on MCP server connections: user can restrict which tools the bot is allowed to call per server
- Configurable rate limits on MCP server (bot-as-server) side, per-client and global
- Per-server sampling budget: configurable token budget for sampling requests to prevent runaway costs
- Separate MCP audit table for all MCP activity (both client and server side)

### Claude's Discretion
- MCP protocol version negotiation and capability advertisement
- Internal architecture for MCP client/server (trait design, connection pooling)
- Sanitization regex/patterns for tool descriptions and results
- Health ping interval and reconnection backoff strategy
- Server preset database format and bundled presets selection
- Rate limiting algorithm (token bucket, sliding window, etc.)

</decisions>

<specifics>
## Specific Ideas

- Tool call blocks in web UI should use collapsible code blocks with syntax highlighting (similar to Claude Code's tool use display)
- `bnity mcp serve` as dedicated subcommand (not integrated into existing `bnity serve`)
- `bnity mcp add` without --bot flag adds to global pool, with --bot slug for per-bot scoping
- Full MCP spec compliance is desired: tools + resources + prompts + sampling on client side; tools + prompts + notifications on server side
- Server presets for quick onboarding with popular MCP servers

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope

</deferred>

---

*Phase: 09-mcp-integration*
*Context gathered: 2026-02-14*
