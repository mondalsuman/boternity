# Phase 7: Builder System - Research

**Researched:** 2026-02-14
**Domain:** LLM-driven interactive builder agent, multi-turn structured conversation, CLI wizard, web UI builder, adaptive question flows, artifact generation (SOUL.md, IDENTITY.md, USER.md, skills)
**Confidence:** HIGH (architecture patterns), HIGH (CLI stack), HIGH (structured output), MEDIUM (builder memory persistence), HIGH (draft saving)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Question Flow & Adaptiveness:**
- Depth detection: hybrid approach -- purpose-based heuristic categorization (simple utility, complex analyst, creative, etc.) combined with LLM-driven judgment from the user's initial description
- Input style: multi-choice options with an 'Other' free-text escape hatch on every question
- Question generation: fully dynamic -- LLM generates each next question based on all context so far, no fixed question skeleton
- Vague input handling: depth-dependent -- simple agents get smart defaults + confirmation, complex agents get probing follow-up questions
- Question cap: soft guidance (aim for brevity), no hard maximum -- LLM judges when enough context is gathered
- Option explanations: brief context on every option ("Formal tone -- best for professional/enterprise use cases")
- Progress indication: phase labels shown during flow ("Setting up basics..." -> "Defining personality..." -> "Choosing skills...")
- Reconfigure mode: show current config and ask "What would you like to adjust?" (not re-walk the full flow)
- Batch creation: supported with shared base config -- builder asks "What makes this variant different?" for each variant
- Transition to assembly: explicit confirmation -- builder shows full summary and asks "Ready to create?" before building anything
- Builder memory: remembers past builder sessions and suggests similar choices ("Last time you made a coding bot, you chose formal tone -- same here?")

**Builder Personality & UX:**
- Tone: friendly guide -- warm, encouraging, slightly casual, like a helpful teammate walking through setup
- Live preview: shown after each phase label (basics, personality, skills) -- growing preview of what's configured so far
- Web UI: both surfaces available -- chat-based builder bot (Forge) AND step-by-step wizard overlay
- Web wizard structure: step-by-step pages (Basics -> Personality -> Model -> Skills -> Review) with back/next navigation
- Surface adaptation: same core builder agent but adapted per surface -- CLI gets compact output, web gets richer UI (dropdowns, previews, inline help)
- Builder bot identity: named character "Forge" with its own avatar and SOUL.md personality
- Undo/back: full back navigation -- user can go back to any previous phase and change answers, subsequent answers re-evaluated
- CLI interaction: interactive numbered list with arrow-key selection (inquire/dialoguer style)
- Entry points: wizard accessible from dashboard ("Create Bot" button) and bot detail page ("Reconfigure" button); Forge chat bot always available
- Skill suggestions: top 3-5 relevant suggestions highlighted with "browse all" option to see the full catalog
- Reasoning: always explain suggestions -- every recommendation comes with a brief reason

**Output & Assembly:**
- Artifacts: full bot created -- SOUL.md + IDENTITY.md + USER.md + attached skills
- Review depth: structured summary by default with "Show raw files" toggle to see actual generated content
- Soul generation: LLM writes unique soul content following structural templates -- consistent Personality + Purpose + Boundaries sections, unique content per bot
- Identity config: smart defaults based on purpose (coding bot = low temp, creative = high temp) with user override option; always ask model choice
- USER.md: seeded with initial user context inferred from the builder conversation
- Post-create: immediately open chat with the newly created bot
- Soul import: supported -- user can paste existing SOUL.md content or provide file path as starting point
- Missing provider: warn and offer inline provider setup
- Draft saving: auto-save builder progress -- interrupted sessions can be resumed
- CLI post-create output: detailed summary

**Skill Creation via Builder:**
- Skill input: natural language description + guided refinement follow-ups to fill gaps
- Skill types: builder can create both local (trusted) and WASM (sandboxed) skills
- Auto-attach: skills created during bot builder flow are auto-attached, removable at review step
- Standalone mode: separate "Create Skill" flow available
- Code generation: full source code generated for WASM skills (Rust as default language), not just manifests
- Validation: builder compiles/runs a basic test of the skill before presenting final review
- CLI path: both Forge chat and `bnity skill create` CLI command available for standalone skill creation
- Permissions: auto-suggest capabilities based on skill description, user confirms
- Existing skills: in reconfigure mode, show current skills and suggest complementary additions
- Origin tracking: builder-created skills get a `builder-created` metadata tag
- Skill editing: Forge can modify/update existing skills

### Claude's Discretion
- Exact phase label wording for progress indication
- Builder memory storage format and retrieval strategy
- Forge's avatar design and exact SOUL.md content
- Draft auto-save interval and storage mechanism
- Specific smart default values for temperature/max_tokens per purpose category
- Test execution strategy for skill validation (compile-only vs. runtime test)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

## Summary

This phase builds the interactive builder system that powers agent and skill creation across CLI and web UI. The core architectural challenge is designing a **universal builder agent** ("Forge") that drives an LLM-powered multi-turn conversation, generating adaptive questions based on accumulated context, and ultimately assembling complete bot artifacts (SOUL.md, IDENTITY.md, USER.md, attached skills).

The standard approach uses Claude's **structured output** (`output_config.format` with JSON schema) to force the LLM to produce parseable question/answer structures on each turn, rather than free-text responses that need brittle parsing. The builder agent maintains a `BuilderState` that accumulates answers across turns. Each LLM call receives the full accumulated state plus the system prompt defining Forge's personality and builder instructions, and returns structured JSON with the next question (options, explanations, phase label) or a completion signal with the assembled configuration. The CLI surface uses `dialoguer::Select` for arrow-key multi-choice, while the web surface receives the same structured JSON and renders it as rich UI components.

