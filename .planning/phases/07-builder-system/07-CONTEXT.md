# Phase 7: Builder System - Context

**Gathered:** 2026-02-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Interactive guided agent and skill creation powered by a universal builder agent ("Forge"). Users create fully-configured bots and skills through CLI wizard or web UI (chat-based builder bot + step-by-step wizard). The builder adapts question depth to complexity, remembers past sessions, supports reconfiguration of existing bots, and can create/modify skills following the agentskills.io spec. Workflow automation, MCP integration, and observability dashboards are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Question Flow & Adaptiveness
- Depth detection: hybrid approach — purpose-based heuristic categorization (simple utility, complex analyst, creative, etc.) combined with LLM-driven judgment from the user's initial description
- Input style: multi-choice options with an 'Other' free-text escape hatch on every question
- Question generation: fully dynamic — LLM generates each next question based on all context so far, no fixed question skeleton
- Vague input handling: depth-dependent — simple agents get smart defaults + confirmation, complex agents get probing follow-up questions
- Question cap: soft guidance (aim for brevity), no hard maximum — LLM judges when enough context is gathered
- Option explanations: brief context on every option ("Formal tone — best for professional/enterprise use cases")
- Progress indication: phase labels shown during flow ("Setting up basics..." → "Defining personality..." → "Choosing skills...")
- Reconfigure mode: show current config and ask "What would you like to adjust?" (not re-walk the full flow)
- Batch creation: supported with shared base config — builder asks "What makes this variant different?" for each variant
- Transition to assembly: explicit confirmation — builder shows full summary and asks "Ready to create?" before building anything
- Builder memory: remembers past builder sessions and suggests similar choices ("Last time you made a coding bot, you chose formal tone — same here?")

### Builder Personality & UX
- Tone: friendly guide — warm, encouraging, slightly casual, like a helpful teammate walking through setup
- Live preview: shown after each phase label (basics, personality, skills) — growing preview of what's configured so far
- Web UI: both surfaces available — chat-based builder bot (Forge) AND step-by-step wizard overlay
- Web wizard structure: step-by-step pages (Basics → Personality → Model → Skills → Review) with back/next navigation
- Surface adaptation: same core builder agent but adapted per surface — CLI gets compact output, web gets richer UI (dropdowns, previews, inline help)
- Builder bot identity: named character "Forge" with its own avatar and SOUL.md personality
- Undo/back: full back navigation — user can go back to any previous phase and change answers, subsequent answers re-evaluated
- CLI interaction: interactive numbered list with arrow-key selection (inquire/dialoguer style)
- Entry points: wizard accessible from dashboard ("Create Bot" button) and bot detail page ("Reconfigure" button); Forge chat bot always available
- Skill suggestions: top 3-5 relevant suggestions highlighted with "browse all" option to see the full catalog
- Reasoning: always explain suggestions — every recommendation comes with a brief reason ("I'm suggesting web-search because you mentioned research tasks")

### Output & Assembly
- Artifacts: full bot created — SOUL.md + IDENTITY.md + USER.md + attached skills
- Review depth: structured summary by default with "Show raw files" toggle to see actual generated content
- Soul generation: LLM writes unique soul content following structural templates — consistent Personality + Purpose + Boundaries sections, unique content per bot
- Identity config: smart defaults based on purpose (coding bot = low temp, creative = high temp) with user override option; always ask model choice
- USER.md: seeded with initial user context inferred from the builder conversation
- Post-create: immediately open chat with the newly created bot
- Soul import: supported — user can paste existing SOUL.md content or provide file path as starting point
- Missing provider: warn and offer inline provider setup ("You haven't configured Anthropic yet — set it up now?")
- Draft saving: auto-save builder progress — interrupted sessions can be resumed
- CLI post-create output: detailed summary — bot name, slug, file paths, attached skills, model, everything at a glance

### Skill Creation via Builder
- Skill input: natural language description + guided refinement follow-ups to fill gaps
- Skill types: builder can create both local (trusted) and WASM (sandboxed) skills
- Auto-attach: skills created during bot builder flow are auto-attached, removable at review step
- Standalone mode: separate "Create Skill" flow available — not only during bot creation
- Code generation: full source code generated for WASM skills (Rust as default language), not just manifests
- Validation: builder compiles/runs a basic test of the skill before presenting final review
- CLI path: both Forge chat and `bnity skill create` CLI command available for standalone skill creation
- Permissions: auto-suggest capabilities based on skill description, user confirms ("This skill needs network access and file read")
- Existing skills: in reconfigure mode, show current skills and suggest complementary additions
- Origin tracking: builder-created skills get a `builder-created` metadata tag
- Skill editing: Forge can modify/update existing skills — full lifecycle management (create, modify, update)

### Claude's Discretion
- Exact phase label wording for progress indication
- Builder memory storage format and retrieval strategy
- Forge's avatar design and exact SOUL.md content
- Draft auto-save interval and storage mechanism
- Specific smart default values for temperature/max_tokens per purpose category
- Test execution strategy for skill validation (compile-only vs. runtime test)

</decisions>

<specifics>
## Specific Ideas

- Builder bot is named "Forge" — implies crafting/building, a distinct character with its own soul
- SOUL.md template follows three sections: Personality, Purpose, Boundaries — LLM fills in unique content per bot
- Batch creation works by defining a shared base, then asking "What makes this variant different?" per variant
- CLI uses inquire/dialoguer-style interactive selectors (arrow keys to highlight, enter to select)
- Web wizard is step-by-step pages (not single scrollable form) — Basics → Personality → Model → Skills → Review
- In reconfigure mode, builder shows existing skills and suggests complementary additions ("You have web-search, maybe add summarize?")
- Builder always explains its reasoning for suggestions — builds trust and teaches users what capabilities exist

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 07-builder-system*
*Context gathered: 2026-02-14*
