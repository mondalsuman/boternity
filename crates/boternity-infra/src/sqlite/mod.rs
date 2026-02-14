//! SQLite storage layer.
//!
//! Repository implementations backed by SQLite with WAL mode and split
//! read/write connection pools.

pub mod audit;
pub mod bot;
pub mod chat;
pub mod file_metadata;
pub mod kv;
pub mod memory;
pub mod pool;
pub mod provider_health;
pub mod secret;
pub mod skill_audit;
pub mod soul;
