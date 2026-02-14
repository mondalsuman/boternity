# Phase 7: Builder System - Research

**Researched:** 2026-02-14 (updated with deep-dive corrections)
**Domain:** LLM-driven interactive builder agent, multi-turn structured conversation, CLI wizard, web UI builder, adaptive question flows, artifact generation (SOUL.md, IDENTITY.md, USER.md, skills)
**Confidence:** HIGH (architecture patterns), HIGH (CLI stack), HIGH (structured output), HIGH (storage strategy), HIGH (service integration), MEDIUM (web builder protocol)

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

**Primary recommendation:** Use Claude structured outputs (`output_config.format` with `json_schema`) to enforce parseable LLM responses on every builder turn. Use `dialoguer` (already in workspace) for CLI interaction. Store builder drafts and memory in a dedicated `builder_drafts` SQLite table (NOT the bot-scoped KvStore). Generate artifacts by calling the existing `BotService::create_bot` and `SoulService::write_and_save_soul` after assembly.

---

## CORRECTIONS: Deep-Dive Findings (2026-02-14)

This section documents corrections to the initial research based on actual codebase verification.

### CORRECTION 1: KvStore is Bot-Scoped, NOT Namespace-Scoped

**Initial assumption (WRONG):** The research assumed `SqliteKvStore` has namespace-based operations like `set(NAMESPACE, key, value)` and `list(NAMESPACE)`.

**Actual codebase (VERIFIED):**

```rust
// crates/boternity-core/src/storage/kv_store.rs
pub trait KvStore: Send + Sync {
    fn get(&self, bot_id: &Uuid, key: &str) -> impl Future<Output = Result<Option<serde_json::Value>, RepositoryError>> + Send;
    fn set(&self, bot_id: &Uuid, key: &str, value: &serde_json::Value) -> impl Future<Output = Result<(), RepositoryError>> + Send;
    fn delete(&self, bot_id: &Uuid, key: &str) -> impl Future<Output = Result<(), RepositoryError>> + Send;
    fn list_keys(&self, bot_id: &Uuid) -> impl Future<Output = Result<Vec<String>, RepositoryError>> + Send;
    fn get_entry(&self, bot_id: &Uuid, key: &str) -> impl Future<Output = Result<Option<KvEntry>, RepositoryError>> + Send;
}
```

The `SqliteKvStore` implementation uses a `bot_kv_store` table with `(bot_id, key)` as the compound primary key. There is **no namespace concept**. All operations require a `bot_id: &Uuid`.

**Impact on builder:** Builder drafts and builder memory CANNOT use the existing KvStore because:
- Builder data is not bot-scoped (drafts exist before any bot is created)
- There is no "Forge bot_id" to use as a scope (Forge should be embedded, not a real bot)
- There is no `list_prefix` method, and no namespace concept

**Resolution: Create a dedicated `BuilderDraftRepository` trait + `SqliteBuilderDraftStore` implementation.**

This is the correct pattern because:
1. Builder drafts are a fundamentally different data model (session-scoped, not bot-scoped)
2. Builder memory entries need listing by category prefix -- requires a `LIKE` query on keys
3. Adding namespace support to KvStore would pollute its clean bot-scoped API
4. A dedicated table (`builder_drafts`) with proper schema is cleaner than abusing JSON blobs
5. This follows the existing codebase pattern: each domain has its own repository trait and SQLite implementation

**Recommended schema:**

```sql
CREATE TABLE builder_drafts (
    session_id TEXT PRIMARY KEY,
    state_json TEXT NOT NULL,           -- serialized BuilderState
    schema_version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE builder_memory (
    id TEXT PRIMARY KEY,                -- UUID
    purpose_category TEXT NOT NULL,     -- e.g., "coding", "creative"
    bot_name TEXT NOT NULL,
    choices_json TEXT NOT NULL,         -- serialized BuilderMemoryEntry
    created_at TEXT NOT NULL
);

CREATE INDEX idx_builder_memory_category ON builder_memory(purpose_category);
```

**Recommended trait:**

```rust
// crates/boternity-core/src/builder/draft_store.rs
pub trait BuilderDraftStore: Send + Sync {
    /// Save or update a builder draft.
    fn save_draft(
        &self,
        session_id: &str,
        state: &BuilderState,
    ) -> impl Future<Output = Result<(), BuilderError>> + Send;

    /// Load a builder draft by session ID.
    fn load_draft(
        &self,
        session_id: &str,
    ) -> impl Future<Output = Result<Option<BuilderState>, BuilderError>> + Send;

    /// List all active drafts, most recently updated first.
    fn list_drafts(
        &self,
    ) -> impl Future<Output = Result<Vec<BuilderDraftSummary>, BuilderError>> + Send;

    /// Delete a draft (after successful assembly or manual discard).
    fn delete_draft(
        &self,
        session_id: &str,
    ) -> impl Future<Output = Result<(), BuilderError>> + Send;
}

// crates/boternity-core/src/builder/memory_store.rs
pub trait BuilderMemoryStore: Send + Sync {
    /// Record choices from a completed builder session.
    fn record_memory(
        &self,
        entry: &BuilderMemoryEntry,
    ) -> impl Future<Output = Result<(), BuilderError>> + Send;

    /// Recall past sessions for a given purpose category.
    fn recall_by_category(
        &self,
        category: &PurposeCategory,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<BuilderMemoryEntry>, BuilderError>> + Send;
}
```

**Confidence: HIGH** -- verified by reading actual `KvStore` trait, `SqliteKvStore` implementation, and `bot_kv_store` table schema.

### CORRECTION 2: CompletionRequest Needs `output_config` Extension

**Initial assumption (PARTIALLY CORRECT):** The research correctly identified that `CompletionRequest` lacks `output_config`, but the approach needs more specificity.

**Actual codebase (VERIFIED):**

