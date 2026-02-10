# Phase 2: Single-Agent Chat + LLM - Research

**Researched:** 2026-02-10
**Domain:** LLM provider abstraction, streaming chat, agent engine, session memory, observability (Rust)
**Confidence:** MEDIUM-HIGH

## Summary

Phase 2 transforms Boternity from a bot identity manager into a conversational system. The core challenges are: (1) building a pluggable LLM provider abstraction that supports streaming, (2) implementing a single-agent execution loop that reads the bot's SOUL.md and maintains conversation context, (3) delivering token-by-token streaming output in the CLI, (4) extracting and persisting session memory key points, (5) persisting full chat history, and (6) wiring up structured tracing with OpenTelemetry GenAI semantic conventions.

The Rust ecosystem for LLM integration is fragmented and fast-moving. The `llm` crate (graniet) v1.3.7 provides a multi-provider unified API but has evolved rapidly (1.2.4 to 1.3.7 in one month), has only 68% documentation coverage, and owns the streaming/parsing/retry concerns that Boternity needs fine-grained control over. The `genai` crate v0.5.3 is more mature in design but pre-1.0. **The recommended approach is a thin custom provider abstraction with `reqwest` + `reqwest-eventsource` for the Anthropic provider, keeping full control over the streaming protocol, token counting, and error handling.** This abstraction is designed from day one for future providers (Phase 3), with the trait interface inspired by patterns from `genai`, `rig-core`, and `flyllm`.

For session memory, the pattern is LLM-assisted extraction: after a conversation session ends (or at periodic checkpoints), the LLM itself extracts key facts from the conversation into structured memory entries. This is the approach used by Mem0 and aligns with recent research (ProMem, Jan 2026). For the CLI interactive chat, `rustyline-async` v0.4.7 provides async readline with `crossterm` backend, enabling concurrent input handling and streaming output.

**Primary recommendation:** Build a thin custom LLM provider trait (`LlmProvider`) with a direct `reqwest`+`reqwest-eventsource` Anthropic implementation. Use `tracing` spans following OpenTelemetry GenAI semantic conventions for every LLM call. Extract session memory via an LLM summarization call at session end.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `reqwest` | 0.12.x | HTTP client for LLM API calls | De facto Rust HTTP client; async, streaming body support, TLS |
| `reqwest-eventsource` | 0.6.x | SSE stream consumer for LLM streaming | Wraps reqwest with proper SSE parsing; handles reconnection |
| `eventsource-stream` | 0.2.x | Low-level SSE byte stream parser | Used by `reqwest-eventsource` under the hood |
| `tracing` | 0.1.44 | Structured logging and span instrumentation | THE async Rust instrumentation crate (Tokio team) |
| `tracing-subscriber` | 0.3.x | Log formatting, filtering, layered subscribers | Standard companion to `tracing` |
| `tracing-opentelemetry` | 0.32.1 | Bridge tracing spans to OpenTelemetry | Connects Rust spans to OTel distributed traces |
| `opentelemetry` | 0.31.0 | OpenTelemetry API | Industry standard observability API |
| `opentelemetry_sdk` | 0.31.x | OTel SDK implementation | Required runtime for OTel |
| `opentelemetry-stdout` | 0.31.x | OTel stdout exporter (dev/debug) | Logs traces to console for local development |
| `sea-orm` | 1.1.x | Async ORM for chat persistence | Already decided in Phase 1; backend-agnostic |
| `rustyline-async` | 0.4.7 | Async readline for CLI chat | Async-compatible line editor with crossterm backend |
| `crossterm` | 0.28.x | Terminal manipulation (raw mode, colors, cursor) | Cross-platform terminal library; used by rustyline-async |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serde` | 1.0.x | Serialization for API request/response types | Every API call |
| `serde_json` | 1.0.x | JSON serialization | Parsing Anthropic SSE data payloads |
| `tokio` | 1.49.x | Async runtime (already in stack) | Everything async |
| `futures-util` | 0.3.x | Stream combinators for SSE processing | Processing SSE event streams |
| `uuid` | 1.x | Session IDs, message IDs | Every new chat session and message |
| `chrono` | 0.4.x | Timestamps for messages and sessions | Chat history timestamps |
| `thiserror` | 2.x | Error type derivation | LLM provider errors, agent errors |
| `secrecy` | 0.10.x | API key wrapping | API keys never leak to logs/debug |
| `async-stream` | 0.3.x | Proc-macro-free async stream creation | Creating streams with `stream!`/`try_stream!` macros for SSE event transformation |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom provider + reqwest | `llm` crate (graniet) v1.3.7 | `llm` provides multi-provider out of the box but: rapid version churn (1.2.4 -> 1.3.7 in ~1 month), 68% doc coverage, owns streaming/retry logic reducing Boternity's control. Use if time-to-market trumps control. |
| Custom provider + reqwest | `genai` crate v0.5.3 | More stable design than `llm`, better streaming engine (v0.5), but pre-1.0 API may break. Good fallback if custom approach proves too expensive. |
| Custom provider + reqwest | `rig-core` v0.30.0 | Full agent framework with OTel support, but too opinionated for Boternity's custom agent model. Good reference for trait design. |
| `rustyline-async` | `reedline` (nushell) | More features (syntax highlighting, completions) but heavier; designed for shell editors not chat input. |
| `rustyline-async` | Raw `crossterm` EventStream | Full control but you rebuild readline behavior (line editing, history, ctrl-c). |

**Installation:**
```toml
# In boternity-core/Cargo.toml (trait definitions)
[dependencies]
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
thiserror = "2"
tokio = { version = "1.49", features = ["sync"] }
futures-util = "0.3"
uuid = { version = "1", features = ["v4", "v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"

# In boternity-infra/Cargo.toml (Anthropic implementation)
[dependencies]
reqwest = { version = "0.12", features = ["json", "stream"] }
reqwest-eventsource = "0.6"
async-stream = "0.3"
serde_json = "1"
secrecy = { version = "0.10", features = ["serde"] }
tokio = { version = "1.49", features = ["full"] }
tracing = "0.1"
pin-project-lite = "0.2"

# In boternity-api/Cargo.toml (CLI chat)
[dependencies]
rustyline-async = "0.4"
crossterm = { version = "0.28", features = ["event-stream"] }
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
        agent.rs         # AgentContext, AgentConfig, AgentResult
        memory.rs        # SessionMemory, MemoryEntry, KeyPoint
      events/
        llm_events.rs    # LlmCallStarted, LlmCallCompleted, TokenStreamed
        chat_events.rs   # SessionStarted, SessionEnded, MessageSent
        agent_events.rs  # AgentExecutionStarted, AgentExecutionCompleted

  boternity-core/
    src/
      llm/
        mod.rs           # Re-exports
        provider.rs      # LlmProvider trait + ProviderCapabilities
        types.rs         # CompletionRequest, CompletionResponse, StreamChunk
        token_budget.rs  # TokenBudget, ContextAllocator (budget per segment)
      agent/
        mod.rs           # Re-exports
        engine.rs        # AgentEngine: single-agent execution loop
        context.rs       # AgentContext: soul, memory, conversation state
        prompt.rs        # SystemPromptBuilder: assembles soul + memory + user msg
      chat/
        mod.rs           # Re-exports
        service.rs       # ChatService trait: start_session, send_message, etc.
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
          streaming.rs   # SSE stream parser, event type handling
          types.rs       # Anthropic-specific request/response types
          token_counter.rs  # Anthropic token counting API wrapper
      storage/
        chat_repo.rs     # ChatRepository: SeaORM impl for chat persistence
        memory_repo.rs   # SessionMemoryRepository: SeaORM impl for memory

  boternity-observe/
    src/
      tracing_setup.rs   # Initialize tracing subscriber + OTel layer
      llm_layer.rs       # Custom tracing layer for LLM call metrics
      genai_attrs.rs     # OpenTelemetry GenAI semantic convention constants
```

### Pattern 1: LLM Provider Trait (Dependency Inversion)
**What:** Define an `LlmProvider` trait in `boternity-core` that abstracts all LLM interactions. `boternity-infra` provides concrete implementations (Anthropic for Phase 2, more in Phase 3). The trait uses `Pin<Box<dyn Stream>>` for streaming responses.
**When to use:** Every LLM interaction in the system.
**Why:** boternity-core must never depend on boternity-infra. The trait boundary enforces this. Future providers (OpenAI, Gemini) implement the same trait without touching core logic.

```rust
// Source: Pattern derived from genai, rig-core, and flyllm trait designs
// In boternity-core/src/llm/provider.rs

use async_trait::async_trait;
use futures_util::Stream;
use std::pin::Pin;

/// What this provider can do
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tool_calling: bool,
    pub vision: bool,
    pub max_context_tokens: u32,
}

/// A chunk from a streaming response
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Text content delta
    TextDelta { text: String },
    /// Thinking content delta (extended thinking)
    ThinkingDelta { thinking: String },
    /// Tool use request
    ToolUse { id: String, name: String, input: serde_json::Value },
    /// Usage metadata (cumulative)
    Usage { input_tokens: u32, output_tokens: u32 },
    /// Stream completed with stop reason
    Done { stop_reason: StopReason },
    /// Error during streaming
    Error { message: String },
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider display name (e.g., "anthropic", "openai")
    fn name(&self) -> &str;

    /// What this provider supports
    fn capabilities(&self) -> &ProviderCapabilities;

    /// Non-streaming completion
    async fn complete(
        &self,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse, LlmError>;

    /// Streaming completion - returns a stream of events
    fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>;

    /// Count tokens in a request (pre-send estimation)
    async fn count_tokens(
        &self,
        request: &CompletionRequest,
    ) -> Result<TokenCount, LlmError>;
}
```

### Pattern 2: Single-Agent Execution Loop
**What:** A single-agent loop that: (1) loads bot soul + session memory, (2) builds system prompt with token budgets, (3) streams LLM response, (4) checks for tool calls, (5) if tool call: execute tool and loop, (6) if no tool call: return streamed response. Each iteration is a tracing span.
**When to use:** Every chat interaction with a bot.

```rust
// In boternity-core/src/agent/engine.rs
// Simplified single-agent loop (Phase 2 - no sub-agents)

pub struct AgentEngine {
    provider: Arc<dyn LlmProvider>,
    memory_store: Arc<dyn SessionMemoryStore>,
    prompt_builder: SystemPromptBuilder,
}

impl AgentEngine {
    #[tracing::instrument(
        skip(self, session),
        fields(
            gen_ai.operation.name = "chat",
            gen_ai.provider.name = %self.provider.name(),
            bot_id = %session.bot_id,
            session_id = %session.session_id,
        )
    )]
    pub async fn execute(
        &self,
        session: &mut ChatSession,
        user_message: &str,
    ) -> Result<impl Stream<Item = Result<StreamEvent, AgentError>>, AgentError> {
        // 1. Record user message
        session.add_user_message(user_message);

        // 2. Build system prompt (soul + memory + context budget)
        let system_prompt = self.prompt_builder.build(
            &session.soul,
            &session.memory_context,
            &session.user_md,
        )?;

        // 3. Build completion request with token budget
        let request = CompletionRequest {
            model: session.config.model.clone(),
            system: Some(system_prompt),
            messages: session.messages_for_context(),
            max_tokens: session.config.max_tokens,
            temperature: session.config.temperature,
            stream: true,
        };

        // 4. Stream response from LLM
        let stream = self.provider.stream(request);

        // 5. Return stream (CLI or API handler consumes it)
        Ok(stream)
    }
}
```

### Pattern 3: Anthropic SSE Stream Processing
**What:** Parse Anthropic's streaming SSE events (message_start, content_block_start, content_block_delta, content_block_stop, message_delta, message_stop) into the unified `StreamEvent` enum. Uses `reqwest-eventsource` to handle the HTTP SSE connection.
**When to use:** Inside the Anthropic provider implementation.

```rust
// In boternity-infra/src/llm/anthropic/streaming.rs
// Source: Anthropic Messages Streaming API documentation

