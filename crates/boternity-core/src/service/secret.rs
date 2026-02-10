//! Secret management service.
//!
//! SecretService resolves secrets through a chain of providers in priority order.
//! Resolution precedence: env vars > per-bot keys > global vault.
//!
//! This service lives in `boternity-core` and depends only on `boternity-types`
//! and the `BoxSecretProvider` trait -- never on concrete infra implementations.

use crate::repository::secret::DynSecretProvider;
use boternity_types::error::RepositoryError;
use boternity_types::secret::{SecretEntry, SecretScope};

/// Service for managing secrets across multiple storage backends.
///
/// Providers are ordered by precedence (first match wins).
/// Default chain: `[EnvSecretProvider, KeychainProvider, VaultSecretProvider]`
///
/// For bot-scoped secrets, the service first tries providers with the bot scope,
/// then falls back to global scope (env vars > per-bot > global vault).
pub struct SecretService {
    providers: Vec<DynSecretProvider>,
}

impl SecretService {
    /// Create a new SecretService with the given provider chain.
    ///
    /// Providers should be ordered by precedence (highest priority first).
    pub fn new(providers: Vec<DynSecretProvider>) -> Self {
        Self { providers }
    }

    /// Resolve a secret value by iterating through providers in priority order.
    ///
    /// For `SecretScope::Bot`: first tries providers with bot scope, then falls
    /// back to global scope. This implements the precedence:
    /// env vars > per-bot keys > global vault.
    pub async fn get_secret(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> Result<Option<String>, RepositoryError> {
        // For bot scope: first try bot-scoped, then fall back to global
        if let SecretScope::Bot(_) = scope {
            // Try bot-scoped first across all providers
            for provider in &self.providers {
                if let Some(value) = provider.get_boxed(key, scope).await? {
                    return Ok(Some(value));
                }
            }

            // Fall back to global scope
            for provider in &self.providers {
                if let Some(value) = provider.get_boxed(key, &SecretScope::Global).await? {
                    return Ok(Some(value));
                }
            }

            Ok(None)
        } else {
            // Global scope: iterate providers, first match wins
            for provider in &self.providers {
                if let Some(value) = provider.get_boxed(key, scope).await? {
                    return Ok(Some(value));
                }
            }
            Ok(None)
        }
    }

    /// Store a secret value in the first writable provider.
    ///
    /// Iterates providers in order and writes to the first one that accepts
    /// the write. Read-only providers (e.g., env vars) will return an error,
    /// which is silently skipped.
    pub async fn set_secret(
        &self,
        key: &str,
        value: &str,
        scope: &SecretScope,
    ) -> Result<(), RepositoryError> {
        for provider in &self.providers {
            match provider.set_boxed(key, value, scope).await {
                Ok(()) => return Ok(()),
                Err(_) => continue, // Skip read-only providers
            }
        }

        Err(RepositoryError::Query(
            "no writable secret provider available".to_string(),
        ))
    }

    /// Delete a secret from all providers that have it.
    ///
    /// Errors from individual providers are ignored (they may not have the key).
    pub async fn delete_secret(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> Result<(), RepositoryError> {
        let mut deleted = false;

        for provider in &self.providers {
            match provider.delete_boxed(key, scope).await {
                Ok(()) => deleted = true,
                Err(RepositoryError::NotFound) => continue, // Not in this provider
                Err(_) => continue, // Provider unavailable
            }
        }

        if !deleted {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    /// List all secrets, aggregated from all providers and deduplicated.
    ///
    /// First provider wins for duplicate keys (preserves precedence).
    pub async fn list_secrets(
        &self,
        scope: &SecretScope,
    ) -> Result<Vec<SecretEntry>, RepositoryError> {
        let mut seen_keys = std::collections::HashSet::new();
        let mut entries = Vec::new();

        for provider in &self.providers {
            match provider.list_boxed(scope).await {
                Ok(provider_entries) => {
                    for entry in provider_entries {
                        if seen_keys.insert(entry.key.clone()) {
                            entries.push(entry);
                        }
                    }
                }
                Err(_) => continue, // Provider unavailable
            }
        }

        Ok(entries)
    }

    /// Mask a secret value, showing only the last 4 characters.
    ///
    /// - "sk-abcdefghijklmnop" -> "****mnop"
    /// - "abc" -> "****" (too short to show any chars)
    pub fn mask_secret(value: &str) -> String {
        if value.len() <= 4 {
            "****".to_string()
        } else {
            format!("****{}", &value[value.len() - 4..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::secret::SecretKey;
    use std::sync::Arc;

    // --- Mock providers for testing ---

    /// A mock provider that returns predefined values.
    struct MockProvider {
        name: &'static str,
        values: std::collections::HashMap<(String, String), String>,
        writable: bool,
    }

    impl MockProvider {
        fn new(name: &'static str, writable: bool) -> Self {
            Self {
                name,
                values: std::collections::HashMap::new(),
                writable,
            }
        }

        fn with_value(
            mut self,
            key: &str,
            scope: &SecretScope,
            value: &str,
        ) -> Self {
            self.values.insert(
                (key.to_string(), scope.to_string()),
                value.to_string(),
            );
            self
        }
    }

    impl crate::repository::secret::SecretProvider for MockProvider {
        async fn get(
            &self,
            key: &str,
            scope: &SecretScope,
        ) -> Result<Option<String>, RepositoryError> {
            Ok(self
                .values
                .get(&(key.to_string(), scope.to_string()))
                .cloned())
        }

        async fn set(
            &self,
            _key: &str,
            _value: &str,
            _scope: &SecretScope,
        ) -> Result<(), RepositoryError> {
            if self.writable {
                Ok(())
            } else {
                Err(RepositoryError::Query(format!(
                    "{} is read-only",
                    self.name
                )))
            }
        }

        async fn delete(
            &self,
            key: &str,
            scope: &SecretScope,
        ) -> Result<(), RepositoryError> {
            if self.values.contains_key(&(key.to_string(), scope.to_string())) {
                Ok(())
            } else {
                Err(RepositoryError::NotFound)
            }
        }

        async fn list(
            &self,
            scope: &SecretScope,
        ) -> Result<Vec<SecretEntry>, RepositoryError> {
            let entries: Vec<SecretEntry> = self
                .values
                .keys()
                .filter(|(_, s)| s == &scope.to_string())
                .map(|(k, _)| SecretEntry {
                    key: SecretKey::new(k.clone()),
                    provider: boternity_types::secret::SecretProvider::Vault,
                    scope: scope.clone(),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                })
                .collect();
            Ok(entries)
        }
    }

    // --- Tests ---

    #[tokio::test]
    async fn test_precedence_env_over_vault() {
        let env_provider = MockProvider::new("env", false)
            .with_value("API_KEY", &SecretScope::Global, "env-value");
        let vault_provider = MockProvider::new("vault", true)
            .with_value("API_KEY", &SecretScope::Global, "vault-value");

        let service = SecretService::new(vec![
            Arc::new(env_provider),
            Arc::new(vault_provider),
        ]);

        let result = service
            .get_secret("API_KEY", &SecretScope::Global)
            .await
            .unwrap();

        assert_eq!(result, Some("env-value".to_string()));
    }

    #[tokio::test]
    async fn test_fallback_to_vault_when_env_missing() {
        let env_provider = MockProvider::new("env", false); // No values
        let vault_provider = MockProvider::new("vault", true)
            .with_value("API_KEY", &SecretScope::Global, "vault-value");

        let service = SecretService::new(vec![
            Arc::new(env_provider),
            Arc::new(vault_provider),
        ]);

        let result = service
            .get_secret("API_KEY", &SecretScope::Global)
            .await
            .unwrap();

        assert_eq!(result, Some("vault-value".to_string()));
    }

    #[tokio::test]
    async fn test_bot_scope_falls_back_to_global() {
        let bot_id = boternity_types::bot::BotId::new();
        let bot_scope = SecretScope::Bot(bot_id);

        // Provider has only global key, not bot-scoped
        let vault_provider = MockProvider::new("vault", true)
            .with_value("API_KEY", &SecretScope::Global, "global-value");

        let service = SecretService::new(vec![Arc::new(vault_provider)]);

        let result = service
            .get_secret("API_KEY", &bot_scope)
            .await
            .unwrap();

        assert_eq!(result, Some("global-value".to_string()));
    }

    #[tokio::test]
    async fn test_bot_scope_prefers_bot_scoped_over_global() {
        let bot_id = boternity_types::bot::BotId::new();
        let bot_scope = SecretScope::Bot(bot_id);

        let vault_provider = MockProvider::new("vault", true)
            .with_value("API_KEY", &SecretScope::Global, "global-value")
            .with_value("API_KEY", &bot_scope, "bot-value");

        let service = SecretService::new(vec![Arc::new(vault_provider)]);

        let result = service
            .get_secret("API_KEY", &bot_scope)
            .await
            .unwrap();

        assert_eq!(result, Some("bot-value".to_string()));
    }

    #[tokio::test]
    async fn test_set_skips_readonly_provider() {
        let env_provider = MockProvider::new("env", false); // Read-only
        let vault_provider = MockProvider::new("vault", true); // Writable

        let service = SecretService::new(vec![
            Arc::new(env_provider),
            Arc::new(vault_provider),
        ]);

        // Should succeed by writing to vault (skipping env)
        let result = service
            .set_secret("NEW_KEY", "value", &SecretScope::Global)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_set_fails_when_no_writable_provider() {
        let env_provider = MockProvider::new("env", false);

        let service = SecretService::new(vec![Arc::new(env_provider)]);

        let result = service
            .set_secret("KEY", "value", &SecretScope::Global)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_from_provider() {
        let vault_provider = MockProvider::new("vault", true)
            .with_value("KEY", &SecretScope::Global, "val");

        let service = SecretService::new(vec![Arc::new(vault_provider)]);

        let result = service
            .delete_secret("KEY", &SecretScope::Global)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_returns_not_found() {
        let vault_provider = MockProvider::new("vault", true);

        let service = SecretService::new(vec![Arc::new(vault_provider)]);

        let result = service
            .delete_secret("NONEXISTENT", &SecretScope::Global)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_deduplicates() {
        let provider1 = MockProvider::new("p1", false)
            .with_value("KEY_A", &SecretScope::Global, "v1")
            .with_value("KEY_B", &SecretScope::Global, "v2");
        let provider2 = MockProvider::new("p2", true)
            .with_value("KEY_B", &SecretScope::Global, "v3") // Duplicate
            .with_value("KEY_C", &SecretScope::Global, "v4");

        let service = SecretService::new(vec![
            Arc::new(provider1),
            Arc::new(provider2),
        ]);

        let entries = service.list_secrets(&SecretScope::Global).await.unwrap();
        let keys: Vec<&str> = entries.iter().map(|e| e.key.0.as_str()).collect();

        // Should have 3 unique keys (KEY_B from provider1 wins)
        assert_eq!(entries.len(), 3);
        assert!(keys.contains(&"KEY_A"));
        assert!(keys.contains(&"KEY_B"));
        assert!(keys.contains(&"KEY_C"));
    }

    #[test]
    fn test_mask_secret_long() {
        assert_eq!(SecretService::mask_secret("sk-abcdefghijklmnop"), "****mnop");
    }

    #[test]
    fn test_mask_secret_short() {
        assert_eq!(SecretService::mask_secret("abc"), "****");
    }

    #[test]
    fn test_mask_secret_exactly_4() {
        assert_eq!(SecretService::mask_secret("abcd"), "****");
    }

    #[test]
    fn test_mask_secret_5_chars() {
        assert_eq!(SecretService::mask_secret("abcde"), "****bcde");
    }

    #[test]
    fn test_mask_secret_empty() {
        assert_eq!(SecretService::mask_secret(""), "****");
    }
}
