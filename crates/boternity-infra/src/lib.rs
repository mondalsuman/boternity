//! Infrastructure layer for Boternity.
//!
//! Contains implementations of the repository traits defined in `boternity-core`:
//! SQLite storage, OS keychain integration, filesystem adapters, and cryptographic
//! operations (AES-256-GCM vault, SHA-256 hashing).

pub mod crypto;
pub mod filesystem;
pub mod keychain;
pub mod secret;
pub mod sqlite;