use reqwest_eventsource::{Event, EventSource};
use futures_util::StreamExt;

pub fn create_anthropic_stream(
    client: &reqwest::Client,
    url: &str,
    body: AnthropicRequest,
    api_key: &secrecy::SecretString,
) -> impl Stream<Item = Result<StreamEvent, LlmError>> {
    async_stream::stream! {
        let request = client.post(url)
            .header("x-api-key", api_key.expose_secret())
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body);

        let mut es = EventSource::new(request)
            .map_err(|e| LlmError::Connection(e.to_string()))?;

        while let Some(event) = es.next().await {
            match event {
                Ok(Event::Message(msg)) => {
                    match msg.event.as_str() {
                        "content_block_delta" => {
                            let data: serde_json::Value =
                                serde_json::from_str(&msg.data)?;
                            match data["delta"]["type"].as_str() {
                                Some("text_delta") => {
                                    if let Some(text) = data["delta"]["text"].as_str() {
                                        yield Ok(StreamEvent::TextDelta {
                                            text: text.to_string(),
                                        });
                                    }
                                }
                                Some("thinking_delta") => {
                                    if let Some(thinking) = data["delta"]["thinking"].as_str() {
                                        yield Ok(StreamEvent::ThinkingDelta {
                                            thinking: thinking.to_string(),
                                        });
                                    }
                                }
                                Some("input_json_delta") => {
                                    // Accumulate tool input JSON
                                }
                                _ => {} // Unknown delta type - handle gracefully
                            }
                        }
                        "message_delta" => {
                            let data: serde_json::Value =
                                serde_json::from_str(&msg.data)?;
                            if let Some(usage) = data.get("usage") {
                                yield Ok(StreamEvent::Usage {
                                    input_tokens: usage["input_tokens"].as_u64().unwrap_or(0) as u32,
                                    output_tokens: usage["output_tokens"].as_u64().unwrap_or(0) as u32,
                                });
                            }
                        }
                        "message_stop" => {
                            yield Ok(StreamEvent::Done {
                                stop_reason: StopReason::EndTurn,
                            });
                        }
                        "error" => {
                            let data: serde_json::Value =
                                serde_json::from_str(&msg.data)?;
                            yield Err(LlmError::Provider {
                                message: data["error"]["message"]
                                    .as_str()
                                    .unwrap_or("Unknown error")
                                    .to_string(),
                            });
                        }
                        "ping" => {} // Ignore keepalive pings
                        _ => {}      // Unknown event types - handle gracefully per Anthropic docs
                    }
                }
                Ok(Event::Open) => {} // Connection established
                Err(e) => {
                    yield Err(LlmError::Stream(e.to_string()));
                    break;
                }
            }
        }
    }
}
```

### Pattern 4: Token Budget Allocation
**What:** Fixed token allocations for each context segment to prevent context window overflow. The system reserves space for soul, memory, tool descriptions, and leaves the remainder for conversation history and output.
**When to use:** Every prompt construction. This is a Phase 2 requirement from pitfalls research (context window overflow is a critical pitfall).

```rust
// In boternity-core/src/llm/token_budget.rs

pub struct TokenBudget {
    pub total_context: u32,         // Model's context window (e.g., 200_000 for Claude)
    pub system_prompt_reserve: u32, // Reserved for system prompt structure (~500)
    pub soul_reserve: u32,          // Reserved for SOUL.md content (~1_000)
    pub memory_reserve: u32,        // Reserved for session memory context (~2_000)
    pub user_md_reserve: u32,       // Reserved for USER.md content (~500)
    pub output_reserve: u32,        // Reserved for model output (max_tokens setting)
    // Remainder: available for conversation history
}

impl TokenBudget {
    pub fn available_for_history(&self) -> u32 {
        self.total_context
            .saturating_sub(self.system_prompt_reserve)
            .saturating_sub(self.soul_reserve)
            .saturating_sub(self.memory_reserve)
            .saturating_sub(self.user_md_reserve)
            .saturating_sub(self.output_reserve)
    }
}
```

### Pattern 5: Session Memory Extraction (LLM-Assisted)
**What:** At session end (or periodically during long sessions), use the LLM to extract key facts/points from the conversation. Store these as structured memory entries tagged with session ID, bot ID, and timestamp. On the next session start, retrieve relevant memories and include them in the system prompt.
**When to use:** End of every chat session; at periodic checkpoints during long sessions (e.g., every 20 turns).

```rust
// In boternity-core/src/memory/extractor.rs

const EXTRACTION_PROMPT: &str = r#"
Extract the key facts, preferences, and important points from this conversation.
Return a JSON array of objects with:
- "fact": the key point (one sentence)
- "category": one of "preference", "fact", "decision", "context"
- "importance": 1-5 (5 = critical to remember)

Only include information worth remembering for future conversations.
Do NOT include pleasantries, greetings, or trivial exchanges.
"#;

pub struct SessionMemoryExtractor {
    provider: Arc<dyn LlmProvider>,
}

impl SessionMemoryExtractor {
    #[tracing::instrument(skip(self, session), fields(session_id = %session.session_id))]
    pub async fn extract_key_points(
        &self,
        session: &ChatSession,
    ) -> Result<Vec<KeyPoint>, MemoryError> {
        let conversation_text = session.format_for_extraction();

        let request = CompletionRequest {
            model: "claude-sonnet-4-5".to_string(), // Use cheaper model for extraction
            system: Some(EXTRACTION_PROMPT.to_string()),
            messages: vec![ChatMessage::user(conversation_text)],
            max_tokens: 2048,
            temperature: 0.1, // Low temperature for factual extraction
            stream: false,
        };

        let response = self.provider.complete(&request).await?;
        let key_points: Vec<KeyPoint> = serde_json::from_str(&response.text)?;

        Ok(key_points)
    }
}
```

### Pattern 6: CLI Interactive Chat with Streaming
**What:** Use `rustyline-async` for async readline input, and print streaming tokens directly to stdout. Handle concurrent input (user typing) and output (tokens streaming) gracefully using raw terminal mode.
**When to use:** `bnity chat <bot-slug>` command.

```rust
// In boternity-cli (simplified pattern)

use rustyline_async::{Readline, ReadlineEvent};
use futures_util::StreamExt;

pub async fn run_chat_session(bot_slug: &str, engine: &AgentEngine) -> Result<()> {
    let (mut rl, mut stdout) = Readline::new(format!("{} > ", bot_slug))?;

    loop {
        match rl.readline().await? {
            ReadlineEvent::Line(input) => {
                let input = input.trim();
                if input.is_empty() { continue; }
                if input == "/quit" || input == "/exit" { break; }

                // Disable readline while streaming
                // Stream response tokens
                let mut stream = engine.execute(&mut session, input).await?;

                while let Some(event) = stream.next().await {
                    match event? {
                        StreamEvent::TextDelta { text } => {
                            write!(stdout, "{}", text)?;
                            stdout.flush()?;
                        }
                        StreamEvent::Usage { input_tokens, output_tokens } => {
                            // Log to tracing, don't display in chat
                            tracing::info!(
                                input_tokens,
                                output_tokens,
                                "LLM usage"
                            );
                        }
                        StreamEvent::Done { .. } => {
                            writeln!(stdout)?; // Newline after response
                        }
                        _ => {}
                    }
                }
            }
            ReadlineEvent::Eof | ReadlineEvent::Interrupted => break,
        }
    }

    // Extract session memory on exit
    engine.extract_and_save_memory(&session).await?;

    Ok(())
}
```

### Pattern 7: OpenTelemetry GenAI Tracing
**What:** Follow OpenTelemetry GenAI semantic conventions for all LLM call spans. Each LLM call creates a span with standardized attributes for provider, model, token usage, and timing. This enables integration with any OTel-compatible backend (Jaeger, Grafana Tempo, etc.).
**When to use:** Every LLM call, every agent execution.

```rust
// In boternity-observe/src/genai_attrs.rs
// Source: OpenTelemetry GenAI Semantic Conventions

/// Standard attribute keys following OTel GenAI semantic conventions
pub mod gen_ai {
    pub const OPERATION_NAME: &str = "gen_ai.operation.name";
    pub const PROVIDER_NAME: &str = "gen_ai.provider.name";
    pub const REQUEST_MODEL: &str = "gen_ai.request.model";
    pub const REQUEST_TEMPERATURE: &str = "gen_ai.request.temperature";
    pub const REQUEST_MAX_TOKENS: &str = "gen_ai.request.max_tokens";
    pub const USAGE_INPUT_TOKENS: &str = "gen_ai.usage.input_tokens";
    pub const USAGE_OUTPUT_TOKENS: &str = "gen_ai.usage.output_tokens";
    pub const RESPONSE_FINISH_REASON: &str = "gen_ai.response.finish_reasons";
}

