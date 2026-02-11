# Phase 2: Single-Agent Chat + LLM - Research

**Researched:** 2026-02-11
**Domain:** LLM provider abstraction, streaming chat, agent engine, session memory, observability (Rust)
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Streaming chat feel
- **Rendering:** Character-by-character streaming -- each token appears as it arrives, typewriter effect
- **Thinking indicator:** Animated spinner (e.g., braille) with "thinking..." text while waiting for first token
- **Message formatting:** Full markdown rendering in terminal -- bold, italic, code blocks with syntax highlighting, lists, headers
- **Bot identity in chat:** Bot emoji + colored name + accent color on every bot message -- each bot visually distinct
- **Metadata per message:** Timestamps on each message + token count after bot responses -- always visible
- **Stats footer:** After every bot response show: tokens used, response time, model -- always visible (e.g., "| 128 tokens . 1.2s . claude-sonnet-4-20250514")
- **Multiline input:** Shift+Enter for newlines AND paste-aware (pasted multiline stays as-is until sent) -- both behaviors
- **In-chat commands:** Slash commands (/help, /clear, /exit, /new, /history) for discoverability + keyboard shortcuts (Ctrl+L, Ctrl+D) for power users -- both
- **Long output:** Stream everything inline, user scrolls up -- simple and predictable, no auto-paging
- **Error display:** Show error in chat, then offer choice: retry / switch model / abort -- user stays in control
- **Welcome banner:** Full banner on session start -- bot emoji + name + description + model + session info + hint about /help
- **User prompt:** Styled "You >" in a distinct color -- clear visual separation between user and bot

