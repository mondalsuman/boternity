//! Bot-to-bot message bus with direct mailboxes, pub/sub channels, and send-and-wait.
//!
//! The `MessageBus` is the runtime hub for inter-bot communication. Each registered
//! bot gets a bounded `mpsc` mailbox for direct messages. Pub/sub channels use
//! `broadcast` for one-to-many delivery. The `send_and_wait` method supports
//! synchronous request/response patterns with a configurable timeout.

use std::sync::Arc;
use std::time::Duration;

use boternity_types::message::{BotMessage, MessageRecipient};
use dashmap::DashMap;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, warn};
use uuid::Uuid;

use super::router::LoopGuard;

/// Buffer size for per-bot direct message mailboxes (mpsc).
const DIRECT_BUFFER: usize = 256;

/// Buffer size for pub/sub broadcast channels.
const BROADCAST_BUFFER: usize = 1024;

/// Errors that can occur during message bus operations.
#[derive(Debug, Error)]
pub enum MessageError {
    /// The target bot is not registered with the bus.
    #[error("bot {0} is not registered")]
    NotRegistered(Uuid),

    /// A send-and-wait call timed out before receiving a reply.
    #[error("send_and_wait timed out after {0:?}")]
    Timeout(Duration),

    /// The direct message mailbox is full.
    #[error("mailbox full for bot {0}")]
    ChannelFull(Uuid),

    /// Loop guard rejected the message (depth or rate limit exceeded).
    #[error("loop detected: {0}")]
    LoopDetected(String),

    /// Internal send failure (channel closed, etc.).
    #[error("send failed: {0}")]
    SendFailed(String),
}

/// Central message bus for bot-to-bot communication.
///
/// Provides three delivery modes:
/// - **Direct:** One-to-one via per-bot `mpsc` mailboxes.
/// - **Pub/sub:** One-to-many via named `broadcast` channels.
/// - **Send-and-wait:** Synchronous request/response with timeout via `oneshot`.
pub struct MessageBus {
    /// Per-bot direct message senders (bot_id -> mpsc sender).
    direct_senders: DashMap<Uuid, mpsc::Sender<BotMessage>>,
    /// Per-channel broadcast senders (channel_name -> broadcast sender).
    channel_senders: DashMap<String, broadcast::Sender<BotMessage>>,
    /// Pending reply channels for send_and_wait (message_id -> oneshot sender).
    reply_channels: DashMap<Uuid, oneshot::Sender<BotMessage>>,
    /// Loop guard for depth and rate limiting.
    loop_guard: Arc<LoopGuard>,
}

impl MessageBus {
    /// Create a new message bus with the given loop guard.
    pub fn new(loop_guard: Arc<LoopGuard>) -> Self {
        Self {
            direct_senders: DashMap::new(),
            channel_senders: DashMap::new(),
            reply_channels: DashMap::new(),
            loop_guard,
        }
    }

    /// Register a bot and return its message receiver.
    ///
    /// The returned `mpsc::Receiver` is the bot's mailbox for direct messages.
    /// If the bot is already registered, the old mailbox is replaced.
    pub fn register_bot(&self, bot_id: Uuid) -> mpsc::Receiver<BotMessage> {
        let (tx, rx) = mpsc::channel(DIRECT_BUFFER);
        self.direct_senders.insert(bot_id, tx);
        debug!(%bot_id, "registered bot with message bus");
        rx
    }

    /// Unregister a bot, dropping its mailbox sender.
    ///
    /// Returns `true` if the bot was registered.
    pub fn unregister_bot(&self, bot_id: &Uuid) -> bool {
        let removed = self.direct_senders.remove(bot_id).is_some();
        if removed {
            debug!(%bot_id, "unregistered bot from message bus");
        }
        removed
    }

    /// Check if a bot is currently registered.
    pub fn is_registered(&self, bot_id: &Uuid) -> bool {
        self.direct_senders.contains_key(bot_id)
    }

