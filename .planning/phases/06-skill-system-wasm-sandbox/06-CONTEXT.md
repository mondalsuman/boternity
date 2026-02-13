# Phase 6: Skill System + WASM Sandbox - Context

**Gathered:** 2026-02-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Extend agents with modular skills: define skills following skills.sh/Claude skills conventions, execute local skills with shell access, sandbox untrusted registry skills in WASM, discover and install skills from pluggable registries, enforce fine-grained permissions with trust tiers, and support skill inheritance via mixin composition. Includes both CLI (with interactive TUI browser) and web UI for skill management. Builder-powered skill creation is Phase 7. Workflow-level skill composition is Phase 8.

</domain>

<decisions>
## Implementation Decisions

### Skill definition & format
- Follow skills.sh / Claude skills conventions for format, parameters, and progressive capability discovery
- Two skill types supported: prompt-based (system prompt injection) and tool-based (callable functions with structured I/O)
- Unified format for all skills (local and registry) — same manifest structure, publish-ready by default
- Global skill library (`~/.boternity/skills/`) with per-bot configuration referencing attached skills and overrides
- Per-bot skill config supports overrides and enable/disable toggles per skill
- Skills declare dependencies on other skills; installing one auto-resolves the dependency tree
- Semantic versioning for skills; bots pin to specific versions
- Tool-based skills return structured JSON; prompt-based skills produce natural language in conversation
- Skills can declare `conflicts_with` to prevent incompatible skills on the same bot

### Permission & trust model
- Three trust tiers: local (full trust, shell access), verified registry (relaxed sandbox), untrusted registry (strict WASM sandbox)
- Fine-grained capabilities: specific operations (read-file, write-file, http-get, http-post, exec-command, read-env, etc.)
- Install-time approval: user reviews and approves all required capabilities during installation, no runtime prompts
- Granular revocation: individual capabilities can be revoked, skill degrades gracefully
- Permission violations terminate skill execution immediately (strict enforcement)
- Full audit logging: every skill invocation logged with capabilities used, inputs, outputs, duration
- Users can escalate trust tier (e.g., treat registry skill as local) with clear warnings
- Defense-in-depth: WASM sandbox inside OS-level sandbox (seccomp/seatbelt) — double barrier
- Configurable resource limits (CPU, memory, I/O) per trust tier, overridable per-skill
- Skills get read-only access to bot memory (no writes)
- Skills can access declared secrets (listed in manifest, approved at install, injected at runtime)
- Enable/disable toggle per-bot — skills stay installed but can be deactivated

### Registry & discovery UX
- Pluggable registry system: skills.sh and ComposioHQ as defaults, users can add custom registry endpoints
- Interactive TUI browser for CLI skill discovery (categories, search, previews)
- Full detail skill preview: name, description, author, version, trust tier, capabilities, dependencies, install count, ratings
- Semver-based update policy: patches auto-update, minor/major require manual approval
- `bnity skill publish` command for publishing local skills with validation, signing, and upload
- Both CLI and web UI for skill management (matching 06-06 plan scope)
- Clear source badges showing registry source and trust tier prominently
- Local cache for offline use — installed skills work offline, registry only needed for discovery/updates
- Dependency conflicts fail with clear explanation — user resolves manually
- Predefined categories for structured browsing across CLI TUI and web UI

### Skill inheritance & composition
- Mixin/composition model — child composes parent's capabilities additively, no overriding
- Max 3 levels of inheritance depth (skill, parent, grandparent)
- Multiple parent composition — a skill can mix in capabilities from several parents
- Last-wins ordering for capability conflicts when composing multiple parents
- Child inherits parent's capability permissions — combined set approved at install
- Automatic skill chaining at runtime — agent can invoke multiple skills in sequence, output feeding next input
- `bnity skill inspect <name>` shows resolved capability set after all inheritance (also in web UI)
- Circular inheritance detected and prevented at install time with clear cycle error
- Parent update triggers re-validation of dependent children with compatibility warnings
- Explicit `conflicts_with` declarations enforced across inheritance chains

### Claude's Discretion
- Prompt-based skill integration point in system prompt (how/where skills inject into the prompt structure)
- Exact manifest format details (YAML vs TOML, field naming)
- Internal architecture for WASM runtime (Wasmtime configuration specifics)
- OS-level sandbox implementation details (seccomp vs seatbelt per platform)
- Skill chaining orchestration internals
- TUI browser library choice and interaction design

</decisions>

<specifics>
## Specific Ideas

- "Similar concept like skills.sh skills" — the format, parameter handling, and progressive discovery should feel native to the skills.sh ecosystem
- "It should be exactly like claude skills or like skills.sh skills" — follow established conventions, don't reinvent
- Interactive TUI browser for skill discovery (like a mini package manager)
- Defense-in-depth is a hard requirement from the roadmap success criteria — WASM + OS sandbox

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-skill-system-wasm-sandbox*
*Context gathered: 2026-02-13*