The `CompletionRequest` (in `boternity-types/src/llm.rs`) has these fields:
```rust
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub system: Option<String>,
    pub max_tokens: u32,
    pub temperature: Option<f64>,
    pub stream: bool,
    pub stop_sequences: Option<Vec<String>>,
}
```

The `AnthropicProvider` converts `CompletionRequest` into `AnthropicRequest` via `to_anthropic_request()`:
```rust
fn to_anthropic_request(&self, request: &CompletionRequest, stream: bool) -> AnthropicRequest {
    AnthropicRequest {
        model: request.model.clone(),
        max_tokens: request.max_tokens,
        messages: /* converted */,
        system: request.system.clone(),
        stream,
        temperature: request.temperature,
        stop_sequences: request.stop_sequences.clone(),
    }
}
```

The `BedrockProvider` similarly converts to `BedrockRequest` which has the same fields minus `model` plus `anthropic_version`.

Neither `AnthropicRequest` nor `BedrockRequest` currently has an `output_config` field.

**Resolution: Add `output_config` to ALL THREE types.**

Step 1: Add to `CompletionRequest` (boternity-types):
```rust
/// Optional structured output configuration.
/// When present, constrains Claude's response to match the given JSON schema.
#[serde(skip_serializing_if = "Option::is_none")]
pub output_config: Option<OutputConfig>,
```

Step 2: Define `OutputConfig` types (boternity-types):
```rust
/// Configuration for structured output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub format: OutputFormat,
}

/// Output format specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputFormat {
    /// Constrain output to a JSON schema.
    JsonSchema {
        /// The JSON Schema that the output must conform to.
        schema: serde_json::Value,
    },
}
```

Step 3: Add to `AnthropicRequest` (boternity-infra):
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub output_config: Option<OutputConfig>,
```

Step 4: Add to `BedrockRequest` (boternity-infra):
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub output_config: Option<OutputConfig>,
```

Step 5: Wire through in both provider `to_*_request()` methods:
```rust
output_config: request.output_config.clone(),
```

**Bedrock compatibility:** Bedrock's structured output support is confirmed GA for the same models. The `output_config` field is passed in the request body the same way as direct API. The Bedrock request omits `model` (in URL path) and adds `anthropic_version`, but `output_config` serializes identically. No special handling needed.

**Backward compatibility:** `Option<OutputConfig>` with `skip_serializing_if = "Option::is_none"` means existing calls that don't use structured output are unaffected. The field simply doesn't appear in the serialized JSON.

**Response handling:** Structured output responses use the same `content[0].text` field as normal responses. The `CompletionResponse.content: String` already handles this. The response content is valid JSON matching the schema. Callers parse it with `serde_json::from_str::<BuilderTurn>(&response.content)`.

**Confidence: HIGH** -- verified by reading all three request types, both provider implementations, and confirmed against official Anthropic documentation.

### CORRECTION 3: Structured Output API is GA (Not Beta)

**Initial concern:** Whether structured outputs are beta or GA, and what headers are needed.

**Verified (2026-02-14) from official Anthropic docs:**

Structured outputs are **generally available** on:
- Claude API: Opus 4.6, Sonnet 4.5, Opus 4.5, Haiku 4.5
- Amazon Bedrock: same models
- Microsoft Foundry: still in public beta

**No beta header needed.** The old `anthropic-beta: structured-outputs-2025-11-13` header is not required for GA models. The `output_format` parameter has been deprecated and moved to `output_config.format`.

The existing `AnthropicProvider` uses `anthropic-version: 2023-06-01` which is correct and sufficient. No header changes needed.

**API shape confirmed:**
```json
{
  "model": "claude-opus-4-6",
  "max_tokens": 1024,
  "messages": [...],
  "output_config": {
    "format": {
      "type": "json_schema",
      "schema": {
        "type": "object",
        "properties": {...},
        "required": [...],
        "additionalProperties": false
      }
    }
  }
}
```

**Confidence: HIGH** -- verified directly from https://platform.claude.com/docs/en/build-with-claude/structured-outputs.

### FINDING 1: Tagged Enum Schema Compatibility with Claude

**Question:** Does `schemars::schema_for!(BuilderTurn)` generate a valid JSON schema for Claude's structured output when using `#[serde(tag = "action")]`?

**Answer: YES, with the `additionalProperties: false` requirement handled.**

For an internally-tagged enum like:
```rust
#[derive(JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BuilderTurn {
    AskQuestion { question: String, options: Vec<BuilderOption> },
    ReadyToAssemble { summary: BuilderSummary },
    Clarify { question: String, context: String },
}
```

Schemars generates a schema using `anyOf` where each variant is an object with:
- The tag property (`"action"`) set to a `const` value matching the variant name
- The variant's fields as additional properties
- All properties in `required`

Approximate generated schema structure:
```json
{
  "anyOf": [
    {
      "type": "object",
      "properties": {
        "action": { "const": "ask_question" },
        "question": { "type": "string" },
        "options": { "type": "array", "items": { "$ref": "#/$defs/BuilderOption" } },
        "allow_other": { "type": "boolean" },
        "phase_label": { "type": "string" },
        "config_preview": { "type": "string" }
      },
      "required": ["action", "question", "options", "allow_other", "phase_label", "config_preview"]
    },
    {
      "type": "object",
      "properties": {
        "action": { "const": "ready_to_assemble" },
        "summary": { "$ref": "#/$defs/BuilderSummary" },
        "confirmation_message": { "type": "string" }
      },
      "required": ["action", "summary", "confirmation_message"]
    },
    {
      "type": "object",
      "properties": {
        "action": { "const": "clarify" },
        "phase_label": { "type": "string" },
        "question": { "type": "string" },
        "context": { "type": "string" }
      },
      "required": ["action", "phase_label", "question", "context"]
    }
  ],
  "$defs": {
    "BuilderOption": { ... },
    "BuilderSummary": { ... }
  }
}
```