#### Session lifecycle
- **Start:** `bnity chat <bot>` always starts a new session by default; user can opt to resume a previous session (e.g., `--resume` flag or session ID argument)
- **End:** Explicit exit only (/exit or Ctrl+D) -- no auto-timeout, no surprises
- **Context window management:** Sliding window with LLM-generated summary -- when approaching context limit, older messages are summarized and kept as context; bot stays coherent without user noticing
- **Parallel sessions:** Fully supported -- multiple terminal tabs can have independent active sessions with the same bot simultaneously
- **Session browser:** `bnity sessions <bot>` lists past sessions with date, duration, title, and preview of first/last messages -- pick one to resume
- **Session titles:** Auto-generated from first exchange by the LLM (like ChatGPT's conversation naming)
- **Session export:** Both Markdown (human-readable, default) and JSON (--json flag, full metadata) -- consistent with Phase 1 CLI conventions
- **Session delete:** `bnity delete session <id>` with confirmation prompt, consistent with bot delete pattern from Phase 1

#### Cross-session memory
- **Extraction timing:** Both periodic during session (every N messages) + final extraction at session end -- resilient to crashes
- **Extraction logic:** LLM judges what's worth remembering -- facts, preferences, decisions, relationships -- no predefined categories
- **Memory recall:** Automatic and invisible -- relevant memories injected into context silently; bot just "knows" things without announcing "I remember..."
- **Memory loading:** All memories loaded into system prompt at session start -- full recall, works for early usage with modest memory counts
- **Memory browser:** `bnity memories <bot>` lists all extracted memories with full provenance: content, source session title, date extracted, and the message that triggered it -- user can delete individual entries
- **Manual memory injection:** `bnity remember <bot> 'fact'` or /remember in chat -- explicit knowledge injection supported
- **Memory wipe:** `bnity forget <bot>` wipes all memories with confirmation -- clean slate
- **Crash recovery:** Best-effort -- periodic extraction already captured some; on ungraceful exit, extract from whatever messages were saved to disk
- **Memory notification:** Silent extraction -- no notification to user when memories are saved
- **Memory scope:** Shared across all sessions for the same bot -- every session reads from and writes to the same memory pool
- **Cross-bot memory:** Own memories only by default in Phase 2, but data model should support cross-bot access for Phase 3
- **USER.md relationship:** USER.md and memory are separate systems -- USER.md is curated standing instructions, memory is auto-extracted knowledge. No sync between them.
- **Memory limit:** No limit -- keep everything forever, storage is local. User can manually prune via memory browser.

#### Personality expression
- **Greeting:** Bot speaks first -- sends a personality-driven greeting message when session opens; feels like the bot is waiting for you
- **Personality strength:** Strong personality -- the bot's voice, tone, and personality should be unmistakable in every response; you should be able to tell which bot is talking without seeing the name
- **Context mapping:** All three files (SOUL.md + IDENTITY.md config + USER.md) compose into the system prompt -- LLM sees everything as its identity/instructions
- **Bot distinctness:** Radically different -- a creative writing bot and a research bot should feel like completely different beings; vocabulary, tone, response length, formatting all differ based on soul

### Claude's Discretion
- Exact markdown rendering library/approach for terminal
- Spinner animation style and timing
- System prompt template structure and ordering of SOUL/IDENTITY/USER sections
- Memory extraction prompt design
- Sliding window summary prompt and threshold
- Token counting approach (estimated vs exact)
- Session resume UX details
- Keyboard shortcut assignments
- Color scheme and theming details
- Auto-title generation prompt

### Deferred Ideas (OUT OF SCOPE)
- Vector-based semantic memory search (relevance-based recall instead of loading all) -- Phase 3
- Cross-bot shared memory with trust partitioning -- Phase 3
- Memory deduplication and merging -- Phase 3
- Web UI chat interface -- Phase 4
</user_constraints>

## Summary

Phase 2 transforms Boternity from a bot identity manager into a conversational system. The core challenges are: (1) building a pluggable LLM provider abstraction that supports streaming, (2) implementing a single-agent execution loop that reads the bot's SOUL.md and maintains conversation context, (3) delivering token-by-token streaming output in the CLI with rich markdown rendering, (4) extracting and persisting session memory key points, (5) persisting full chat history, and (6) wiring up structured tracing with OpenTelemetry GenAI semantic conventions.

The Rust ecosystem for LLM integration is fragmented and fast-moving. The recommended approach is a thin custom provider abstraction with `reqwest` + `reqwest-eventsource` for the Anthropic provider, keeping full control over the streaming protocol, token counting, and error handling. This abstraction is designed from day one for future providers (Phase 3), with the trait interface inspired by patterns from `genai`, `rig-core`, and `flyllm`.

**Critical alignment with existing codebase:** The project uses Rust 2024 edition with RPITIT (no `async_trait` macro), raw `sqlx` (not SeaORM), generic services over trait bounds (not trait objects), and `BoxSecretProvider` with blanket impl for dynamic dispatch. All Phase 2 code MUST follow these established patterns. The LlmProvider trait is a special case: it uses `Pin<Box<dyn Stream>>` for streaming (streams are inherently object-safe via boxing) but regular async methods use RPITIT pattern where possible. For dynamic dispatch of the provider (needed because the provider is selected at runtime based on bot config), a `BoxLlmProvider` wrapper with blanket impl follows the established `BoxSecretProvider` pattern.

**Primary recommendation:** Build a thin custom LLM provider trait (`LlmProvider`) with a direct `reqwest`+`reqwest-eventsource` Anthropic implementation. Use `termimad` + `syntect` for markdown rendering with syntax-highlighted code blocks. Use `rustyline-async` for the interactive chat input loop. Use `indicatif` (already in workspace) for the thinking spinner. Use `tracing` spans following OpenTelemetry GenAI semantic conventions for every LLM call. Extract session memory via an LLM summarization call at session end and periodically during long sessions.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `reqwest` | 0.12.x | HTTP client for LLM API calls | De facto Rust HTTP client; async, streaming body support, TLS. Already in use for Axum tower-http. |
| `reqwest-eventsource` | 0.6.0 | SSE stream consumer for LLM streaming | Wraps reqwest with proper SSE parsing; handles retry/reconnection. 2.3M downloads. |
| `eventsource-stream` | 0.2.x | Low-level SSE byte stream parser | Used by reqwest-eventsource under the hood |
| `termimad` | 0.34.1 | Markdown rendering in terminal | Purpose-built for terminal markdown: wrapping, tables, styled text. Uses crossterm backend. |
| `syntect` | 5.x | Syntax highlighting for code blocks | Standard Rust syntax highlighting library. Used by bat, delta, zola. Pure-rust regex backend available. |
| `rustyline-async` | 0.4.7 | Async readline for CLI chat input | Async-compatible line editor with crossterm backend. Supports multiline, history, ctrl-c/ctrl-d. |
| `crossterm` | 0.28.x | Terminal manipulation (raw mode, colors, cursor) | Cross-platform terminal library; already used by rustyline-async and termimad |
| `indicatif` | 0.17.x | Spinner animation while waiting for first token | Already in workspace dependencies. Supports steady_tick for background animation. |
| `tracing` | 0.1.x | Structured logging and span instrumentation | Already in workspace. THE async Rust instrumentation crate. |
| `tracing-subscriber` | 0.3.x | Log formatting, filtering, layered subscribers | Already in workspace. Standard companion to tracing. |
| `tracing-opentelemetry` | 0.32.1 | Bridge tracing spans to OpenTelemetry | Connects Rust spans to OTel distributed traces. Published 2026-01-12. |
| `opentelemetry` | 0.31.0 | OpenTelemetry API | Industry standard observability API. Published 2025-09-25. |
| `opentelemetry_sdk` | 0.31.x | OTel SDK implementation | Required runtime for OTel |
| `opentelemetry-stdout` | 0.31.x | OTel stdout exporter (dev/debug) | Logs traces to console for local development |
| `sqlx` | 0.8.x | Async SQL for chat/memory persistence | Already in workspace. Raw SQL queries with compile-time checking. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serde` | 1.0.x | Serialization for API request/response types | Every API call (already in workspace) |
| `serde_json` | 1.0.x | JSON serialization | Parsing Anthropic SSE data payloads (already in workspace) |
| `tokio` | 1.x | Async runtime (already in stack) | Everything async (already in workspace) |
| `futures-util` | 0.3.x | Stream combinators for SSE processing | Processing SSE event streams |
| `async-stream` | 0.3.x | Async stream creation macros | Creating streams with `stream!`/`try_stream!` macros for SSE event transformation |
| `uuid` | 1.20.x | Session IDs, message IDs | Every new chat session and message (already in workspace) |
| `chrono` | 0.4.x | Timestamps for messages and sessions | Chat history timestamps (already in workspace) |
| `thiserror` | 1.x | Error type derivation | LLM provider errors, agent errors (already in workspace) |
| `secrecy` | 0.10.x | API key wrapping | API keys never leak to logs/debug |
| `console` | 0.15.x | Terminal styling (colors, bold, dim) | Already in workspace. Chat UI styling. |
| `pin-project-lite` | 0.2.x | Safe pin projections for stream types | Custom stream combinators |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom provider + reqwest | `llm` crate (graniet) v1.3.7 | Multi-provider out of box but: rapid version churn (1.2.4->1.3.7 in ~1 month), 68% doc coverage, owns streaming/retry logic reducing control. |
| Custom provider + reqwest | `genai` crate v0.5.3 | More stable design, better streaming engine (v0.5), but pre-1.0 API may break. Good fallback if custom is too expensive. |
| `termimad` | `ratatui` | Full TUI framework is overkill for a chat interface. Termimad is markdown-first, lighter. |
| `termimad` | Raw crossterm + pulldown-cmark | Full control but you rebuild all markdown rendering. Massive effort for tables, wrapping, styling. |
| `rustyline-async` | `reedline` (nushell) | More features (syntax highlighting, completions) but heavier; designed for shell editors not chat input. |
| `reqwest-eventsource` | `reqwest-sse` | Newer, lighter, but less battle-tested. reqwest-eventsource has 2.3M downloads and proven retry logic. |

**Installation (workspace Cargo.toml additions):**
```toml
# New workspace dependencies for Phase 2
reqwest = { version = "0.12", features = ["json", "stream"] }
reqwest-eventsource = "0.6"
eventsource-stream = "0.2"
async-stream = "0.3"
futures-util = "0.3"
termimad = "0.34"
syntect = { version = "5", default-features = false, features = ["default-fancy"] }
rustyline-async = "0.4"
secrecy = { version = "0.10", features = ["serde"] }
pin-project-lite = "0.2"
opentelemetry = "0.31"
opentelemetry_sdk = "0.31"
opentelemetry-stdout = "0.31"
tracing-opentelemetry = "0.32"
```

## Architecture Patterns

### Recommended Module Structure (Phase 2 additions)
```
crates/
  boternity-types/
    src/
      models/
        llm.rs           # LLM request/response types, StreamChunk, Usage
        chat.rs          # ChatSession, ChatMessage, MessageRole
        agent.rs         # AgentConfig
        memory.rs        # MemoryEntry, KeyPoint

  boternity-core/
    src/
      llm/
        mod.rs           # Re-exports
        provider.rs      # LlmProvider trait + ProviderCapabilities
        types.rs         # CompletionRequest, CompletionResponse, StreamEvent
        token_budget.rs  # TokenBudget, context allocation
        box_provider.rs  # BoxLlmProvider for dynamic dispatch (like BoxSecretProvider)
      agent/
        mod.rs           # Re-exports
        engine.rs        # AgentEngine: single-agent execution loop
        context.rs       # AgentContext: soul, memory, conversation state
        prompt.rs        # SystemPromptBuilder: assembles soul + memory + user msg
      chat/
        mod.rs           # Re-exports
        service.rs       # ChatService: start_session, send_message, etc.
        session.rs       # Session management, turn tracking
      memory/
        mod.rs           # Re-exports
        extractor.rs     # SessionMemoryExtractor: extracts key points via LLM
        store.rs         # SessionMemoryStore trait (persistence)

  boternity-infra/
    src/
      llm/
        mod.rs           # Re-exports
        anthropic/
          mod.rs         # AnthropicProvider: implements LlmProvider
          client.rs      # Raw HTTP client for Anthropic Messages API
          streaming.rs   # SSE stream parser, event type handling, state machine
          types.rs       # Anthropic-specific request/response types
      sqlite/
        chat.rs          # ChatRepository: sqlx impl for chat persistence
        memory.rs        # MemoryRepository: sqlx impl for memory persistence

  boternity-observe/     # NEW crate for observability
    src/
      lib.rs
      tracing_setup.rs   # Initialize tracing subscriber + OTel layer
      genai_attrs.rs     # OpenTelemetry GenAI semantic convention constants
```

### Pattern 1: LLM Provider Trait with RPITIT + BoxProvider
**What:** Define an `LlmProvider` trait in `boternity-core` using RPITIT for regular async methods and `Pin<Box<dyn Stream>>` for streaming. Create a `BoxLlmProvider` wrapper for runtime dynamic dispatch, following the established `BoxSecretProvider` pattern.
**When to use:** Every LLM interaction in the system.
**Why:** boternity-core must never depend on boternity-infra. The project uses RPITIT (not async_trait). Streaming returns boxed dyn Stream because streams need to be object-safe for dynamic dispatch. The `BoxLlmProvider` enables runtime provider selection based on bot config.

```rust
// In boternity-core/src/llm/provider.rs
// NOTE: Uses RPITIT pattern consistent with existing codebase (no async_trait)

use futures_util::Stream;
use std::pin::Pin;

#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tool_calling: bool,
    pub vision: bool,
    pub extended_thinking: bool,
    pub max_context_tokens: u32,
    pub max_output_tokens: u32,
}

