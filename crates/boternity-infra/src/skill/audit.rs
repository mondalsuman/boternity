//! Skill audit logging trait and utilities.
//!
//! Defines the interface for skill invocation auditing. The concrete
//! SQLite implementation lives in [`crate::sqlite::skill_audit`].

pub use crate::sqlite::skill_audit::SqliteSkillAuditLog;
