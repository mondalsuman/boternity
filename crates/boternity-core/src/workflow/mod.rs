//! Workflow engine core: definition parsing, DAG execution, and durable checkpointing.
//!
//! This module contains the "brain" of the workflow engine:
//! - `definition` -- YAML parsing, validation, filesystem load/save
//! - `dag` -- DAG builder, cycle detection, parallel wave computation
//! - `context` -- Execution context with step output tracking and template resolution
//! - `expression` -- JEXL evaluator for conditions and filters
//! - `retry` -- Retry handler with simple and LLM self-correction strategies
//! - `checkpoint` -- Durable checkpoint manager for crash recovery
//! - `executor` -- Wave-based parallel DAG executor
//! - `step_runner` -- Step type dispatchers for all 8 step types

pub mod checkpoint;
pub mod context;
pub mod dag;
pub mod definition;
pub mod executor;
pub mod expression;
pub mod retry;
pub mod step_runner;