**Claude's support for this:**
- `anyOf` is explicitly supported by Claude structured outputs
- `const` values are supported
- `$ref` and `$defs` / `definitions` are supported (external `$ref` is not, but schemars uses local refs)
- `additionalProperties: false` is **required by Claude** on all objects -- schemars does NOT add this by default

**CRITICAL: Post-processing required.** After generating the schema with `schemars::schema_for!()`, the schema MUST be post-processed to add `"additionalProperties": false` to every object type. Without this, Claude returns a 400 error.

```rust
/// Post-process a schemars-generated schema to add additionalProperties: false
/// to all object types, as required by Claude's structured output.
fn add_additional_properties_false(schema: &mut serde_json::Value) {
    if let Some(obj) = schema.as_object_mut() {
        // If this is an object type, add additionalProperties: false
        if obj.get("type").and_then(|t| t.as_str()) == Some("object") {
            obj.insert("additionalProperties".to_string(), serde_json::json!(false));
        }
        // Recurse into nested schemas
        for key in ["properties", "items", "anyOf", "allOf", "$defs", "definitions"] {
            if let Some(nested) = obj.get_mut(key) {
                if let Some(arr) = nested.as_array_mut() {
                    for item in arr {
                        add_additional_properties_false(item);
                    }
                } else if let Some(obj_nested) = nested.as_object_mut() {
                    for (_, v) in obj_nested {
                        add_additional_properties_false(v);
                    }
                }
            }
        }
    }
}
```

**Schema complexity assessment:** The `BuilderTurn` enum has 3 variants with flat fields. `BuilderSummary` is a flat object with 11 string/number/array fields. `BuilderOption` has 2 string fields. This is well within Claude's schema complexity limits. The total schema is roughly equivalent to 3 tool definitions, which is trivial.

**JSON Schema limitations that apply:**
- No recursive schemas (not applicable -- BuilderTurn is not recursive)
- `additionalProperties` must be `false` on all objects (requires post-processing)
- No `minimum`/`maximum`/`minLength`/`maxLength` constraints (not applicable)
- `enum` values must be primitives only (string enums for PurposeCategory are fine)

**Confidence: HIGH** -- `anyOf` confirmed supported by official Anthropic docs, schemars tagged enum behavior confirmed by library docs and source code analysis.

### FINDING 2: Exact Service Method Signatures for Assembly

**Verified from codebase:**

**`BotService::create_bot`:**
```rust
pub async fn create_bot(&self, request: CreateBotRequest) -> Result<Bot, BotError>

pub struct CreateBotRequest {
    pub name: String,
    pub description: Option<String>,
    pub category: Option<BotCategory>,
    pub tags: Option<Vec<String>>,
}

pub enum BotCategory {
    Assistant, Creative, Research, Utility,
}
```
The builder's `PurposeCategory` (7 variants) is richer than `BotCategory` (4 variants). The builder must map its categories to `BotCategory`:
- `SimpleUtility` -> `Utility`
- `ComplexAnalyst` -> `Assistant`
- `Creative` -> `Creative`
- `Coding` -> `Assistant`
- `Research` -> `Research`
- `CustomerService` -> `Assistant`
- `Custom` -> `Assistant`

**`SoulService::write_and_save_soul`:**
```rust
pub async fn write_and_save_soul(
    &self,
    bot_id: &BotId,
    content: &str,
    soul_path: &Path,
) -> Result<Soul, SoulError>
```
Creates parent directory, writes file, computes SHA-256 hash, saves version to DB. This is the ONLY correct path for writing SOUL.md after `create_bot`.

**`SoulService::write_identity` and `write_user`:**
```rust
pub async fn write_identity(&self, content: &str, identity_path: &Path) -> Result<(), SoulError>
pub async fn write_user(&self, content: &str, user_path: &Path) -> Result<(), SoulError>
```
Simple file writes, no versioning. The builder generates custom content instead of using `generate_default_identity()`.

**Assembly sequence must be:**
1. `bot_service.create_bot(request)` -- creates bot record + default SOUL/IDENTITY/USER files
2. `soul_service.write_and_save_soul(&bot.id, &custom_soul, &soul_path)` -- overwrites default soul with builder-generated content (creates version 2)
3. `soul_service.write_identity(&custom_identity, &identity_path)` -- overwrites default identity
4. `soul_service.write_user(&custom_user, &user_path)` -- overwrites default user
5. Attach skills via `SkillStore`

Note: `create_bot` already writes default files (version 1). The builder overwrites them in steps 2-4 (version 2 for soul). This is slightly wasteful (write twice) but correct and safe. An alternative is to refactor `create_bot` to accept optional content overrides, but that would change an existing stable API.

**Confidence: HIGH** -- all signatures read directly from source code.

### FINDING 3: Web Builder Protocol Design

**Existing WebSocket at `/ws/events`:**

```rust
// crates/boternity-api/src/http/handlers/ws.rs
enum WsCommand {
    CancelAgent { agent_id: String },
    BudgetContinue { request_id: String },
    BudgetStop { request_id: String },
    Ping,
}
```

This is a one-to-many event broadcast + command handler for agent lifecycle. It is NOT suitable for the builder because:
- It broadcasts all events to all connected clients (no session isolation)
- It has no concept of request/response pairing (builder needs: "I sent answer X, give me the next question")
- It uses `tokio::sync::broadcast` which is fire-and-forget, not session-scoped

**Recommended approach: REST for wizard, WebSocket for Forge chat.**

**Step-by-step wizard (REST endpoints):**
```
POST   /api/v1/builder/sessions              -- Start new builder session, returns first question
POST   /api/v1/builder/sessions/{id}/answer   -- Submit answer, returns next question
POST   /api/v1/builder/sessions/{id}/back     -- Go back to previous phase
GET    /api/v1/builder/sessions/{id}          -- Get current state (for resume)
GET    /api/v1/builder/sessions               -- List active drafts
POST   /api/v1/builder/sessions/{id}/assemble -- Confirm and build
DELETE /api/v1/builder/sessions/{id}          -- Discard draft
```

