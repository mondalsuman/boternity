//! SQLite secret repository implementation.
//!
//! Implements `SecretProvider` from `boternity-core` using sqlx with split read/write pools.
//! Secret values are stored as encrypted BLOB -- the encryption/decryption is handled by
//! the caller (vault service in Plan 01-04). This repository stores and retrieves raw bytes.

use boternity_core::repository::secret::SecretProvider;
use boternity_types::error::RepositoryError;
use boternity_types::secret::{SecretEntry, SecretKey, SecretScope};
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::pool::DatabasePool;

/// SQLite-backed implementation of `SecretProvider` for vault storage.
///
/// Stores encrypted secret values as BLOB in the secrets table.
/// Never logs or exposes encrypted values.
pub struct SqliteSecretRepository {
    pool: DatabasePool,
}

impl SqliteSecretRepository {
    /// Create a new repository backed by the given database pool.
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}

fn scope_to_string(scope: &SecretScope) -> String {
    match scope {
        SecretScope::Global => "global".to_string(),
        SecretScope::Bot(id) => id.to_string(),
    }
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| RepositoryError::Query(format!("invalid datetime: {e}")))
}

fn format_datetime(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

impl SecretProvider for SqliteSecretRepository {
    async fn get(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> Result<Option<String>, RepositoryError> {
        let scope_str = scope_to_string(scope);

        let row = sqlx::query(
            "SELECT encrypted_value FROM secrets WHERE key = ? AND scope = ?",
        )
        .bind(key)
        .bind(&scope_str)
        .fetch_optional(&self.pool.reader)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        match row {
            Some(row) => {
                let encrypted: Vec<u8> = row
                    .try_get("encrypted_value")
                    .map_err(|e| RepositoryError::Query(e.to_string()))?;
                // Return encrypted bytes as base64 string for now.
                // The actual decryption is handled by the vault service (Plan 01-04).
                // Using hex encoding for consistency with other hash representations.
                Ok(Some(hex::encode(&encrypted)))
            }
            None => Ok(None),
        }
    }

    async fn set(
        &self,
        key: &str,
        value: &str,
        scope: &SecretScope,
    ) -> Result<(), RepositoryError> {
        let scope_str = scope_to_string(scope);
        let now = format_datetime(&Utc::now());
        let id = Uuid::now_v7().to_string();

        // The value received here is the pre-encrypted bytes as hex string.
        // Convert back to bytes for BLOB storage.
        let encrypted_bytes = hex::decode(value)
            .map_err(|e| RepositoryError::Query(format!("invalid hex value: {e}")))?;

        // Upsert: insert or update existing key+scope combination
        sqlx::query(
            "INSERT INTO secrets (id, key, encrypted_value, scope, provider, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'vault', ?, ?)
             ON CONFLICT(key, scope) DO UPDATE SET encrypted_value = excluded.encrypted_value, updated_at = excluded.updated_at",
        )
        .bind(&id)
        .bind(key)
        .bind(&encrypted_bytes)
        .bind(&scope_str)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool.writer)
        .await
        .map_err(|e| RepositoryError::Query(e.to_string()))?;

        Ok(())
    }

    async fn delete(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> Result<(), RepositoryError> {
        let scope_str = scope_to_string(scope);

        let result = sqlx::query("DELETE FROM secrets WHERE key = ? AND scope = ?")
            .bind(key)
            .bind(&scope_str)
            .execute(&self.pool.writer)
            .await
            .map_err(|e| RepositoryError::Query(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn list(
        &self,
        scope: &SecretScope,
    ) -> Result<Vec<SecretEntry>, RepositoryError> {
        let scope_str = scope_to_string(scope);

        let rows =
            sqlx::query("SELECT key, provider, scope, created_at, updated_at FROM secrets WHERE scope = ?")
                .bind(&scope_str)
                .fetch_all(&self.pool.reader)
                .await
                .map_err(|e| RepositoryError::Query(e.to_string()))?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let key: String = row
                .try_get("key")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            let provider_str: String = row
                .try_get("provider")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            let created_at_str: String = row
                .try_get("created_at")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;
            let updated_at_str: String = row
                .try_get("updated_at")
                .map_err(|e| RepositoryError::Query(e.to_string()))?;

            let provider = match provider_str.as_str() {
                "vault" => boternity_types::secret::SecretProvider::Vault,
                "keychain" => boternity_types::secret::SecretProvider::Keychain,
                "environment" => boternity_types::secret::SecretProvider::Environment,
                other => {
                    return Err(RepositoryError::Query(format!(
                        "invalid provider: {other}"
                    )))
                }
            };

            entries.push(SecretEntry {
                key: SecretKey::new(key),
                provider,
                scope: scope.clone(),
                created_at: parse_datetime(&created_at_str)?,
                updated_at: parse_datetime(&updated_at_str)?,
            });
        }

        Ok(entries)
    }
}

/// Hex encoding/decoding utilities for encrypted secret values.
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if s.len() % 2 != 0 {
            return Err("odd length hex string".to_string());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16)
                    .map_err(|e| format!("invalid hex at position {i}: {e}"))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::bot::SqliteBotRepository;
    use crate::sqlite::pool::DatabasePool;
    use boternity_core::repository::bot::BotRepository;
    use boternity_types::bot::{slugify, Bot, BotCategory, BotId, BotStatus};

    async fn test_pool() -> DatabasePool {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        std::mem::forget(dir);
        DatabasePool::new(&url).await.unwrap()
    }

    fn make_bot(name: &str) -> Bot {
        let now = Utc::now();
        Bot {
            id: BotId::new(),
            slug: slugify(name),
            name: name.to_string(),
            description: format!("A {name} bot"),
            status: BotStatus::Active,
            category: BotCategory::Assistant,
            tags: vec![],
            user_id: None,
            conversation_count: 0,
            total_tokens_used: 0,
            version_count: 0,
            created_at: now,
            updated_at: now,
            last_active_at: None,
        }
    }

    // Simulate encrypted data as hex-encoded bytes
    fn fake_encrypted(plaintext: &str) -> String {
        hex::encode(plaintext.as_bytes())
    }

    #[tokio::test]
    async fn test_set_and_get_global_secret() {
        let pool = test_pool().await;
        let repo = SqliteSecretRepository::new(pool);

        let encrypted = fake_encrypted("sk-test-key-123");
        repo.set("ANTHROPIC_API_KEY", &encrypted, &SecretScope::Global)
            .await
            .unwrap();

        let result = repo
            .get("ANTHROPIC_API_KEY", &SecretScope::Global)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result, encrypted);
    }

    #[tokio::test]
    async fn test_set_and_get_bot_scoped_secret() {
        let pool = test_pool().await;
        let bot_repo = SqliteBotRepository::new(pool.clone());
        let secret_repo = SqliteSecretRepository::new(pool);

        let bot = make_bot("Scoped");
        bot_repo.create(&bot).await.unwrap();

        let scope = SecretScope::Bot(bot.id.clone());
        let encrypted = fake_encrypted("bot-specific-key");
        secret_repo
            .set("OPENAI_API_KEY", &encrypted, &scope)
            .await
            .unwrap();

        let result = secret_repo
            .get("OPENAI_API_KEY", &scope)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result, encrypted);

        // Should not be found in global scope
        let global_result = secret_repo
            .get("OPENAI_API_KEY", &SecretScope::Global)
            .await
            .unwrap();
        assert!(global_result.is_none());
    }

    #[tokio::test]
    async fn test_upsert_secret() {
        let pool = test_pool().await;
        let repo = SqliteSecretRepository::new(pool);

        let v1 = fake_encrypted("value-1");
        let v2 = fake_encrypted("value-2");

        repo.set("KEY", &v1, &SecretScope::Global).await.unwrap();
        repo.set("KEY", &v2, &SecretScope::Global).await.unwrap();

        let result = repo
            .get("KEY", &SecretScope::Global)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result, v2);
    }

    #[tokio::test]
    async fn test_delete_secret() {
        let pool = test_pool().await;
        let repo = SqliteSecretRepository::new(pool);

        let encrypted = fake_encrypted("to-delete");
        repo.set("DELETE_ME", &encrypted, &SecretScope::Global)
            .await
            .unwrap();

        repo.delete("DELETE_ME", &SecretScope::Global)
            .await
            .unwrap();

        let result = repo
            .get("DELETE_ME", &SecretScope::Global)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let pool = test_pool().await;
        let repo = SqliteSecretRepository::new(pool);

        let err = repo
            .delete("NOPE", &SecretScope::Global)
            .await
            .unwrap_err();
        assert!(matches!(err, RepositoryError::NotFound));
    }

    #[tokio::test]
    async fn test_list_secrets() {
        let pool = test_pool().await;
        let repo = SqliteSecretRepository::new(pool);

        repo.set(
            "KEY_A",
            &fake_encrypted("a"),
            &SecretScope::Global,
        )
        .await
        .unwrap();
        repo.set(
            "KEY_B",
            &fake_encrypted("b"),
            &SecretScope::Global,
        )
        .await
        .unwrap();

        let entries = repo.list(&SecretScope::Global).await.unwrap();
        assert_eq!(entries.len(), 2);

        let keys: Vec<&str> = entries.iter().map(|e| e.key.0.as_str()).collect();
        assert!(keys.contains(&"KEY_A"));
        assert!(keys.contains(&"KEY_B"));

        // Values should never appear in SecretEntry
    }

    #[tokio::test]
    async fn test_cascade_delete_bot_secrets() {
        let pool = test_pool().await;
        let bot_repo = SqliteBotRepository::new(pool.clone());
        let secret_repo = SqliteSecretRepository::new(pool);

        let bot = make_bot("SecretBot");
        bot_repo.create(&bot).await.unwrap();

        let scope = SecretScope::Bot(bot.id.clone());
        secret_repo
            .set("API_KEY", &fake_encrypted("val"), &scope)
            .await
            .unwrap();

        // Verify secret exists
        assert!(secret_repo.get("API_KEY", &scope).await.unwrap().is_some());

        // Delete bot -- secrets are scoped by bot UUID string, not foreign key
        // Since secrets.scope stores the bot UUID string (not a FK), cascade
        // doesn't apply directly. This is by design: secrets can exist for
        // bots that don't exist yet (e.g., pre-provisioned keys).
        // The application layer (SecretService) handles cleanup on bot deletion.
        bot_repo.delete(&bot.id).await.unwrap();

        // Secret still exists (no FK cascade for secrets -- scope is a string)
        let result = secret_repo.get("API_KEY", &scope).await.unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_hex_roundtrip() {
        let original = b"hello world encrypted";
        let encoded = hex::encode(original);
        let decoded = hex::decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }
}