/// Provider-agnostic LLM interface.
/// Uses RPITIT for regular async methods per project convention.
/// Streaming uses Pin<Box<dyn Stream>> because streams are inherently dynamic.
pub trait LlmProvider: Send + Sync {
    /// Provider identifier (e.g., "anthropic", "openai")
    fn name(&self) -> &str;

    /// What this provider supports
    fn capabilities(&self) -> &ProviderCapabilities;

    /// Non-streaming completion
    fn complete(
        &self,
        request: &CompletionRequest,
    ) -> impl std::future::Future<Output = Result<CompletionResponse, LlmError>> + Send;

    /// Streaming completion -- returns a boxed stream of events.
    /// Uses Pin<Box<dyn Stream>> (not RPITIT) because:
    /// 1. Streams are consumed dynamically (different providers at runtime)
    /// 2. The stream captures the HTTP connection and must be 'static
    fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>>;

    /// Count tokens in a request (pre-send estimation)
    fn count_tokens(
        &self,
        request: &CompletionRequest,
    ) -> impl std::future::Future<Output = Result<TokenCount, LlmError>> + Send;
}
```

```rust
// In boternity-core/src/llm/box_provider.rs
// Dynamic dispatch wrapper following BoxSecretProvider pattern from Phase 1

use std::pin::Pin;
use futures_util::Stream;

/// Object-safe wrapper for dynamic dispatch of LlmProvider.
/// Follows the same BoxSecretProvider pattern established in Phase 1 (01-04).
pub struct BoxLlmProvider {
    inner: Box<dyn LlmProviderDyn + Send + Sync>,
}

/// Object-safe version of LlmProvider (async fns return boxed futures)
trait LlmProviderDyn: Send + Sync {
    fn name(&self) -> &str;
    fn capabilities(&self) -> &ProviderCapabilities;
    fn complete(
        &self,
        request: &CompletionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<CompletionResponse, LlmError>> + Send + '_>>;
    fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>>;
    fn count_tokens(
        &self,
        request: &CompletionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<TokenCount, LlmError>> + Send + '_>>;
}

/// Blanket impl: any T: LlmProvider automatically implements LlmProviderDyn
impl<T: LlmProvider> LlmProviderDyn for T {
    fn name(&self) -> &str { LlmProvider::name(self) }
    fn capabilities(&self) -> &ProviderCapabilities { LlmProvider::capabilities(self) }
    fn complete(
        &self,
        request: &CompletionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<CompletionResponse, LlmError>> + Send + '_>> {
        Box::pin(LlmProvider::complete(self, request))
    }
    fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
        LlmProvider::stream(self, request)
    }
    fn count_tokens(
        &self,
        request: &CompletionRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<TokenCount, LlmError>> + Send + '_>> {
        Box::pin(LlmProvider::count_tokens(self, request))
    }
}

impl BoxLlmProvider {
    pub fn new<T: LlmProvider + 'static>(provider: T) -> Self {
        Self { inner: Box::new(provider) }
    }
}
```

### Pattern 2: Anthropic SSE Stream Processing (State Machine)
**What:** Parse Anthropic's streaming SSE events into the unified `StreamEvent` enum using a state machine that accumulates tool input JSON fragments per content block index.
**When to use:** Inside the Anthropic provider implementation in `boternity-infra`.

The Anthropic SSE event flow (verified against official docs, 2026-02-11):
1. `message_start` -- contains Message object with empty content and initial usage
2. Per content block: `content_block_start` -> N x `content_block_delta` -> `content_block_stop`
3. `message_delta` -- top-level changes (stop_reason, cumulative usage)
4. `message_stop` -- final event
5. `ping` events may appear anywhere
6. `error` events may appear mid-stream (e.g., `overloaded_error`)

Delta types: `text_delta`, `thinking_delta`, `input_json_delta`, `signature_delta`
Content block types: `text`, `tool_use`, `thinking`, `server_tool_use`, `web_search_tool_result`

**Critical:** Tool use input arrives as partial JSON fragments. Accumulate per block index, parse only after `content_block_stop`.

```rust
// In boternity-infra/src/llm/anthropic/streaming.rs
// State machine for SSE event processing

use std::collections::HashMap;

struct StreamState {
    tool_input_buffers: HashMap<u32, ToolUseAccumulator>,
    message_id: Option<String>,
    model: Option<String>,
}

struct ToolUseAccumulator {
    id: String,
    name: String,
    json_buffer: String,
}