    /// Send a direct message (fire-and-forget).
    ///
    /// The message is delivered to the recipient's mailbox. Returns an error
    /// if the recipient is not registered, the mailbox is full, or the loop
    /// guard rejects the message.
    pub async fn send(&self, msg: BotMessage) -> Result<(), MessageError> {
        // Extract recipient bot_id for direct messages
        let recipient_id = match &msg.recipient {
            MessageRecipient::Direct { bot_id } => *bot_id,
            MessageRecipient::Channel { .. } => {
                return Err(MessageError::SendFailed(
                    "use publish() for channel messages".to_string(),
                ));
            }
        };

        // Check loop guard
        self.loop_guard
            .check(msg.sender_bot_id, recipient_id)
            .map_err(|e| MessageError::LoopDetected(e))?;

        // Get sender for recipient
        let sender = self
            .direct_senders
            .get(&recipient_id)
            .ok_or(MessageError::NotRegistered(recipient_id))?;

        sender
            .try_send(msg)
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => MessageError::ChannelFull(recipient_id),
                mpsc::error::TrySendError::Closed(_) => MessageError::SendFailed(format!(
                    "mailbox closed for bot {recipient_id}"
                )),
            })?;

        Ok(())
    }

    /// Send a direct message and wait for a reply (with timeout).
    ///
    /// The caller blocks until the recipient calls `reply()` with the original
    /// message ID, or the timeout expires. This enables synchronous
    /// request/response patterns between bots.
    pub async fn send_and_wait(
        &self,
        msg: BotMessage,
        timeout: Duration,
    ) -> Result<BotMessage, MessageError> {
        let msg_id = msg.id;

        // Install reply channel before sending
        let (reply_tx, reply_rx) = oneshot::channel();
        self.reply_channels.insert(msg_id, reply_tx);

        // Send the message
        if let Err(e) = self.send(msg).await {
            // Clean up reply channel on send failure
            self.reply_channels.remove(&msg_id);
            return Err(e);
        }

        // Wait for reply with timeout
        match tokio::time::timeout(timeout, reply_rx).await {
            Ok(Ok(reply)) => Ok(reply),
            Ok(Err(_)) => {
                // oneshot sender dropped without sending
                Err(MessageError::SendFailed(
                    "reply channel closed without response".to_string(),
                ))
            }
            Err(_) => {
                // Timeout -- clean up pending reply channel
                self.reply_channels.remove(&msg_id);
                Err(MessageError::Timeout(timeout))
            }
        }
    }

    /// Send a reply to a pending `send_and_wait` call.
    ///
    /// The `original_msg_id` must match the ID of a message sent via
    /// `send_and_wait`. If no pending reply channel exists (e.g., the caller
    /// already timed out), the reply is silently dropped.
    pub fn reply(&self, original_msg_id: Uuid, reply_msg: BotMessage) -> bool {
        if let Some((_, tx)) = self.reply_channels.remove(&original_msg_id) {
            match tx.send(reply_msg) {
                Ok(()) => true,
                Err(_) => {
                    warn!(%original_msg_id, "reply channel already closed");
                    false
                }
            }
        } else {
            debug!(%original_msg_id, "no pending reply channel (caller may have timed out)");
            false
        }
    }

    /// Subscribe a bot to a named pub/sub channel.
    ///
    /// Creates the channel if it does not exist. Returns a broadcast receiver
    /// for consuming channel messages.
    pub fn subscribe(&self, channel_name: &str) -> broadcast::Receiver<BotMessage> {
        let entry = self
            .channel_senders
            .entry(channel_name.to_string())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(BROADCAST_BUFFER);
                tx
            });
        entry.subscribe()
    }

    /// Publish a message to a named pub/sub channel.
    ///
    /// Delivers to all current subscribers. If the channel does not exist or
    /// has no subscribers, the message is silently dropped.
    pub fn publish(&self, msg: BotMessage) -> Result<usize, MessageError> {
        let channel_name = match &msg.recipient {
            MessageRecipient::Channel { name } => name.clone(),
            MessageRecipient::Direct { .. } => {
                return Err(MessageError::SendFailed(
                    "use send() for direct messages".to_string(),
                ));
            }
        };

        if let Some(sender) = self.channel_senders.get(&channel_name) {
            match sender.send(msg) {
                Ok(count) => {
                    debug!(%channel_name, count, "published message to channel");
                    Ok(count)
                }
                Err(_) => {
                    // No active subscribers
                    debug!(%channel_name, "no active subscribers on channel");
                    Ok(0)
                }
            }
        } else {
            debug!(%channel_name, "channel does not exist, message dropped");
            Ok(0)
        }
    }

    /// Get the number of registered bots.
    pub fn registered_bot_count(&self) -> usize {
        self.direct_senders.len()
    }

    /// Get access to the loop guard.
    pub fn loop_guard(&self) -> &LoopGuard {
        &self.loop_guard
    }
}

