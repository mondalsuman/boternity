//! Secret provider trait definition.

use boternity_types::error::RepositoryError;
use boternity_types::secret::{SecretEntry, SecretScope};

/// Trait for secret storage backends (vault, keychain, environment).
///
/// Each provider stores and retrieves secret values. The SecretService
/// (defined in a later plan) chains multiple providers in priority order.
pub trait SecretProvider: Send + Sync {
    /// Retrieve a secret value by key and scope.
    /// Returns None if the secret does not exist in this provider.
    fn get(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> impl std::future::Future<Output = Result<Option<String>, RepositoryError>> + Send;

    /// Store a secret value.
    fn set(
        &self,
        key: &str,
        value: &str,
        scope: &SecretScope,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// Delete a secret.
    fn delete(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send;

    /// List all secret entries (metadata only, no values) for a given scope.
    fn list(
        &self,
        scope: &SecretScope,
    ) -> impl std::future::Future<Output = Result<Vec<SecretEntry>, RepositoryError>> + Send;
}
