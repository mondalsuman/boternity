//! Workflow engine core: definition parsing, DAG validation, and execution context.
//!
//! This module contains the "brain" of the workflow engine:
//! - `definition` -- YAML parsing, validation, filesystem load/save
//! - `dag` -- DAG builder, cycle detection, parallel wave computation
//! - `context` -- Execution context with step output tracking and template resolution

pub mod context;
pub mod dag;
pub mod definition;
pub mod expression;
pub mod retry;
