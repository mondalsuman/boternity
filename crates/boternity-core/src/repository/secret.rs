//! Secret provider trait definitions.
//!
//! Two traits are provided:
//! - `SecretProvider`: Uses RPITIT (`impl Future`) for zero-cost async in concrete types.
//! - `BoxSecretProvider`: Object-safe version using `Pin<Box<dyn Future>>` for dynamic dispatch
//!   in `SecretService` which chains multiple provider types.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use boternity_types::error::RepositoryError;
use boternity_types::secret::{SecretEntry, SecretScope};

/// Trait for secret storage backends (vault, keychain, environment).
///
/// Each provider stores and retrieves secret values. The SecretService
/// chains multiple providers in priority order.
///
/// This trait uses RPITIT for zero-cost async. For dynamic dispatch
/// (trait objects), see [`BoxSecretProvider`].
pub trait SecretProvider: Send + Sync {
    /// Retrieve a secret value by key and scope.
    /// Returns None if the secret does not exist in this provider.
    fn get(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> impl Future<Output = Result<Option<String>, RepositoryError>> + Send;

    /// Store a secret value.
    fn set(
        &self,
        key: &str,
        value: &str,
        scope: &SecretScope,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Delete a secret.
    fn delete(
        &self,
        key: &str,
        scope: &SecretScope,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// List all secret entries (metadata only, no values) for a given scope.
    fn list(
        &self,
        scope: &SecretScope,
    ) -> impl Future<Output = Result<Vec<SecretEntry>, RepositoryError>> + Send;
}

/// Object-safe version of [`SecretProvider`] for dynamic dispatch.
///
/// Used by `SecretService` to hold a `Vec<Arc<dyn BoxSecretProvider>>`
/// combining providers of different concrete types (env, keychain, vault).
///
/// A blanket implementation is provided for all types implementing `SecretProvider`.
pub trait BoxSecretProvider: Send + Sync {
    fn get_boxed<'a>(
        &'a self,
        key: &'a str,
        scope: &'a SecretScope,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>, RepositoryError>> + Send + 'a>>;

    fn set_boxed<'a>(
        &'a self,
        key: &'a str,
        value: &'a str,
        scope: &'a SecretScope,
    ) -> Pin<Box<dyn Future<Output = Result<(), RepositoryError>> + Send + 'a>>;

    fn delete_boxed<'a>(
        &'a self,
        key: &'a str,
        scope: &'a SecretScope,
    ) -> Pin<Box<dyn Future<Output = Result<(), RepositoryError>> + Send + 'a>>;

    fn list_boxed<'a>(
        &'a self,
        scope: &'a SecretScope,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SecretEntry>, RepositoryError>> + Send + 'a>>;
}

/// Blanket implementation: any `SecretProvider` automatically implements `BoxSecretProvider`.
impl<T: SecretProvider> BoxSecretProvider for T {
    fn get_boxed<'a>(
        &'a self,
        key: &'a str,
        scope: &'a SecretScope,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>, RepositoryError>> + Send + 'a>> {
        Box::pin(self.get(key, scope))
    }

    fn set_boxed<'a>(
        &'a self,
        key: &'a str,
        value: &'a str,
        scope: &'a SecretScope,
    ) -> Pin<Box<dyn Future<Output = Result<(), RepositoryError>> + Send + 'a>> {
        Box::pin(self.set(key, value, scope))
    }

    fn delete_boxed<'a>(
        &'a self,
        key: &'a str,
        scope: &'a SecretScope,
    ) -> Pin<Box<dyn Future<Output = Result<(), RepositoryError>> + Send + 'a>> {
        Box::pin(self.delete(key, scope))
    }

    fn list_boxed<'a>(
        &'a self,
        scope: &'a SecretScope,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SecretEntry>, RepositoryError>> + Send + 'a>> {
        Box::pin(self.list(scope))
    }
}

/// Type alias for a dynamically-dispatched secret provider.
pub type DynSecretProvider = Arc<dyn BoxSecretProvider>;
