//! Workflow infrastructure: webhook handlers and filesystem watchers.
//!
//! This module provides concrete implementations for workflow trigger
//! subsystems:
//! - `webhook_handler` -- HMAC-SHA256/bearer token auth, webhook registry
//! - `file_trigger` -- Debounced filesystem watcher with glob filtering

pub mod file_trigger;
pub mod webhook_handler;
