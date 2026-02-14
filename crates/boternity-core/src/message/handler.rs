//! Message processor trait for pluggable message handling pipelines.
//!
//! Defines the `MessageProcessor` trait that bot message handlers implement.
//! The default implementation is a placeholder; actual LLM/skill wiring happens
//! in Plan 09 (inter-bot delegation).

use boternity_types::message::BotMessage;
use uuid::Uuid;

/// Trait for processing incoming bot-to-bot messages.
///
/// Implementations decide how a bot responds to an incoming message --
/// whether via skill intercept, LLM fallback, or some other pipeline.
pub trait MessageProcessor: Send + Sync {
    /// Process an incoming message for the given bot.
    ///
    /// Returns `Some(reply)` if the bot produces a response, or `None` if the
    /// message is consumed silently (e.g., a status update with no reply needed).
    fn process_message(
        &self,
        bot_id: Uuid,
        message: &BotMessage,
    ) -> impl std::future::Future<Output = Option<BotMessage>> + Send;
}

/// Default placeholder processor that always returns `None`.
///
/// Used until the full LLM/skill pipeline is wired in Plan 09.
pub struct DefaultMessageProcessor;

impl MessageProcessor for DefaultMessageProcessor {
    async fn process_message(&self, _bot_id: Uuid, _message: &BotMessage) -> Option<BotMessage> {
        None
    }
}
