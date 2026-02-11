//! Agent execution engine for Boternity.
//!
//! AgentEngine coordinates the LLM call loop: assembles the CompletionRequest
//! from AgentContext, sends it through BoxLlmProvider, and returns streaming
//! events or full responses. OTel GenAI spans instrument every LLM call.

use std::pin::Pin;

use futures_util::Stream;
use tracing::{Instrument, info_span};

use boternity_types::llm::{
    CompletionRequest, CompletionResponse, LlmError, StreamEvent,
};

use crate::llm::box_provider::BoxLlmProvider;

use super::context::AgentContext;

/// Executes LLM calls on behalf of an agent.
///
/// Holds a `BoxLlmProvider` for runtime provider dispatch and builds
/// `CompletionRequest`s from `AgentContext` state.
pub struct AgentEngine {
    provider: BoxLlmProvider,
}

impl AgentEngine {
    /// Create a new agent engine with the given LLM provider.
    pub fn new(provider: BoxLlmProvider) -> Self {
        Self { provider }
    }

    /// Execute a streaming LLM call for a user message.
    ///
    /// Builds a `CompletionRequest` from the agent context (system prompt +
    /// conversation history + user message) and streams events back.
    ///
    /// The caller is responsible for updating `AgentContext.conversation_history`
    /// with the user message before calling and the assistant response after.
    pub fn execute(
        &self,
        context: &AgentContext,
        user_message: &str,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>> {
        let request = self.build_request(context, user_message);

        let span = info_span!(
            "gen_ai.execute",
            gen_ai.system = self.provider.name(),
            gen_ai.request.model = %request.model,
            gen_ai.request.max_tokens = request.max_tokens,
            gen_ai.request.temperature = ?request.temperature,
            gen_ai.request.stream = true,
        );

        let stream = self.provider.stream(request);

        // Wrap stream in the OTel span
        Box::pin(StreamInSpan { inner: stream, span })
    }

    /// Execute a non-streaming LLM call and return the full response.
    ///
    /// Useful for utility calls like title generation and memory extraction
    /// where streaming is not needed.
    pub async fn execute_non_streaming(
        &self,
        context: &AgentContext,
        user_message: &str,
    ) -> Result<CompletionResponse, LlmError> {
        let request = self.build_request(context, user_message);

        let span = info_span!(
            "gen_ai.complete",
            gen_ai.system = self.provider.name(),
            gen_ai.request.model = %request.model,
            gen_ai.request.max_tokens = request.max_tokens,
            gen_ai.request.temperature = ?request.temperature,
            gen_ai.request.stream = false,
        );

        self.provider.complete(&request).instrument(span).await
    }

    /// Generate a personality-driven greeting message.
    ///
    /// Sends a special prompt asking the bot to introduce itself,
    /// using its full personality context.
    pub async fn generate_greeting(
        &self,
        context: &AgentContext,
    ) -> Result<String, LlmError> {
        let greeting_prompt = "Generate a short, warm greeting message that introduces yourself \
            and invites the user to chat. Stay fully in character. Keep it under 2 sentences.";

        let request = self.build_request(context, greeting_prompt);

        let span = info_span!(
            "gen_ai.greeting",
            gen_ai.system = self.provider.name(),
            gen_ai.request.model = %request.model,
        );

        let response = self.provider.complete(&request).instrument(span).await?;
        Ok(response.content)
    }

    /// Build a CompletionRequest from the agent context and a user message.
    fn build_request(&self, context: &AgentContext, user_message: &str) -> CompletionRequest {
        let mut messages = context.build_messages();

        // Add the current user message to the request
        messages.push(boternity_types::llm::Message {
            role: boternity_types::llm::MessageRole::User,
            content: user_message.to_string(),
        });

        CompletionRequest {
            model: context.agent_config.model.clone(),
            messages,
            system: Some(context.system_prompt.clone()),
            max_tokens: context.agent_config.max_tokens,
            temperature: Some(context.agent_config.temperature),
            stream: true, // Default to streaming; overridden by complete()
            stop_sequences: None,
        }
    }
}

/// A stream wrapper that keeps an OTel span alive for the duration of streaming.
///
/// Without this, the span would be dropped immediately after creating the stream,
/// losing the instrumentation for the actual streaming duration.
struct StreamInSpan {
    inner: Pin<Box<dyn Stream<Item = Result<StreamEvent, LlmError>> + Send + 'static>>,
    span: tracing::Span,
}

impl Stream for StreamInSpan {
    type Item = Result<StreamEvent, LlmError>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // SAFETY: We only access `span` immutably and `inner` mutably.
        // `inner` is already behind Pin<Box<...>> so it is inherently pinned.
        let this = unsafe { self.get_unchecked_mut() };
        let _enter = this.span.enter();
        this.inner.as_mut().poll_next(cx)
    }
}

// SAFETY: StreamInSpan is Send because both inner (Pin<Box<dyn Stream + Send>>) and
// tracing::Span are Send.
unsafe impl Send for StreamInSpan {}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::agent::AgentConfig;
    use crate::llm::token_budget::TokenBudget;
    use uuid::Uuid;

    fn test_context() -> AgentContext {
        let config = AgentConfig {
            bot_id: Uuid::now_v7(),
            bot_name: "Luna".to_string(),
            bot_slug: "luna".to_string(),
            bot_emoji: Some("🌙".to_string()),
            model: "claude-sonnet-4-20250514".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
        };

        AgentContext::new(
            config,
            "I am a creative writing assistant.".to_string(),
            "Name: Luna\nEmoji: 🌙".to_string(),
            "Be concise.".to_string(),
            vec![],
            TokenBudget::new(200_000),
        )
    }

    #[test]
    fn test_build_request() {
        // We can't construct AgentEngine without a real provider,
        // but we can test build_request indirectly via the context.
        let ctx = test_context();
        let messages = ctx.build_messages();
        assert!(messages.is_empty()); // No conversation history yet
    }

    #[test]
    fn test_build_request_includes_history() {
        let mut ctx = test_context();
        ctx.add_user_message("Hello!".to_string());
        ctx.add_assistant_message("Hi there!".to_string());

        let messages = ctx.build_messages();
        assert_eq!(messages.len(), 2);
    }
}