// Usage in provider implementation:
#[tracing::instrument(
    name = "chat anthropic",  // "{operation_name} {provider_name}" per spec
    skip(self, request),
    fields(
        gen_ai.operation.name = "chat",
        gen_ai.provider.name = "anthropic",
        gen_ai.request.model = %request.model,
        gen_ai.request.temperature = request.temperature,
        gen_ai.request.max_tokens = request.max_tokens,
        // Token usage is recorded when response completes
    )
)]
async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, LlmError> {
    let response = self.client.post_messages(request).await?;

    // Record usage on the current span
    let span = tracing::Span::current();
    span.record("gen_ai.usage.input_tokens", response.usage.input_tokens);
    span.record("gen_ai.usage.output_tokens", response.usage.output_tokens);
    span.record("gen_ai.response.finish_reasons", &response.stop_reason);

    Ok(response)
}
```

### Anti-Patterns to Avoid
- **Using the `llm` crate directly as the provider layer:** Its streaming abstractions hide the SSE event details that Boternity needs for fine-grained token counting, error recovery, and custom event routing. Build the thin wrapper to own this complexity.
- **Buffering the entire stream before delivering to the CLI:** The whole point of streaming is token-by-token delivery. Never collect-then-print. Pipe the stream directly to stdout.
- **Storing full conversation text as session memory:** This wastes context window in future sessions. Extract key points only. Full conversation history goes to chat persistence (separate concern).
- **Skipping tracing on LLM calls:** This is a Phase 2 requirement (OBSV-01, OBSV-07). Every LLM call must produce a trace. Add instrumentation from the first implementation, not later.
- **Hard-coding Anthropic types in core:** All Anthropic-specific types belong in `boternity-infra`. Core only knows about the `LlmProvider` trait and generic types.
- **Ignoring context window limits:** Without token budgeting, long conversations will silently degrade bot personality and behavior. Implement `TokenBudget` from day one.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SSE stream parsing | Custom HTTP chunked parser for Anthropic events | `reqwest-eventsource` + `eventsource-stream` | SSE has edge cases (multi-line data, retry fields, UTF-8 BOM handling, reconnection). The crate handles all of them. |
| Token counting for Anthropic | Local tokenizer (character-based estimation) | Anthropic's `/v1/messages/count_tokens` API (free, rate-limited) | Claude uses a proprietary tokenizer different from tiktoken. Local estimation is inaccurate. The API is free and provides exact counts. |
| Structured tracing setup | Custom logging framework | `tracing` + `tracing-subscriber` + `tracing-opentelemetry` | Async-aware span propagation, context inheritance across `.await` points, and OTel export are extremely hard to get right. The tracing ecosystem handles it. |
| Async readline for CLI | Raw stdin polling with tokio | `rustyline-async` (0.4.7) | Line editing, history, ctrl-c/ctrl-d handling, and async compatibility all in one package. Rebuilding this from crossterm events is weeks of work. |
| OTel GenAI attribute names | Inventing custom span attribute names | Follow OTel GenAI Semantic Conventions v1.37+ | Standard attributes (`gen_ai.operation.name`, `gen_ai.usage.input_tokens`, etc.) enable compatibility with Datadog, Grafana, Jaeger, and any OTel backend. Custom names mean custom dashboards forever. |
| Error recovery on SSE disconnect | Custom retry logic | `reqwest-eventsource` retry mechanism + Anthropic's partial response continuation | Anthropic documents a specific error recovery strategy: save partial response, construct continuation request with partial assistant message. The SSE library handles connection retry; you add the message continuation. |
| JSON streaming accumulation (tool use) | Manual string concatenation of partial JSON | `serde_json::from_str` on accumulated string after `content_block_stop` | Anthropic streams tool input as partial JSON fragments. Accumulate the raw string and parse once on block stop, not incrementally. |

**Key insight:** The Anthropic Messages API is well-documented with clear SSE event types. Building a thin wrapper over `reqwest-eventsource` gives you full control while avoiding the complexity of raw HTTP chunked transfer parsing. The `llm` crate adds an abstraction layer that hides details you need to see.

## Common Pitfalls

### Pitfall 1: Context Window Overflow Degrading Bot Personality
**What goes wrong:** As conversations grow, SOUL.md instructions get deprioritized by the model because they are at the beginning of a large context. The bot gradually loses its personality and starts behaving generically.
**Why it happens:** LLMs exhibit a "lost in the middle" effect where information at the start and end of context is recalled better than the middle. Without token budgeting, conversation history pushes soul instructions into the "forgotten middle."
**How to avoid:** Implement `TokenBudget` from day one. Reserve fixed allocations for soul (high priority), memory, and user context. Implement sliding window conversation history that trims old messages when budget is exceeded. Consider repeating key soul traits at the end of the system prompt.
**Warning signs:** Bot personality drift during conversations longer than 20 turns. Bot ignoring instructions that are clearly in SOUL.md.

### Pitfall 2: Blocking the Tokio Runtime with Synchronous Operations
**What goes wrong:** SQLite writes for chat persistence, session memory extraction, and tracing export block the Tokio event loop, causing streaming token delivery to stutter or freeze.
**Why it happens:** SeaORM uses SQLx which is async, but certain operations (tracing subscriber flush, file I/O for soul reading) may be synchronous. Mixing sync and async causes thread starvation.
**How to avoid:** Use `tokio::task::spawn_blocking` for any synchronous operation. SeaORM/SQLx is already async. For file I/O (reading SOUL.md), use `tokio::fs`. For tracing export, use async exporters (`opentelemetry-otlp` with the `rt-tokio` feature). Never call `.block_on()` inside an async context.
**Warning signs:** Streaming tokens arrive in bursts rather than smoothly. `tokio-console` shows blocked worker threads.

### Pitfall 3: Streaming Backpressure Causing Memory Growth
**What goes wrong:** If the CLI output is slower than the LLM token generation (e.g., slow terminal, piped output), the SSE stream buffer grows unboundedly, eventually consuming excessive memory.
**Why it happens:** By default, `reqwest-eventsource` and Tokio streams will buffer all incoming data. If the consumer (CLI stdout) is slow, the buffer grows.
**How to avoid:** Use bounded channels (`tokio::sync::mpsc::channel` with capacity) between the SSE stream consumer and the CLI output writer. If the channel fills, apply backpressure (pause reading from the SSE stream). In practice, for a single-user CLI, this is unlikely to be an issue, but the pattern should be established for Phase 4 (web UI with multiple concurrent streams).
**Warning signs:** Memory usage growing linearly during long streaming responses. `tokio-console` shows backed-up message queues.

### Pitfall 4: Session Memory Extraction Failing Silently
**What goes wrong:** The LLM call to extract session memory fails (rate limit, network error, malformed response) and the system silently discards the memory, leading to amnesia across sessions.
**Why it happens:** Memory extraction is a "fire and forget" operation at session end. If it fails, the user has already closed the terminal.
**How to avoid:** Queue failed memory extractions for retry. Store the raw conversation in the chat persistence layer (which is independent of memory extraction). On the next session start, if memory extraction is pending, retry it. Log extraction failures prominently.
**Warning signs:** Bot that never references previous conversations. Empty session_memories table despite multiple sessions.

### Pitfall 5: SSE Event Ordering Assumptions
**What goes wrong:** Code assumes SSE events arrive in a strict order (message_start, then content_block_start, then deltas). But Anthropic docs say they may add new event types, and ping events can appear anywhere.
**Why it happens:** Developers test with simple responses and miss edge cases (tool use blocks interleaved with text blocks, error events mid-stream, multiple content blocks).
**How to avoid:** Implement a state machine for SSE event processing. Handle unknown event types gracefully (log and skip). Test with tool-use responses (which have multiple content blocks: text + tool_use). Test with error events mid-stream.
**Warning signs:** Panics or garbled output during tool-use responses. Missing text when the model produces multiple content blocks.

### Pitfall 6: API Key Leaking to Logs via Tracing
**What goes wrong:** The Anthropic API key appears in tracing spans because the HTTP request is instrumented with request headers.
**Why it happens:** `tower-http::TraceLayer` logs request headers by default. The `x-api-key` header contains the API key.
**How to avoid:** Wrap API keys in `secrecy::SecretString`. Configure `tower-http::TraceLayer` to redact sensitive headers. Never log the full HTTP request in production. The `secrecy` crate's `Debug` impl prints `[REDACTED]`.
**Warning signs:** API keys visible in log output or trace data.

## Code Examples

### Chat Persistence Schema (SeaORM)
```rust
// Source: SeaORM entity pattern for chat history
// In boternity-infra/src/storage/entities/

// chat_sessions table
#[derive(Clone, Debug, DeriveEntityModel)]
#[sea_orm(table_name = "chat_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,              // Session ID (UUIDv7 for time-ordering)
    pub bot_id: Uuid,          // Which bot
    pub started_at: DateTime,
    pub ended_at: Option<DateTime>,
    pub title: Option<String>, // Auto-generated or user-set
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub message_count: i32,
}

// chat_messages table
#[derive(Clone, Debug, DeriveEntityModel)]
#[sea_orm(table_name = "chat_messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,              // Message ID (UUIDv7)
    pub session_id: Uuid,      // FK to chat_sessions
    pub role: String,          // "user", "assistant", "system"
    pub content: String,       // Message text
    pub created_at: DateTime,
    pub input_tokens: Option<i32>,  // For assistant messages
    pub output_tokens: Option<i32>, // For assistant messages
    pub model: Option<String>,      // Which model generated this
    pub stop_reason: Option<String>,
}

// session_memories table
#[derive(Clone, Debug, DeriveEntityModel)]
#[sea_orm(table_name = "session_memories")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub bot_id: Uuid,          // Which bot's memory
    pub session_id: Uuid,      // From which session
    pub fact: String,          // The extracted key point
    pub category: String,      // "preference", "fact", "decision", "context"
    pub importance: i32,       // 1-5
    pub created_at: DateTime,
}
```

### Tracing Setup with OTel GenAI Conventions
```rust
// Source: tracing-opentelemetry 0.32.1 docs + OTel GenAI semantic conventions
// In boternity-observe/src/tracing_setup.rs

