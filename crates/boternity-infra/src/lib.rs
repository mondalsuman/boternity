//! Infrastructure layer for Boternity.
//!
//! Contains implementations of the repository traits defined in `boternity-core`:
//! SQLite storage, OS keychain integration, filesystem adapters, and cryptographic
//! operations (AES-256-GCM vault, SHA-256 hashing).

pub mod config;
pub mod crypto;
pub mod filesystem;
pub mod keychain;
pub mod llm;
pub mod secret;
pub mod sqlite;
pub mod storage;
pub mod vector;