impl std::fmt::Debug for MessageBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageBus")
            .field("registered_bots", &self.direct_senders.len())
            .field("channels", &self.channel_senders.len())
            .field("pending_replies", &self.reply_channels.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::envelope;
    use serde_json::json;

    fn make_bus() -> MessageBus {
        MessageBus::new(Arc::new(LoopGuard::default()))
    }

    #[tokio::test]
    async fn direct_send_receive() {
        let bus = make_bus();
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        bus.register_bot(bot_a);
        let mut rx_b = bus.register_bot(bot_b);

        let msg = envelope::direct(bot_a, "bot-a", bot_b, "hello", json!({"text": "hi"}));
        bus.send(msg.clone()).await.unwrap();

        let received = rx_b.recv().await.unwrap();
        assert_eq!(received.sender_bot_id, bot_a);
        assert_eq!(received.message_type, "hello");
    }

    #[tokio::test]
    async fn send_and_wait_with_reply() {
        let bus = Arc::new(make_bus());
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        bus.register_bot(bot_a);
        let mut rx_b = bus.register_bot(bot_b);

        let msg = envelope::direct(bot_a, "bot-a", bot_b, "question", json!({"q": "what?"}));
        let msg_id = msg.id;

        let bus_clone = Arc::clone(&bus);
        let reply_handle = tokio::spawn(async move {
            // Bot B receives and replies
            let received = rx_b.recv().await.unwrap();
            let reply_msg =
                envelope::reply(bot_b, "bot-b", &received, "answer", json!({"a": "this!"}));
            bus_clone.reply(msg_id, reply_msg);
        });

        let reply = bus
            .send_and_wait(msg, Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(reply.message_type, "answer");
        assert_eq!(reply.reply_to, Some(msg_id));

        reply_handle.await.unwrap();
    }

    #[tokio::test]
    async fn send_and_wait_timeout() {
        let bus = make_bus();
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        bus.register_bot(bot_a);
        let _rx_b = bus.register_bot(bot_b);

        let msg = envelope::direct(bot_a, "bot-a", bot_b, "question", json!({}));
        let result = bus.send_and_wait(msg, Duration::from_millis(50)).await;

        assert!(matches!(result, Err(MessageError::Timeout(_))));
    }

    #[tokio::test]
    async fn channel_publish_to_subscribers() {
        let bus = make_bus();
        let bot_a = Uuid::now_v7();

        let mut rx1 = bus.subscribe("news");
        let mut rx2 = bus.subscribe("news");

        let msg = envelope::channel(bot_a, "bot-a", "news", "broadcast", json!({"headline": "test"}));
        let count = bus.publish(msg).unwrap();
        assert_eq!(count, 2);

        let m1 = rx1.recv().await.unwrap();
        let m2 = rx2.recv().await.unwrap();
        assert_eq!(m1.message_type, "broadcast");
        assert_eq!(m2.message_type, "broadcast");
    }

    #[tokio::test]
    async fn publish_no_subscribers_returns_zero() {
        let bus = make_bus();
        let bot_a = Uuid::now_v7();
        let msg = envelope::channel(bot_a, "bot-a", "empty-channel", "test", json!({}));
        let count = bus.publish(msg).unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn send_to_unregistered_bot_errors() {
        let bus = make_bus();
        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        bus.register_bot(bot_a);
        // bot_b is NOT registered

        let msg = envelope::direct(bot_a, "bot-a", bot_b, "hello", json!({}));
        let result = bus.send(msg).await;

        assert!(matches!(result, Err(MessageError::NotRegistered(_))));
    }

    #[tokio::test]
    async fn unregister_bot_removes_mailbox() {
        let bus = make_bus();
        let bot_a = Uuid::now_v7();

        bus.register_bot(bot_a);
        assert!(bus.is_registered(&bot_a));

        let removed = bus.unregister_bot(&bot_a);
        assert!(removed);
        assert!(!bus.is_registered(&bot_a));
    }

    #[tokio::test]
    async fn publish_direct_message_errors() {
        let bus = make_bus();
        let msg = envelope::direct(Uuid::now_v7(), "a", Uuid::now_v7(), "t", json!({}));
        let result = bus.publish(msg);
        assert!(matches!(result, Err(MessageError::SendFailed(_))));
    }

    #[tokio::test]
    async fn send_channel_message_errors() {
        let bus = make_bus();
        let msg = envelope::channel(Uuid::now_v7(), "a", "ch", "t", json!({}));
        let result = bus.send(msg).await;
        assert!(matches!(result, Err(MessageError::SendFailed(_))));
    }

    #[test]
    fn debug_impl() {
        let bus = make_bus();
        let debug = format!("{bus:?}");
        assert!(debug.contains("MessageBus"));
        assert!(debug.contains("registered_bots"));
    }
}
