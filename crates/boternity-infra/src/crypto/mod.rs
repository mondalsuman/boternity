//! Cryptographic operations for Boternity.
//!
//! - `hash`: SHA-256 content hashing for SOUL.md integrity
//! - `vault`: AES-256-GCM encryption for secrets at rest

pub mod hash;
pub mod vault;
