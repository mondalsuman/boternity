//! Bot-to-bot messaging domain types for Boternity.
//!
//! Defines the `BotMessage` envelope for direct (1:1) and pub/sub (channel)
//! inter-bot communication, along with channel and subscription types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A message sent between bots.
///
/// The `BotMessage` envelope supports both direct messaging (1:1) and pub/sub
/// channels (one-to-many). The body is flexible JSON to accommodate any
/// message payload structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotMessage {
    /// UUIDv7 message ID.
    pub id: Uuid,
    /// ID of the sending bot.
    pub sender_bot_id: Uuid,
    /// Display name of the sending bot (denormalized).
    pub sender_bot_name: String,
    /// Where this message is going.
    pub recipient: MessageRecipient,
    /// User-defined type tag (e.g. "question", "delegation", "status_update").
    pub message_type: String,
    /// Flexible JSON body.
    pub body: serde_json::Value,
    /// When the message was created.
    pub timestamp: DateTime<Utc>,
    /// Optional reference to a previous message for conversation threading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Uuid>,
}

/// The recipient of a bot-to-bot message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageRecipient {
    /// Direct message to a specific bot.
    Direct { bot_id: Uuid },
    /// Publish to a named channel.
    Channel { name: String },
}

/// A pub/sub channel for bot-to-bot communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    /// Channel name (unique identifier).
    pub name: String,
    /// When the channel was created.
    pub created_at: DateTime<Utc>,
    /// Bot that created the channel.
    pub created_by_bot_id: Uuid,
}

/// A bot's subscription to a pub/sub channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSubscription {
    /// Subscribing bot ID.
    pub bot_id: Uuid,
    /// Name of the channel subscribed to.
    pub channel_name: String,
    /// When the subscription was created.
    pub subscribed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_bot_message_direct_json_roundtrip() {
        let msg = BotMessage {
            id: Uuid::now_v7(),
            sender_bot_id: Uuid::now_v7(),
            sender_bot_name: "researcher".to_string(),
            recipient: MessageRecipient::Direct {
                bot_id: Uuid::now_v7(),
            },
            message_type: "question".to_string(),
            body: json!({"text": "What is the latest on AI?", "urgency": "high"}),
            timestamp: Utc::now(),
            reply_to: None,
        };
        let json_str = serde_json::to_string(&msg).unwrap();

        // Verify structure (compact JSON)
        assert!(json_str.contains("\"type\":\"direct\""));
        assert!(json_str.contains("\"message_type\":\"question\""));
        assert!(json_str.contains("\"urgency\":\"high\""));
        // reply_to should be omitted when None
        assert!(!json_str.contains("reply_to"));

        // Roundtrip
        let parsed: BotMessage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.sender_bot_name, "researcher");
        assert_eq!(parsed.message_type, "question");
        assert!(matches!(parsed.recipient, MessageRecipient::Direct { .. }));
    }

    #[test]
    fn test_bot_message_channel_json_roundtrip() {
        let msg = BotMessage {
            id: Uuid::now_v7(),
            sender_bot_id: Uuid::now_v7(),
            sender_bot_name: "news-bot".to_string(),
            recipient: MessageRecipient::Channel {
                name: "news-feed".to_string(),
            },
            message_type: "broadcast".to_string(),
            body: json!({"headline": "New discovery"}),
            timestamp: Utc::now(),
            reply_to: Some(Uuid::now_v7()),
        };
        let json_str = serde_json::to_string(&msg).unwrap();

        assert!(json_str.contains("\"type\":\"channel\""));
        assert!(json_str.contains("\"name\":\"news-feed\""));
        assert!(json_str.contains("reply_to"));

        let parsed: BotMessage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.sender_bot_name, "news-bot");
        assert!(matches!(
            parsed.recipient,
            MessageRecipient::Channel { ref name } if name == "news-feed"
        ));
        assert!(parsed.reply_to.is_some());
    }

    #[test]
    fn test_message_recipient_direct_serde() {
        let recipient = MessageRecipient::Direct {
            bot_id: Uuid::now_v7(),
        };
        let json = serde_json::to_string(&recipient).unwrap();
        assert!(json.contains("\"type\":\"direct\""));
        let parsed: MessageRecipient = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, MessageRecipient::Direct { .. }));
    }

    #[test]
    fn test_message_recipient_channel_serde() {
        let recipient = MessageRecipient::Channel {
            name: "alerts".to_string(),
        };
        let json = serde_json::to_string(&recipient).unwrap();
        assert!(json.contains("\"type\":\"channel\""));
        let parsed: MessageRecipient = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, MessageRecipient::Channel { .. }));
    }

    #[test]
    fn test_channel_serde() {
        let ch = Channel {
            name: "research-feed".to_string(),
            created_at: Utc::now(),
            created_by_bot_id: Uuid::now_v7(),
        };
        let json_str = serde_json::to_string(&ch).unwrap();
        let parsed: Channel = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.name, "research-feed");
    }

    #[test]
    fn test_bot_subscription_serde() {
        let sub = BotSubscription {
            bot_id: Uuid::now_v7(),
            channel_name: "news-feed".to_string(),
            subscribed_at: Utc::now(),
        };
        let json_str = serde_json::to_string(&sub).unwrap();
        let parsed: BotSubscription = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.channel_name, "news-feed");
    }

    #[test]
    fn test_bot_message_delegation_pattern() {
        // Verify the delegation use case from CONTEXT.md
        let msg = BotMessage {
            id: Uuid::now_v7(),
            sender_bot_id: Uuid::now_v7(),
            sender_bot_name: "manager".to_string(),
            recipient: MessageRecipient::Direct {
                bot_id: Uuid::now_v7(),
            },
            message_type: "delegation".to_string(),
            body: json!({
                "task": "Research quantum computing advances",
                "deadline": "2026-03-01",
                "priority": "medium"
            }),
            timestamp: Utc::now(),
            reply_to: None,
        };
        let json_str = serde_json::to_string(&msg).unwrap();
        let parsed: BotMessage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.message_type, "delegation");
        assert_eq!(parsed.body["task"], "Research quantum computing advances");
    }
}