The key architectural insight is the **surface adapter pattern**: the core `BuilderAgent` trait is surface-agnostic. It accepts user input (selected option index or free text) and returns a `BuilderTurn` (question with options, preview update, or completion). CLI and web adapters translate `BuilderTurn` into their respective UIs. This means the LLM interaction logic, state management, and artifact generation are written once.

**Primary recommendation:** Use Claude structured outputs (`output_config.format` with `json_schema`) to enforce parseable LLM responses on every builder turn. Use `dialoguer` (already in workspace) for CLI interaction. Store builder state as JSON in SQLite via the existing `SqliteKvStore`. Generate artifacts by calling the existing `BotService::create_bot` and `SoulService::write_and_save_soul` after assembly.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| dialoguer | 0.11 (already in workspace) | CLI multi-choice selection with arrow-key navigation | Already used for bot creation prompts; `Select`, `MultiSelect`, `Input`, `Confirm` cover all builder interactions |
| schemars | 1.x | JSON Schema generation from Rust structs | Derive `JsonSchema` on builder response types, feed schema to Claude's `output_config.format` for guaranteed structured output |
| serde_json | 1 (already in workspace) | JSON serialization for builder state and LLM structured output | Already in workspace; used for parsing structured LLM responses and persisting builder state |
| console | 0.15 (already in workspace) | Terminal styling for builder output | Already used for colored/styled CLI output in bot.rs |
| indicatif | 0.17 (already in workspace) | Progress spinners and phase indicators | Already used for creation spinners; reuse for "Setting up basics..." phase labels |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| sqlx | 0.8 (already in workspace) | Builder draft persistence via SqliteKvStore | Auto-saving builder progress; session drafts stored as JSON blobs in KV store |
| chrono | 0.4 (already in workspace) | Timestamps for draft sessions and builder memory | Draft created_at, last_modified, builder memory timestamps |
| uuid | 1.20 (already in workspace) | Session IDs for builder drafts | Each builder session gets a unique ID for draft persistence and resumption |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| dialoguer Select | inquire crate | inquire has richer built-in features (autocompletion, date pickers) but would add a new dependency; dialoguer is already in the workspace and sufficient for multi-choice + text input |
| schemars JSON Schema | Hand-written JSON schema strings | schemars auto-generates schemas from Rust types and stays in sync with serde; hand-written schemas drift and are error-prone |
| SQLite KV store for drafts | File-based JSON drafts | KV store is already available (SqliteKvStore), atomic, and supports TTL-based cleanup; file-based requires manual cleanup |
| Claude structured output | Tool use for structured extraction | Structured output (`output_config.format`) is cleaner for this use case -- we want formatted responses, not function calls. Tool use would work but adds unnecessary complexity for what is essentially "return this JSON shape" |

**Installation (new dependencies only):**
```bash
cargo add schemars@1 --features derive
```

All other dependencies are already in the workspace.

## Architecture Patterns

### Recommended Project Structure

```
crates/boternity-types/src/
  builder.rs              # BuilderState, BuilderTurn, BuilderPhase, BuilderConfig,
                          #   PurposeCategory, SmartDefaults, BuilderDraft

crates/boternity-core/src/
  builder/
    mod.rs                # Module re-exports
    agent.rs              # BuilderAgent trait -- surface-agnostic builder logic
    state.rs              # BuilderState accumulator -- tracks answers, phase, config-so-far
    assembler.rs          # BotAssembler -- generates SOUL.md, IDENTITY.md, USER.md from state
    defaults.rs           # SmartDefaults -- purpose-category-based default values
    memory.rs             # BuilderMemory -- recalls past sessions for suggestions
    prompt.rs             # Forge system prompt construction with builder instructions
    skill_builder.rs      # SkillBuilder -- natural language to SKILL.md + code generation

crates/boternity-infra/src/
  builder/
    mod.rs                # Module re-exports
    llm_builder.rs        # LlmBuilderAgent -- concrete implementation using AgentEngine
    draft_store.rs        # Draft persistence via SqliteKvStore
    memory_store.rs       # Builder memory persistence via SqliteKvStore

crates/boternity-api/src/
  cli/
    builder.rs            # CLI wizard surface adapter -- dialoguer-based interaction loop
    skill_create.rs       # `bnity skill create` standalone CLI flow
  http/
    handlers/
      builder.rs          # REST API handlers for web builder (question/answer exchange)
      builder_ws.rs       # WebSocket handler for real-time builder chat (Forge bot)
```

### Pattern 1: Structured Output for Builder Turns

