//! OpenTelemetry GenAI Semantic Convention attribute constants.
//!
//! These follow the OTel GenAI Semantic Conventions specification for consistent
//! LLM call instrumentation across the codebase. All constants are string slices
//! usable in `tracing::span!` and `tracing::info_span!` field names.
//!
//! Span naming convention: `"{operation} {model}"` (e.g., `"chat claude-sonnet-4-20250514"`)

// --- Required attributes ---

/// The name of the operation being performed (e.g., "chat", "invoke_agent").
pub const GEN_AI_OPERATION_NAME: &str = "gen_ai.operation.name";

/// The name of the GenAI provider (e.g., "anthropic").
pub const GEN_AI_PROVIDER_NAME: &str = "gen_ai.provider.name";

// --- Recommended attributes ---

/// The model ID requested (e.g., "claude-sonnet-4-20250514").
pub const GEN_AI_REQUEST_MODEL: &str = "gen_ai.request.model";

/// The sampling temperature for the request.
pub const GEN_AI_REQUEST_TEMPERATURE: &str = "gen_ai.request.temperature";

/// The maximum number of output tokens requested.
pub const GEN_AI_REQUEST_MAX_TOKENS: &str = "gen_ai.request.max_tokens";

/// The number of input tokens consumed.
pub const GEN_AI_USAGE_INPUT_TOKENS: &str = "gen_ai.usage.input_tokens";

/// The number of output tokens generated.
pub const GEN_AI_USAGE_OUTPUT_TOKENS: &str = "gen_ai.usage.output_tokens";

/// The finish reasons for the response (e.g., "end_turn", "tool_use").
pub const GEN_AI_RESPONSE_FINISH_REASONS: &str = "gen_ai.response.finish_reasons";

/// The unique response/message ID from the provider.
pub const GEN_AI_RESPONSE_ID: &str = "gen_ai.response.id";

// --- Agent-specific attributes ---

/// The unique identifier of the agent (bot_id).
pub const GEN_AI_AGENT_ID: &str = "gen_ai.agent.id";

/// The display name of the agent (bot display name).
pub const GEN_AI_AGENT_NAME: &str = "gen_ai.agent.name";

// --- Operation name values ---

/// Standard chat completion operation.
pub const OP_CHAT: &str = "chat";

/// Agent invocation operation.
pub const OP_INVOKE_AGENT: &str = "invoke_agent";

/// Memory extraction from a session.
pub const OP_EXTRACT_MEMORY: &str = "extract_memory";

/// Auto-title generation for a session.
pub const OP_GENERATE_TITLE: &str = "generate_title";

/// Context summarization for sliding window.
pub const OP_SUMMARIZE_CONTEXT: &str = "summarize_context";

// --- Provider name values ---

/// Anthropic provider identifier.
pub const PROVIDER_ANTHROPIC: &str = "anthropic";