pub fn create_anthropic_stream(
    client: &reqwest::Client,
    url: &str,
    body: AnthropicRequest,
    api_key: &secrecy::SecretString,
) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
    let client = client.clone();
    let url = url.to_string();
    let api_key_str = api_key.expose_secret().to_string();

    Box::pin(async_stream::try_stream! {
        let request = client.post(&url)
            .header("x-api-key", &api_key_str)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body);

        let mut es = EventSource::new(request)
            .expect("EventSource creation should not fail");

        let mut state = StreamState {
            tool_input_buffers: HashMap::new(),
            message_id: None,
            model: None,
        };

        while let Some(event) = es.next().await {
            match event {
                Ok(Event::Open) => {
                    yield StreamEvent::Connected;
                }
                Ok(Event::Message(msg)) => {
                    let sse_event: AnthropicSseEvent = serde_json::from_str(&msg.data)
                        .map_err(LlmError::Deserialization)?;

                    match sse_event {
                        AnthropicSseEvent::MessageStart { message } => {
                            state.message_id = Some(message.id);
                            state.model = Some(message.model);
                            if let Some(usage) = message.usage {
                                yield StreamEvent::Usage(Usage {
                                    input_tokens: usage.input_tokens,
                                    output_tokens: usage.output_tokens,
                                    ..Default::default()
                                });
                            }
                        }
                        AnthropicSseEvent::ContentBlockStart { index, content_block } => {
                            if let AnthropicContentBlock::ToolUse { id, name, .. } = &content_block {
                                state.tool_input_buffers.insert(index, ToolUseAccumulator {
                                    id: id.clone(),
                                    name: name.clone(),
                                    json_buffer: String::new(),
                                });
                            }
                            // Yield for CLI to know what type of content is coming
                            yield StreamEvent::ContentBlockStart { index, content_type: content_block.type_name().to_string() };
                        }
                        AnthropicSseEvent::ContentBlockDelta { index, delta } => {
                            match delta {
                                AnthropicDelta::TextDelta { text } => {
                                    yield StreamEvent::TextDelta { index, text };
                                }
                                AnthropicDelta::ThinkingDelta { thinking } => {
                                    yield StreamEvent::ThinkingDelta { index, thinking };
                                }
                                AnthropicDelta::InputJsonDelta { partial_json } => {
                                    if let Some(acc) = state.tool_input_buffers.get_mut(&index) {
                                        acc.json_buffer.push_str(&partial_json);
                                    }
                                }
                                AnthropicDelta::SignatureDelta { .. } => {
                                    // Signature for thinking block verification -- skip for now
                                }
                            }
                        }
                        AnthropicSseEvent::ContentBlockStop { index } => {
                            if let Some(acc) = state.tool_input_buffers.remove(&index) {
                                let input = if acc.json_buffer.is_empty() {
                                    serde_json::Value::Object(Default::default())
                                } else {
                                    serde_json::from_str(&acc.json_buffer)
                                        .map_err(LlmError::Deserialization)?
                                };
                                yield StreamEvent::ToolUseComplete {
                                    id: acc.id,
                                    name: acc.name,
                                    input,
                                };
                            }
                            yield StreamEvent::ContentBlockStop { index };
                        }
                        AnthropicSseEvent::MessageDelta { delta, usage } => {
                            let stop_reason = match delta.stop_reason.as_deref() {
                                Some("end_turn") => StopReason::EndTurn,
                                Some("tool_use") => StopReason::ToolUse,
                                Some("max_tokens") => StopReason::MaxTokens,
                                Some("stop_sequence") => StopReason::StopSequence,
                                Some("pause_turn") => StopReason::PauseTurn,
                                _ => StopReason::EndTurn,
                            };
                            yield StreamEvent::Usage(Usage {
                                input_tokens: usage.input_tokens,
                                output_tokens: usage.output_tokens,
                                cache_creation_input_tokens: usage.cache_creation_input_tokens,
                                cache_read_input_tokens: usage.cache_read_input_tokens,
                            });
                            yield StreamEvent::MessageDelta { stop_reason };
                        }
                        AnthropicSseEvent::MessageStop {} => {
                            yield StreamEvent::Done;
                        }
                        AnthropicSseEvent::Ping {} => { /* keepalive */ }
                        AnthropicSseEvent::Error { error } => {
                            let err = match error.error_type.as_str() {
                                "overloaded_error" => LlmError::Overloaded(error.message),
                                "rate_limit_error" => LlmError::RateLimited { retry_after_ms: None },
                                "authentication_error" => LlmError::AuthenticationFailed,
                                _ => LlmError::Provider { message: error.message },
                            };
                            Err(err)?;
                        }
                    }
                }
                Err(reqwest_eventsource::Error::StreamEnded) => break,
                Err(e) => Err(LlmError::Stream(e.to_string()))?,
            }
        }
    })
}
```

### Pattern 3: Chat Service with Generic Repository Traits (RPITIT)
**What:** Follow the existing `BotService<B, S, F, H>` pattern -- services are generic over repository traits. No trait objects for repositories.
**When to use:** ChatService, MemoryService, and any new service in Phase 2.

```rust
// In boternity-core/src/chat/service.rs
// Following the BotService<B, S, F, H> pattern from Phase 1

pub struct ChatService<C: ChatRepository, M: MemoryRepository> {
    chat_repo: C,
    memory_repo: M,
}

// In boternity-core/src/chat/repository.rs (trait)
pub trait ChatRepository: Send + Sync {
    fn create_session(
        &self,
        session: &ChatSession,
    ) -> impl std::future::Future<Output = Result<ChatSession, RepositoryError>> + Send;

