//! Secret provider implementations.
//!
//! - `env`: Environment variable provider (read-only, highest priority)
//! - `chain`: Secret chain builder wiring all providers together
//! - `VaultSecretProvider`: Encrypts/decrypts secrets using AES-256-GCM vault + SQLite storage

pub mod chain;
pub mod env;

use boternity_core::repository::secret::SecretProvider;
use boternity_types::error::RepositoryError;
use boternity_types::secret::{SecretEntry, SecretScope};

use crate::crypto::vault::VaultCrypto;
use crate::sqlite::secret::SqliteSecretRepository;

/// Secret provider that encrypts values with AES-256-GCM before storing in SQLite.
///
/// This combines:
/// - `VaultCrypto` for AES-256-GCM encryption/decryption
/// - `SqliteSecretRepository` for persistent BLOB storage
///
/// Values are encrypted before storage and decrypted on retrieval.
/// The SQLite layer stores hex-encoded encrypted bytes.
pub struct VaultSecretProvider {
    repo: SqliteSecretRepository,
    crypto: VaultCrypto,
}

impl VaultSecretProvider {
    /// Create a new vault provider from a SQLite repository and VaultCrypto instance.
    pub fn new(repo: SqliteSecretRepository, crypto: VaultCrypto) -> Self {
        Self { repo, crypto }
    }
}

/// Hex-encode bytes to string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex-decode a string to bytes.
fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
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

impl SecretProvider for VaultSecretProvider {
    async fn get(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> Result<Option<String>, RepositoryError> {
        // Retrieve hex-encoded encrypted bytes from SQLite
        let hex_encrypted = match self.repo.get(key, scope).await? {
            Some(hex) => hex,
            None => return Ok(None),
        };

        // Decode hex to raw encrypted bytes
        let encrypted_bytes = hex_decode(&hex_encrypted)
            .map_err(|e| RepositoryError::Query(format!("corrupt vault data: {e}")))?;

        // Decrypt with AES-256-GCM
        let plaintext = self
            .crypto
            .decrypt(&encrypted_bytes)
            .map_err(|_| RepositoryError::Query("decryption failed".to_string()))?;

        // Convert decrypted bytes back to string
        String::from_utf8(plaintext)
            .map(|s| Some(s))
            .map_err(|_| RepositoryError::Query("decrypted value is not valid UTF-8".to_string()))
    }

    async fn set(
        &self,
        key: &str,
        value: &str,
        scope: &SecretScope,
    ) -> Result<(), RepositoryError> {
        // Encrypt the plaintext value
        let encrypted_bytes = self
            .crypto
            .encrypt(value.as_bytes())
            .map_err(|_| RepositoryError::Query("encryption failed".to_string()))?;

        // Hex-encode for storage in SQLite (hex transport layer)
        let hex_encrypted = hex_encode(&encrypted_bytes);

        // Store in SQLite
        self.repo.set(key, &hex_encrypted, scope).await
    }

    async fn delete(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> Result<(), RepositoryError> {
        self.repo.delete(key, scope).await
    }

    async fn list(
        &self,
        scope: &SecretScope,
    ) -> Result<Vec<SecretEntry>, RepositoryError> {
        self.repo.list(scope).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::pool::DatabasePool;

    async fn test_pool() -> DatabasePool {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        std::mem::forget(dir);
        DatabasePool::new(&url).await.unwrap()
    }

    fn test_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = i as u8;
        }
        key
    }

    #[tokio::test]
    async fn test_vault_provider_roundtrip() {
        let pool = test_pool().await;
        let repo = SqliteSecretRepository::new(pool);
        let crypto = VaultCrypto::new(&test_key());
        let provider = VaultSecretProvider::new(repo, crypto);

        // Store a secret
        provider
            .set("API_KEY", "sk-secret-value-123", &SecretScope::Global)
            .await
            .unwrap();

        // Retrieve and verify it's decrypted
        let result = provider
            .get("API_KEY", &SecretScope::Global)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result, "sk-secret-value-123");
    }

    #[tokio::test]
    async fn test_vault_provider_missing_key() {
        let pool = test_pool().await;
        let repo = SqliteSecretRepository::new(pool);
        let crypto = VaultCrypto::new(&test_key());
        let provider = VaultSecretProvider::new(repo, crypto);

        let result = provider
            .get("NONEXISTENT", &SecretScope::Global)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_vault_provider_overwrite() {
        let pool = test_pool().await;
        let repo = SqliteSecretRepository::new(pool);
        let crypto = VaultCrypto::new(&test_key());
        let provider = VaultSecretProvider::new(repo, crypto);

        provider
            .set("KEY", "value-1", &SecretScope::Global)
            .await
            .unwrap();
        provider
            .set("KEY", "value-2", &SecretScope::Global)
            .await
            .unwrap();

        let result = provider
            .get("KEY", &SecretScope::Global)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result, "value-2");
    }

    #[tokio::test]
    async fn test_vault_provider_delete() {
        let pool = test_pool().await;
        let repo = SqliteSecretRepository::new(pool);
        let crypto = VaultCrypto::new(&test_key());
        let provider = VaultSecretProvider::new(repo, crypto);

        provider
            .set("TO_DELETE", "val", &SecretScope::Global)
            .await
            .unwrap();
        provider
            .delete("TO_DELETE", &SecretScope::Global)
            .await
            .unwrap();

        let result = provider
            .get("TO_DELETE", &SecretScope::Global)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_vault_provider_scoped() {
        let pool = test_pool().await;
        let repo = SqliteSecretRepository::new(pool);
        let crypto = VaultCrypto::new(&test_key());
        let provider = VaultSecretProvider::new(repo, crypto);

        let bot_id = boternity_types::bot::BotId::new();
        let scope = SecretScope::Bot(bot_id);

        provider
            .set("BOT_KEY", "bot-secret", &scope)
            .await
            .unwrap();

        // Should find in bot scope
        let result = provider.get("BOT_KEY", &scope).await.unwrap();
        assert_eq!(result, Some("bot-secret".to_string()));

        // Should NOT find in global scope
        let global = provider
            .get("BOT_KEY", &SecretScope::Global)
            .await
            .unwrap();
        assert!(global.is_none());
    }
}
