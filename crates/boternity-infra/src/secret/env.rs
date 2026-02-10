//! Environment variable secret provider.
//!
//! A read-only secret provider that checks environment variables.
//! This is the highest-priority provider in the resolution chain:
//! env vars override all other backends.
//!
//! Key resolution:
//! - Global scope: checks `key` directly (e.g., "ANTHROPIC_API_KEY")
//! - Bot scope: first checks `BOTERNITY_{SLUG}_{KEY}`, then falls back to `key` directly

use boternity_core::repository::secret::SecretProvider;
use boternity_types::error::RepositoryError;
use boternity_types::secret::{SecretEntry, SecretScope};

/// Environment variable secret provider.
///
/// Read-only: `set()` and `delete()` return `ProviderUnavailable`
/// because environment variables cannot be persistently modified.
pub struct EnvSecretProvider;

impl EnvSecretProvider {
    /// Create a new environment variable secret provider.
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnvSecretProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretProvider for EnvSecretProvider {
    async fn get(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> Result<Option<String>, RepositoryError> {
        // For bot-scoped secrets, also check BOTERNITY_{SLUG}_{KEY} format
        if let SecretScope::Bot(bot_id) = scope {
            let slug_key = format!(
                "BOTERNITY_{}_{}",
                bot_id.to_string().replace('-', "_").to_uppercase(),
                key
            );
            if let Ok(val) = std::env::var(&slug_key) {
                return Ok(Some(val));
            }
        }

        // Check the key directly as an env var name
        match std::env::var(key) {
            Ok(val) => Ok(Some(val)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => {
                // Env var exists but has invalid Unicode -- treat as not found
                // rather than erroring, since secrets must be valid strings
                Ok(None)
            }
        }
    }

    async fn set(
        &self,
        _key: &str,
        _value: &str,
        _scope: &SecretScope,
    ) -> Result<(), RepositoryError> {
        // Environment variables are read-only in the context of secret storage.
        // Users set them via shell config, not through our API.
        Err(RepositoryError::Query(
            "environment variable provider is read-only".to_string(),
        ))
    }

    async fn delete(
        &self,
        _key: &str,
        _scope: &SecretScope,
    ) -> Result<(), RepositoryError> {
        Err(RepositoryError::Query(
            "environment variable provider is read-only".to_string(),
        ))
    }

    async fn list(
        &self,
        _scope: &SecretScope,
    ) -> Result<Vec<SecretEntry>, RepositoryError> {
        // Cannot enumerate environment variables for a specific scope.
        // The vault (SQLite) maintains the key index.
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_env_provider_get_existing() {
        // Set an env var for testing
        // SAFETY: This test runs serially (single-threaded test) and we clean up after.
        unsafe { std::env::set_var("BOTERNITY_TEST_SECRET_1", "test-value-123") };

        let provider = EnvSecretProvider::new();
        let result = provider
            .get("BOTERNITY_TEST_SECRET_1", &SecretScope::Global)
            .await
            .unwrap();

        assert_eq!(result, Some("test-value-123".to_string()));

        // Cleanup
        // SAFETY: This test runs serially and the var was just set above.
        unsafe { std::env::remove_var("BOTERNITY_TEST_SECRET_1") };
    }

    #[tokio::test]
    async fn test_env_provider_get_missing() {
        let provider = EnvSecretProvider::new();
        let result = provider
            .get("NONEXISTENT_VAR_XYZ_123", &SecretScope::Global)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_env_provider_set_returns_error() {
        let provider = EnvSecretProvider::new();
        let result = provider
            .set("KEY", "value", &SecretScope::Global)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_env_provider_delete_returns_error() {
        let provider = EnvSecretProvider::new();
        let result = provider
            .delete("KEY", &SecretScope::Global)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_env_provider_list_returns_empty() {
        let provider = EnvSecretProvider::new();
        let result = provider.list(&SecretScope::Global).await.unwrap();

        assert!(result.is_empty());
    }
}