use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing() -> Result<(), Box<dyn std::error::Error>> {
    // OTel tracer provider (stdout exporter for Phase 2, OTLP later)
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
        .build();

    let tracer = provider.tracer("boternity");

    // Build layered subscriber
    tracing_subscriber::registry()
        // Structured JSON logs for production
        .with(tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE))
        // OTel trace export
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        // Environment filter (RUST_LOG=info,boternity=debug)
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    Ok(())
}
```

### System Prompt Assembly with Token Budget
```rust
// In boternity-core/src/agent/prompt.rs

pub struct SystemPromptBuilder;

impl SystemPromptBuilder {
    pub fn build(
        &self,
        soul: &str,           // Raw SOUL.md content
        memories: &[KeyPoint], // Retrieved session memories
        user_md: &str,        // USER.md content
    ) -> Result<String, PromptError> {
        let mut prompt = String::new();

        // Soul section (highest priority - always included in full)
        prompt.push_str("<soul>\n");
        prompt.push_str(soul);
        prompt.push_str("\n</soul>\n\n");

        // User context section
        if !user_md.is_empty() {
            prompt.push_str("<user_context>\n");
            prompt.push_str(user_md);
            prompt.push_str("\n</user_context>\n\n");
        }

        // Session memory section (if any memories exist)
        if !memories.is_empty() {
            prompt.push_str("<session_memory>\n");
            prompt.push_str("Key points from previous conversations:\n");
            for memory in memories {
                prompt.push_str(&format!("- {} [{}]\n", memory.fact, memory.category));
            }
            prompt.push_str("</session_memory>\n\n");
        }

        // XML tag demarcation helps prevent prompt injection
        // (from pitfalls research: use tags for content boundaries)

        Ok(prompt)
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed-size conversation buffer | Sliding window with LLM-generated summaries | 2025 (Mem0, ProMem) | Better memory retention with lower token cost |
| Custom logging for LLM calls | OTel GenAI Semantic Conventions | 2025 (OTel v1.37+) | Standard attributes enable cross-vendor observability |
| Third-party crate for multi-provider LLM | Thin custom trait + per-provider impl | Current best practice | Better control, stability, and debuggability |
| Client-side token estimation (tiktoken-like) | Provider token counting APIs (Anthropic /count_tokens) | 2025 | Exact counts; provider-specific tokenizers differ significantly |
| Synchronous readline for CLI chat | `rustyline-async` with crossterm backend | 2025 | Enables concurrent streaming output and user input |
| Static session memory (dump full history) | LLM-assisted key point extraction (ProMem-style) | Jan 2026 | Compact, relevant memories instead of raw conversation dumps |

**Deprecated/outdated:**
- `rustformers/llm`: Explicitly unmaintained; README says to look elsewhere
- `llm-chain` crate: Inspired by LangChain; not actively maintained
- `tiktoken-rs` for Claude token counting: Claude uses a different tokenizer than GPT; use Anthropic's counting API instead
- `tui-rs`: Archived in favor of `ratatui` (forked in 2023)

---

## Deep Dive: LLM Provider Trait Design

**Confidence:** HIGH (verified against rig-core 0.30.0, genai 0.5.3, Anthropic official docs)

### Design Inspirations from Existing Crates

**rig-core 0.30.0** uses these key traits:
- `CompletionModel` -- low-level trait for model providers to implement
- `Prompt` -- high-level simple prompt-in/response-out interface
- `Chat` -- high-level chat interface (prompt + history in, response out)
- `Completion` -- low-level customization interface
- `CompletionRequest` / `CompletionResponse` / `ToolDefinition` / `Usage` as core types
- Only 32.54% documented, but trait structure is sound

**genai 0.5.3** uses adapter-based dispatch:
- `Client` struct with `exec_chat()` and `exec_chat_stream()` methods
- Model name string determines provider (e.g., "claude-*" routes to Anthropic adapter)
- `ChatRequest` / `ChatResponse` / `ChatStream` core types
- `AuthResolver` and `AdapterKindResolver` for extensibility

**Key takeaway:** Both crates separate the request/response types from the provider trait. The provider trait is narrow (complete, stream). Higher-level concerns (agent loops, tool dispatch) are built ON TOP of the trait, not inside it.

### Complete LlmProvider Trait (Refined)

```rust
// In boternity-core/src/llm/provider.rs
// Refined based on rig-core, genai, and Anthropic API analysis

use async_trait::async_trait;
use futures_util::Stream;
use std::pin::Pin;

// ---------- Capability Discovery ----------

/// What a provider can do. Queried once at startup.
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tool_calling: bool,
    pub vision: bool,
    pub extended_thinking: bool,
    pub max_context_tokens: u32,
    pub max_output_tokens: u32,
    /// Models available from this provider (populated lazily or at init)
    pub available_models: Vec<ModelInfo>,
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,             // e.g., "claude-sonnet-4-5"
    pub display_name: String,   // e.g., "Claude Sonnet 4.5"
    pub context_window: u32,    // e.g., 200_000
    pub max_output: u32,        // e.g., 128_000
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub supports_thinking: bool,
}

// ---------- Request / Response Types ----------

/// Provider-agnostic completion request.
/// Maps cleanly to Anthropic Messages API, OpenAI Chat API, etc.
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub tools: Vec<ToolDefinition>,
    pub tool_choice: Option<ToolChoice>,
    pub stream: bool,
    /// Provider-specific options (e.g., thinking budget, top_k)
    pub extra: Option<serde_json::Value>,
}

/// A single message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// Content block within a message -- supports text, tool use, and tool results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },

    #[serde(rename = "thinking")]
    Thinking { thinking: String },
}

/// Tool definition following Anthropic's JSON Schema format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolChoice {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "any")]
    Any,
    #[serde(rename = "tool")]
    Tool { name: String },
    #[serde(rename = "none")]
    None,
}

/// Non-streaming completion response
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub usage: Usage,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
    PauseTurn,
}

#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: Option<u32>,
    pub cache_read_input_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct TokenCount {
    pub input_tokens: u32,
}

// ---------- Streaming Types ----------

/// Events yielded during streaming. Maps to Anthropic SSE events but is
/// provider-agnostic.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Connection established (informational)
    Connected,
    /// New content block started (index, type)
    ContentBlockStart { index: u32, content_type: String },
    /// Text content delta
    TextDelta { index: u32, text: String },
    /// Thinking content delta (extended thinking)
    ThinkingDelta { index: u32, thinking: String },
    /// Tool use input JSON delta (accumulate, parse on ContentBlockStop)
    InputJsonDelta { index: u32, partial_json: String },
    /// Content block finished (index)
    ContentBlockStop { index: u32 },
    /// Completed tool use block (fully parsed after accumulation)
    ToolUseComplete { id: String, name: String, input: serde_json::Value },
    /// Usage metadata (cumulative from message_delta)
    Usage(Usage),
    /// Stream completed with stop reason (from message_delta)
    MessageDelta { stop_reason: StopReason },
    /// Final message_stop
    Done,
}

// ---------- Error Types ----------

/// Error hierarchy: provider-specific errors wrap into LlmError
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Request failed: status={status}, message={message}")]
    Api { status: u16, message: String },

    #[error("Provider error: {message}")]
    Provider { message: String },

    #[error("Streaming error: {0}")]
    Stream(String),

    #[error("Overloaded: {0}")]
    Overloaded(String),

    #[error("Rate limited: retry_after={retry_after_ms:?}ms")]
    RateLimited { retry_after_ms: Option<u64> },

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Token limit exceeded: {0}")]
    TokenLimitExceeded(String),

    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),

    #[error("Timeout after {0:?}")]
    Timeout(std::time::Duration),
}

impl LlmError {
    /// Whether this error is safe to retry automatically
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            LlmError::Connection(_)
                | LlmError::Overloaded(_)
                | LlmError::RateLimited { .. }
                | LlmError::Timeout(_)
                | LlmError::Stream(_)
        )
    }
}

// ---------- The Trait ----------

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider identifier (e.g., "anthropic", "openai")
    fn name(&self) -> &str;

    /// What this provider supports
    fn capabilities(&self) -> &ProviderCapabilities;

    /// Non-streaming completion
    async fn complete(
        &self,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse, LlmError>;

    /// Streaming completion -- returns a boxed stream of events
    fn stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>>;

    /// Count tokens in a request (pre-send estimation)
    async fn count_tokens(
        &self,
        request: &CompletionRequest,
    ) -> Result<TokenCount, LlmError>;
}
```

### Design Decisions and Rationale

**Why `Pin<Box<dyn Stream>>` for streaming (not channels, not async iterators):**
- `Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>` is the established pattern in the Rust async ecosystem. Both `genai` and `rig-core` use this approach.
- Channels (`tokio::sync::mpsc`) add unnecessary indirection for the provider layer. The stream IS the natural abstraction for SSE consumption.
- Rust's native `AsyncIterator` is still unstable as of Rust 1.84. The `Stream` trait from `futures-core` (re-exported by `futures-util`) is the stable equivalent.
- The `'static` lifetime bound is necessary because the stream captures the HTTP connection and outlives the `stream()` method call.

**Why `ContentBlock` enum with `#[serde(tag = "type")]`:**
- Anthropic's Messages API models content as arrays of typed blocks: `text`, `tool_use`, `tool_result`, `thinking`. This is the native representation.
- OpenAI has a similar structure for function calling. Mapping is straightforward.
- Using a tagged enum gives exhaustive pattern matching at compile time.

**Why separate `StreamEvent::InputJsonDelta` and `StreamEvent::ToolUseComplete`:**
- Anthropic streams tool input as partial JSON fragments. You MUST accumulate them and parse after `content_block_stop`. Emitting a raw `InputJsonDelta` lets the consumer (stream processor in `boternity-infra`) handle accumulation internally, then emit `ToolUseComplete` with the fully parsed input.
- This keeps the trait clean (it yields events) while the Anthropic implementation handles accumulation.

**Why explicit `LlmError::is_retryable()`:**
- Connection errors, overload, rate limits, and timeouts are retryable. Auth failures and invalid requests are not.
- The agent engine uses this to decide whether to retry a failed LLM call.

### Extensibility for Phase 3 (Multi-Provider)

Adding a new provider requires only:
1. A new struct implementing `LlmProvider` in `boternity-infra`
2. A type mapping from provider-specific request/response JSON to `CompletionRequest`/`CompletionResponse`
3. An SSE stream adapter that maps provider SSE events to `StreamEvent`

The `extra: Option<serde_json::Value>` field on `CompletionRequest` allows passing provider-specific options (e.g., Anthropic's `thinking.budget_tokens`, OpenAI's `logprobs`) without changing the core type.

Providers with different capabilities (e.g., some lack tool calling) declare this via `ProviderCapabilities`. The agent engine checks capabilities before including tools in requests.

---

## Deep Dive: Session Memory Extraction

**Confidence:** MEDIUM-HIGH (Mem0 architecture verified via arXiv paper; ProMem verified via arXiv; Rust-specific implementation is original design)

### How Mem0 Works (Verified Architecture)

Source: [Mem0: Building Production-Ready AI Agents with Scalable Long-Term Memory](https://arxiv.org/html/2504.19413v1)

Mem0 operates in two phases:

**Phase 1 -- Extraction:**
The system constructs a prompt combining: conversation summary S, the m most recent messages (default m=10), and the current message pair. An LLM function extracts candidate memories as natural language facts.

**Phase 2 -- Update (AUDN Cycle):**
For each extracted candidate, the system:
1. **Searches** the vector store for the top s=10 semantically similar existing memories
2. **Delegates** to the LLM via tool calls to decide an operation:
   - **ADD** -- new fact, no semantic equivalent exists
   - **UPDATE** -- augments an existing memory with new information
   - **DELETE** -- removes a memory contradicted by new information
   - **NOOP** -- no change needed

**Key insight from Mem0:** The LLM decides AUDN operations directly via tool calling rather than using a separate classifier. This is simpler to implement and leverages the LLM's semantic understanding.

### How ProMem Differs (Verified Architecture)

Source: [Beyond Static Summarization: Proactive Memory Extraction for LLM Agents](https://arxiv.org/html/2601.04463)

ProMem introduces a **recurrent feedback loop** that Mem0 lacks:
1. **Initial extraction** -- standard one-pass extraction (like Mem0 phase 1)
2. **Semantic alignment** -- maps extracted facts back to dialogue turns using embedding similarity (cosine > 0.6 threshold)
3. **Missing recovery** -- identifies "uncovered turns" (dialogue segments not captured by any fact) and re-extracts from them
4. **Self-questioning** -- for each memory entry, generates a probing question. If the dialogue cannot answer the question, the entry is discarded as potentially hallucinated
5. **Deduplication** -- merges entries with embedding similarity > 0.8

**ProMem achieves 73.8% memory integrity vs ~42% for one-pass extraction.** However, it requires embeddings and multiple LLM calls per extraction, making it expensive.

### Boternity Phase 2 Approach: Simplified Mem0

For Phase 2 (session memory, not long-term vector memory), use a simplified single-pass extraction. The full AUDN cycle and ProMem feedback loop are deferred to Phase 3 when vector memory (LanceDB) is available.

**Phase 2 extraction = Mem0 Phase 1 only (one-pass LLM extraction, no vector search, no AUDN).**

### Complete Extraction Prompt

```rust
// In boternity-core/src/memory/extractor.rs

/// System prompt for session memory extraction.
/// Designed for structured output that parses reliably.
const EXTRACTION_SYSTEM_PROMPT: &str = r#"You are a memory extraction assistant. Your job is to extract key facts, preferences, and important points from conversations that would be useful to remember for future conversations with this user.

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

/// User message template for extraction.
/// Formats the conversation as a labeled transcript.
const EXTRACTION_USER_TEMPLATE: &str = r#"Extract key memories from this conversation:

<conversation>
{conversation}
</conversation>

Return the JSON array of extracted memories."#;
```

### SQLite Schema for Session Memories

```sql
-- In boternity-infra migrations

CREATE TABLE session_memories (
    id              TEXT PRIMARY KEY,           -- UUIDv7 (time-sortable)
    bot_id          TEXT NOT NULL,              -- FK to bots.id
    session_id      TEXT NOT NULL,              -- FK to chat_sessions.id
    fact            TEXT NOT NULL,              -- The extracted key point
    category        TEXT NOT NULL               -- preference|fact|decision|context|correction
        CHECK (category IN ('preference', 'fact', 'decision', 'context', 'correction')),
    importance      INTEGER NOT NULL            -- 1-5
        CHECK (importance BETWEEN 1 AND 5),
    source_turn     INTEGER,                   -- Which turn in the conversation (nullable)
    superseded_by   TEXT,                       -- FK to session_memories.id (for corrections)
    created_at      TEXT NOT NULL,              -- ISO 8601 timestamp
    expires_at      TEXT,                       -- Optional TTL for temporary context

    FOREIGN KEY (bot_id) REFERENCES bots(id),
    FOREIGN KEY (session_id) REFERENCES chat_sessions(id)
);

-- Index for retrieval: get all memories for a bot, ordered by importance and recency
CREATE INDEX idx_memories_bot_importance
    ON session_memories(bot_id, importance DESC, created_at DESC);

-- Index for session-specific queries
CREATE INDEX idx_memories_session
    ON session_memories(session_id);

-- Pending extractions queue (for retry on failure)
CREATE TABLE pending_memory_extractions (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL,
    bot_id          TEXT NOT NULL,
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TEXT,
    next_attempt_at TEXT NOT NULL,
    error_message   TEXT,
    created_at      TEXT NOT NULL,

    FOREIGN KEY (session_id) REFERENCES chat_sessions(id),
    FOREIGN KEY (bot_id) REFERENCES bots(id)
);
```

### Retrieval Strategy for New Sessions

```rust
// In boternity-core/src/memory/store.rs

#[async_trait]
pub trait SessionMemoryStore: Send + Sync {
    /// Retrieve memories for a bot, ordered by importance then recency.
    /// Returns at most `limit` memories. Filters out superseded memories.
    async fn get_bot_memories(
        &self,
        bot_id: &BotId,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError>;

    /// Save extracted memories from a session.
    async fn save_memories(
        &self,
        bot_id: &BotId,
        session_id: &Uuid,
        memories: Vec<KeyPoint>,
    ) -> Result<(), MemoryError>;

    /// Queue a failed extraction for retry.
    async fn queue_pending_extraction(
        &self,
        session_id: &Uuid,
        bot_id: &BotId,
        error: &str,
    ) -> Result<(), MemoryError>;

    /// Get pending extractions ready for retry.
    async fn get_pending_extractions(
        &self,
        limit: usize,
    ) -> Result<Vec<PendingExtraction>, MemoryError>;
}
```

**Phase 2 retrieval strategy:** Full dump with importance sorting. Retrieve the top N memories (default N=50) for the bot, sorted by importance DESC then created_at DESC. Include all of them in the system prompt under `<session_memory>` tags. This is simple but effective for Phase 2 where memory volume is low. Phase 3 adds vector-based semantic retrieval via LanceDB.

### Extraction Trigger Strategy

| Trigger | When | Tradeoff |
|---------|------|----------|
| **Session end (recommended for Phase 2)** | User types `/quit`, `/exit`, or Ctrl-C | Most reliable. Captures full conversation context. Risk: user kills terminal without clean exit. |
| **Periodic (every N turns)** | After every 20 user messages | Catches long conversations. Risk: redundant extractions, higher LLM cost. |
| **On disconnect** | Tokio signal handler catches SIGTERM/SIGINT | Safety net for unclean exits. Must be fast (timeout 5s). |

**Phase 2 implementation:** Extract at session end + queue on signal handler. The `pending_memory_extractions` table catches failures. On next session start for the same bot, check for and retry pending extractions.

### Memory Format: Natural Language Facts (Not JSON, Not Key-Value)

**Decision:** Store memories as natural language sentences, not structured JSON or key-value pairs.

**Rationale:**
- LLMs consume natural language most effectively. A sentence like "User prefers Python over JavaScript" is directly useful in a system prompt.
- Mem0 stores memories as natural language text. ProMem also produces natural language facts.
- Structured JSON (e.g., `{"key": "language_preference", "value": "python"}`) requires a mapping layer to reconstruct context, and loses nuance.
- The `category` and `importance` fields provide enough structure for retrieval and filtering without over-structuring the content.

---

## Deep Dive: Anthropic Streaming Implementation

**Confidence:** HIGH (verified against official Anthropic streaming docs at platform.claude.com)

### Complete SSE Event Type Reference

Source: [Anthropic Messages Streaming API](https://platform.claude.com/docs/en/api/messages-streaming)

The stream follows this flow:
1. `message_start` -- contains Message object with empty content
2. Per content block: `content_block_start` -> N x `content_block_delta` -> `content_block_stop`
3. `message_delta` -- top-level changes (stop_reason, usage) -- usage is CUMULATIVE
4. `message_stop` -- final event
5. `ping` events may appear anywhere
6. `error` events may appear mid-stream (e.g., `overloaded_error`)

### Exact Rust Structs for SSE Event Parsing

```rust
// In boternity-infra/src/llm/anthropic/types.rs
// These are Anthropic-specific types, NOT exposed to boternity-core

use serde::{Deserialize, Serialize};

// ---------- Top-Level SSE Event Wrapper ----------

/// Discriminator for SSE event types (from the `event:` field)
/// The `data:` field contains JSON matching one of these.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicSseEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: AnthropicMessage },

    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: u32,
        content_block: AnthropicContentBlock,
    },

    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: u32,
        delta: AnthropicDelta,
    },

    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },

    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: MessageDeltaBody,
        usage: AnthropicUsage,
    },

    #[serde(rename = "message_stop")]
    MessageStop {},

    #[serde(rename = "ping")]
    Ping {},

    #[serde(rename = "error")]
    Error { error: AnthropicApiError },
}

// ---------- Message ----------

#[derive(Debug, Deserialize)]
pub struct AnthropicMessage {
    pub id: String,
    pub role: String,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Option<AnthropicUsage>,
    pub content: Vec<AnthropicContentBlock>,
}

// ---------- Content Blocks ----------

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "thinking")]
    Thinking { thinking: String },

    #[serde(rename = "server_tool_use")]
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "web_search_tool_result")]
    WebSearchToolResult {
        tool_use_id: String,
        content: serde_json::Value,
    },
}

// ---------- Deltas ----------

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },

    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },

    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },

    #[serde(rename = "signature_delta")]
    SignatureDelta { signature: String },
}

#[derive(Debug, Deserialize)]
pub struct MessageDeltaBody {
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

// ---------- Usage ----------

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AnthropicUsage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,
}

// ---------- Errors ----------

#[derive(Debug, Deserialize)]
pub struct AnthropicApiError {
    #[serde(rename = "type")]
    pub error_type: String,  // e.g., "overloaded_error", "api_error"
    pub message: String,
}
```

### State Machine Stream Processor

The key insight is that tool use input arrives as partial JSON fragments across multiple `input_json_delta` events. The processor must accumulate these fragments per content block index and emit a complete `ToolUseComplete` event only after `content_block_stop`.

```rust
// In boternity-infra/src/llm/anthropic/streaming.rs

use std::collections::HashMap;

/// Tracks state during stream processing.
/// Each content block has its own accumulator.
struct StreamState {
    /// Accumulated partial JSON for tool_use blocks, keyed by block index
    tool_input_buffers: HashMap<u32, ToolUseAccumulator>,
    /// Message ID from message_start
    message_id: Option<String>,
    /// Model from message_start
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
            .expect("EventSource creation should not fail with valid RequestBuilder");

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
                    // Parse the JSON data payload
                    let sse_event: AnthropicSseEvent = serde_json::from_str(&msg.data)
                        .map_err(|e| LlmError::Deserialization(e))?;

                    match sse_event {
                        AnthropicSseEvent::MessageStart { message } => {
                            state.message_id = Some(message.id);
                            state.model = Some(message.model);
                            // input_tokens available here in message.usage
                            if let Some(usage) = message.usage {
                                yield StreamEvent::Usage(crate::Usage {
                                    input_tokens: usage.input_tokens,
                                    output_tokens: usage.output_tokens,
                                    ..Default::default()
                                });
                            }
                        }

                        AnthropicSseEvent::ContentBlockStart { index, content_block } => {
                            match &content_block {
                                AnthropicContentBlock::ToolUse { id, name, .. } => {
                                    state.tool_input_buffers.insert(index, ToolUseAccumulator {
                                        id: id.clone(),
                                        name: name.clone(),
                                        json_buffer: String::new(),
                                    });
                                }
                                _ => {}
                            }
                            let content_type = match &content_block {
                                AnthropicContentBlock::Text { .. } => "text",
                                AnthropicContentBlock::ToolUse { .. } => "tool_use",
                                AnthropicContentBlock::Thinking { .. } => "thinking",
                                _ => "unknown",
                            };
                            yield StreamEvent::ContentBlockStart {
                                index,
                                content_type: content_type.to_string(),
                            };
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
                                    // Accumulate -- do NOT parse yet
                                    if let Some(acc) = state.tool_input_buffers.get_mut(&index) {
                                        acc.json_buffer.push_str(&partial_json);
                                    }
                                    yield StreamEvent::InputJsonDelta { index, partial_json };
                                }
                                AnthropicDelta::SignatureDelta { .. } => {
                                    // Signature for thinking block verification
                                    // Store if needed, skip for now
                                }
                            }
                        }

                        AnthropicSseEvent::ContentBlockStop { index } => {
                            // If this was a tool_use block, parse the accumulated JSON
                            if let Some(acc) = state.tool_input_buffers.remove(&index) {
                                let input: serde_json::Value = if acc.json_buffer.is_empty() {
                                    serde_json::Value::Object(Default::default())
                                } else {
                                    serde_json::from_str(&acc.json_buffer)
                                        .map_err(|e| LlmError::Deserialization(e))?
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
                            yield StreamEvent::Usage(crate::Usage {
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

                        AnthropicSseEvent::Ping {} => {
                            // Keepalive -- ignore
                        }

                        AnthropicSseEvent::Error { error } => {
                            let err = match error.error_type.as_str() {
                                "overloaded_error" => LlmError::Overloaded(error.message),
                                "rate_limit_error" => LlmError::RateLimited { retry_after_ms: None },
                                "authentication_error" => LlmError::AuthenticationFailed,
                                _ => LlmError::Provider { message: error.message },
                            };
                            Err(err)?; // try_stream! propagates this
                        }
                    }
                }
                Err(reqwest_eventsource::Error::StreamEnded) => {
                    break;
                }
                Err(e) => {
                    Err(LlmError::Stream(e.to_string()))?;
                }
            }
        }
    })
}
```

### Mid-Stream Error Recovery

When Anthropic sends an `overloaded_error` or the connection drops mid-stream, the recovery strategy is:

1. **Save partial response** -- accumulate all text deltas and tool use blocks received so far
2. **Construct continuation request** -- create a new API request with the partial assistant response included as an assistant message, so the model continues from where it left off
3. **Resume streaming** -- the new stream picks up from the partial response

```rust
// In boternity-infra/src/llm/anthropic/mod.rs

/// Recovery data saved when a stream is interrupted
#[derive(Debug, Clone)]
pub struct PartialResponse {
    pub content_blocks: Vec<ContentBlock>,
    pub usage: Usage,
}

impl AnthropicProvider {
    /// Build a continuation request from a partial response.
    /// Anthropic docs: include partial assistant message, then continue.
    pub fn build_continuation_request(
        &self,
        original_request: &CompletionRequest,
        partial: &PartialResponse,
    ) -> CompletionRequest {
        let mut messages = original_request.messages.clone();

        // Append the partial assistant response
        messages.push(Message {
            role: Role::Assistant,
            content: partial.content_blocks.clone(),
        });

        CompletionRequest {
            messages,
            // NOTE: tool_use and thinking blocks cannot be partially recovered.
            // Only resume from the most recent complete text block.
            ..original_request.clone()
        }
    }
}
```

**Important caveat from Anthropic docs:** Tool use and extended thinking blocks cannot be partially recovered. You can only resume from the most recent complete text block.

### Backpressure Strategy

For Phase 2 (single-user CLI), backpressure is not a practical concern. However, the pattern should be established for Phase 4:

```rust
// Pattern for Phase 4 preparation (not required for Phase 2 CLI)
// Use a bounded channel between stream consumer and output writer

let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamEvent>(64); // 64-event buffer

// Producer task: reads from SSE stream, sends to channel
tokio::spawn(async move {
    let mut stream = provider.stream(request);
    while let Some(event) = stream.next().await {
        match event {
            Ok(ev) => {
                // send() will await if channel is full (backpressure)
                if tx.send(ev).await.is_err() {
                    break; // receiver dropped
                }
            }
            Err(e) => { /* handle error */ break; }
        }
    }
});

// Consumer: reads from channel, writes to output
while let Some(event) = rx.recv().await {
    // Process event (write to stdout, websocket, etc.)
}
```

For Phase 2, pipe the stream directly to stdout without a channel -- the simplicity is worth it for a single-user CLI.

### Connection Management

```rust
// In boternity-infra/src/llm/anthropic/client.rs

/// Anthropic HTTP client configuration
pub struct AnthropicClient {
    client: reqwest::Client,
    base_url: String,
    api_key: secrecy::SecretString,
}

impl AnthropicClient {
    pub fn new(api_key: secrecy::SecretString) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))  // 5 min for long responses
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(2)  // Keep 2 connections warm
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            base_url: "https://api.anthropic.com".to_string(),
            api_key,
        }
    }
}
```

---

## Deep Dive: Agent Engine Architecture

**Confidence:** HIGH for single-agent (Phase 2); MEDIUM for sub-agent extensibility (Phase 5 design is speculative)

### AgentContext Struct

```rust
// In boternity-core/src/agent/context.rs

use crate::llm::provider::{CompletionRequest, Message, ContentBlock, ToolDefinition, Usage};
use boternity_types::bot::BotId;
use uuid::Uuid;

/// All state needed for a running agent within a single request.
/// Designed for Phase 2 single-agent, extensible for Phase 5 sub-agents.
#[derive(Debug)]
pub struct AgentContext {
    // ---------- Identity ----------
    /// Which bot this agent serves
    pub bot_id: BotId,
    /// Unique ID for this agent execution (for tracing)
    pub execution_id: Uuid,
    /// The session this agent is operating within
    pub session_id: Uuid,

    // ---------- Soul & Memory ----------
    /// SOUL.md content (loaded once per session, immutable)
    pub soul: String,
    /// USER.md content (loaded once per session)
    pub user_md: String,
    /// Extracted memories from previous sessions
    pub memory_context: Vec<MemoryEntry>,

    // ---------- Conversation State ----------
    /// Full conversation history for this session
    pub messages: Vec<Message>,
    /// Accumulated assistant response for the current turn
    /// (used for partial response recovery and persistence)
    pub current_response_blocks: Vec<ContentBlock>,

    // ---------- Tools ----------
    /// Tools available to this agent (Phase 2: empty or minimal)
    pub tools: Vec<ToolDefinition>,

    // ---------- Budget Tracking ----------
    /// Token budget for this execution context
    pub budget: TokenBudget,
    /// Cumulative usage across all LLM calls in this agent turn
    pub cumulative_usage: Usage,
    /// Maximum number of LLM round-trips in a single turn (prevents infinite tool loops)
    pub max_iterations: u32,
    /// Current iteration count
    pub iteration: u32,

    // ---------- Agent Hierarchy (Phase 5 preparation) ----------
    /// Depth in the agent hierarchy (0 = root agent)
    pub depth: u32,
    /// Maximum allowed depth (enforced at AGNT-04: hard cap of 3)
    pub max_depth: u32,
    /// Parent agent's execution ID (None for root agent)
    pub parent_execution_id: Option<Uuid>,
}

impl AgentContext {
    /// Create a new root agent context for a chat session
    pub fn new_root(
        bot_id: BotId,
        session_id: Uuid,
        soul: String,
        user_md: String,
        memory_context: Vec<MemoryEntry>,
        budget: TokenBudget,
    ) -> Self {
        Self {
            bot_id,
            execution_id: Uuid::now_v7(),
            session_id,
            soul,
            user_md,
            memory_context,
            messages: Vec::new(),
            current_response_blocks: Vec::new(),
            tools: Vec::new(),
            budget,
            cumulative_usage: Usage::default(),
            max_iterations: 10,  // Default: max 10 LLM calls per turn
            iteration: 0,
            depth: 0,
            max_depth: 3,
            parent_execution_id: None,
        }
    }

    /// Can this agent spawn a sub-agent? (Phase 5)
    pub fn can_spawn_child(&self) -> bool {
        self.depth < self.max_depth
    }

    /// Create a child context for a sub-agent (Phase 5)
    pub fn spawn_child(&self, child_budget: TokenBudget) -> Self {
        Self {
            bot_id: self.bot_id.clone(),
            execution_id: Uuid::now_v7(),
            session_id: self.session_id,
            soul: self.soul.clone(),
            user_md: String::new(), // Sub-agents don't get user context
            memory_context: Vec::new(), // Sub-agents start fresh
            messages: Vec::new(),
            current_response_blocks: Vec::new(),
            tools: Vec::new(), // Set by parent based on task
            budget: child_budget,
            cumulative_usage: Usage::default(),
            max_iterations: 5, // Sub-agents get fewer iterations
            iteration: 0,
            depth: self.depth + 1,
            max_depth: self.max_depth,
            parent_execution_id: Some(self.execution_id),
        }
    }

    /// Add token usage and check budget
    pub fn track_usage(&mut self, usage: &Usage) -> Result<(), AgentError> {
        self.cumulative_usage.input_tokens += usage.input_tokens;
        self.cumulative_usage.output_tokens += usage.output_tokens;

        let total = self.cumulative_usage.input_tokens + self.cumulative_usage.output_tokens;
        let budget_total = self.budget.total_context; // Simplified check

        if total > budget_total {
            return Err(AgentError::BudgetExceeded {
                used: total,
                budget: budget_total,
            });
        }
        Ok(())
    }
}
```

### Complete Agent Turn Loop

The agent turn loop is the core execution pattern. It handles the LLM call -> tool use -> LLM call cycle.

```rust
// In boternity-core/src/agent/engine.rs

pub struct AgentEngine {
    provider: Arc<dyn LlmProvider>,
    memory_store: Arc<dyn SessionMemoryStore>,
    prompt_builder: SystemPromptBuilder,
    tool_registry: Arc<dyn ToolRegistry>, // Phase 2: empty impl
}

/// Result of a single agent turn
pub enum TurnResult {
    /// Agent produced a final text response (stream of events)
    Response(Pin<Box<dyn Stream<Item = Result<StreamEvent, AgentError>> + Send>>),
    /// Agent needs to execute tool(s) and continue
    ToolCalls(Vec<ToolCall>),
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

impl AgentEngine {
    /// Execute a full agent turn. This is the OUTER loop that handles
    /// tool use cycles. Returns a stream for the FINAL response.
    ///
    /// Flow:
    /// 1. Build prompt (soul + memory + history + user message)
    /// 2. Call LLM (streaming)
    /// 3. If stop_reason == "tool_use": execute tools, add results, goto 2
    /// 4. If stop_reason == "end_turn": return the response stream
    /// 5. If max_iterations exceeded: return partial response + warning
    #[tracing::instrument(
        skip(self, ctx, user_message),
        fields(
            gen_ai.operation.name = "chat",
            bot_id = %ctx.bot_id,
            session_id = %ctx.session_id,
            execution_id = %ctx.execution_id,
            depth = ctx.depth,
        )
    )]
    pub async fn execute_turn(
        &self,
        ctx: &mut AgentContext,
        user_message: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, AgentError>> + Send>>, AgentError> {
        // Add user message to context
        ctx.messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: user_message.to_string() }],
        });

        // The agentic loop
        loop {
            ctx.iteration += 1;
            if ctx.iteration > ctx.max_iterations {
                return Err(AgentError::MaxIterationsExceeded {
                    max: ctx.max_iterations,
                });
            }

            // Build system prompt
            let system_prompt = self.prompt_builder.build(
                &ctx.soul,
                &ctx.memory_context,
                &ctx.user_md,
            )?;

            // Build completion request
            let request = CompletionRequest {
                model: "claude-sonnet-4-5".to_string(), // From bot config
                system: Some(system_prompt),
                messages: ctx.messages.clone(),
                max_tokens: ctx.budget.output_reserve,
                temperature: Some(0.7),
                tools: ctx.tools.clone(),
                tool_choice: if ctx.tools.is_empty() { None } else { Some(ToolChoice::Auto) },
                stream: true,
                extra: None,
            };

            // Collect the stream to determine if tool use is needed.
            // For the FINAL response (no tool calls), we stream directly to the CLI.
            // For intermediate tool-call responses, we must collect fully to extract tool inputs.
            let stream = self.provider.stream(request);
            let (content_blocks, usage, stop_reason) = self.collect_stream(stream).await?;

            // Track usage
            ctx.track_usage(&usage)?;

            // Add assistant response to conversation history
            ctx.messages.push(Message {
                role: Role::Assistant,
                content: content_blocks.clone(),
            });

            match stop_reason {
                StopReason::EndTurn | StopReason::MaxTokens | StopReason::StopSequence => {
                    // Final response -- return a stream that replays the collected content
                    // (or for optimization, return the live stream on the last iteration)
                    let replay_stream = self.replay_content_as_stream(content_blocks);
                    return Ok(replay_stream);
                }

                StopReason::ToolUse => {
                    // Extract tool calls from content blocks
                    let tool_calls: Vec<ToolCall> = content_blocks
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                input: input.clone(),
                            }),
                            _ => None,
                        })
                        .collect();

                    // Execute all tools (parallel for independent tools)
                    let tool_results = self.execute_tools(&tool_calls).await?;

                    // Add tool results as a user message
                    // CRITICAL: All tool_result blocks must be in a SINGLE user message
                    // CRITICAL: tool_result blocks must come FIRST before any text
                    let result_blocks: Vec<ContentBlock> = tool_results
                        .into_iter()
                        .map(|result| ContentBlock::ToolResult {
                            tool_use_id: result.tool_use_id,
                            content: result.content,
                            is_error: result.is_error,
                        })
                        .collect();

                    ctx.messages.push(Message {
                        role: Role::User,
                        content: result_blocks,
                    });

                    // Loop back to call LLM again with tool results
                    continue;
                }

                StopReason::PauseTurn => {
                    // Server tool paused -- continue the turn
                    // Add assistant content to messages and re-call
                    continue;
                }
            }
        }
    }

    /// Collect a stream into content blocks, usage, and stop reason.
    /// Used for intermediate tool-call responses where we need the full response.
    async fn collect_stream(
        &self,
        mut stream: Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send>>,
    ) -> Result<(Vec<ContentBlock>, Usage, StopReason), AgentError> {
        let mut text_buffers: HashMap<u32, String> = HashMap::new();
        let mut thinking_buffers: HashMap<u32, String> = HashMap::new();
        let mut tool_uses: Vec<(String, String, serde_json::Value)> = Vec::new();
        let mut usage = Usage::default();
        let mut stop_reason = StopReason::EndTurn;

        while let Some(event) = stream.next().await {
            match event.map_err(AgentError::Llm)? {
                StreamEvent::TextDelta { index, text } => {
                    text_buffers.entry(index).or_default().push_str(&text);
                }
                StreamEvent::ThinkingDelta { index, thinking } => {
                    thinking_buffers.entry(index).or_default().push_str(&thinking);
                }
                StreamEvent::ToolUseComplete { id, name, input } => {
                    tool_uses.push((id, name, input));
                }
                StreamEvent::Usage(u) => {
                    usage = u;
                }
                StreamEvent::MessageDelta { stop_reason: sr } => {
                    stop_reason = sr;
                }
                StreamEvent::Done => break,
                _ => {} // ContentBlockStart, ContentBlockStop, InputJsonDelta, Connected
            }
        }

        // Build content blocks in order
        let mut blocks = Vec::new();
        for (_idx, thinking) in &thinking_buffers {
            blocks.push(ContentBlock::Thinking { thinking: thinking.clone() });
        }
        for (_idx, text) in &text_buffers {
            blocks.push(ContentBlock::Text { text: text.clone() });
        }
        for (id, name, input) in tool_uses {
            blocks.push(ContentBlock::ToolUse { id, name, input });
        }

        Ok((blocks, usage, stop_reason))
    }

    /// Execute tool calls. Phase 2 returns errors for all tools (no tools registered).
    /// Phase 6 will implement actual tool execution.
    async fn execute_tools(
        &self,
        tool_calls: &[ToolCall],
    ) -> Result<Vec<ToolResult>, AgentError> {
        let mut results = Vec::new();
        for call in tool_calls {
            match self.tool_registry.execute(&call.name, &call.input).await {
                Ok(output) => results.push(ToolResult {
                    tool_use_id: call.id.clone(),
                    content: output,
                    is_error: false,
                }),
                Err(e) => results.push(ToolResult {
                    tool_use_id: call.id.clone(),
                    content: format!("Error: {}", e),
                    is_error: true,
                }),
            }
        }
        Ok(results)
    }
}

#[derive(Debug)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}
```

### Tool Dispatch Pattern (Anthropic Messages API)

Source: [How to implement tool use](https://platform.claude.com/docs/en/agents-and-tools/tool-use/implement-tool-use)

The conversation flow for tool use is:

```
User message
    |
    v
[LLM Call] -> stop_reason: "tool_use"
    |
    v
Assistant message contains: [text block, tool_use block(s)]
    |
    v
Execute tool(s), get results
    |
    v
User message contains: [tool_result block(s)]  <-- ALL results in ONE message
    |                                               tool_result blocks FIRST
    v
[LLM Call] -> stop_reason: "end_turn" (or more tool calls)
    |
    v
Assistant message contains: [text block with final answer]
```

**Critical formatting rules from Anthropic docs:**
1. `tool_result` blocks must immediately follow their `tool_use` blocks (no intervening messages)
2. ALL `tool_result` blocks go in a SINGLE user message
3. `tool_result` blocks must come FIRST in the content array, before any text
4. Each `tool_result` must have a `tool_use_id` matching the `id` from the `tool_use` block
5. Parallel tool calls: Claude may return multiple `tool_use` blocks. Execute all, return all results in one message.

### Soul Injection in System Prompt

The system prompt structure follows Anthropic's best practices for system prompts:

```
<soul>
[Full SOUL.md content -- personality, values, behavior, goals]
</soul>

<user_context>
[USER.md content -- user name, preferences, communication style]
</user_context>

<session_memory>
Key points from previous conversations:
- [fact 1] [category]
- [fact 2] [category]
...
</session_memory>

<instructions>
You are {bot_name}. Always stay in character as defined in your soul.
When you are unsure about something, say so rather than guessing.
Reference previous conversation context from session memory when relevant.
</instructions>
```

The `<soul>` section is placed FIRST because Anthropic models pay strong attention to the beginning of the system prompt. XML tags prevent content injection across sections.

### Budget Enforcement Across Multiple LLM Calls

```rust
// In boternity-core/src/llm/token_budget.rs (refined)

/// Per-request token budget. Tracks usage across multiple LLM calls
/// in a single agent turn (e.g., tool use cycles).
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Maximum tokens for the entire request (all iterations combined)
    pub max_total_tokens: u32,
    /// Maximum tokens for model output per single LLM call
    pub max_output_per_call: u32,
    /// Reserved for system prompt (soul + memory + user_md + instructions)
    pub system_prompt_reserve: u32,
    /// Reserved for tool definitions in the request
    pub tool_definitions_reserve: u32,

    // --- Running totals ---
    /// Total input tokens consumed across all calls in this turn
    pub total_input_used: u32,
    /// Total output tokens consumed across all calls in this turn
    pub total_output_used: u32,
}

impl TokenBudget {
    pub fn new(model_context_window: u32) -> Self {
        Self {
            max_total_tokens: model_context_window / 2, // Conservative: half the window per turn
            max_output_per_call: 8_192,                  // Default per-call output limit
            system_prompt_reserve: 4_000,                // ~4K for soul + memory + user
            tool_definitions_reserve: 2_000,             // ~2K for tool schemas
            total_input_used: 0,
            total_output_used: 0,
        }
    }

    /// Available tokens for conversation history in the next LLM call
    pub fn available_for_history(&self) -> u32 {
        self.max_total_tokens
            .saturating_sub(self.system_prompt_reserve)
            .saturating_sub(self.tool_definitions_reserve)
            .saturating_sub(self.max_output_per_call)
            .saturating_sub(self.total_input_used)
    }

    /// Record usage from an LLM call. Returns error if budget exceeded.
    pub fn record_usage(&mut self, input: u32, output: u32) -> Result<(), BudgetError> {
        self.total_input_used += input;
        self.total_output_used += output;

        let total = self.total_input_used + self.total_output_used;
        if total > self.max_total_tokens {
            return Err(BudgetError::Exceeded {
                used: total,
                budget: self.max_total_tokens,
            });
        }
        Ok(())
    }

    /// Percentage of budget consumed
    pub fn utilization_percent(&self) -> f32 {
        let total = self.total_input_used + self.total_output_used;
        (total as f32 / self.max_total_tokens as f32) * 100.0
    }
}
```

### Phase 5 Sub-Agent Extensibility

The `AgentContext` is designed so that sub-agent spawning is a natural extension:

1. **`depth` and `max_depth`** fields enforce AGNT-04 (hard cap of 3 levels)
2. **`parent_execution_id`** creates a traceable parent-child chain for OBSV-01
3. **`spawn_child()`** creates a child context with reduced budget and iteration limits
4. **`can_spawn_child()`** is checked before any spawn attempt

In Phase 5, the `AgentEngine.execute_turn()` method would add a `SpawnAgent` tool to the tool registry. When the LLM calls this tool, the engine calls `ctx.spawn_child()`, creates a child `AgentEngine` execution, and returns the result as a tool result to the parent.

```
Root Agent (depth=0)
  |-- LLM call -> tool_use: "spawn_agent"
  |-- spawn_child() -> child_ctx (depth=1)
  |     |-- Child Agent.execute_turn()
  |     |-- Returns result string
  |-- tool_result: child's output
  |-- LLM call -> final response incorporating child result
```

---

## Open Questions

1. **`llm` crate (graniet) stability for Phase 3**
   - What we know: v1.3.7 published Jan 9, 2026. Rapid version churn but feature-rich.
   - What's unclear: Whether its multi-provider abstraction is stable enough for Phase 3 when we add OpenAI, Gemini, etc.
   - Recommendation: Build the custom provider trait now for Anthropic. Evaluate `llm` or `genai` for Phase 3's additional providers -- by then we will know if the custom approach scales or if using a multi-provider crate saves significant effort.

2. **Session memory extraction model choice**
   - What we know: Using the LLM to extract key points works well. Cheaper models (claude-sonnet-4-5, claude-haiku) can handle extraction.
   - What's unclear: Whether to use the same provider as the chat model or always use a cheap model. Token budget impact if the user's provider has low rate limits.
   - Recommendation: Default to the same provider but a cheaper model variant. Make the extraction model configurable in IDENTITY.md.

3. **Conversation history sliding window size**
   - What we know: Need to keep recent messages + summarize older ones. Typical approach: keep last 10-20 messages verbatim, summarize the rest.
   - What's unclear: Optimal window size for different context windows (200K for Claude vs 128K for other models).
   - Recommendation: Make window size proportional to model context window. Start with 20 recent messages + summary. Tune based on testing.

4. **`reqwest-eventsource` vs `async-sse` for SSE consumption**
   - What we know: `reqwest-eventsource` (0.6.x) wraps reqwest nicely. It handles retry and reconnection.
   - What's unclear: Whether it handles Anthropic's specific error events (like `overloaded_error` mid-stream) correctly.
   - Recommendation: Use `reqwest-eventsource` as the starting point. If it does not handle mid-stream errors from Anthropic correctly, fall back to raw `eventsource-stream` over `reqwest`'s response body stream.

5. **Streaming optimization for the final response**
   - What we know: The agent turn loop must collect intermediate tool-call responses fully (to extract tool inputs), but the FINAL response should stream directly to the CLI for token-by-token delivery.
   - What's unclear: Whether to always collect then replay, or detect "no tools" upfront and stream directly.
   - Recommendation: For Phase 2, always collect then replay (simpler). Optimize to direct-stream the final iteration in a later phase if latency is noticeable.

## Sources

### Primary (HIGH confidence)
- [Anthropic Messages Streaming API](https://platform.claude.com/docs/en/api/messages-streaming) -- Full SSE event type documentation, event flow, tool use streaming, error recovery
- [Anthropic Tool Use Implementation](https://platform.claude.com/docs/en/agents-and-tools/tool-use/implement-tool-use) -- Complete tool_use/tool_result format, parallel tool calls, conversation loop, error handling
- [Anthropic Token Counting API](https://platform.claude.com/docs/en/build-with-claude/token-counting) -- Free token counting endpoint, rate limits, supported content types
- [OpenTelemetry GenAI Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-spans/) -- Standard attributes for LLM call spans
- [tracing-opentelemetry 0.32.1](https://docs.rs/crate/tracing-opentelemetry/latest) -- Published 2026-01-12, compatible with OTel 0.31.0
- [opentelemetry 0.31.0](https://docs.rs/crate/opentelemetry/latest) -- Published 2025-09-25
- [rustyline-async 0.4.7](https://docs.rs/crate/rustyline-async/latest) -- Published 2025-07-21, async readline with crossterm
- [reqwest-eventsource 0.6.0](https://docs.rs/reqwest-eventsource/latest/reqwest_eventsource/) -- SSE stream consumer wrapping reqwest; 2.3M downloads, #19 in HTTP client category

### Secondary (MEDIUM confidence)
- [Mem0 arXiv Paper](https://arxiv.org/html/2504.19413v1) -- Full architecture: two-phase extraction+update, AUDN cycle, memory schema, ~7K tokens per conversation
- [ProMem arXiv Paper](https://arxiv.org/html/2601.04463) -- Recurrent feedback loop, self-questioning verification, 73.8% memory integrity, semantic alignment
- [rig-core v0.30.0](https://docs.rs/crate/rig-core/latest) -- CompletionModel trait, Agent struct, Tool trait, 32.54% documented
- [genai crate v0.5.3](https://github.com/jeremychone/rust-genai) -- Adapter-based dispatch, ChatRequest/ChatStream, model-name routing
- [misanthropy (Rust Anthropic SDK)](https://github.com/cortesi/misanthropy) -- Tool/ToolResult types, streaming via messages_stream()
- [async-stream crate](https://docs.rs/async-stream/latest/async_stream/) -- stream!/try_stream! macros for async stream creation

### Tertiary (LOW confidence)
- [Datadog OTel GenAI support](https://www.datadoghq.com/blog/llm-otel-semantic-convention/) -- Validates GenAI semantic conventions are production-ready
- [OTel Agentic Systems Proposal (GitHub Issue #2664)](https://github.com/open-telemetry/semantic-conventions/issues/2664) -- Proposed conventions for agent tracing (not yet merged)
- [ADK-Rust](https://docs.rs/adk-rust/latest/adk_rust/) -- Agent Development Kit for Rust; production-ready agent patterns
- [AutoAgents](https://github.com/liquidos-ai/AutoAgents) -- Rust multi-agent framework with Ractor

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- `reqwest`, `tracing`, `sea-orm`, `rustyline-async` are all established, version-stable crates
- Architecture: HIGH -- Provider trait pattern is well-established across multiple Rust LLM libraries; Anthropic SSE protocol is fully documented
- LLM provider trait design: HIGH -- Verified against rig-core, genai, and Anthropic official docs; concrete Rust code provided
- Anthropic streaming: HIGH -- All SSE event types verified against official docs; complete struct definitions and state machine provided
- Agent engine: HIGH for Phase 2 single-agent loop; MEDIUM for Phase 5 sub-agent design (speculative but structurally sound)
- Session memory extraction: MEDIUM-HIGH -- Mem0 and ProMem architectures verified via arXiv papers; Rust implementation is original design based on verified patterns
- Pitfalls: HIGH -- All pitfalls verified against project research (PITFALLS.md) and Anthropic's official documentation

**Research date:** 2026-02-10
**Deep dive date:** 2026-02-10
**Valid until:** 2026-03-10 (30 days; Rust LLM ecosystem is fast-moving but core libraries are stable)

---
*Phase 2 research for: Boternity -- Single-Agent Chat + LLM*
*Researched: 2026-02-10*
*Deep dive: LLM Provider Trait, Session Memory, Anthropic Streaming, Agent Engine*
