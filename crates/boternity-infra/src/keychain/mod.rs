//! OS keychain adapter for secret storage.
//!
//! Uses the `keyring` crate to store/retrieve secrets via:
//! - macOS Keychain
//! - Linux Secret Service (GNOME Keyring, KDE Wallet)
//! - Windows Credential Manager
//!
//! Note: The keyring API does not support enumeration, so `list()` always
//! returns an empty vec. The vault (SQLite) maintains the key index.

use boternity_core::repository::secret::SecretProvider;
use boternity_types::error::RepositoryError;
use boternity_types::secret::{SecretEntry, SecretScope};

/// OS keychain secret provider using the `keyring` crate.
///
/// Stores secrets under a service name with optional bot-scoped prefixes.
/// - Global scope: key is used as-is (e.g., "ANTHROPIC_API_KEY")
/// - Bot scope: key is prefixed with "bot/{bot_id}/" (e.g., "bot/abc123/OPENAI_API_KEY")
pub struct KeychainProvider {
    service_name: String,
}

impl KeychainProvider {
    /// Create a new KeychainProvider with the default service name "boternity".
    pub fn new() -> Self {
        Self {
            service_name: "boternity".to_string(),
        }
    }

    /// Create a KeychainProvider with a custom service name (useful for testing).
    pub fn with_service(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    /// Build the keychain entry key based on scope.
    fn scoped_key(&self, key: &str, scope: &SecretScope) -> String {
        match scope {
            SecretScope::Global => key.to_string(),
            SecretScope::Bot(id) => format!("bot/{id}/{key}"),
        }
    }

    /// Create a keyring::Entry for the given key and scope.
    fn entry(&self, key: &str, scope: &SecretScope) -> Result<keyring::Entry, RepositoryError> {
        let scoped = self.scoped_key(key, scope);
        keyring::Entry::new(&self.service_name, &scoped)
            .map_err(|e| RepositoryError::Query(format!("keychain entry error: {e}")))
    }
}

impl Default for KeychainProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretProvider for KeychainProvider {
    async fn get(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> Result<Option<String>, RepositoryError> {
        let entry = self.entry(key, scope)?;

        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(RepositoryError::Query(format!("keychain get error: {e}"))),
        }
    }

    async fn set(
        &self,
        key: &str,
        value: &str,
        scope: &SecretScope,
    ) -> Result<(), RepositoryError> {
        let entry = self.entry(key, scope)?;

        entry
            .set_password(value)
            .map_err(|e| RepositoryError::Query(format!("keychain set error: {e}")))
    }

    async fn delete(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> Result<(), RepositoryError> {
        let entry = self.entry(key, scope)?;

        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Err(RepositoryError::NotFound),
            Err(e) => Err(RepositoryError::Query(format!(
                "keychain delete error: {e}"
            ))),
        }
    }

    async fn list(
        &self,
        _scope: &SecretScope,
    ) -> Result<Vec<SecretEntry>, RepositoryError> {
        // The keyring crate does not support listing/enumerating credentials.
        // The vault (SQLite) maintains the key index; keychain is a storage backend only.
        // TODO: If future keyring versions add enumeration, implement this.
        Ok(Vec::new())
    }
}
