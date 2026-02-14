//! SQLite message repository implementation.
//!
//! Implements `MessageRepository` from `boternity-core` using sqlx with split
//! read/write pools. Persists bot-to-bot messages, channels, and subscriptions
//! for audit trail and restart recovery.

use boternity_core::repository::message::MessageRepository;
use boternity_types::error::RepositoryError;
use boternity_types::message::{BotMessage, BotSubscription, Channel, MessageRecipient};
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::pool::DatabasePool;

/// SQLite-backed implementation of `MessageRepository`.
pub struct SqliteMessageRepository {
    pool: DatabasePool,
}

impl SqliteMessageRepository {
    /// Create a new repository backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

// ---------------------------------------------------------------------------
// Internal row types
// ---------------------------------------------------------------------------

struct BotMessageRow {
    id: String,
    sender_bot_id: String,
    sender_bot_name: String,
    recipient_type: String,
    recipient_bot_id: Option<String>,
    recipient_channel: Option<String>,
    message_type: String,
    body: String,
    reply_to: Option<String>,
    timestamp: String,
}

impl BotMessageRow {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            sender_bot_id: row.try_get("sender_bot_id")?,
            sender_bot_name: row.try_get("sender_bot_name")?,
            recipient_type: row.try_get("recipient_type")?,
            recipient_bot_id: row.try_get("recipient_bot_id")?,
            recipient_channel: row.try_get("recipient_channel")?,
            message_type: row.try_get("message_type")?,
            body: row.try_get("body")?,
            reply_to: row.try_get("reply_to")?,
            timestamp: row.try_get("timestamp")?,
        })
    }

    fn into_message(self) -> Result<BotMessage, RepositoryError> {
        let id = parse_uuid(&self.id)?;
        let sender_bot_id = parse_uuid(&self.sender_bot_id)?;

        let recipient = match self.recipient_type.as_str() {
            "direct" => {
                let bot_id = self
                    .recipient_bot_id
                    .as_deref()
                    .ok_or_else(|| {
                        RepositoryError::Query("direct message missing recipient_bot_id".into())
                    })
                    .and_then(parse_uuid)?;
                MessageRecipient::Direct { bot_id }
            }
            "channel" => {
                let name = self.recipient_channel.ok_or_else(|| {
                    RepositoryError::Query("channel message missing recipient_channel".into())
                })?;
                MessageRecipient::Channel { name }
            }
            other => {
                return Err(RepositoryError::Query(format!(
                    "unknown recipient_type: {other}"
                )));
            }
        };

        let body: serde_json::Value = serde_json::from_str(&self.body)
            .map_err(|e| RepositoryError::Query(format!("invalid message body JSON: {e}")))?;

        let reply_to = self
            .reply_to
            .as_deref()
            .map(parse_uuid)
            .transpose()?;

        let timestamp = parse_datetime(&self.timestamp)?;

        Ok(BotMessage {
            id,
            sender_bot_id,
            sender_bot_name: self.sender_bot_name,
            recipient,
            message_type: self.message_type,
            body,
            timestamp,
            reply_to,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_uuid(s: &str) -> Result<Uuid, RepositoryError> {
    s.parse::<Uuid>()
        .map_err(|e| RepositoryError::Query(format!("invalid UUID: {e}")))
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| RepositoryError::Query(format!("invalid datetime: {e}")))
}

fn format_datetime(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

// ---------------------------------------------------------------------------
// MessageRepository impl
// ---------------------------------------------------------------------------

impl MessageRepository for SqliteMessageRepository {
    async fn save_message(&self, msg: &BotMessage) -> Result<(), RepositoryError> {
        let (recipient_type, recipient_bot_id, recipient_channel) = match &msg.recipient {
            MessageRecipient::Direct { bot_id } => ("direct", Some(bot_id.to_string()), None),
            MessageRecipient::Channel { name } => ("channel", None, Some(name.clone())),
        };

        let body_json = serde_json::to_string(&msg.body)
            .map_err(|e| RepositoryError::Query(format!("serialize body: {e}")))?;

        sqlx::query(
            r#"INSERT INTO bot_messages
               (id, sender_bot_id, sender_bot_name, recipient_type, recipient_bot_id,
                recipient_channel, message_type, body, reply_to, timestamp)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(msg.id.to_string())
        .bind(msg.sender_bot_id.to_string())
        .bind(&msg.sender_bot_name)
        .bind(recipient_type)
        .bind(&recipient_bot_id)
        .bind(&recipient_channel)
        .bind(&msg.message_type)
        .bind(&body_json)
        .bind(msg.reply_to.as_ref().map(|id| id.to_string()))
        .bind(format_datetime(&msg.timestamp))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn get_messages_between(
        &self,
        bot_a: &Uuid,
        bot_b: &Uuid,
        limit: u32,
    ) -> Result<Vec<BotMessage>, RepositoryError> {
        let rows = sqlx::query(
            r#"SELECT * FROM bot_messages
               WHERE recipient_type = 'direct'
                 AND ((sender_bot_id = ? AND recipient_bot_id = ?)
                   OR (sender_bot_id = ? AND recipient_bot_id = ?))
               ORDER BY timestamp DESC
               LIMIT ?"#,
        )
        .bind(bot_a.to_string())
        .bind(bot_b.to_string())
        .bind(bot_b.to_string())
        .bind(bot_a.to_string())
        .bind(limit as i64)
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut msgs = Vec::with_capacity(rows.len());
        for row in &rows {
            let r =
                BotMessageRow::from_row(row).map_err(|e| RepositoryError::Query(e.to_string()))?;
            msgs.push(r.into_message()?);
        }
        Ok(msgs)
    }

    async fn get_channel_messages(
        &self,
        channel: &str,
        limit: u32,
    ) -> Result<Vec<BotMessage>, RepositoryError> {
        let rows = sqlx::query(
            r#"SELECT * FROM bot_messages
               WHERE recipient_type = 'channel' AND recipient_channel = ?
               ORDER BY timestamp DESC
               LIMIT ?"#,
        )
        .bind(channel)
        .bind(limit as i64)
        .fetch_all(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut msgs = Vec::with_capacity(rows.len());
        for row in &rows {
            let r =
                BotMessageRow::from_row(row).map_err(|e| RepositoryError::Query(e.to_string()))?;
            msgs.push(r.into_message()?);
        }
        Ok(msgs)
    }

    async fn list_channels(&self) -> Result<Vec<Channel>, RepositoryError> {
        let rows = sqlx::query("SELECT * FROM bot_channels ORDER BY name ASC")
            .fetch_all(&self.pool.reader)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut channels = Vec::with_capacity(rows.len());
        for row in &rows {
            let name: String =
                row.try_get("name").map_err(|e| RepositoryError::Query(e.to_string()))?;
            let created_at: String = row
                .try_get("created_at")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            let created_by: String = row
                .try_get("created_by_bot_id")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;

            channels.push(Channel {
                name,
                created_at: parse_datetime(&created_at)?,
                created_by_bot_id: parse_uuid(&created_by)?,
            });
        }
        Ok(channels)
    }

    async fn create_channel(&self, channel: &Channel) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            "INSERT INTO bot_channels (name, created_at, created_by_bot_id) VALUES (?, ?, ?)",
        )
        .bind(&channel.name)
        .bind(format_datetime(&channel.created_at))
        .bind(channel.created_by_bot_id.to_string())
        .execute(&self.pool.writer)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db_err)) if db_err.message().contains("UNIQUE") => Err(
                RepositoryError::Conflict(format!("channel '{}' already exists", channel.name)),
            ),
            Err(e) => Err(RepositoryError::Query(e.to_string())),
        }
    }

    async fn subscribe(&self, sub: &BotSubscription) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT OR IGNORE INTO bot_subscriptions (bot_id, channel_name, subscribed_at) VALUES (?, ?, ?)",
        )
        .bind(sub.bot_id.to_string())
        .bind(&sub.channel_name)
        .bind(format_datetime(&sub.subscribed_at))
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn unsubscribe(
        &self,
        bot_id: &Uuid,
        channel_name: &str,
    ) -> Result<bool, RepositoryError> {
        let result =
            sqlx::query("DELETE FROM bot_subscriptions WHERE bot_id = ? AND channel_name = ?")
                .bind(bot_id.to_string())
                .bind(channel_name)
                .execute(&self.pool.writer)
                .await
                .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn get_subscriptions(
        &self,
        bot_id: &Uuid,
    ) -> Result<Vec<BotSubscription>, RepositoryError> {
        let rows =
            sqlx::query("SELECT * FROM bot_subscriptions WHERE bot_id = ? ORDER BY channel_name")
                .bind(bot_id.to_string())
                .fetch_all(&self.pool.reader)
                .await
                .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut subs = Vec::with_capacity(rows.len());
        for row in &rows {
            let bot_id_str: String =
                row.try_get("bot_id").map_err(|e| RepositoryError::Query(e.to_string()))?;
            let channel_name: String = row
                .try_get("channel_name")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            let subscribed_at: String = row
                .try_get("subscribed_at")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;

            subs.push(BotSubscription {
                bot_id: parse_uuid(&bot_id_str)?,
                channel_name,
                subscribed_at: parse_datetime(&subscribed_at)?,
            });
        }
        Ok(subs)
    }

    async fn get_channel_subscribers(
        &self,
        channel_name: &str,
    ) -> Result<Vec<Uuid>, RepositoryError> {
        let rows =
            sqlx::query("SELECT bot_id FROM bot_subscriptions WHERE channel_name = ?")
                .bind(channel_name)
                .fetch_all(&self.pool.reader)
                .await
                .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut ids = Vec::with_capacity(rows.len());
        for row in &rows {
            let bot_id: String =
                row.try_get("bot_id").map_err(|e| RepositoryError::Query(e.to_string()))?;
            ids.push(parse_uuid(&bot_id)?);
        }
        Ok(ids)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::pool::DatabasePool;
    use serde_json::json;

    async fn test_pool() -> DatabasePool {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        std::mem::forget(dir);
        DatabasePool::new(&url).await.unwrap()
    }

    fn make_direct_message(sender: Uuid, recipient: Uuid) -> BotMessage {
        BotMessage {
            id: Uuid::now_v7(),
            sender_bot_id: sender,
            sender_bot_name: "sender-bot".to_string(),
            recipient: MessageRecipient::Direct { bot_id: recipient },
            message_type: "question".to_string(),
            body: json!({"text": "Hello!"}),
            timestamp: Utc::now(),
            reply_to: None,
        }
    }

    fn make_channel_message(sender: Uuid, channel: &str) -> BotMessage {
        BotMessage {
            id: Uuid::now_v7(),
            sender_bot_id: sender,
            sender_bot_name: "broadcaster".to_string(),
            recipient: MessageRecipient::Channel {
                name: channel.to_string(),
            },
            message_type: "broadcast".to_string(),
            body: json!({"headline": "Breaking news"}),
            timestamp: Utc::now(),
            reply_to: None,
        }
    }

    // -- Messages --

    #[tokio::test]
    async fn test_save_and_get_direct_messages() {
        let pool = test_pool().await;
        let repo = SqliteMessageRepository::new(pool);

        let bot_a = Uuid::now_v7();
        let bot_b = Uuid::now_v7();

        let msg1 = make_direct_message(bot_a, bot_b);
        let msg2 = make_direct_message(bot_b, bot_a);

        repo.save_message(&msg1).await.unwrap();
        repo.save_message(&msg2).await.unwrap();

        let messages = repo.get_messages_between(&bot_a, &bot_b, 10).await.unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn test_save_and_get_channel_messages() {
        let pool = test_pool().await;
        let repo = SqliteMessageRepository::new(pool);

        let sender = Uuid::now_v7();
        let msg = make_channel_message(sender, "news-feed");

        repo.save_message(&msg).await.unwrap();

        let messages = repo.get_channel_messages("news-feed", 10).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender_bot_name, "broadcaster");
    }

    #[tokio::test]
    async fn test_channel_messages_isolation() {
        let pool = test_pool().await;
        let repo = SqliteMessageRepository::new(pool);

        let sender = Uuid::now_v7();
        repo.save_message(&make_channel_message(sender, "ch-a"))
            .await
            .unwrap();
        repo.save_message(&make_channel_message(sender, "ch-b"))
            .await
            .unwrap();

        let a_msgs = repo.get_channel_messages("ch-a", 10).await.unwrap();
        assert_eq!(a_msgs.len(), 1);

        let b_msgs = repo.get_channel_messages("ch-b", 10).await.unwrap();
        assert_eq!(b_msgs.len(), 1);
    }

    // -- Channels --

    #[tokio::test]
    async fn test_create_and_list_channels() {
        let pool = test_pool().await;
        let repo = SqliteMessageRepository::new(pool);

        let ch = Channel {
            name: "research-feed".to_string(),
            created_at: Utc::now(),
            created_by_bot_id: Uuid::now_v7(),
        };

        repo.create_channel(&ch).await.unwrap();

        let channels = repo.list_channels().await.unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "research-feed");
    }

    #[tokio::test]
    async fn test_create_channel_conflict() {
        let pool = test_pool().await;
        let repo = SqliteMessageRepository::new(pool);

        let ch = Channel {
            name: "dup".to_string(),
            created_at: Utc::now(),
            created_by_bot_id: Uuid::now_v7(),
        };

        repo.create_channel(&ch).await.unwrap();
        let err = repo.create_channel(&ch).await.unwrap_err();
        assert!(matches!(err, RepositoryError::Conflict(_)));
    }

    // -- Subscriptions --

    #[tokio::test]
    async fn test_subscribe_and_get_subscriptions() {
        let pool = test_pool().await;
        let repo = SqliteMessageRepository::new(pool);

        let ch = Channel {
            name: "news".to_string(),
            created_at: Utc::now(),
            created_by_bot_id: Uuid::now_v7(),
        };
        repo.create_channel(&ch).await.unwrap();

        let bot_id = Uuid::now_v7();
        let sub = BotSubscription {
            bot_id,
            channel_name: "news".to_string(),
            subscribed_at: Utc::now(),
        };

        repo.subscribe(&sub).await.unwrap();

        let subs = repo.get_subscriptions(&bot_id).await.unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].channel_name, "news");
    }

    #[tokio::test]
    async fn test_subscribe_idempotent() {
        let pool = test_pool().await;
        let repo = SqliteMessageRepository::new(pool);

        let ch = Channel {
            name: "updates".to_string(),
            created_at: Utc::now(),
            created_by_bot_id: Uuid::now_v7(),
        };
        repo.create_channel(&ch).await.unwrap();

        let sub = BotSubscription {
            bot_id: Uuid::now_v7(),
            channel_name: "updates".to_string(),
            subscribed_at: Utc::now(),
        };

        repo.subscribe(&sub).await.unwrap();
        // Second subscribe should not error
        repo.subscribe(&sub).await.unwrap();

        let subs = repo.get_subscriptions(&sub.bot_id).await.unwrap();
        assert_eq!(subs.len(), 1);
    }

    #[tokio::test]
    async fn test_unsubscribe() {
        let pool = test_pool().await;
        let repo = SqliteMessageRepository::new(pool);

        let ch = Channel {
            name: "alerts".to_string(),
            created_at: Utc::now(),
            created_by_bot_id: Uuid::now_v7(),
        };
        repo.create_channel(&ch).await.unwrap();

        let bot_id = Uuid::now_v7();
        let sub = BotSubscription {
            bot_id,
            channel_name: "alerts".to_string(),
            subscribed_at: Utc::now(),
        };
        repo.subscribe(&sub).await.unwrap();

        let removed = repo.unsubscribe(&bot_id, "alerts").await.unwrap();
        assert!(removed);

        let subs = repo.get_subscriptions(&bot_id).await.unwrap();
        assert!(subs.is_empty());

        // Unsubscribe again should return false
        let removed_again = repo.unsubscribe(&bot_id, "alerts").await.unwrap();
        assert!(!removed_again);
    }

    #[tokio::test]
    async fn test_get_channel_subscribers() {
        let pool = test_pool().await;
        let repo = SqliteMessageRepository::new(pool);

        let ch = Channel {
            name: "general".to_string(),
            created_at: Utc::now(),
            created_by_bot_id: Uuid::now_v7(),
        };
        repo.create_channel(&ch).await.unwrap();

        let bot1 = Uuid::now_v7();
        let bot2 = Uuid::now_v7();

        repo.subscribe(&BotSubscription {
            bot_id: bot1,
            channel_name: "general".to_string(),
            subscribed_at: Utc::now(),
        })
        .await
        .unwrap();

        repo.subscribe(&BotSubscription {
            bot_id: bot2,
            channel_name: "general".to_string(),
            subscribed_at: Utc::now(),
        })
        .await
        .unwrap();

        let subscribers = repo.get_channel_subscribers("general").await.unwrap();
        assert_eq!(subscribers.len(), 2);
        assert!(subscribers.contains(&bot1));
        assert!(subscribers.contains(&bot2));
    }
}
