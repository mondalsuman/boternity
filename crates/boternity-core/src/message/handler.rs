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
///
/// The processing pipeline is:
/// 1. Skill intercept -- check if a registered skill handles the message type
/// 2. LLM fallback -- if no skill matched, pass to the bot's LLM for a response
/// 3. Return `None` -- if the message requires no reply (e.g., status updates)
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

/// Echo processor that replies with the same body (useful for testing).
#[cfg(test)]
pub(crate) struct EchoProcessor;

#[cfg(test)]
impl MessageProcessor for EchoProcessor {
    async fn process_message(&self, bot_id: Uuid, message: &BotMessage) -> Option<BotMessage> {
        use boternity_types::message::MessageRecipient;
        use chrono::Utc;

        Some(BotMessage {
            id: Uuid::now_v7(),
            sender_bot_id: bot_id,
            sender_bot_name: format!("echo-{}", &bot_id.to_string()[..8]),
            recipient: MessageRecipient::Direct {
                bot_id: message.sender_bot_id,
            },
            message_type: "echo".to_string(),
            body: message.body.clone(),
            timestamp: Utc::now(),
            reply_to: Some(message.id),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::message::MessageRecipient;
    use chrono::Utc;
    use serde_json::json;

    fn sample_message(sender: Uuid, recipient: Uuid) -> BotMessage {
        BotMessage {
            id: Uuid::now_v7(),
            sender_bot_id: sender,
            sender_bot_name: "test-sender".to_string(),
            recipient: MessageRecipient::Direct {
                bot_id: recipient,
            },
            message_type: "question".to_string(),
            body: json!({"text": "hello"}),
            timestamp: Utc::now(),
            reply_to: None,
        }
    }

    #[tokio::test]
    async fn default_processor_returns_none() {
        let processor = DefaultMessageProcessor;
        let bot = Uuid::now_v7();
        let msg = sample_message(Uuid::now_v7(), bot);
        let result = processor.process_message(bot, &msg).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn echo_processor_returns_reply() {
        let processor = EchoProcessor;
        let sender = Uuid::now_v7();
        let bot = Uuid::now_v7();
        let msg = sample_message(sender, bot);
        let msg_id = msg.id;

        let reply = processor.process_message(bot, &msg).await.unwrap();
        assert_eq!(reply.sender_bot_id, bot);
        assert_eq!(reply.message_type, "echo");
        assert_eq!(reply.reply_to, Some(msg_id));
        assert_eq!(reply.body, json!({"text": "hello"}));
        assert!(matches!(
            reply.recipient,
            MessageRecipient::Direct { bot_id } if bot_id == sender
        ));
    }

    #[tokio::test]
    async fn processor_trait_is_object_safe_via_impl() {
        // Verify the trait can be used with concrete types
        async fn process_with<P: MessageProcessor>(p: &P, bot_id: Uuid, msg: &BotMessage) -> Option<BotMessage> {
            p.process_message(bot_id, msg).await
        }

        let bot = Uuid::now_v7();
        let msg = sample_message(Uuid::now_v7(), bot);

        let default = DefaultMessageProcessor;
        assert!(process_with(&default, bot, &msg).await.is_none());

        let echo = EchoProcessor;
        assert!(process_with(&echo, bot, &msg).await.is_some());
    }
}
