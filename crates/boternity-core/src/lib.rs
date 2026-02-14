//! Business logic and repository trait definitions for Boternity.
//!
//! This crate defines the "ports" (repository traits) that the infrastructure
//! layer implements. It depends only on `boternity-types` -- never on
//! `boternity-infra` or any database/IO crate.

pub mod agent;
pub mod builder;
pub mod chat;
pub mod event;
pub mod llm;
pub mod memory;
pub mod repository;
pub mod service;
pub mod skill;
pub mod storage;
pub mod workflow;
