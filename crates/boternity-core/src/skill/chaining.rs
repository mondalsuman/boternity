//! Skill chaining: pipe output of one skill as input to the next.
//!
//! Chaining allows sequential skill composition where each skill receives
//! the previous skill's output as its input. Resource metrics (fuel, timing)
//! are accumulated across the chain for accurate audit reporting.

use std::time::Duration;

use anyhow::Context;
use boternity_types::skill::InstalledSkill;

use crate::skill::executor::SkillExecutionResult;
use crate::skill::permission::CapabilityEnforcer;

use super::executor::SkillExecutor;

/// Execute a chain of skills sequentially, piping each output to the next.
///
/// The `initial_input` is passed to the first skill. Each subsequent skill
/// receives the output of the previous one. The final result includes the
/// last skill's output with accumulated fuel and timing from all skills.
///
/// # Errors
///
/// Returns an error with context identifying the failing skill position
/// if any skill in the chain fails.
pub async fn chain_skills<E: SkillExecutor>(
    executor: &E,
    skills: &[&InstalledSkill],
    initial_input: &str,
    enforcer: &CapabilityEnforcer,
) -> anyhow::Result<SkillExecutionResult> {
    anyhow::ensure!(
        !skills.is_empty(),
        "skill chain must contain at least one skill"
    );

    let mut current_input = initial_input.to_string();
    let mut total_fuel: u64 = 0;
    let mut total_duration = Duration::ZERO;
    let mut peak_memory: Option<usize> = None;

    for (i, skill) in skills.iter().enumerate() {
        let result = executor
            .execute(skill, &current_input, enforcer)
            .await
            .with_context(|| {
                format!(
                    "skill chain failed at position {} (skill '{}')",
                    i, skill.manifest.name
                )
            })?;

        // Accumulate fuel
        if let Some(fuel) = result.fuel_consumed {
            total_fuel += fuel;
        }

        // Track peak memory (take the maximum across the chain)
        match (peak_memory, result.memory_peak_bytes) {
            (Some(prev), Some(curr)) => peak_memory = Some(prev.max(curr)),
            (None, Some(curr)) => peak_memory = Some(curr),
            _ => {}
        }

        // Accumulate duration
        total_duration += result.duration;

        // Pipe output to next skill input
        current_input = result.output;
    }

    Ok(SkillExecutionResult {
        output: current_input,
        fuel_consumed: if total_fuel > 0 {
            Some(total_fuel)
        } else {
            None
        },
        memory_peak_bytes: peak_memory,
        duration: total_duration,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::executor::SkillExecutor;
    use crate::skill::permission::CapabilityEnforcer;
    use boternity_types::skill::{
        Capability, InstalledSkill, PermissionGrant, SkillManifest, SkillSource,
    };
    use chrono::Utc;
    use std::path::PathBuf;
    use std::time::Duration;

    /// A mock executor that appends the skill name to the input.
    struct MockChainExecutor;

    impl SkillExecutor for MockChainExecutor {
        async fn execute(
            &self,
            skill: &InstalledSkill,
            input: &str,
            _enforcer: &CapabilityEnforcer,
        ) -> anyhow::Result<SkillExecutionResult> {
            Ok(SkillExecutionResult {
                output: format!("{input} -> {}", skill.manifest.name),
                fuel_consumed: Some(100),
                memory_peak_bytes: Some(1024),
                duration: Duration::from_millis(10),
            })
        }
    }

    /// A mock executor that fails on a specific skill.
    struct FailingExecutor {
        fail_on: String,
    }

    impl SkillExecutor for FailingExecutor {
        async fn execute(
            &self,
            skill: &InstalledSkill,
            _input: &str,
            _enforcer: &CapabilityEnforcer,
        ) -> anyhow::Result<SkillExecutionResult> {
            if skill.manifest.name == self.fail_on {
                anyhow::bail!("skill execution error");
            }
            Ok(SkillExecutionResult {
                output: "ok".to_string(),
                fuel_consumed: None,
                memory_peak_bytes: None,
                duration: Duration::from_millis(5),
            })
        }
    }

    fn make_skill(name: &str) -> InstalledSkill {
        InstalledSkill {
            manifest: SkillManifest {
                name: name.to_string(),
                description: format!("{name} skill"),
                license: None,
                compatibility: None,
                metadata: None,
                allowed_tools: None,
            },
            body: String::new(),
            source: SkillSource::Local,
            install_path: PathBuf::from(format!("/skills/{name}")),
            wasm_path: None,
        }
    }

    fn make_enforcer() -> CapabilityEnforcer {
        let grants = vec![PermissionGrant {
            skill_name: "test".to_string(),
            capability: Capability::HttpGet,
            granted: true,
            granted_at: Utc::now(),
        }];
        CapabilityEnforcer::new("test", &grants).unwrap()
    }

    #[tokio::test]
    async fn chain_single_skill() {
        let executor = MockChainExecutor;
        let skill = make_skill("alpha");
        let enforcer = make_enforcer();

        let result = chain_skills(&executor, &[&skill], "hello", &enforcer)
            .await
            .unwrap();

        assert_eq!(result.output, "hello -> alpha");
        assert_eq!(result.fuel_consumed, Some(100));
        assert_eq!(result.duration, Duration::from_millis(10));
    }

    #[tokio::test]
    async fn chain_multiple_skills_pipes_output() {
        let executor = MockChainExecutor;
        let skill_a = make_skill("alpha");
        let skill_b = make_skill("beta");
        let skill_c = make_skill("gamma");
        let enforcer = make_enforcer();

        let result = chain_skills(
            &executor,
            &[&skill_a, &skill_b, &skill_c],
            "start",
            &enforcer,
        )
        .await
        .unwrap();

        assert_eq!(result.output, "start -> alpha -> beta -> gamma");
        assert_eq!(result.fuel_consumed, Some(300));
        assert_eq!(result.duration, Duration::from_millis(30));
        assert_eq!(result.memory_peak_bytes, Some(1024));
    }

    #[tokio::test]
    async fn chain_empty_skills_returns_error() {
        let executor = MockChainExecutor;
        let enforcer = make_enforcer();

        let result = chain_skills(&executor, &[], "input", &enforcer).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one skill")
        );
    }

    #[tokio::test]
    async fn chain_error_includes_position_context() {
        let executor = FailingExecutor {
            fail_on: "beta".to_string(),
        };
        let skill_a = make_skill("alpha");
        let skill_b = make_skill("beta");
        let enforcer = make_enforcer();

        let result = chain_skills(&executor, &[&skill_a, &skill_b], "input", &enforcer).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_str = format!("{err:#}");
        assert!(
            err_str.contains("position 1"),
            "error should include position: {err_str}"
        );
        assert!(
            err_str.contains("beta"),
            "error should include skill name: {err_str}"
        );
    }
}
