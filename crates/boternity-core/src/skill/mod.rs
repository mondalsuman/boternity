//! Skill system business logic.
//!
//! SKILL.md manifest parsing, validation, permission enforcement, dependency
//! resolution, and inheritance composition. This module defines the "how" of
//! skill execution policy; the domain types live in `boternity-types::skill`.

pub mod inheritance;
pub mod manifest;
pub mod permission;
pub mod resolver;
