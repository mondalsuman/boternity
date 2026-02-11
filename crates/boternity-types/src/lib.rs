//! Shared domain types for Boternity.
//!
//! This crate contains the core domain types used across the Boternity platform:
//! Bot, Soul, Identity, Secret, and their associated error types.
//!
//! Zero infrastructure dependencies -- only serde, uuid, chrono, thiserror.

pub mod agent;
pub mod bot;
pub mod chat;
pub mod error;
pub mod identity;
pub mod llm;
pub mod memory;
pub mod secret;
pub mod soul;