    fn save_message(
        &self,
        message: &ChatMessage,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    fn get_session(
        &self,
        session_id: &Uuid,
    ) -> impl std::future::Future<Output = Result<Option<ChatSession>, RepositoryError>> + Send;

    // ... etc
}
```

### Pattern 4: System Prompt Assembly
**What:** Build the system prompt from SOUL.md + IDENTITY.md config + USER.md + session memories. Use XML tags for section boundaries (prevents prompt injection between sections).
**Ordering:** Soul FIRST (highest attention), then identity instructions, then user context, then memories, then behavioral reminders.

```rust
// In boternity-core/src/agent/prompt.rs
// Recommended system prompt template

const SYSTEM_PROMPT_TEMPLATE: &str = r#"<soul>
{soul_content}
</soul>

<identity>
Name: {display_name}
Emoji: {emoji}
Model: {model}
</identity>

<user_context>
{user_md_content}
</user_context>

<session_memory>
Key points from previous conversations with this user:
{memory_entries}
</session_memory>

<instructions>
You are {display_name}. Always stay in character as defined in your soul.
Express your personality strongly in every response -- your voice should be unmistakable.
When referencing past conversations, do so naturally without saying "I remember..."
When uncertain, acknowledge it rather than guessing.
</instructions>"#;
```

### Pattern 5: Markdown Rendering in Terminal
**What:** Use `termimad` for markdown rendering with custom skin for bot-themed styling. Use `syntect` separately for code block syntax highlighting since termimad does not have built-in syntax highlighting.
**When to use:** Rendering every bot response in the CLI chat.

```rust
// In boternity-api/src/cli/chat/renderer.rs
use termimad::MadSkin;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub struct ChatRenderer {
    skin: MadSkin,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl ChatRenderer {
    pub fn new(accent_color: Option<&str>) -> Self {
        let mut skin = MadSkin::default();
        // Customize skin based on bot's accent color
        // skin.bold.set_fg(accent_color);
        // etc.

        Self {
            skin,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    /// Render markdown text to terminal.
    /// For code blocks, use syntect for syntax highlighting.
    /// For everything else, use termimad.
    pub fn render(&self, markdown: &str) -> String {
        // Strategy: Split markdown into code blocks and non-code blocks.
        // Non-code: render with termimad skin
        // Code blocks: highlight with syntect, format for terminal
        // Combine and return
        todo!()
    }
}
```

### Pattern 6: OpenTelemetry GenAI Semantic Conventions
**What:** Follow OTel GenAI semantic conventions for all LLM call spans and agent invocation spans.
**Verified against:** OpenTelemetry GenAI Semantic Conventions (gen-ai-spans and gen-ai-agent-spans specs)

Span naming: `"{operation_name} {model_name}"` (e.g., "chat claude-sonnet-4-20250514")

Required attributes:
- `gen_ai.operation.name` -- "chat", "invoke_agent", "create_agent"
- `gen_ai.provider.name` -- "anthropic"

Recommended attributes:
- `gen_ai.request.model` -- the model ID
- `gen_ai.request.temperature` -- sampling temperature
- `gen_ai.request.max_tokens` -- max output tokens
- `gen_ai.usage.input_tokens` -- recorded on completion
- `gen_ai.usage.output_tokens` -- recorded on completion
- `gen_ai.response.finish_reasons` -- "end_turn", "tool_use", etc.
- `gen_ai.response.id` -- message ID from provider

Agent-specific attributes (for invoke_agent span):
- `gen_ai.agent.id` -- bot_id
- `gen_ai.agent.name` -- bot display name

### Anti-Patterns to Avoid
- **Using `async_trait` macro:** The project uses Rust 2024 RPITIT. Use `impl Future` returns and BoxLlmProvider for dynamic dispatch.
- **Using SeaORM:** The project uses raw `sqlx`. All new SQL persistence follows the existing `SqliteBotRepository` pattern with raw queries.
- **Buffering entire stream before CLI delivery:** Stream tokens to stdout as they arrive. Only collect fully for intermediate tool-call responses.
- **Storing full conversation text as session memory:** Extract key points only. Full history goes to `chat_messages` table (separate concern).
- **Skipping tracing on LLM calls:** Every LLM call MUST produce a trace span. Add instrumentation from the first implementation.
- **Hard-coding Anthropic types in core:** All Anthropic-specific types belong in `boternity-infra`. Core only knows about the `LlmProvider` trait and generic types.
- **Ignoring context window limits:** Implement `TokenBudget` from day one. Without it, long conversations silently degrade personality.
- **Trait objects for repositories:** Use generic services like `ChatService<C: ChatRepository, M: MemoryRepository>`, matching Phase 1's `BotService<B, S, F, H>` pattern.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SSE stream parsing | Custom HTTP chunked parser | `reqwest-eventsource` + `eventsource-stream` | SSE has edge cases (multi-line data, retry fields, UTF-8 BOM, reconnection). The crate handles all of them. |
| Token counting for Anthropic | Local tokenizer (char-based estimation) | Anthropic's `/v1/messages/count_tokens` API (free, rate-limited) | Claude uses a proprietary tokenizer. Local estimation is inaccurate. The API is free. |
| Terminal markdown rendering | Custom ANSI escape code writer | `termimad` (0.34.1) | Wrapping, table balancing, bold/italic/code/lists/headers, crossterm integration. Rebuilding this is weeks of work. |
| Code block syntax highlighting | Custom regex-based colorizer | `syntect` (5.x) | Sublime Text syntax definitions, 300+ languages, pure-rust backend. Used by bat, delta, mdcat. |
| Spinner animation | Custom crossterm cursor animation loop | `indicatif` (already in workspace) | `ProgressBar::new_spinner()` with `enable_steady_tick()`. Background thread handles animation. |
| Structured tracing setup | Custom logging framework | `tracing` + `tracing-subscriber` + `tracing-opentelemetry` | Async-aware span propagation, context inheritance across `.await`, OTel export. |
| Async readline for CLI | Raw stdin polling with tokio | `rustyline-async` (0.4.7) | Line editing, history, ctrl-c/ctrl-d, async compat, multiline support. |
| OTel GenAI attribute names | Custom span attribute names | OTel GenAI Semantic Conventions | Standard attributes enable compatibility with Datadog, Grafana, Jaeger. |
| JSON streaming accumulation | Manual string concat of partial JSON | `serde_json::from_str` after `content_block_stop` | Anthropic streams tool input as partial JSON fragments. Accumulate raw string, parse once on block stop. |
| Error recovery on SSE disconnect | Custom retry logic | `reqwest-eventsource` retry + Anthropic partial response continuation | Save partial response, construct continuation request with partial assistant message. |

**Key insight:** The Anthropic Messages API is well-documented with clear SSE event types. A thin wrapper over `reqwest-eventsource` gives full control while avoiding the complexity of raw HTTP chunked transfer parsing.

## Common Pitfalls

### Pitfall 1: Context Window Overflow Degrading Bot Personality
**What goes wrong:** As conversations grow, SOUL.md instructions get deprioritized by the model because they are at the beginning of a large context. The bot gradually loses its personality.
**Why it happens:** LLMs exhibit a "lost in the middle" effect. Without token budgeting, conversation history pushes soul instructions into the forgotten middle.
**How to avoid:** Implement `TokenBudget` from day one. Reserve fixed allocations for soul (high priority), memory, and user context. Implement sliding window conversation history. Consider repeating key soul traits at the END of the system prompt (in the `<instructions>` section).
**Warning signs:** Bot personality drift during conversations longer than 20 turns. Bot ignoring SOUL.md instructions.

### Pitfall 2: Blocking the Tokio Runtime with Synchronous Operations
**What goes wrong:** File I/O for soul reading, tracing subscriber flush, or synchronous computations block the Tokio event loop, causing streaming token delivery to stutter or freeze.
**Why it happens:** Mixing sync and async causes thread starvation. SQLx is async-safe, but file I/O and some crypto operations are not.
**How to avoid:** Use `tokio::fs` for file I/O (reading SOUL.md, USER.md). Use `tokio::task::spawn_blocking` for any synchronous computation. Never call `.block_on()` inside an async context. Use async exporters for tracing.
**Warning signs:** Streaming tokens arrive in bursts rather than smoothly.

### Pitfall 3: Session Memory Extraction Failing Silently
**What goes wrong:** The LLM call to extract session memory fails (rate limit, network error, malformed response) and the system silently discards memories.
**Why it happens:** Memory extraction is a "fire and forget" operation at session end. If it fails, the user has already closed the terminal.
**How to avoid:** Queue failed extractions for retry via `pending_memory_extractions` table. On next session start for the same bot, check for and retry pending extractions. Store raw conversation in `chat_messages` (independent of memory extraction). Log extraction failures prominently.
**Warning signs:** Bot that never references previous conversations. Empty `session_memories` table despite multiple sessions.

### Pitfall 4: SSE Event Ordering Assumptions
**What goes wrong:** Code assumes strict event order but Anthropic may add new event types. Ping events appear anywhere. Error events can occur mid-stream.
**Why it happens:** Testing with simple responses misses edge cases (tool use with multiple content blocks, error events mid-stream, web search results).
**How to avoid:** Implement a state machine for SSE processing. Handle unknown event types gracefully (log and skip, per Anthropic's versioning policy). Test with tool-use and extended thinking responses.
**Warning signs:** Panics or garbled output during tool-use responses.

### Pitfall 5: API Key Leaking to Logs via Tracing
**What goes wrong:** The Anthropic API key appears in tracing spans because HTTP request headers are instrumented.
**Why it happens:** Default tracing/tower-http layers log request headers.
**How to avoid:** Wrap API keys in `secrecy::SecretString`. The `Debug` impl prints `[REDACTED]`. Configure tower-http to redact sensitive headers. Never log full HTTP requests in production.
**Warning signs:** API keys visible in log output or trace data.

### Pitfall 6: Concurrent Terminal I/O Corruption
**What goes wrong:** The spinner animation, streaming text output, and user input all write to the same terminal simultaneously, causing garbled output.
**Why it happens:** Without coordination, multiple writers to stdout interleave ANSI escape sequences.
**How to avoid:** Use `rustyline-async`'s built-in mechanism for concurrent reading and writing. Stop the spinner BEFORE writing the first token. Use a single output channel. Disable readline prompt during streaming output.
**Warning signs:** Spinner characters appearing inside bot responses. Prompt text overlapping with streaming output.

### Pitfall 7: Message Persistence Losing Data on Crash
**What goes wrong:** User has a long conversation, force-quits (kill -9), and the entire conversation is lost because messages were only in memory.
**Why it happens:** If messages are only persisted at session end, a crash means total data loss.
**How to avoid:** Persist each message to SQLite IMMEDIATELY after it completes (user message: after Enter; assistant message: after stream ends). The session record can be finalized later. This also enables the `--resume` feature.
**Warning signs:** Users reporting lost conversations after crashes or Ctrl+C.

## Code Examples

### SQLite Schema for Chat Persistence (using sqlx, not SeaORM)
```sql
-- Migration: 20260211_002_chat_and_memory.sql

-- Chat sessions table
CREATE TABLE IF NOT EXISTS chat_sessions (
    id              TEXT PRIMARY KEY NOT NULL,           -- UUIDv7
    bot_id          TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    title           TEXT,                                -- Auto-generated from first exchange
    started_at      TEXT NOT NULL,                       -- ISO 8601
    ended_at        TEXT,                                -- NULL if session is active
    total_input_tokens  INTEGER NOT NULL DEFAULT 0,
    total_output_tokens INTEGER NOT NULL DEFAULT 0,
    message_count   INTEGER NOT NULL DEFAULT 0,
    model           TEXT NOT NULL,                       -- Model used for this session
    status          TEXT NOT NULL DEFAULT 'active'       -- active|completed|crashed
        CHECK (status IN ('active', 'completed', 'crashed'))
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_bot_id ON chat_sessions(bot_id);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_started_at ON chat_sessions(started_at DESC);

-- Chat messages table
CREATE TABLE IF NOT EXISTS chat_messages (
    id              TEXT PRIMARY KEY NOT NULL,           -- UUIDv7
    session_id      TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role            TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content         TEXT NOT NULL,                       -- Message text
    created_at      TEXT NOT NULL,                       -- ISO 8601
    input_tokens    INTEGER,                             -- For assistant messages
    output_tokens   INTEGER,                             -- For assistant messages
    model           TEXT,                                -- Which model generated this
    stop_reason     TEXT,                                -- end_turn|tool_use|max_tokens
    response_ms     INTEGER                              -- Response time in milliseconds
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at);

-- Session memories table
CREATE TABLE IF NOT EXISTS session_memories (
    id              TEXT PRIMARY KEY NOT NULL,           -- UUIDv7
    bot_id          TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    session_id      TEXT NOT NULL REFERENCES chat_sessions(id),
    fact            TEXT NOT NULL,                       -- The extracted key point
    category        TEXT NOT NULL                        -- preference|fact|decision|context|correction
        CHECK (category IN ('preference', 'fact', 'decision', 'context', 'correction')),
    importance      INTEGER NOT NULL CHECK (importance BETWEEN 1 AND 5),
    source_message_id TEXT,                              -- Message that triggered this memory
    superseded_by   TEXT,                                -- FK to session_memories.id (for corrections)
    created_at      TEXT NOT NULL,                       -- ISO 8601
    is_manual       INTEGER NOT NULL DEFAULT 0,          -- 1 if injected via /remember or bnity remember

    FOREIGN KEY (bot_id) REFERENCES bots(id),
    FOREIGN KEY (session_id) REFERENCES chat_sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_memories_bot_importance
    ON session_memories(bot_id, importance DESC, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memories_session
    ON session_memories(session_id);

-- Pending extractions queue (retry on failure)
CREATE TABLE IF NOT EXISTS pending_memory_extractions (
    id              TEXT PRIMARY KEY NOT NULL,
    session_id      TEXT NOT NULL REFERENCES chat_sessions(id),
    bot_id          TEXT NOT NULL REFERENCES bots(id),
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TEXT,
    next_attempt_at TEXT NOT NULL,
    error_message   TEXT,
    created_at      TEXT NOT NULL
);

-- Context summaries for sliding window
CREATE TABLE IF NOT EXISTS context_summaries (
    id              TEXT PRIMARY KEY NOT NULL,           -- UUIDv7
    session_id      TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    summary         TEXT NOT NULL,                       -- LLM-generated summary of older messages
    messages_start  INTEGER NOT NULL,                    -- First message index summarized
    messages_end    INTEGER NOT NULL,                    -- Last message index summarized
    token_count     INTEGER NOT NULL,                    -- Estimated tokens in this summary
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_context_summaries_session
    ON context_summaries(session_id, created_at DESC);
```

### Chat Repository (sqlx pattern matching existing codebase)
```rust
// In boternity-infra/src/sqlite/chat.rs
// Following existing SqliteBotRepository pattern with raw sqlx queries

use boternity_core::chat::repository::ChatRepository;
use crate::sqlite::pool::DatabasePool;

pub struct SqliteChatRepository {
    pool: DatabasePool,
}

impl SqliteChatRepository {
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

impl ChatRepository for SqliteChatRepository {
    fn create_session(
        &self,
        session: &ChatSession,
    ) -> impl std::future::Future<Output = Result<ChatSession, RepositoryError>> + Send {
        async move {
            sqlx::query(
                r#"INSERT INTO chat_sessions (id, bot_id, title, started_at, model, status)
                   VALUES (?, ?, ?, ?, ?, ?)"#
            )
            .bind(session.id.to_string())
            .bind(session.bot_id.to_string())
            .bind(&session.title)
            .bind(session.started_at.to_rfc3339())
            .bind(&session.model)
            .bind("active")
            .execute(self.pool.writer())
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

            Ok(session.clone())
        }
    }

    fn save_message(
        &self,
        message: &ChatMessage,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
        async move {
            sqlx::query(
                r#"INSERT INTO chat_messages (id, session_id, role, content, created_at, input_tokens, output_tokens, model, stop_reason, response_ms)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
            )
            .bind(message.id.to_string())
            .bind(message.session_id.to_string())
            .bind(message.role.as_str())
            .bind(&message.content)
            .bind(message.created_at.to_rfc3339())
            .bind(message.input_tokens)
            .bind(message.output_tokens)
            .bind(&message.model)
            .bind(message.stop_reason.as_deref())
            .bind(message.response_ms)
            .execute(self.pool.writer())
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

            Ok(())
        }
    }
    // ... additional methods
}
```

### Memory Extraction Prompt
```rust
// In boternity-core/src/memory/extractor.rs

const EXTRACTION_SYSTEM_PROMPT: &str = r#"You are a memory extraction assistant. Extract key facts, preferences, and important points from conversations that would be useful to remember for future conversations with this user.

Rules:
1. Extract ONLY information worth remembering across sessions
2. Each fact must be a single, self-contained sentence
3. Do NOT include greetings, pleasantries, or trivial exchanges
4. Do NOT include information the user explicitly asked to forget
5. Prefer specific facts over vague observations
6. Include the user's name, preferences, and stated goals if mentioned
7. Include decisions made during the conversation
8. Include any corrections the user made (e.g., "Actually, I prefer X not Y")

Return a JSON array. Each element must have exactly these fields:
- "fact": string (one sentence, the key point)
- "category": string (one of: "preference", "fact", "decision", "context", "correction")
- "importance": integer (1-5, where 5 = critical to remember)

If there is nothing worth extracting, return an empty array: []

Example output:
[
  {"fact": "User's name is Alex and they work as a data engineer", "category": "fact", "importance": 5},
  {"fact": "User prefers concise responses without code examples unless asked", "category": "preference", "importance": 4},
  {"fact": "User decided to use PostgreSQL instead of MySQL for the project", "category": "decision", "importance": 3}
]"#;
```

### Auto-Title Generation Prompt
```rust
// In boternity-core/src/chat/service.rs

const TITLE_GENERATION_PROMPT: &str = r#"Generate a short, descriptive title (3-7 words) for this conversation based on the first exchange. The title should capture the main topic or intent. Return ONLY the title text, nothing else.

Examples:
- "Debugging Rust lifetime errors"
- "Planning a weekend trip to Tokyo"
- "Understanding quantum computing basics"
- "Recipe ideas for dinner party""#;
```

### Sliding Window Summary Prompt
```rust
// In boternity-core/src/agent/prompt.rs

const SUMMARY_PROMPT: &str = r#"Summarize the following conversation segment concisely. Preserve:
1. Key decisions and conclusions
2. Important facts mentioned
3. The user's current goals and context
4. Any unresolved questions

Keep the summary under 500 words. Write in third person (e.g., "The user asked about..." "The assistant recommended...").

<conversation>
{conversation_segment}
</conversation>"#;
```

### Tracing Setup with OTel
```rust
// In boternity-observe/src/tracing_setup.rs

use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing(enable_otel: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut layers = vec![];

    // Structured fmt layer (for console and log files)
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE);

    // OTel layer (optional, for trace export)
    if enable_otel {
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
            .build();
        let tracer = provider.tracer("boternity");
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        // Add both layers
        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(otel_layer)
            .with(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    }

    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed-size conversation buffer | Sliding window with LLM-generated summaries | 2025 (Mem0, ProMem) | Better memory retention with lower token cost |
| Custom logging for LLM calls | OTel GenAI Semantic Conventions + Agent Spans | 2025-2026 (OTel v1.37+) | Standard attributes + agent-specific conventions enable cross-vendor observability |
| Third-party crate for multi-provider LLM | Thin custom trait + per-provider impl | Current best practice | Better control, stability, debuggability |
| Client-side token estimation (tiktoken-like) | Provider token counting APIs (Anthropic /count_tokens) | 2025 | Exact counts; provider tokenizers differ significantly |
| `async_trait` crate for async in traits | Native RPITIT (Rust 2024 edition) | Rust 1.75+ (2023) | No heap allocation for futures; compile-time dispatch. BoxProvider wrapper for dynamic dispatch. |
| SeaORM for SQL abstraction | Raw sqlx with compile-time checked queries | Project decision (Phase 1) | Lighter, more control, consistent with existing codebase |
| Static session memory (dump full history) | LLM-assisted key point extraction (Mem0-style) | 2025-2026 | Compact, relevant memories instead of raw conversation dumps |

**Deprecated/outdated:**
- `async_trait` macro: The project uses RPITIT. Do NOT use `async_trait` in new code.
- `sea-orm`: The project uses raw `sqlx`. Do NOT introduce SeaORM.
- `rustformers/llm`: Explicitly unmaintained
- `llm-chain` crate: Not actively maintained
- `tiktoken-rs` for Claude token counting: Claude uses a different tokenizer; use Anthropic's counting API
- `tui-rs`: Archived in favor of `ratatui`

## Open Questions

1. **Termimad + syntect integration for code block highlighting**
   - What we know: Termimad does not have built-in syntax highlighting for code blocks. Syntect provides the highlighting engine. Several projects (mdcat, bat) use syntect for terminal code rendering.
   - What's unclear: The exact integration pattern -- whether to pre-process markdown, extract code blocks, highlight with syntect, then re-insert styled text before passing to termimad, or render in two passes.
   - Recommendation: Parse markdown with a simple state machine to identify code blocks. Render code blocks directly with syntect (outputting ANSI-colored text). Render non-code markdown with termimad. Interleave the results. This avoids fighting termimad's code block rendering.
   - Confidence: MEDIUM

2. **Streaming text + markdown rendering timing**
   - What we know: Tokens arrive one at a time. Markdown rendering needs complete blocks (e.g., a full code fence) to render correctly. Streaming raw tokens looks good for prose but breaks for code blocks and tables.
   - What's unclear: When to apply markdown rendering -- per-token (broken for block elements), per-sentence, or after the full response completes.
   - Recommendation: Stream raw text character-by-character during delivery (the "typewriter" effect). After the full response completes, re-render the complete response with markdown formatting and replace the raw text. This gives instant feedback during streaming while delivering polished final output. Alternatively, maintain a running markdown parser and render incrementally when block boundaries are detected.
   - Confidence: MEDIUM

3. **Session memory extraction model choice**
   - What we know: Using the LLM to extract key points works. Cheaper models can handle extraction.
   - What's unclear: Whether to use the same provider as the chat model or always use a cheap model. Token budget impact if the user's provider has low rate limits.
   - Recommendation: Default to the same provider but use the cheapest available model variant. Make the extraction model configurable.
   - Confidence: MEDIUM

4. **`reqwest-eventsource` handling of Anthropic mid-stream errors**
   - What we know: reqwest-eventsource 0.6.0 handles SSE retry and reconnection. Anthropic can send `error` events mid-stream.
   - What's unclear: Whether reqwest-eventsource's error handling interacts well with Anthropic's specific error event format.
   - Recommendation: Use reqwest-eventsource as starting point. The SSE `error` events are parsed as regular Message events (they have `event: error` in the SSE stream), so they should be handled by the application-level state machine, not by reqwest-eventsource's connection retry logic. Test with simulated overloaded_error events.
   - Confidence: HIGH (error events are regular SSE messages, not connection-level errors)

5. **Streaming optimization: collect-then-replay vs direct-stream**
   - What we know: The agent loop must collect intermediate tool-call responses fully. The FINAL response should stream directly.
   - What's unclear: Whether to always collect then replay, or detect "no tools" upfront and stream directly.
   - Recommendation: For Phase 2, detect the simple case (no tools defined) and stream directly. When tools are present, collect all intermediate iterations and stream only the final one. This is more complex but avoids noticeable latency for the common case (no-tool conversations).
   - Confidence: MEDIUM

## Sources

### Primary (HIGH confidence)
- [Anthropic Messages Streaming API](https://platform.claude.com/docs/en/build-with-claude/streaming) -- Full SSE event type documentation, event flow, tool use streaming, error recovery, extended thinking streaming, web search streaming. Verified 2026-02-11.
- [Anthropic Token Counting API](https://platform.claude.com/docs/en/build-with-claude/token-counting) -- Free token counting endpoint, rate limits, supported content types
- [OpenTelemetry GenAI Client Spans](https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-spans/) -- Standard attributes for LLM call spans. Span naming: "{operation} {model}". Required: gen_ai.operation.name, gen_ai.provider.name.
- [OpenTelemetry GenAI Agent Spans](https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-agent-spans/) -- Agent-specific conventions: create_agent, invoke_agent operations. Development status.
- [tracing-opentelemetry 0.32.1](https://docs.rs/crate/tracing-opentelemetry/latest) -- Published 2026-01-12, depends on opentelemetry ^0.31.0
- [opentelemetry 0.31.0](https://docs.rs/crate/opentelemetry/latest) -- Published 2025-09-25
- [termimad 0.34.1](https://docs.rs/crate/termimad/latest) -- Published 2025-11-24, markdown terminal rendering with crossterm
- [rustyline-async 0.4.7](https://docs.rs/crate/rustyline-async/latest) -- Published 2025-07-21, async readline with crossterm
- [reqwest-eventsource 0.6.0](https://docs.rs/reqwest-eventsource/latest/reqwest_eventsource/) -- SSE stream consumer wrapping reqwest; 2.3M downloads
- Existing codebase: `boternity-core`, `boternity-infra`, `boternity-types`, `boternity-api` -- Phase 1 patterns verified by reading actual source files

### Secondary (MEDIUM confidence)
- [Mem0 arXiv Paper](https://arxiv.org/html/2504.19413v1) -- Two-phase extraction+update, AUDN cycle, memory schema
- [ProMem arXiv Paper](https://arxiv.org/html/2601.04463) -- Recurrent feedback loop, self-questioning verification, 73.8% memory integrity
- [rig-core v0.30.0](https://docs.rs/crate/rig-core/latest) -- CompletionModel trait, Agent struct, Tool trait design patterns
- [genai crate v0.5.3](https://github.com/jeremychone/rust-genai) -- Adapter-based dispatch design patterns
- [syntect](https://github.com/trishume/syntect) -- Syntax highlighting library; used by bat, delta, mdcat, zola

### Tertiary (LOW confidence)
- [OTel Agentic Systems Proposal](https://github.com/open-telemetry/semantic-conventions/issues/2664) -- Proposed conventions for agent tracing (development status)
- [Rust observability blog post](https://dasroot.net/posts/2026/01/rust-observability-opentelemetry-tokio/) -- Community patterns for OTel + Tokio

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All libraries verified via docs.rs with published dates. Workspace compatibility confirmed.
- Architecture (provider trait): HIGH -- RPITIT+BoxProvider pattern verified against existing BoxSecretProvider pattern in codebase. Streaming types verified against Anthropic official docs.
- Architecture (chat persistence): HIGH -- SQLx pattern verified against existing SqliteBotRepository code. Schema follows existing migration conventions.
- Architecture (agent engine): HIGH for Phase 2 single-agent loop; MEDIUM for Phase 5 sub-agent design (structurally sound but speculative)
- Session memory extraction: MEDIUM-HIGH -- Mem0 architecture verified via arXiv paper. Rust implementation is original design based on verified patterns.
- CLI chat rendering: MEDIUM -- Termimad verified but syntect integration for code blocks is an open question. Streaming + markdown rendering timing needs prototyping.
- Pitfalls: HIGH -- All pitfalls verified against project research and Anthropic official documentation

**Research date:** 2026-02-11
**Valid until:** 2026-03-11 (30 days; Rust LLM ecosystem is fast-moving but core libraries are stable)

---
*Phase 2 research for: Boternity -- Single-Agent Chat + LLM*
*Researched: 2026-02-11*
*Key corrections from previous research: Uses RPITIT not async_trait, uses sqlx not SeaORM, adds termimad+syntect for markdown rendering, adds BoxLlmProvider pattern, updated OTel GenAI agent span conventions*