REST is better for the wizard because:
- Each step is a discrete request/response (no streaming needed)
- Natural HTTP semantics (POST for mutations, GET for reads)
- Easy to implement back navigation (just POST to `/back`)
- Client can disconnect and resume later (drafts are persisted)
- Works with any HTTP client (no WebSocket library needed)

**Forge chat bot (WebSocket at `/ws/builder/{session_id}`):**
```rust
// Client -> Server messages
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BuilderClientMessage {
    /// Free-form chat message to Forge
    Chat { message: String },
    /// Select an option from a previous question
    SelectOption { option_index: usize },
    /// Go back to a previous phase
    GoBack { target_phase: String },
    /// Confirm the final summary
    Confirm,
    /// Reject and continue editing
    Reject,
    /// Keep-alive
    Ping,
}

// Server -> Client messages
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BuilderServerMessage {
    /// A builder turn (question, ready_to_assemble, clarify)
    Turn { turn: BuilderTurn },
    /// Streaming text delta from Forge (for conversational feel)
    TextDelta { text: String },
    /// Error occurred
    Error { message: String },
    /// Session complete, bot created
    Complete { bot_slug: String, bot_id: String },
    /// Pong response
    Pong,
}
```

WebSocket is better for Forge chat because:
- Real-time conversational feel (streaming Forge's responses)
- Bidirectional: client sends answers, server pushes follow-ups
- Lower latency than polling REST

**Both share the same `BuilderAgent` implementation.** The REST handlers and WebSocket handler both call the same `BuilderAgent::answer()` method. The difference is only in transport:
- REST: receive answer as JSON body, return `BuilderTurn` as JSON response
- WebSocket: receive answer as WS message, push `BuilderTurn` as WS message

**Router additions:**
```rust
// In crates/boternity-api/src/http/router.rs
let api_routes = Router::new()
    // ... existing routes ...
    // Builder (wizard REST)
    .route("/builder/sessions", post(handlers::builder::create_session))
    .route("/builder/sessions", get(handlers::builder::list_sessions))
    .route("/builder/sessions/{id}", get(handlers::builder::get_session))
    .route("/builder/sessions/{id}", delete(handlers::builder::delete_session))
    .route("/builder/sessions/{id}/answer", post(handlers::builder::submit_answer))
    .route("/builder/sessions/{id}/back", post(handlers::builder::go_back))
    .route("/builder/sessions/{id}/assemble", post(handlers::builder::assemble));

let mut router = Router::new()
    .nest("/api/v1", api_routes)
    // ... existing WebSocket ...
    .route("/ws/events", get(handlers::ws::ws_handler))
    // Builder WebSocket for Forge chat
    .route("/ws/builder/{session_id}", get(handlers::builder_ws::builder_ws_handler));
```

**Confidence: MEDIUM** -- protocol design is sound based on existing patterns, but has not been tested. The REST vs WebSocket split is a design decision, not a verified pattern.

---

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
| sqlx | 0.8 (already in workspace) | Builder draft persistence (dedicated table) | Auto-saving builder progress; session drafts stored as JSON in `builder_drafts` table |
| chrono | 0.4 (already in workspace) | Timestamps for draft sessions and builder memory | Draft created_at, last_modified, builder memory timestamps |
| uuid | 1.20 (already in workspace) | Session IDs for builder drafts | Each builder session gets a unique ID for draft persistence and resumption |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| dialoguer Select | inquire crate | inquire has richer built-in features (autocompletion, date pickers) but would add a new dependency; dialoguer is already in the workspace and sufficient for multi-choice + text input |
| schemars JSON Schema | Hand-written JSON schema strings | schemars auto-generates schemas from Rust types and stays in sync with serde; hand-written schemas drift and are error-prone |
| Dedicated builder_drafts table | Bot-scoped KvStore | KvStore requires a bot_id which doesn't exist during building; dedicated table has proper schema and indexing for builder-specific queries |
| Claude structured output | Tool use for structured extraction | Structured output (`output_config.format`) is cleaner for this use case -- we want formatted responses, not function calls. Tool use would work but adds unnecessary complexity for what is essentially "return this JSON shape" |
| REST wizard + WS chat | Single WebSocket for everything | REST is simpler for step-by-step wizard (discrete request/response); WebSocket only adds value for Forge's conversational streaming |

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
                          #   PurposeCategory, SmartDefaults, BuilderDraft,
                          #   OutputConfig, OutputFormat (new LLM types)

crates/boternity-core/src/
  builder/
    mod.rs                # Module re-exports
    agent.rs              # BuilderAgent trait -- surface-agnostic builder logic
    state.rs              # BuilderState accumulator -- tracks answers, phase, config-so-far
    assembler.rs          # BotAssembler -- generates SOUL.md, IDENTITY.md, USER.md from state
    defaults.rs           # SmartDefaults -- purpose-category-based default values
    memory.rs             # BuilderMemory trait -- recalls past sessions for suggestions
    draft_store.rs        # BuilderDraftStore trait -- draft persistence
    prompt.rs             # Forge system prompt construction with builder instructions
    skill_builder.rs      # SkillBuilder -- natural language to SKILL.md + code generation

crates/boternity-infra/src/
  builder/
    mod.rs                # Module re-exports
    llm_builder.rs        # LlmBuilderAgent -- concrete implementation using LlmProvider
    sqlite_draft_store.rs # SqliteBuilderDraftStore -- draft persistence in builder_drafts table
    sqlite_memory_store.rs # SqliteBuilderMemoryStore -- memory persistence in builder_memory table

crates/boternity-api/src/
  cli/
    builder.rs            # CLI wizard surface adapter -- dialoguer-based interaction loop
    skill_create.rs       # `bnity skill create` standalone CLI flow
  http/
    handlers/
      builder.rs          # REST API handlers for web builder wizard
      builder_ws.rs       # WebSocket handler for Forge chat bot
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

/// Generate the JSON schema for BuilderTurn, post-processed for Claude compatibility.
fn builder_turn_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(BuilderTurn)).unwrap();
    // CRITICAL: Claude requires additionalProperties: false on all objects
    add_additional_properties_false(&mut schema);
    schema
}

