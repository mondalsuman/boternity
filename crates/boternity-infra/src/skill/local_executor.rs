//! Local skill executor for host-native skill execution.
//!
//! Implements [`SkillExecutor`] for skills with [`TrustTier::Local`].
//! Local skills run with full trust on the host machine via process
//! spawning. Tool-based local skills must have a `run.sh` or `run.py`
//! script in their `scripts/` directory.

use std::time::Instant;

use anyhow::{bail, Context};
use boternity_core::skill::executor::{SkillExecutionResult, SkillExecutor};
use boternity_core::skill::permission::CapabilityEnforcer;
use boternity_types::skill::{Capability, InstalledSkill, SkillSource, SkillType};
use tokio::io::AsyncWriteExt;

/// Timeout for local skill execution (60 seconds).
const LOCAL_EXECUTION_TIMEOUT_SECS: u64 = 60;

/// Local skill executor that runs skills via process spawning.
///
/// Only accepts skills with [`SkillSource::Local`] and enforces the
/// [`Capability::ExecCommand`] permission before spawning.
#[derive(Debug, Clone)]
pub struct LocalSkillExecutor;

impl LocalSkillExecutor {
    /// Create a new local skill executor.
    pub fn new() -> Self {
        Self
    }

    /// Find the executable script in the skill's `scripts/` directory.
    ///
    /// Looks for `run.sh` first, then `run.py`. Returns the path to the
    /// first script found, or an error if neither exists.
    fn find_script(skill: &InstalledSkill) -> anyhow::Result<std::path::PathBuf> {
        let scripts_dir = skill.install_path.join("scripts");

        let run_sh = scripts_dir.join("run.sh");
        if run_sh.exists() {
            return Ok(run_sh);
        }

        let run_py = scripts_dir.join("run.py");
        if run_py.exists() {
            return Ok(run_py);
        }

        bail!(
            "No run.sh or run.py found in {}/scripts/",
            skill.install_path.display()
        );
    }
}

impl Default for LocalSkillExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillExecutor for LocalSkillExecutor {
    async fn execute(
        &self,
        skill: &InstalledSkill,
        input: &str,
        enforcer: &CapabilityEnforcer,
    ) -> anyhow::Result<SkillExecutionResult> {
        // 1. Verify skill source is Local
        if !matches!(skill.source, SkillSource::Local) {
            bail!(
                "LocalSkillExecutor only handles Local skills, got {:?}",
                skill.source
            );
        }

        // 2. Check ExecCommand capability
        enforcer
            .check(&Capability::ExecCommand)
            .context("LocalSkillExecutor requires ExecCommand capability")?;

        // 3. Determine skill type -- prompt skills don't need process spawning
        let skill_type = skill
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.skill_type.as_ref());

        if matches!(skill_type, Some(SkillType::Prompt)) {
            // Prompt skills inject into system prompt, not executed as processes.
            // Return the body as the "output" for the caller to inject.
            let start = Instant::now();
            return Ok(SkillExecutionResult {
                output: skill.body.clone(),
                fuel_consumed: None,
                memory_peak_bytes: None,
                duration: start.elapsed(),
            });
        }

        // 4. Find the executable script
        let script_path = Self::find_script(skill)?;

        // 5. Determine the interpreter based on extension
        let (interpreter, args): (&str, Vec<&str>) = if script_path
            .extension()
            .is_some_and(|ext| ext == "py")
        {
            ("python3", vec![])
        } else {
            ("bash", vec![])
        };

        // 6. Spawn the process with input on stdin
        let start = Instant::now();

