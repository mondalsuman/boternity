//! Helper constructors for `BotMessage` envelopes.
//!
//! Reduces boilerplate when building messages for direct or channel delivery.

use boternity_types::message::{BotMessage, MessageRecipient};
use chrono::Utc;
use uuid::Uuid;

/// Build a direct message from one bot to another.
pub fn direct(
    sender_bot_id: Uuid,
    sender_bot_name: impl Into<String>,
    recipient_bot_id: Uuid,
    message_type: impl Into<String>,
    body: serde_json::Value,
) -> BotMessage {
    BotMessage {
        id: Uuid::now_v7(),
        sender_bot_id,
        sender_bot_name: sender_bot_name.into(),
        recipient: MessageRecipient::Direct {
            bot_id: recipient_bot_id,
        },
        message_type: message_type.into(),
        body,
        timestamp: Utc::now(),
        reply_to: None,
    }
}

/// Build a channel broadcast message.
pub fn channel(
    sender_bot_id: Uuid,
    sender_bot_name: impl Into<String>,
    channel_name: impl Into<String>,
    message_type: impl Into<String>,
    body: serde_json::Value,
) -> BotMessage {
    BotMessage {
        id: Uuid::now_v7(),
        sender_bot_id,
        sender_bot_name: sender_bot_name.into(),
        recipient: MessageRecipient::Channel {
            name: channel_name.into(),
        },
        message_type: message_type.into(),
        body,
        timestamp: Utc::now(),
        reply_to: None,
    }
}

/// Build a reply message referencing the original.
pub fn reply(
    sender_bot_id: Uuid,
    sender_bot_name: impl Into<String>,
    original: &BotMessage,
    message_type: impl Into<String>,
    body: serde_json::Value,
) -> BotMessage {
    BotMessage {
        id: Uuid::now_v7(),
        sender_bot_id,
        sender_bot_name: sender_bot_name.into(),
        recipient: MessageRecipient::Direct {
            bot_id: original.sender_bot_id,
        },
        message_type: message_type.into(),
        body,
        timestamp: Utc::now(),
        reply_to: Some(original.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn direct_builds_correct_envelope() {
        let sender = Uuid::now_v7();
        let recipient = Uuid::now_v7();
        let msg = direct(sender, "bot-a", recipient, "question", json!({"q": "hi"}));

        assert_eq!(msg.sender_bot_id, sender);
        assert_eq!(msg.sender_bot_name, "bot-a");
        assert_eq!(msg.message_type, "question");
        assert!(matches!(
            msg.recipient,
            MessageRecipient::Direct { bot_id } if bot_id == recipient
        ));
        assert!(msg.reply_to.is_none());
    }

    #[test]
    fn channel_builds_correct_envelope() {
        let sender = Uuid::now_v7();
        let msg = channel(sender, "news-bot", "alerts", "broadcast", json!({"alert": true}));

        assert!(matches!(
            msg.recipient,
            MessageRecipient::Channel { ref name } if name == "alerts"
        ));
    }

    #[test]
    fn reply_references_original() {
        let sender_a = Uuid::now_v7();
        let sender_b = Uuid::now_v7();
        let original = direct(sender_a, "bot-a", sender_b, "question", json!({}));
        let resp = reply(sender_b, "bot-b", &original, "answer", json!({"a": 42}));

        assert_eq!(resp.reply_to, Some(original.id));
        assert!(matches!(
            resp.recipient,
            MessageRecipient::Direct { bot_id } if bot_id == sender_a
        ));
    }
}