/// Post-process a schemars-generated schema to add additionalProperties: false
/// to all object types, as required by Claude's structured output.
fn add_additional_properties_false(schema: &mut serde_json::Value) {
    if let Some(obj) = schema.as_object_mut() {
        if obj.get("type").and_then(|t| t.as_str()) == Some("object") {
            obj.insert("additionalProperties".to_string(), serde_json::json!(false));
        }
        for key in ["properties", "items", "anyOf", "allOf", "$defs", "definitions"] {
            if let Some(nested) = obj.get_mut(key) {
                if let Some(arr) = nested.as_array_mut() {
                    for item in arr {
                        add_additional_properties_false(item);
                    }
                } else if let Some(obj_nested) = nested.as_object_mut() {
                    for (_, v) in obj_nested {
                        add_additional_properties_false(v);
                    }
                }
            }
        }
    }
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
    async fn start(&self, initial_description: &str) -> Result<(String, BuilderTurn), BuilderError>;
    //                                                          ^session_id

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
    /// Schema version for draft forward-compatibility
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn default_schema_version() -> u32 { 1 }

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
            phase_label, question, options, allow_other, config_preview,
        } => {
            println!();
            println!("  {}", style(phase_label).cyan().bold());
            println!();

            if !config_preview.is_empty() {
                println!("  {}", style("Current config:").dim());
                for line in config_preview.lines() {
                    println!("    {}", style(line).dim());
                }
                println!();
            }

            let mut labels: Vec<String> = options.iter()
                .map(|opt| format!("{} -- {}", opt.label, style(&opt.explanation).dim()))
                .collect();
            if *allow_other {
                labels.push(format!("{}", style("Other (type your own)").yellow()));
            }

            let selection = Select::new()
                .with_prompt(question)
                .items(&labels)
                .default(0)
                .interact()?;

            if *allow_other && selection == options.len() {
                let text = Input::<String>::new()
                    .with_prompt("Your answer")
                    .interact_text()?;
                Ok(BuilderAnswer::FreeText(text))
            } else {
                Ok(BuilderAnswer::OptionIndex(selection))
            }
        }
        BuilderTurn::ReadyToAssemble { summary, confirmation_message } => {
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
fn build_forge_system_prompt(
    forge_soul: &str,
    state: &BuilderState,
    builder_memories: &[BuilderMemoryEntry],
    available_skills: &[SkillSummary],
) -> String {
    let mut sections = Vec::new();

    sections.push(format!("<soul>\n{}\n</soul>", forge_soul));

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
        QUESTION LIMITS BY CATEGORY:\n\
        - simple_utility: 3-5 questions, heavy defaults\n\
        - coding: 5-7 questions\n\
        - creative: 5-7 questions\n\
        - complex_analyst: 7-10 questions\n\
        - research: 6-8 questions\n\
        - customer_service: 5-7 questions\n\
        After the recommended number, strongly consider moving to review.\n\
        After 3 more beyond that, you MUST proceed to review.\n\
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

    sections.push(format!(
        "<builder_state>\n\
        Questions asked so far: {}\n\
        {}\n\
        </builder_state>",
        state.conversation.len(),
        serde_json::to_string_pretty(state).unwrap_or_default()
    ));

    if !builder_memories.is_empty() {
        let memory_lines: Vec<String> = builder_memories.iter()
            .map(|m| format!("- {} bot '{}': tone={}, temp={}",
                m.purpose_category_str(), m.bot_name, m.tone, m.temperature))
            .collect();
        sections.push(format!(
            "<builder_memory>\nPast sessions:\n{}\n</builder_memory>",
            memory_lines.join("\n")
        ));
    }

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
    llm_provider: &BoxLlmProvider,
) -> Result<AssemblyResult, BuilderError> {
    let config = &state.config;

    // Step 1: Map PurposeCategory to BotCategory
    let bot_category = match state.purpose_category.as_ref() {
        Some(PurposeCategory::Creative) => BotCategory::Creative,
        Some(PurposeCategory::Research) => BotCategory::Research,
        Some(PurposeCategory::SimpleUtility) => BotCategory::Utility,
        _ => BotCategory::Assistant,
    };

    // Step 2: Create the bot record (generates slug, creates directory, writes DEFAULT files)
    let request = CreateBotRequest {
        name: config.name.clone().unwrap_or("Unnamed Bot".to_string()),
        description: config.description.clone(),
        category: Some(bot_category),
        tags: config.tags.clone(),
    };
    let bot = bot_service.create_bot(request).await?;
    let bot_dir = bot_service.bot_dir(&bot.slug);

    // Step 3: Generate unique SOUL.md content via LLM and OVERWRITE the default
    let soul_content = generate_soul_content(llm_provider, state).await?;
    let soul_path = bot_dir.join("SOUL.md");
    // This creates version 2 (version 1 was the default from create_bot)
    bot_service.soul_service()
        .write_and_save_soul(&bot.id, &soul_content, &soul_path)
        .await?;

    // Step 4: Generate IDENTITY.md with builder-derived config and OVERWRITE the default
    let identity_content = generate_identity_content(config);
    let identity_path = bot_dir.join("IDENTITY.md");
    bot_service.soul_service()
        .write_identity(&identity_content, &identity_path)
        .await?;

    // Step 5: Generate USER.md seeded with context from builder conversation
    let user_content = generate_user_content(state);
    let user_path = bot_dir.join("USER.md");
    bot_service.soul_service()
        .write_user(&user_content, &user_path)
        .await?;

    // Step 6: Attach suggested skills
    // ... via SkillStore (Phase 6 system)

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

- **Using the bot-scoped KvStore for builder drafts:** The existing `KvStore` requires a `bot_id: &Uuid` for all operations. Builder drafts exist before any bot is created. Use a dedicated `builder_drafts` table instead.

- **Skipping the confirmation step:** Per locked decision, the builder must show a full summary and ask "Ready to create?" before building. Never auto-create without explicit confirmation.

- **Forgetting `additionalProperties: false` in the schema:** Claude's structured output REQUIRES this on all object types. Schemars does not add it by default. Post-process the schema before sending.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON Schema from Rust types | Manual JSON schema construction | `schemars` derive macro + `add_additional_properties_false` post-processor | Schemas stay in sync with Rust types automatically; manual schemas drift when types change |
| CLI multi-choice prompts | Custom terminal input parsing | `dialoguer::Select` with arrow-key navigation | Handles terminal raw mode, cursor movement, color themes, cross-platform input -- already in workspace |
| CLI progress indication | Custom spinner implementation | `indicatif::ProgressBar` with spinner style | Already used in bot creation flow; battle-tested animation and clearing |
| Draft persistence | JSON files or bot-scoped KvStore | Dedicated `builder_drafts` SQLite table with `SqliteBuilderDraftStore` | KvStore is bot-scoped (wrong scope); files lack atomicity; dedicated table has proper schema |
| Bot file creation | Duplicate directory/file creation | `BotService::create_bot` + `SoulService::write_and_save_soul` | Handles slug uniqueness, hash computation, version tracking, directory structure |
| Structured LLM output parsing | Regex/string parsing of LLM text | Claude `output_config.format` with `json_schema` | Guaranteed schema compliance at token generation level; zero parse errors |
| SOUL.md template structure | Custom markdown templating | LLM generation with structural prompt | User decided: "consistent Personality + Purpose + Boundaries sections, unique content per bot" -- the LLM ensures section consistency while writing unique content |
| Builder memory retrieval | Custom vector search | Dedicated `builder_memory` SQLite table with category index | Builder memory is simple category-based lookup, not semantic search; SQL index is fast |
| PurposeCategory to BotCategory mapping | Ad-hoc string conversion | Explicit `From<PurposeCategory> for BotCategory` impl | BotCategory has 4 variants; PurposeCategory has 7; mapping must be explicit |

**Key insight:** The builder system is primarily an LLM orchestration layer. Nearly all infrastructure (bot creation, soul management, skill attachment, LLM providers) already exists. The new code is: (1) the Forge system prompt, (2) the structured output schema types + post-processor, (3) the builder state accumulator, (4) the surface adapters, (5) the assembly orchestration, (6) two new SQLite tables for drafts and memory, and (7) `OutputConfig` types wired through the LLM provider stack.

## Common Pitfalls

### Pitfall 1: Structured Output Schema Missing `additionalProperties: false`

**What goes wrong:** The schema generated by `schemars::schema_for!()` does not include `"additionalProperties": false` on object types. Claude returns a 400 error: "additionalProperties must be set to false for objects".
**Why it happens:** Schemars follows standard JSON Schema conventions where `additionalProperties` defaults to `true`. Claude's structured output requires the opposite.
**How to avoid:** Always post-process the schemars output with `add_additional_properties_false()` before sending to the API. This must recursively walk all nested objects, including those inside `anyOf` branches and `$defs`.
**Warning signs:** 400 errors from Claude containing "additionalProperties".

### Pitfall 2: Context Window Bloat Across Builder Turns

**What goes wrong:** After 10+ builder turns, the accumulated conversation history plus system prompt exceeds the context window, causing failures or degraded output quality.
**Why it happens:** Each turn adds the full system prompt (with BuilderState JSON) plus conversation messages. The state grows linearly with each question.
**How to avoid:** Include `BuilderState` in the system prompt (which summarizes all decisions) but keep conversation messages minimal -- only include the last 3-5 exchange pairs, not the full history. The state object is the authoritative record of all decisions, not the conversation transcript.
**Warning signs:** Responses becoming vague or losing earlier context; token count approaching model limits.

### Pitfall 3: Structured Output with Streaming

**What goes wrong:** Attempting to use structured output (`output_config.format`) with streaming causes confusion because partial JSON arrives incrementally.
**Why it happens:** Structured output works with streaming (confirmed by Anthropic docs), but you receive JSON tokens incrementally. You must buffer the full response before parsing.
**How to avoid:** For the builder flow, use non-streaming (`complete()`) since each turn is a discrete question. Streaming adds complexity without benefit here -- the user is waiting for the next question, not watching text appear. Reserve streaming for the actual bot chat, not the builder wizard.
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
**How to avoid:** Embed Forge's SOUL.md as a compile-time constant (`include_str!` or a `const`). Forge is a system component, not a user bot. This avoids slug collisions, filesystem dependency, and makes Forge available immediately without initialization. The embedded constant lives in `crates/boternity-core/src/builder/forge_soul.rs`.
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

### Pitfall 9: Assembly Creates Default Files Then Overwrites

**What goes wrong:** `BotService::create_bot` writes default SOUL.md (version 1), IDENTITY.md, and USER.md. Then the builder overwrites all three with custom content. The soul gets version 2 immediately, and version 1 is a pointless default that clutters the version history.
**Why it happens:** `create_bot` was designed for the simple flow where defaults are the final content.
**How to avoid:** Accept this as a minor inefficiency in v1. The version history shows v1 (default) and v2 (builder-generated), which is actually useful for debugging. If this becomes a UX concern, a future refactor could add a `create_bot_skeleton` method that creates the record and directory without writing default files. For now, the double-write is correct and harmless.
**Warning signs:** Users seeing "Version 1" in soul history that they didn't create.

### Pitfall 10: Bedrock Provider Not Forwarding `output_config`

**What goes wrong:** `output_config` is added to `CompletionRequest` and `AnthropicRequest` but not to `BedrockRequest`. Bedrock calls silently ignore the structured output constraint and return free-form text.
**Why it happens:** Forgetting to wire the new field through the Bedrock request type.
**How to avoid:** Add `output_config: Option<OutputConfig>` to `BedrockRequest` and wire it in `to_bedrock_request()`. Add a test that verifies the field appears in serialized Bedrock requests when present.
**Warning signs:** Builder working fine with direct Anthropic but producing parse errors on Bedrock.

## Code Examples

### CompletionRequest with Structured Output

```rust
// Source: Verified against Anthropic API docs (https://platform.claude.com/docs/en/build-with-claude/structured-outputs)

// New types in boternity-types/src/llm.rs:

/// Configuration for structured output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub format: OutputFormat,
}

/// Output format specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputFormat {
    /// Constrain output to a JSON schema.
    JsonSchema {
        schema: serde_json::Value,
    },
}

// Updated CompletionRequest:
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,  // NEW
}
```

### LLM Builder Agent Implementation

```rust
// Source: Derived from codebase analysis of LlmProvider, AnthropicProvider, BoxLlmProvider

/// Concrete implementation of BuilderAgent using an LLM provider.
struct LlmBuilderAgent {
    provider: BoxLlmProvider,
    draft_store: Arc<dyn BuilderDraftStore>,
    memory_store: Arc<dyn BuilderMemoryStore>,
    model: String,
    forge_soul: String, // embedded const
    schema: serde_json::Value, // pre-computed, post-processed BuilderTurn schema
}

impl BuilderAgent for LlmBuilderAgent {
    async fn start(&self, initial_description: &str) -> Result<(String, BuilderTurn), BuilderError> {
        let session_id = Uuid::now_v7().to_string();
        let state = BuilderState::new(session_id.clone(), initial_description);

        // Build system prompt with empty state
        let system = build_forge_system_prompt(&self.forge_soul, &state, &[], &[]);

        // Make LLM call with structured output
        let request = CompletionRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: MessageRole::User,
                content: initial_description.to_string(),
            }],
            system: Some(system),
            max_tokens: 2048,
            temperature: Some(0.7),
            stream: false,
            stop_sequences: None,
            output_config: Some(OutputConfig {
                format: OutputFormat::JsonSchema {
                    schema: self.schema.clone(),
                },
            }),
        };

        let response = self.provider.complete(&request).await?;
        let turn: BuilderTurn = serde_json::from_str(&response.content)?;

        // Save initial draft
        self.draft_store.save_draft(&session_id, &state).await?;

        Ok((session_id, turn))
    }

    async fn answer(&self, session_id: &str, answer: BuilderAnswer) -> Result<BuilderTurn, BuilderError> {
        // Load existing state
        let mut state = self.draft_store.load_draft(session_id).await?
            .ok_or(BuilderError::SessionNotFound)?;

        // Record the answer in state
        state.record_answer(answer);

        // Build system prompt with accumulated state
        let memories = self.memory_store
            .recall_by_category(state.purpose_category.as_ref().unwrap_or(&PurposeCategory::Custom), 5)
            .await
            .unwrap_or_default();
        let system = build_forge_system_prompt(&self.forge_soul, &state, &memories, &[]);

        // Build messages: only last 3-5 exchanges to avoid context bloat
        let recent_messages = state.recent_messages(5);

        let request = CompletionRequest {
            model: self.model.clone(),
            messages: recent_messages,
            system: Some(system),
            max_tokens: 2048,
            temperature: Some(0.7),
            stream: false,
            stop_sequences: None,
            output_config: Some(OutputConfig {
                format: OutputFormat::JsonSchema {
                    schema: self.schema.clone(),
                },
            }),
        };

        let response = self.provider.complete(&request).await?;
        let turn: BuilderTurn = serde_json::from_str(&response.content)?;

        // Update and save state
        state.updated_at = chrono::Utc::now();
        self.draft_store.save_draft(session_id, &state).await?;

        Ok(turn)
    }

    // ... go_back, get_draft, assemble implementations
}
```

### Builder Memory for Past Session Suggestions

```rust
/// Record builder choices for future session suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderMemoryEntry {
    pub id: String, // UUID
    pub purpose_category: PurposeCategory,
    pub bot_name: String,
    pub personality_traits: Vec<String>,
    pub tone: String,
    pub model: String,
    pub temperature: f64,
    pub skills_used: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// Implementation in SqliteBuilderMemoryStore:
impl BuilderMemoryStore for SqliteBuilderMemoryStore {
    async fn record_memory(&self, entry: &BuilderMemoryEntry) -> Result<(), BuilderError> {
        let choices_json = serde_json::to_string(entry)?;
        let category_str = serde_json::to_string(&entry.purpose_category)?;
        sqlx::query(
            "INSERT INTO builder_memory (id, purpose_category, bot_name, choices_json, created_at)
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&entry.id)
        .bind(category_str.trim_matches('"'))
        .bind(&entry.bot_name)
        .bind(&choices_json)
        .bind(entry.created_at.to_rfc3339())
        .execute(&self.pool.writer)
        .await?;
        Ok(())
    }

    async fn recall_by_category(
        &self, category: &PurposeCategory, limit: usize,
    ) -> Result<Vec<BuilderMemoryEntry>, BuilderError> {
        let category_str = serde_json::to_string(category)?;
        let rows = sqlx::query(
            "SELECT choices_json FROM builder_memory
             WHERE purpose_category = ? ORDER BY created_at DESC LIMIT ?"
        )
        .bind(category_str.trim_matches('"'))
        .bind(limit as i64)
        .fetch_all(&self.pool.reader)
        .await?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let json: String = row.try_get("choices_json")?;
            if let Ok(entry) = serde_json::from_str::<BuilderMemoryEntry>(&json) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Tool use for structured extraction | `output_config.format` with JSON schema | 2025-11 (beta), 2026 (GA) | Guaranteed schema compliance at token level; no parse errors; simpler than tool_use for response formatting |
| `output_format` parameter (beta) | `output_config.format` (GA) | 2026 | Old parameter deprecated; new location in API; beta headers no longer needed for Opus 4.6/Sonnet 4.5/Haiku 4.5 |
| Free-text LLM + regex parsing | Structured output with schemars-derived schemas | 2025-2026 | Zero parsing failures; type-safe response handling; schemas auto-generated from Rust types |
| Static question wizards | LLM-adaptive question generation | 2024-2025 | Questions adapt to context; no predetermined paths; better UX for diverse bot types |

**Deprecated/outdated:**
- `output_format` top-level parameter: Deprecated, moved to `output_config.format`. Still works temporarily.
- `anthropic-beta: structured-outputs-2025-11-13` header: No longer needed for GA models (Opus 4.6, Sonnet 4.5, Opus 4.5, Haiku 4.5).
- Tool use for response formatting: Still works but `output_config.format` is cleaner for "return this JSON shape" use cases. Use tool use when you need the LLM to invoke actual functions.

## Open Questions

1. **Schema caching across builder turns**
   - What we know: Claude caches compiled grammar artifacts for 24 hours. The builder schema is identical across all turns of all sessions.
   - What's unclear: Whether the first turn of the first session will have noticeable extra latency from grammar compilation, and whether this affects UX.
   - Recommendation: Pre-compute the schema once at startup (`schema_for!(BuilderTurn)` + post-processing). The schema is a `serde_json::Value` stored in the `LlmBuilderAgent`. First-turn latency is expected but should be <1 second. After that, cached.

2. **Forge personality content**
   - What we know: Forge needs a SOUL.md-like personality definition. It should be warm, encouraging, slightly casual.
   - What's unclear: Exact content. This is in "Claude's Discretion."
   - Recommendation: Write it as a const string in `crates/boternity-core/src/builder/forge_soul.rs` following the same Personality + Purpose + Boundaries structure as regular bots. ~30-50 lines. Include specific guidance like "You use casual language but never baby-talk the user."

3. **Web UI component integration**
   - What we know: The wizard needs to live in the existing React SPA. Entry points are "Create Bot" button on dashboard and "Reconfigure" on bot detail.
   - What's unclear: Exact React component architecture for the wizard overlay and Forge chat.
   - Recommendation: This is a frontend concern for the web phase. The backend (REST + WebSocket) is ready. The React wizard renders `BuilderTurn` JSON as form components. Forge chat reuses the existing chat UI component but connects to `/ws/builder/{session_id}` instead of `/ws/events`.

## Sources

### Primary (HIGH confidence)
- [Anthropic Structured Outputs docs (GA)](https://platform.claude.com/docs/en/build-with-claude/structured-outputs) - Verified 2026-02-14: GA API for `output_config.format` with JSON schema; supports Opus 4.6, Sonnet 4.5, Opus 4.5, Haiku 4.5; `anyOf` supported; `additionalProperties: false` required; no beta header needed
- [Schemars attributes docs](https://graham.cool/schemars/deriving/attributes/) - Confirms `#[serde(tag = "...")]` support for internally tagged enum schema generation
- [Schemars serde integration](https://graham.cool/schemars/examples/2-serde_attrs/) - Confirms schemars checks serde attributes and adjusts schema accordingly
- Codebase verification (2026-02-14): `KvStore` trait, `SqliteKvStore`, `CompletionRequest`, `AnthropicRequest`, `BedrockRequest`, `AnthropicProvider::to_anthropic_request`, `BedrockProvider::to_bedrock_request`, `BotService::create_bot`, `SoulService::write_and_save_soul`, `ws_handler`, `router.rs`, `AppState` -- all read and verified

### Secondary (MEDIUM confidence)
- [Anthropic Building Effective Agents](https://www.anthropic.com/engineering/building-effective-agents) - Agent architecture patterns; orchestrator-workers; multi-turn conversation design
- [Schemars GitHub issues](https://github.com/GREsau/schemars/issues/273) - Known issue with `rename_all_fields` on tagged enums (our case uses `rename_all` on the enum itself, which works)
- [Thomas Wiegold blog on Claude structured output](https://thomas-wiegold.com/blog/claude-api-structured-output/) - Community verification of API shape and behavior

### Tertiary (LOW confidence)
- Schemars `anyOf` schema generation for internally-tagged enums -- the exact generated schema structure was inferred from library behavior and documentation rather than empirically testing `schema_for!()` output. Recommend generating and printing the schema during implementation to verify.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in workspace except schemars; schemars is the de facto JSON Schema generator in Rust
- Architecture patterns (surface adapter, state accumulator): HIGH -- derived from codebase analysis and standard trait-based architecture
- Structured output integration: HIGH -- verified against official Anthropic GA documentation; `anyOf` confirmed supported; `additionalProperties: false` requirement documented
- Storage strategy (dedicated tables): HIGH -- KvStore bot-scoped limitation verified by reading actual trait/implementation; dedicated tables are the correct pattern per existing codebase conventions
- Service method signatures: HIGH -- all signatures read directly from source code; assembly sequence verified
- CompletionRequest/provider extension: HIGH -- all three request types and both providers read in full; backward-compatible approach confirmed
- Web builder protocol: MEDIUM -- design is sound based on existing patterns but untested; REST vs WebSocket split is a design decision
- Schemars tagged enum schema shape: MEDIUM -- inferred from documentation, not empirically tested; recommend verifying during implementation

**Research date:** 2026-02-14 (deep-dive update)
**Valid until:** 2026-03-14 (structured output API is GA and stable; schemars 1.x is stable)