**What:** Use Claude's `output_config.format` with JSON schema to guarantee parseable builder responses on every LLM call.
**When to use:** Every builder turn where the LLM generates the next question or signals completion.

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The LLM's response for a single builder turn.
/// Claude is constrained to produce exactly this shape via structured output.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BuilderTurn {
    /// LLM asks a question with options
    AskQuestion {
        /// Phase label for progress indication (e.g., "Defining personality...")
        phase_label: String,
        /// The question text
        question: String,
        /// Multi-choice options with explanations
        options: Vec<BuilderOption>,
        /// Whether this question allows free-text via "Other"
        allow_other: bool,
        /// Brief preview of what's been configured so far
        config_preview: String,
    },
    /// LLM determines enough context has been gathered
    ReadyToAssemble {
        /// Full structured configuration summary
        summary: BuilderSummary,
        /// Confirmation prompt text
        confirmation_message: String,
    },
    /// LLM asks a follow-up for clarification
    Clarify {
        phase_label: String,
        question: String,
        context: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BuilderOption {
    /// The option label
    pub label: String,
    /// Brief explanation of what this option means
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BuilderSummary {
    pub name: String,
    pub description: String,
    pub category: String,
    pub personality_traits: Vec<String>,
    pub tone: String,
    pub model: String,
    pub provider: String,
    pub temperature: f64,
    pub max_tokens: u32,
    pub suggested_skills: Vec<String>,
    pub tags: Vec<String>,
}

// Generate JSON schema for Claude's output_config
fn builder_turn_schema() -> serde_json::Value {
    let schema = schemars::schema_for!(BuilderTurn);
    serde_json::to_value(schema).unwrap()
}
```

### Pattern 2: Surface Adapter Pattern

**What:** Decouple the builder logic from surface-specific rendering. The core returns structured data; adapters render it.
**When to use:** All builder interactions across CLI and web.

```rust
/// Surface-agnostic builder agent interface.
///
/// The builder agent takes user input and returns the next turn.
/// CLI and web adapters translate BuilderTurn into their respective UIs.
#[trait_variant::make(Send)]
pub trait BuilderAgent {
    /// Start a new builder session. Returns the first question.
    async fn start(&self, initial_description: &str) -> Result<BuilderTurn, BuilderError>;

    /// Process a user's answer and return the next turn.
    /// `answer` is either an option index or free text from "Other".
    async fn answer(&self, session_id: &str, answer: BuilderAnswer) -> Result<BuilderTurn, BuilderError>;

    /// Go back to a previous phase and re-ask from there.
    async fn go_back(&self, session_id: &str, target_phase: &str) -> Result<BuilderTurn, BuilderError>;

    /// Get the current draft state for resume.
    async fn get_draft(&self, session_id: &str) -> Result<Option<BuilderState>, BuilderError>;

    /// Confirm and assemble the bot from accumulated state.
    async fn assemble(&self, session_id: &str) -> Result<AssemblyResult, BuilderError>;
}

/// User's answer to a builder question.
#[derive(Debug, Clone)]
pub enum BuilderAnswer {
    /// Selected option by index
    OptionIndex(usize),
    /// Free-text via "Other" escape hatch
    FreeText(String),
    /// Confirmation (yes/no for the final summary)
    Confirm(bool),
}
```

### Pattern 3: Builder State Accumulator

**What:** A growing state object that captures all user decisions across the builder flow.
**When to use:** Passed to the LLM on every turn so it has full context for adaptive question generation.

```rust
/// Accumulated state from the builder conversation.
/// Serialized to JSON and included in the LLM context on each turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderState {
    pub session_id: String,
    pub phase: BuilderPhase,
    /// The user's initial description of what they want to build
    pub initial_description: String,
    /// Detected complexity category
    pub purpose_category: Option<PurposeCategory>,
    /// All question-answer pairs so far (chronological)
    pub conversation: Vec<QAPair>,
    /// Partially assembled configuration (grows as answers come in)
    pub config: PartialBotConfig,
    /// Phase history for back navigation
    pub phase_history: Vec<BuilderPhase>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QAPair {
    pub phase: BuilderPhase,
    pub question: String,
    pub answer: String,
    pub selected_option: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuilderPhase {
    Basics,
    Personality,
    Model,
    Skills,
    Review,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PurposeCategory {
    SimpleUtility,
    ComplexAnalyst,
    Creative,
    Coding,
    Research,
    CustomerService,
    Custom,
}
```

### Pattern 4: CLI Surface Adapter

**What:** Translate `BuilderTurn` into dialoguer interactions.
**When to use:** The CLI wizard flow (`bnity create bot` enhanced path).

```rust
use dialoguer::{Select, Input, Confirm};
use console::style;

/// CLI adapter: render a BuilderTurn as interactive terminal prompts.
fn render_turn_cli(turn: &BuilderTurn) -> Result<BuilderAnswer, anyhow::Error> {
    match turn {
        BuilderTurn::AskQuestion {
            phase_label,
            question,
            options,
            allow_other,
            config_preview,
        } => {
            // Show phase label
            println!();
            println!("  {}", style(phase_label).cyan().bold());
            println!();

            // Show live preview if non-empty
            if !config_preview.is_empty() {
                println!("  {}", style("Current config:").dim());
                for line in config_preview.lines() {
                    println!("    {}", style(line).dim());
                }
                println!();
            }

            // Build option labels with explanations
            let mut labels: Vec<String> = options.iter()
                .map(|opt| format!("{} -- {}", opt.label, style(&opt.explanation).dim()))
                .collect();
            if *allow_other {
                labels.push(format!("{}", style("Other (type your own)").yellow()));
            }

            // Interactive select
            let selection = Select::new()
                .with_prompt(question)
                .items(&labels)
                .default(0)
                .interact()?;

            if *allow_other && selection == options.len() {
                // "Other" was selected -- prompt for free text
                let text = Input::<String>::new()
                    .with_prompt("Your answer")
                    .interact_text()?;
                Ok(BuilderAnswer::FreeText(text))
            } else {
                Ok(BuilderAnswer::OptionIndex(selection))
            }
        }
        BuilderTurn::ReadyToAssemble { summary, confirmation_message } => {
            // Show full summary
            println!();
            println!("  {}", style("=== Bot Configuration ===").green().bold());
            println!("  Name:        {}", style(&summary.name).cyan());
            println!("  Category:    {}", &summary.category);
            println!("  Personality: {}", summary.personality_traits.join(", "));
            println!("  Tone:        {}", &summary.tone);
            println!("  Model:       {}", &summary.model);
            println!("  Temperature: {}", summary.temperature);
            if !summary.suggested_skills.is_empty() {
                println!("  Skills:      {}", summary.suggested_skills.join(", "));
            }
            println!();

            let confirmed = Confirm::new()
                .with_prompt(confirmation_message)
                .default(true)
                .interact()?;
            Ok(BuilderAnswer::Confirm(confirmed))
        }
        BuilderTurn::Clarify { phase_label, question, context } => {
            println!();
            println!("  {}", style(phase_label).cyan().bold());
            println!("  {}", style(context).dim());
            println!();
            let text = Input::<String>::new()
                .with_prompt(question)
                .interact_text()?;
            Ok(BuilderAnswer::FreeText(text))
        }
    }
}
```

### Pattern 5: Forge System Prompt Construction

**What:** Build the system prompt for the Forge builder agent with structured output instructions.
**When to use:** On every LLM call during the builder flow.

```rust
/// Build Forge's system prompt for builder interactions.
///
/// The prompt includes:
/// 1. Forge's personality (from its SOUL.md)
/// 2. Builder instructions with phase definitions
/// 3. Accumulated state from previous turns
/// 4. Smart defaults for the detected purpose category
/// 5. Builder memory from past sessions (if available)
fn build_forge_system_prompt(
    forge_soul: &str,
    state: &BuilderState,
    builder_memories: &[BuilderMemoryEntry],
    available_skills: &[SkillSummary],
) -> String {
    let mut sections = Vec::new();

    // Forge's personality
    sections.push(format!("<soul>\n{}\n</soul>", forge_soul));

    // Builder instructions
    sections.push(format!(
        "<builder_instructions>\n\
        You are Forge, the bot builder. Guide the user through creating a new bot.\n\
        \n\
        RULES:\n\
        - Generate each question dynamically based on all context gathered so far\n\
        - Every multi-choice question MUST include options with brief explanations\n\
        - Adapt question depth to complexity: simple bots get fewer questions\n\
        - For simple purposes, offer smart defaults with confirmation\n\
        - For complex purposes, ask probing follow-up questions\n\
        - Aim for brevity -- stop asking when you have enough to build a good bot\n\
        - Always explain your reasoning for suggestions\n\
        - Track which phase you're in: basics, personality, model, skills, review\n\
        \n\
        PURPOSE CATEGORIES:\n\
        - simple_utility: 3-5 questions, heavy defaults (email assistant, timer, reminder)\n\
        - coding: 5-7 questions, ask about language preferences, style, frameworks\n\
        - creative: 5-7 questions, focus on tone, style, voice, audience\n\
        - complex_analyst: 7-10 questions, deep-dive on data sources, methodology\n\
        - research: 6-8 questions, ask about domains, citation style, depth\n\
        - customer_service: 5-7 questions, ask about brand voice, escalation paths\n\
        \n\
        SMART DEFAULTS BY CATEGORY:\n\
        - coding: temperature=0.3, formal tone, direct style\n\
        - creative: temperature=0.9, expressive tone, playful style\n\
        - simple_utility: temperature=0.5, neutral tone, concise style\n\
        - complex_analyst: temperature=0.4, analytical tone, thorough style\n\
        - research: temperature=0.5, academic tone, precise style\n\
        - customer_service: temperature=0.6, friendly tone, helpful style\n\
        </builder_instructions>"
    ));

    // Current state
    sections.push(format!(
        "<builder_state>\n{}\n</builder_state>",
        serde_json::to_string_pretty(state).unwrap_or_default()
    ));

    // Builder memory from past sessions
    if !builder_memories.is_empty() {
        let memory_lines: Vec<String> = builder_memories.iter()
            .map(|m| format!("- {} ({})", m.suggestion, m.source_session))
            .collect();
        sections.push(format!(
            "<builder_memory>\n\
            Past builder sessions you can reference:\n\
            {}\n\
            </builder_memory>",
            memory_lines.join("\n")
        ));
    }

    // Available skills catalog
    if !available_skills.is_empty() {
        let skill_lines: Vec<String> = available_skills.iter()
            .map(|s| format!("- {} -- {}", s.name, s.description))
            .collect();
        sections.push(format!(
            "<available_skills>\n{}\n</available_skills>",
            skill_lines.join("\n")
        ));
    }

    sections.join("\n\n")
}
```

### Pattern 6: Bot Assembly from Builder State

**What:** Transform accumulated `BuilderState` into actual bot files using existing services.
**When to use:** After the user confirms the final summary.

```rust
/// Assemble a complete bot from the builder state.
/// Uses existing BotService and SoulService -- does NOT duplicate file creation logic.
async fn assemble_bot(
    state: &BuilderState,
    bot_service: &ConcreteBotService,
    soul_service: &ConcreteSoulService,
    llm_provider: &BoxLlmProvider,
) -> Result<AssemblyResult, BuilderError> {
    let config = &state.config;

    // Step 1: Create the bot record (generates slug, creates directory)
    let request = CreateBotRequest {
        name: config.name.clone().unwrap_or("Unnamed Bot".to_string()),
        description: config.description.clone(),
        category: config.category.clone(),
        tags: config.tags.clone(),
    };
    let bot = bot_service.create_bot(request).await?;
    let bot_dir = bot_service.bot_dir(&bot.slug);

    // Step 2: Generate unique SOUL.md content via LLM
    // The LLM gets the full builder state and generates Personality + Purpose + Boundaries
    let soul_content = generate_soul_content(llm_provider, state).await?;
    let soul_path = bot_dir.join("SOUL.md");
    soul_service.write_and_save_soul(&bot.id, &soul_content, &soul_path).await?;

    // Step 3: Generate IDENTITY.md with smart defaults
    let identity_content = generate_identity_content(config);
    let identity_path = bot_dir.join("IDENTITY.md");
    soul_service.write_identity(&identity_content, &identity_path).await?;

    // Step 4: Generate USER.md seeded with context from builder conversation
    let user_content = generate_user_content(state);
    let user_path = bot_dir.join("USER.md");
    soul_service.write_user(&user_content, &user_path).await?;

    // Step 5: Attach suggested skills (using skill system from Phase 6)
    // ... skill attachment logic via SkillStore

    Ok(AssemblyResult {
        bot,
        soul_path,
        identity_path,
        user_path,
        attached_skills: config.suggested_skills.clone().unwrap_or_default(),
    })
}
```

### Anti-Patterns to Avoid

- **Parsing free-text LLM output for structure:** Never regex-parse LLM output to extract questions/options. Use Claude's structured output (`output_config.format`) to guarantee JSON schema compliance. The LLM literally cannot produce tokens that violate the schema.

- **Fixed question skeleton with LLM fill-in:** The user explicitly decided against this. Each question is fully dynamic based on accumulated context. Do NOT build a state machine with predetermined question sequences.

- **Duplicating bot creation logic:** The builder MUST use `BotService::create_bot` and `SoulService::write_and_save_soul` for artifact creation. Never duplicate the slug generation, directory creation, or hash computation logic.

- **Separate LLM session per question:** Maintain the full conversation history across the builder session. Each LLM call gets the accumulated `BuilderState` in the system prompt, but the conversation messages also carry forward for continuity. Starting fresh each turn loses context.

- **Tight coupling between builder logic and surface:** The `BuilderAgent` trait must not import `dialoguer`, `axum`, or any UI-specific types. It returns `BuilderTurn` data structures. Adapters handle rendering.

- **Storing builder drafts in filesystem:** Use the existing `SqliteKvStore` for draft persistence. File-based drafts lack atomicity, are harder to list/expire, and duplicate infrastructure.

- **Skipping the confirmation step:** Per locked decision, the builder must show a full summary and ask "Ready to create?" before building. Never auto-create without explicit confirmation.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON Schema from Rust types | Manual JSON schema construction | `schemars` derive macro | Schemas stay in sync with Rust types automatically; manual schemas drift when types change |
| CLI multi-choice prompts | Custom terminal input parsing | `dialoguer::Select` with arrow-key navigation | Handles terminal raw mode, cursor movement, color themes, cross-platform input -- already in workspace |
| CLI progress indication | Custom spinner implementation | `indicatif::ProgressBar` with spinner style | Already used in bot creation flow; battle-tested animation and clearing |
| Draft persistence | Custom file-based draft storage | `SqliteKvStore` (already in workspace) | Atomic operations, TTL support, namespace isolation, already wired in AppState |
| Bot file creation | Duplicate directory/file creation | `BotService::create_bot` + `SoulService` | Handles slug uniqueness, hash computation, version tracking, directory structure |
| Structured LLM output parsing | Regex/string parsing of LLM text | Claude `output_config.format` with `json_schema` | Guaranteed schema compliance at token generation level; zero parse errors |
| SOUL.md template structure | Custom markdown templating | LLM generation with structural prompt | User decided: "consistent Personality + Purpose + Boundaries sections, unique content per bot" -- the LLM ensures section consistency while writing unique content |
| Builder memory retrieval | Custom vector search for past sessions | `SqliteKvStore` with namespace + JSON query | Builder memory is simple key-value (category -> past choices), not semantic search; KV store is sufficient |

**Key insight:** The builder system is primarily an LLM orchestration layer. Nearly all infrastructure (bot creation, soul management, skill attachment, KV storage, LLM providers) already exists. The new code is: (1) the Forge system prompt, (2) the structured output schema types, (3) the builder state accumulator, (4) the surface adapters, and (5) the assembly orchestration that wires existing services together.

## Common Pitfalls

### Pitfall 1: Structured Output Schema Complexity Explosion

**What goes wrong:** Defining a schema with too many nested variants, recursive types, or complex unions causes Claude to hit schema compilation limits or produce slower responses.
**Why it happens:** Claude's structured output compiles the JSON schema into a grammar. Complex schemas with many `anyOf` branches or deep nesting increase compilation time and may exceed limits.
**How to avoid:** Keep the `BuilderTurn` schema flat and simple. Use string enums instead of complex nested objects where possible. Test schema compilation with a small request before building the full flow. Claude does NOT support recursive schemas in structured output.
**Warning signs:** 400 errors mentioning "Schema is too complex" or "Too many recursive definitions".

### Pitfall 2: Context Window Bloat Across Builder Turns

**What goes wrong:** After 10+ builder turns, the accumulated conversation history plus system prompt exceeds the context window, causing failures or degraded output quality.
**Why it happens:** Each turn adds the full system prompt (with BuilderState JSON) plus conversation messages. The state grows linearly with each question.
**How to avoid:** Include `BuilderState` in the system prompt (which summarizes all decisions) but keep conversation messages minimal -- only include the last 3-5 exchange pairs, not the full history. The state object is the authoritative record of all decisions, not the conversation transcript.
**Warning signs:** Responses becoming vague or losing earlier context; token count approaching model limits.

### Pitfall 3: Structured Output with Streaming

**What goes wrong:** Attempting to use structured output (`output_config.format`) with streaming causes confusion because partial JSON arrives incrementally.
**Why it happens:** Structured output still works with streaming, but you receive JSON tokens incrementally. You must buffer the full response before parsing.
**How to avoid:** For the builder flow, use non-streaming (`execute_non_streaming`) since each turn is a discrete question. Streaming adds complexity without benefit here -- the user is waiting for the next question, not watching text appear. Reserve streaming for the actual bot chat, not the builder wizard.
**Warning signs:** JSON parse errors from partial responses.

### Pitfall 4: Back Navigation Invalidating Subsequent Answers

**What goes wrong:** User goes back to "Basics" phase and changes the bot's purpose from "coding assistant" to "creative writer." All subsequent answers (personality, tone, model) are now inconsistent with the new purpose.
**Why it happens:** Later answers were given in the context of the original purpose. Changing the purpose invalidates the context for those answers.
**How to avoid:** When the user navigates back and changes an answer, discard all answers from subsequent phases. The `phase_history` in `BuilderState` tracks which phases have been visited. On back navigation, truncate `conversation` to remove QA pairs from later phases and let the LLM re-ask from the changed point forward.
**Warning signs:** Bot created with contradictory configuration (e.g., "creative writer" with temperature 0.3 and formal tone).

### Pitfall 5: LLM Generating Too Many Questions

**What goes wrong:** The LLM keeps asking questions indefinitely, never deciding it has "enough context."
**Why it happens:** Without explicit guidance, LLMs tend to keep asking. The soft cap ("aim for brevity") is too vague for the LLM to act on.
**How to avoid:** Include the question count in the `BuilderState` passed to the LLM. Add explicit guidance in the system prompt: "For simple_utility purpose, ask at most 5 questions. After {N} questions, strongly consider moving to the review phase. After {N+3} questions, you MUST proceed to review." The LLM should signal `ReadyToAssemble` when it judges enough context exists.
**Warning signs:** Builder sessions exceeding 15 questions; users abandoning mid-flow.

### Pitfall 6: Forge Identity Collision with User Bots

**What goes wrong:** Forge (the builder bot) is stored as a regular bot in `~/.boternity/bots/forge/`, and a user creates a bot named "Forge" causing a slug collision.
**Why it happens:** Forge uses the same storage mechanism as user bots.
**How to avoid:** Either (a) reserve the "forge" slug in `BotService::create_bot` by checking against a reserved slugs list, or (b) store Forge's identity separately (embedded in the binary or in a `.boternity/system/` directory) rather than as a user-space bot. Option (b) is cleaner -- Forge's SOUL.md can be a compiled-in constant since it never changes per user.
**Warning signs:** "Slug conflict" errors when users try to create bots named "Forge".

### Pitfall 7: Draft Deserialization After Schema Changes

**What goes wrong:** A saved builder draft uses an older version of `BuilderState`. After a code update changes the schema (e.g., adds a new field), loading the draft fails with a deserialization error.
**Why it happens:** Draft JSON is persisted in SQLite. Schema evolution is not handled.
**How to avoid:** Use `#[serde(default)]` on all fields added after v1. Include a `schema_version: u32` field in `BuilderState`. On load, if the version is older, migrate or discard with a user-friendly message ("Your draft was created with an older version. Start fresh?").
**Warning signs:** "missing field" or "unknown variant" errors when resuming drafts after updates.

### Pitfall 8: Soul Import Breaking Hash Integrity

**What goes wrong:** User imports an existing SOUL.md via paste or file path. The builder skips hash computation, and the bot fails integrity checks on first startup.
**Why it happens:** The import path bypasses the normal `write_and_save_soul` flow.
**How to avoid:** ALL soul content -- whether generated by the builder or imported by the user -- MUST go through `SoulService::write_and_save_soul`. This function handles hash computation and version tracking. The import path should feed the content into the same service method.
**Warning signs:** `SoulIntegrityViolation` errors on first chat with a builder-created bot.

## Code Examples

### CompletionRequest with Structured Output

```rust
// Source: Anthropic API docs for structured outputs (https://platform.claude.com/docs/en/build-with-claude/structured-outputs)
// NOTE: The existing CompletionRequest type needs to be extended with output_config

/// Extended completion request with structured output support.
/// The `output_config` field tells Claude to constrain its response to a JSON schema.
///
/// For the builder, this ensures every LLM response is a valid BuilderTurn.
fn build_builder_request(
    context: &AgentContext,
    user_message: &str,
    schema: &serde_json::Value,
) -> serde_json::Value {
    // Build the raw API request with output_config.format
    // (This bypasses the current CompletionRequest type which lacks output_config)
    serde_json::json!({
        "model": context.agent_config.model,
        "max_tokens": context.agent_config.max_tokens,
        "system": context.system_prompt,
        "messages": context.build_messages_with_user(user_message),
        "output_config": {
            "format": {
                "type": "json_schema",
                "schema": schema
            }
        }
    })
}
```

### Draft Persistence via KV Store

```rust
// Source: Existing SqliteKvStore pattern in boternity-infra

const DRAFT_NAMESPACE: &str = "builder_draft";
const MEMORY_NAMESPACE: &str = "builder_memory";

/// Save a builder draft to the KV store.
async fn save_draft(
    kv_store: &SqliteKvStore,
    session_id: &str,
    state: &BuilderState,
) -> anyhow::Result<()> {
    let value = serde_json::to_string(state)?;
    kv_store.set(DRAFT_NAMESPACE, session_id, &value).await?;
    Ok(())
}

/// Load a builder draft from the KV store.
async fn load_draft(
    kv_store: &SqliteKvStore,
    session_id: &str,
) -> anyhow::Result<Option<BuilderState>> {
    match kv_store.get(DRAFT_NAMESPACE, session_id).await? {
        Some(value) => Ok(Some(serde_json::from_str(&value)?)),
        None => Ok(None),
    }
}

/// List all active builder drafts for the "Resume?" prompt.
async fn list_drafts(
    kv_store: &SqliteKvStore,
) -> anyhow::Result<Vec<(String, BuilderState)>> {
    let entries = kv_store.list(DRAFT_NAMESPACE).await?;
    let mut drafts = Vec::new();
    for (key, value) in entries {
        if let Ok(state) = serde_json::from_str::<BuilderState>(&value) {
            drafts.push((key, state));
        }
    }
    // Sort by most recently updated
    drafts.sort_by(|a, b| b.1.updated_at.cmp(&a.1.updated_at));
    Ok(drafts)
}
```

### Builder Memory for Past Session Suggestions

```rust
/// Record builder choices for future session suggestions.
/// Stored as simple JSON in the KV store, keyed by purpose category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderMemoryEntry {
    pub purpose_category: PurposeCategory,
    pub bot_name: String,
    pub personality_traits: Vec<String>,
    pub tone: String,
    pub model: String,
    pub temperature: f64,
    pub skills_used: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Save choices from a completed builder session for future reference.
async fn record_builder_memory(
    kv_store: &SqliteKvStore,
    state: &BuilderState,
) -> anyhow::Result<()> {
    let category = state.purpose_category.clone()
        .unwrap_or(PurposeCategory::Custom);
    let key = format!("{}:{}", category_key(&category), state.session_id);

    let entry = BuilderMemoryEntry {
        purpose_category: category,
        bot_name: state.config.name.clone().unwrap_or_default(),
        personality_traits: state.config.personality_traits.clone().unwrap_or_default(),
        tone: state.config.tone.clone().unwrap_or_default(),
        model: state.config.model.clone().unwrap_or_default(),
        temperature: state.config.temperature.unwrap_or(0.7),
        skills_used: state.config.suggested_skills.clone().unwrap_or_default(),
        created_at: chrono::Utc::now(),
    };

    let value = serde_json::to_string(&entry)?;
    kv_store.set(MEMORY_NAMESPACE, &key, &value).await?;
    Ok(())
}

/// Recall past builder sessions for the same purpose category.
async fn recall_builder_memories(
    kv_store: &SqliteKvStore,
    category: &PurposeCategory,
) -> anyhow::Result<Vec<BuilderMemoryEntry>> {
    let prefix = category_key(category);
    let entries = kv_store.list_prefix(MEMORY_NAMESPACE, &prefix).await?;
    let mut memories: Vec<BuilderMemoryEntry> = entries
        .into_iter()
        .filter_map(|(_, v)| serde_json::from_str(&v).ok())
        .collect();
    // Most recent first
    memories.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    // Return top 5 for context window economy
    memories.truncate(5);
    Ok(memories)
}
```

### Skill Builder -- Natural Language to SKILL.md

```rust
/// Generate a skill from natural language description using the builder agent.
/// Returns the generated SKILL.md content and optionally WASM source code.
async fn build_skill_from_description(
    llm_provider: &BoxLlmProvider,
    description: &str,
    skill_type: SkillType,
    model: &str,
) -> Result<GeneratedSkill, BuilderError> {
    let schema = schemars::schema_for!(GeneratedSkill);
    let schema_json = serde_json::to_value(schema)?;

    // Build the prompt for skill generation
    let system_prompt = format!(
        "<skill_builder_instructions>\n\
        Generate a complete skill based on the user's description.\n\
        \n\
        For prompt-based skills:\n\
        - Generate a SKILL.md with YAML frontmatter (name, description, metadata)\n\
        - Write clear, actionable instructions in the body\n\
        \n\
        For tool-based (WASM) skills:\n\
        - Generate the SKILL.md manifest\n\
        - Generate Rust source code implementing the skill\n\
        - Auto-suggest required capabilities (file access, network, etc.)\n\
        \n\
        ALWAYS follow the agentskills.io SKILL.md format.\n\
        </skill_builder_instructions>"
    );

    // Use structured output to get parseable skill definition
    let response = call_llm_structured(
        llm_provider,
        &system_prompt,
        &format!("Create a {} skill: {}", skill_type, description),
        model,
        &schema_json,
    ).await?;

    let skill: GeneratedSkill = serde_json::from_str(&response.content)?;
    Ok(skill)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedSkill {
    /// The complete SKILL.md content (frontmatter + body)
    pub skill_md_content: String,
    /// Rust source code for tool-based skills (None for prompt-based)
    pub source_code: Option<String>,
    /// Suggested capabilities this skill needs
    pub suggested_capabilities: Vec<String>,
    /// Brief explanation of what the skill does
    pub explanation: String,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Tool use for structured extraction | `output_config.format` with JSON schema | 2025-11 (GA 2026) | Guaranteed schema compliance at token level; no parse errors; simpler than tool_use for response formatting |
| `output_format` parameter (beta) | `output_config.format` (GA) | 2026 | Old parameter deprecated; new location in API; beta headers no longer needed for Opus 4.6/Sonnet 4.5 |
| Free-text LLM + regex parsing | Structured output with schemars-derived schemas | 2025-2026 | Zero parsing failures; type-safe response handling; schemas auto-generated from Rust types |
| Static question wizards | LLM-adaptive question generation | 2024-2025 | Questions adapt to context; no predetermined paths; better UX for diverse bot types |

**Deprecated/outdated:**
- `output_format` top-level parameter: Deprecated, moved to `output_config.format`. Still works temporarily.
- `anthropic-beta: structured-outputs-2025-11-13` header: No longer needed for GA models (Opus 4.6, Sonnet 4.5, Haiku 4.5).
- Tool use for response formatting: Still works but `output_config.format` is cleaner for "return this JSON shape" use cases. Use tool use when you need the LLM to invoke actual functions.

## Open Questions

1. **CompletionRequest extension for `output_config`**
   - What we know: The current `CompletionRequest` type in boternity-types lacks an `output_config` field. Claude's structured output requires `output_config.format.type = "json_schema"` with a schema object.
   - What's unclear: Whether to extend the existing `CompletionRequest` or create a separate `StructuredCompletionRequest` type. The existing `LlmProvider` trait returns `CompletionResponse` which assumes `content: String` -- structured output returns JSON in the same field, so it works, but the request path needs the new field.
   - Recommendation: Add an `output_config: Option<OutputConfig>` field to `CompletionRequest` with `#[serde(skip_serializing_if = "Option::is_none")]`. This is backward-compatible. Provider implementations (Anthropic, Bedrock) serialize it when present. This is the minimal change.

2. **SqliteKvStore `list_prefix` method**
   - What we know: Builder memory uses prefix-based listing (`coding:session1`, `creative:session2`). The existing `SqliteKvStore` may not have a `list_prefix` method.
   - What's unclear: Whether `list_prefix` exists or needs to be added.
   - Recommendation: Check if SqliteKvStore has prefix listing. If not, add a simple `SELECT * FROM kv WHERE namespace = ? AND key LIKE ?` query. This is a small addition.

3. **Forge identity storage**
   - What we know: Forge needs a SOUL.md personality. Options: (a) store as a regular bot, (b) embed in binary as a const, (c) store in `.boternity/system/forge/`.
   - What's unclear: Which approach best avoids namespace collisions and deployment complexity.
   - Recommendation: Embed Forge's SOUL.md as a compile-time constant (`include_str!` or a `const`). It is a system component, not a user bot. This avoids slug collisions, filesystem dependency, and makes Forge available immediately without initialization.

4. **Web builder WebSocket protocol**
   - What we know: The web UI needs both a Forge chat bot (real-time) and a step-by-step wizard. The chat bot needs WebSocket for streaming. The wizard could use REST.
   - What's unclear: Whether the Forge chat bot reuses the existing `/ws/events` WebSocket or needs a dedicated `/ws/builder` endpoint.
   - Recommendation: Create a dedicated `/ws/builder` endpoint. The existing `/ws/events` is designed for agent lifecycle events, not interactive builder flows. The builder WebSocket needs a different command set (submit answer, go back, get preview, confirm).

## Sources

### Primary (HIGH confidence)
- [Anthropic Structured Outputs docs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs) - GA API for `output_config.format` with JSON schema; supports Opus 4.6, Sonnet 4.5, Haiku 4.5; schema compilation and caching details
- [Schemars 1.x docs](https://docs.rs/schemars/latest/schemars/) - JSON Schema generation from Rust types; serde integration; derive macro
- [Dialoguer docs](https://docs.rs/dialoguer/latest/dialoguer/) - Select, MultiSelect, Input, Confirm widgets; theme support; arrow-key navigation
- Existing codebase: `BotService::create_bot`, `SoulService::write_and_save_soul`, `AgentEngine`, `SystemPromptBuilder`, `AppState`, `SqliteKvStore` -- all verified by reading source

### Secondary (MEDIUM confidence)
- [Anthropic Building Effective Agents](https://www.anthropic.com/engineering/building-effective-agents) - Agent architecture patterns; orchestrator-workers; multi-turn conversation design
- [Rust CLI prompts comparison](https://fadeevab.com/comparison-of-rust-cli-prompts/) - dialoguer vs inquire vs cliclack feature comparison

### Tertiary (LOW confidence)
- LLM multi-turn conversation research (arxiv.org) - Academic findings on LLM underspecification handling and multi-turn degradation; informs the "question cap" and "context truncation" design decisions but is not directly actionable
- schemars JSON Schema limitations with complex enums -- needs validation that `#[serde(tag = "action")]` tagged enums generate valid schemas for Claude's structured output parser

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace except schemars; schemars is the de facto JSON Schema generator in Rust
- Architecture patterns (surface adapter, state accumulator): HIGH -- derived from codebase analysis and standard trait-based architecture
- Structured output integration: HIGH -- verified against official Anthropic GA documentation; Claude Opus 4.6 supports it
- Builder memory/draft persistence: MEDIUM -- using SqliteKvStore is sound but the exact query patterns (list_prefix) need verification
- CLI interaction patterns: HIGH -- dialoguer is already used in the codebase for the same purpose
- Skill builder code generation: MEDIUM -- LLM-generated code quality varies; validation strategy (compile-only vs runtime test) is discretionary

**Research date:** 2026-02-14
**Valid until:** 2026-03-14 (structured output API is GA and stable; schemars 1.x is stable)
