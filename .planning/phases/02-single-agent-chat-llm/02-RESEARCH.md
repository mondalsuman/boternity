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

## Sources

### Primary (HIGH confidence)
- [Anthropic Messages Streaming API](https://platform.claude.com/docs/en/api/messages-streaming) -- Full SSE event type documentation, event flow, tool use streaming, error recovery
- [Anthropic Token Counting API](https://platform.claude.com/docs/en/build-with-claude/token-counting) -- Free token counting endpoint, rate limits, supported content types
- [OpenTelemetry GenAI Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-spans/) -- Standard attributes for LLM call spans
- [tracing-opentelemetry 0.32.1](https://docs.rs/crate/tracing-opentelemetry/latest) -- Published 2026-01-12, compatible with OTel 0.31.0
- [opentelemetry 0.31.0](https://docs.rs/crate/opentelemetry/latest) -- Published 2025-09-25
- [rustyline-async 0.4.7](https://docs.rs/crate/rustyline-async/latest) -- Published 2025-07-21, async readline with crossterm
- [reqwest-eventsource](https://docs.rs/reqwest-eventsource/latest/reqwest_eventsource/) -- SSE stream consumer wrapping reqwest

### Secondary (MEDIUM confidence)
- [llm crate (graniet) v1.3.7](https://docs.rs/crate/llm/1.3.7) -- Published 2026-01-09; multi-provider LLM library; 68% doc coverage
- [genai crate v0.5.3](https://docs.rs/crate/genai/latest) -- Published 2026-01-31; Rust multi-provider GenAI client
- [rig-core v0.30.0](https://docs.rs/crate/rig-core/latest) -- Agent framework with OTel support
- [Agent Design Patterns (Lance Martin, Jan 2026)](https://rlancemartin.github.io/2026/01/09/agent_design/) -- Single-agent loop patterns, context caching, context offloading
- [Mem0 Architecture](https://mem0.ai/blog/llm-chat-history-summarization-guide-2025) -- Memory extraction, dual storage, session management
- [Beyond Static Summarization: ProMem (Jan 2026)](https://arxiv.org/html/2601.04463) -- Proactive memory extraction for LLM agents

### Tertiary (LOW confidence)
- [Datadog OTel GenAI support](https://www.datadoghq.com/blog/llm-otel-semantic-convention/) -- Validates GenAI semantic conventions are production-ready
- [OTel Agentic Systems Proposal (GitHub Issue #2664)](https://github.com/open-telemetry/semantic-conventions/issues/2664) -- Proposed conventions for agent tracing (not yet merged)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- `reqwest`, `tracing`, `sea-orm`, `rustyline-async` are all established, version-stable crates
- Architecture: HIGH -- Provider trait pattern is well-established across multiple Rust LLM libraries; Anthropic SSE protocol is fully documented
- LLM provider choice: MEDIUM -- Recommending custom over `llm`/`genai` is based on stability assessment and control needs; may prove over-conservative if those crates stabilize
- Session memory: MEDIUM -- Extraction pattern is well-documented in Python ecosystem (Mem0), but Rust-specific implementations are not widely documented
- Pitfalls: HIGH -- All pitfalls verified against project research (PITFALLS.md) and Anthropic's official documentation

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (30 days; Rust LLM ecosystem is fast-moving but core libraries are stable)

---
*Phase 2 research for: Boternity -- Single-Agent Chat + LLM*
*Researched: 2026-02-10*