        let mut child = tokio::process::Command::new(interpreter)
            .args(&args)
            .arg(&script_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&skill.install_path)
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to spawn {} for skill '{}'",
                    script_path.display(),
                    skill.manifest.name
                )
            })?;

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).await.ok();
            // Drop stdin to close the pipe and signal EOF
        }

        // 7. Wait with timeout
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(LOCAL_EXECUTION_TIMEOUT_SECS),
            child.wait_with_output(),
        )
        .await
        .context("Local skill execution timed out (60s limit)")?
        .context("Failed to wait for skill process")?;

        let duration = start.elapsed();

        // 8. Check exit status
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "Skill '{}' exited with status {}: {}",
                skill.manifest.name,
                output.status,
                stderr.trim()
            );
        }

        let stdout = String::from_utf8(output.stdout)
            .context("Skill output is not valid UTF-8")?;

        Ok(SkillExecutionResult {
            output: stdout,
            fuel_consumed: None,
            memory_peak_bytes: None,
            duration,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::skill::{
        Capability, PermissionGrant, SkillManifest, SkillMetadata, SkillSource, SkillType,
    };
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_grant(skill: &str, cap: Capability, granted: bool) -> PermissionGrant {
        PermissionGrant {
            skill_name: skill.to_owned(),
            capability: cap,
            granted,
            granted_at: Utc::now(),
        }
    }

    fn make_local_tool_skill(name: &str, install_path: PathBuf) -> InstalledSkill {
        InstalledSkill {
            manifest: SkillManifest {
                name: name.to_owned(),
                description: "A test tool skill".to_owned(),
                license: None,
                compatibility: None,
                metadata: Some(SkillMetadata {
                    author: None,
                    version: None,
                    skill_type: Some(SkillType::Tool),
                    capabilities: Some(vec![Capability::ExecCommand]),
                    dependencies: None,
                    conflicts_with: None,
                    trust_tier: None,
                    parents: None,
                    secrets: None,
                    categories: None,
                }),
                allowed_tools: None,
            },
            body: String::new(),
            source: SkillSource::Local,
            install_path,
            wasm_path: None,
        }
    }

    fn make_registry_skill(name: &str) -> InstalledSkill {
        InstalledSkill {
            manifest: SkillManifest {
                name: name.to_owned(),
                description: "A registry skill".to_owned(),
                license: None,
                compatibility: None,
                metadata: None,
                allowed_tools: None,
            },
            body: String::new(),
            source: SkillSource::Registry {
                registry_name: "agentskills.io".to_owned(),
                repo: "test/test".to_owned(),
                path: "/skills/test".to_owned(),
            },
            install_path: PathBuf::from("/tmp/nonexistent"),
            wasm_path: None,
        }
    }

    #[tokio::test]
    async fn rejects_non_local_skills() {
        let executor = LocalSkillExecutor::new();
        let skill = make_registry_skill("remote-skill");
        let grants = vec![make_grant("remote-skill", Capability::ExecCommand, true)];
        let enforcer = CapabilityEnforcer::new("remote-skill", &grants).unwrap();

        let result = executor.execute(&skill, "hello", &enforcer).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("only handles Local skills")
        );
    }

    #[tokio::test]
    async fn checks_exec_command_capability() {
        let tmpdir = tempfile::tempdir().unwrap();
        let executor = LocalSkillExecutor::new();
        let skill = make_local_tool_skill("test-skill", tmpdir.path().to_path_buf());

        // Grant only HttpGet, not ExecCommand
        let grants = vec![make_grant("test-skill", Capability::HttpGet, true)];
        let enforcer = CapabilityEnforcer::new("test-skill", &grants).unwrap();

        let result = executor.execute(&skill, "hello", &enforcer).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("ExecCommand")
        );
    }

    #[tokio::test]
    async fn validates_script_existence() {
        let tmpdir = tempfile::tempdir().unwrap();
        // Create skill dir but no scripts/ directory
        let executor = LocalSkillExecutor::new();
        let skill = make_local_tool_skill("test-skill", tmpdir.path().to_path_buf());

        let grants = vec![make_grant("test-skill", Capability::ExecCommand, true)];
        let enforcer = CapabilityEnforcer::new("test-skill", &grants).unwrap();

        let result = executor.execute(&skill, "hello", &enforcer).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No run.sh or run.py")
        );
    }

    #[tokio::test]
    async fn executes_run_sh_successfully() {
        let tmpdir = tempfile::tempdir().unwrap();
        let scripts_dir = tmpdir.path().join("scripts");
        std::fs::create_dir_all(&scripts_dir).unwrap();

        // Create a simple run.sh that echoes input
        let script = scripts_dir.join("run.sh");
        std::fs::write(&script, "#!/bin/bash\ncat\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let executor = LocalSkillExecutor::new();
        let skill = make_local_tool_skill("echo-skill", tmpdir.path().to_path_buf());

        let grants = vec![make_grant("echo-skill", Capability::ExecCommand, true)];
        let enforcer = CapabilityEnforcer::new("echo-skill", &grants).unwrap();

        let result = executor.execute(&skill, "test input", &enforcer).await;
        assert!(result.is_ok(), "Execution should succeed: {:?}", result.err());

        let exec_result = result.unwrap();
        assert_eq!(exec_result.output.trim(), "test input");
        assert!(exec_result.fuel_consumed.is_none());
        assert!(exec_result.memory_peak_bytes.is_none());
    }

    #[tokio::test]
    async fn prompt_skill_returns_body_directly() {
        let tmpdir = tempfile::tempdir().unwrap();
        let executor = LocalSkillExecutor::new();

        let skill = InstalledSkill {
            manifest: SkillManifest {
                name: "prompt-skill".to_owned(),
                description: "A prompt skill".to_owned(),
                license: None,
                compatibility: None,
                metadata: Some(SkillMetadata {
                    author: None,
                    version: None,
                    skill_type: Some(SkillType::Prompt),
                    capabilities: Some(vec![Capability::ExecCommand]),
                    dependencies: None,
                    conflicts_with: None,
                    trust_tier: None,
                    parents: None,
                    secrets: None,
                    categories: None,
                }),
                allowed_tools: None,
            },
            body: "You are a helpful assistant with special knowledge.".to_owned(),
            source: SkillSource::Local,
            install_path: tmpdir.path().to_path_buf(),
            wasm_path: None,
        };

        let grants = vec![make_grant("prompt-skill", Capability::ExecCommand, true)];
        let enforcer = CapabilityEnforcer::new("prompt-skill", &grants).unwrap();

        let result = executor.execute(&skill, "", &enforcer).await.unwrap();
        assert_eq!(
            result.output,
            "You are a helpful assistant with special knowledge."
        );
    }
}
