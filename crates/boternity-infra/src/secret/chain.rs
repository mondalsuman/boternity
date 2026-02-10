//! Secret chain builder -- wires concrete providers in priority order.
//!
//! This module lives in `boternity-infra` because it assembles concrete
//! provider implementations. The resulting chain is passed to `SecretService`
//! in `boternity-core` via the `DynSecretProvider` abstraction.
//!
//! Default chain order: `[EnvSecretProvider, KeychainProvider, VaultSecretProvider]`

use std::sync::Arc;

use boternity_core::repository::secret::DynSecretProvider;

use crate::keychain::KeychainProvider;
use crate::secret::env::EnvSecretProvider;
use crate::secret::VaultSecretProvider;

/// Build the default secret resolution chain.
///
/// The chain is ordered by precedence (first match wins):
/// 1. Environment variables (if `include_env` is true)
/// 2. OS keychain (if `keychain` is Some)
/// 3. Encrypted vault (always included)
///
/// # Arguments
/// - `vault`: The encrypted vault provider (SQLite + AES-256-GCM)
/// - `keychain`: Optional OS keychain provider (may be unavailable on headless servers)
/// - `include_env`: Whether to include environment variable provider (usually true)
pub fn build_secret_chain(
    vault: VaultSecretProvider,
    keychain: Option<KeychainProvider>,
    include_env: bool,
) -> Vec<DynSecretProvider> {
    let mut chain: Vec<DynSecretProvider> = Vec::new();

    // 1. Environment variables (highest priority)
    if include_env {
        chain.push(Arc::new(EnvSecretProvider::new()));
    }

    // 2. OS keychain (if available)
    if let Some(kc) = keychain {
        chain.push(Arc::new(kc));
    }

    // 3. Encrypted vault (lowest priority, always available)
    chain.push(Arc::new(vault));

    chain
}
