//! Business logic services (use cases).
//!
//! Services orchestrate repository calls, filesystem operations, and
//! business rules. They depend on traits (ports) -- never on concrete
//! infrastructure implementations.

pub mod bot;
pub mod fs;
pub mod hash;
pub mod soul;
