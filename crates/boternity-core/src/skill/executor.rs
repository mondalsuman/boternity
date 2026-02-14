//! Skill execution trait and result types.
//!
//! Defines the [`SkillExecutor`] trait that both local and WASM executors
//! implement. Each executor receives an installed skill, user input, and a
//! capability enforcer, then produces a [`SkillExecutionResult`] with the
//! output and resource-usage metrics.

use std::time::Duration;

use boternity_types::skill::InstalledSkill;

use crate::skill::permission::CapabilityEnforcer;

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

/// The outcome of executing a skill, including output and resource metrics.
#[derive(Debug, Clone)]
pub struct SkillExecutionResult {
    /// The skill's output text (stdout for local, return value for WASM).
    pub output: String,
    /// WASM fuel consumed during execution (None for local skills).
    pub fuel_consumed: Option<u64>,
    /// Peak memory usage in bytes (None for local skills).
    pub memory_peak_bytes: Option<usize>,
    /// Wall-clock duration of the execution.
    pub duration: Duration,
}

// ---------------------------------------------------------------------------
// Executor trait
// ---------------------------------------------------------------------------

/// Trait for executing skills in different runtimes.
///
/// Implementors include:
/// - `LocalSkillExecutor` (process spawning for local trust tier)
/// - `WasmSkillExecutor` (wasmtime sandbox for verified/untrusted tiers)
pub trait SkillExecutor: Send + Sync {
    /// Execute a skill with the given input.
    ///
    /// The enforcer validates that the skill has permission for any
    /// capabilities it attempts to use. Resource limits are applied
    /// according to the skill's trust tier.
    fn execute(
        &self,
        skill: &InstalledSkill,
        input: &str,
        enforcer: &CapabilityEnforcer,
    ) -> impl std::future::Future<Output = anyhow::Result<SkillExecutionResult>> + Send;
}
