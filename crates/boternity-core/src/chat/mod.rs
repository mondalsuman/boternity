//! Chat session and message persistence abstractions for Boternity.
//!
//! This module defines the `ChatRepository` trait that the infrastructure
//! layer implements for session, message, and context summary CRUD,
//! plus the `ChatService` for session lifecycle orchestration and
//! `SessionManager` for turn tracking.

pub mod repository;
pub mod service;
pub mod session;
