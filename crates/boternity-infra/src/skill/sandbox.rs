//! OS-level sandbox dispatch layer for defense-in-depth skill execution.
//!
//! WASM provides the first isolation layer. This module adds a second layer
//! by running the WASM executor in a restricted subprocess. The subprocess
//! model ensures the host process is never restricted -- OS-level sandboxing
//! applies only to the child process.
//!
//! Platform dispatch:
//! - macOS: Seatbelt (`sandbox-exec`) with dynamically generated profiles
//! - Linux: Landlock filesystem restrictions in the child process
//! - Other: Unsupported (returns error)
//!
//! The subprocess is spawned as `self --wasm-sandbox-exec`, which applies
//! OS restrictions before running the WASM component. Communication happens
//! via stdin/stdout JSON.

use std::path::{Path, PathBuf};

use boternity_core::skill::executor::SkillExecutionResult;
use boternity_types::skill::{ResourceLimits, TrustTier};
use serde::{Deserialize, Serialize};

/// Configuration for running a WASM skill inside an OS-level sandbox.
///
/// Passed to the platform-specific sandbox implementation, which spawns
/// a subprocess with the appropriate restrictions applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Path to the compiled WASM component binary.
    pub wasm_path: PathBuf,
    /// JSON-encoded input to pass to the skill's `execute` function.
    pub input: String,
    /// Filesystem paths the skill is allowed to read.
    pub readable_paths: Vec<PathBuf>,
    /// Filesystem paths the skill is allowed to write.
    pub writable_paths: Vec<PathBuf>,
    /// Whether the skill is allowed network access.
    pub allow_network: bool,
    /// Temporary directory for scratch space (always readable/writable).
    pub temp_dir: PathBuf,
    /// Trust tier determines sandbox strictness.
    pub trust_tier: TrustTier,
    /// Resource limits forwarded to the WASM executor in the subprocess.
    pub resource_limits: ResourceLimits,
}

/// JSON protocol for subprocess communication via stdin.
#[derive(Debug, Serialize, Deserialize)]
pub struct SandboxRequest {
    pub wasm_path: PathBuf,
    pub input: String,
    pub trust_tier: TrustTier,
    pub resource_limits: ResourceLimits,
}

/// JSON protocol for subprocess communication via stdout.
#[derive(Debug, Serialize, Deserialize)]
pub struct SandboxResponse {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub fuel_consumed: Option<u64>,
    pub duration_ms: Option<u64>,
}

impl SandboxResponse {
    /// Convert a sandbox subprocess response into a [`SkillExecutionResult`].
    ///
    /// On success, maps the JSON output and fuel tracking. On failure, returns
    /// the error message from the subprocess as an `anyhow` error.
    pub fn into_execution_result(
        self,
        elapsed: std::time::Duration,
    ) -> anyhow::Result<SkillExecutionResult> {
        if self.success {
            Ok(SkillExecutionResult {
                output: self.output.unwrap_or_default(),
                fuel_consumed: self.fuel_consumed,
                memory_peak_bytes: None,
                duration: elapsed,
            })
        } else {
            anyhow::bail!(
                "sandboxed execution failed: {}",
                self.error
                    .unwrap_or_else(|| "unknown error".to_string())
            )
        }
    }
}

/// Build a [`SandboxConfig`] from skill execution parameters.
///
/// Centralizes sandbox configuration construction so that callers only need
/// to provide the skill's WASM path, input, trust tier, and resource limits.
/// The helper sets restrictive defaults:
/// - Readable paths: only the skill's install directory (parent of wasm_path)
/// - Writable paths: none
/// - Network access: denied
/// - Temp dir: `boternity-sandbox` under the system temp directory
pub fn build_config_for_skill(
    wasm_path: &Path,
    input: &str,
    trust_tier: &TrustTier,
    resource_limits: &ResourceLimits,
) -> SandboxConfig {
    // The skill install directory (parent of the .wasm file) must be readable
    // so the subprocess can access the binary.
    let readable_paths = wasm_path
        .parent()
        .map(|p| vec![p.to_path_buf()])
        .unwrap_or_default();

    SandboxConfig {
        wasm_path: wasm_path.to_path_buf(),
        input: input.to_string(),
        readable_paths,
        writable_paths: Vec::new(),
        allow_network: false,
        temp_dir: std::env::temp_dir().join("boternity-sandbox"),
        trust_tier: trust_tier.clone(),
        resource_limits: resource_limits.clone(),
    }
}

/// Run a WASM skill inside an OS-level sandbox subprocess.
///
/// Dispatches to the platform-specific implementation. The subprocess model
/// ensures OS-level restrictions (Seatbelt on macOS, Landlock on Linux) only
/// apply to the child process -- the host process is never restricted.
///
/// # Errors
///
/// Returns an error if:
/// - The current platform is not supported
/// - The subprocess fails to spawn or communicate
/// - The WASM execution inside the subprocess fails
pub async fn run_sandboxed(config: &SandboxConfig) -> anyhow::Result<String> {
    #[cfg(target_os = "macos")]
    {
        return super::sandbox_macos::run_sandboxed_macos(config).await;
    }
    #[cfg(target_os = "linux")]
    {
        return super::sandbox_linux::run_sandboxed_linux(config).await;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        anyhow::bail!("OS-level sandbox not supported on this platform")
    }
}

/// Determine whether OS-level sandboxing should be used for a given trust tier.
///
/// Only `Untrusted` skills trigger the OS sandbox by default. Verified and
/// Local skills rely on WASM-level sandboxing alone (Verified) or run
/// natively without sandboxing (Local).
pub fn should_use_os_sandbox(tier: &TrustTier) -> bool {
    matches!(tier, TrustTier::Untrusted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_config_builds_correctly() {
        let config = SandboxConfig {
            wasm_path: PathBuf::from("/tmp/skill.wasm"),
            input: r#"{"query": "test"}"#.to_string(),
            readable_paths: vec![PathBuf::from("/tmp/data")],
            writable_paths: vec![PathBuf::from("/tmp/output")],
            allow_network: false,
            temp_dir: PathBuf::from("/tmp/sandbox-work"),
            trust_tier: TrustTier::Untrusted,
            resource_limits: ResourceLimits::default(),
        };

        assert_eq!(config.wasm_path, PathBuf::from("/tmp/skill.wasm"));
        assert!(!config.allow_network);
        assert_eq!(config.readable_paths.len(), 1);
        assert_eq!(config.writable_paths.len(), 1);
        assert_eq!(config.trust_tier, TrustTier::Untrusted);
    }

    #[test]
    fn sandbox_config_serializes_roundtrip() {
        let config = SandboxConfig {
            wasm_path: PathBuf::from("/tmp/skill.wasm"),
            input: "hello".to_string(),
            readable_paths: vec![],
            writable_paths: vec![],
            allow_network: true,
            temp_dir: PathBuf::from("/tmp"),
            trust_tier: TrustTier::Verified,
            resource_limits: ResourceLimits::default(),
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: SandboxConfig =
            serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.wasm_path, config.wasm_path);
        assert_eq!(deserialized.input, config.input);
        assert_eq!(deserialized.allow_network, config.allow_network);
    }

    #[test]
    fn should_use_os_sandbox_untrusted_true() {
        assert!(
            should_use_os_sandbox(&TrustTier::Untrusted),
            "Untrusted tier should use OS sandbox"
        );
    }

    #[test]
    fn should_use_os_sandbox_verified_false() {
        assert!(
            !should_use_os_sandbox(&TrustTier::Verified),
            "Verified tier should not use OS sandbox by default"
        );
    }

    #[test]
    fn should_use_os_sandbox_local_false() {
        assert!(
            !should_use_os_sandbox(&TrustTier::Local),
            "Local tier should not use OS sandbox"
        );
    }

    #[test]
    fn sandbox_request_serializes() {
        let req = SandboxRequest {
            wasm_path: PathBuf::from("/tmp/skill.wasm"),
            input: "test input".to_string(),
            trust_tier: TrustTier::Untrusted,
            resource_limits: ResourceLimits::default(),
        };

        let json = serde_json::to_string(&req).expect("serialize request");
        assert!(json.contains("skill.wasm"));
        assert!(json.contains("untrusted"));
    }

    #[test]
    fn sandbox_response_success() {
        let resp = SandboxResponse {
            success: true,
            output: Some("result".to_string()),
            error: None,
            fuel_consumed: Some(42),
            duration_ms: Some(100),
        };

        let json = serde_json::to_string(&resp).expect("serialize response");
        let deserialized: SandboxResponse =
            serde_json::from_str(&json).expect("deserialize response");

        assert!(deserialized.success);
        assert_eq!(deserialized.output.as_deref(), Some("result"));
        assert!(deserialized.error.is_none());
    }

    #[test]
    fn sandbox_response_failure() {
        let resp = SandboxResponse {
            success: false,
            output: None,
            error: Some("timeout".to_string()),
            fuel_consumed: None,
            duration_ms: Some(30_000),
        };

        assert!(!resp.success);
        assert_eq!(resp.error.as_deref(), Some("timeout"));
    }

    #[test]
    fn build_config_for_skill_untrusted() {
        let wasm_path = PathBuf::from("/home/user/.boternity/skills/my-skill/plugin.wasm");
        let input = r#"{"query": "hello"}"#;
        let trust_tier = TrustTier::Untrusted;
        let resource_limits = ResourceLimits::default();

        let config = build_config_for_skill(
            &wasm_path,
            input,
            &trust_tier,
            &resource_limits,
        );

        assert_eq!(config.wasm_path, wasm_path);
        assert_eq!(config.input, input);
        assert_eq!(config.trust_tier, TrustTier::Untrusted);
        // Parent dir is readable
        assert_eq!(config.readable_paths.len(), 1);
        assert_eq!(
            config.readable_paths[0],
            PathBuf::from("/home/user/.boternity/skills/my-skill")
        );
        // No write access for untrusted
        assert!(config.writable_paths.is_empty());
        // No network access
        assert!(!config.allow_network);
        // Temp dir under system temp
        assert!(config.temp_dir.ends_with("boternity-sandbox"));
        // Resource limits forwarded
        assert_eq!(config.resource_limits.max_memory_bytes, resource_limits.max_memory_bytes);
        assert_eq!(config.resource_limits.max_fuel, resource_limits.max_fuel);
    }

    #[test]
    fn sandbox_response_into_result_success() {
        let resp = SandboxResponse {
            success: true,
            output: Some("skill output".to_string()),
            error: None,
            fuel_consumed: Some(12345),
            duration_ms: Some(50),
        };

        let elapsed = std::time::Duration::from_millis(55);
        let result = resp.into_execution_result(elapsed);
        assert!(result.is_ok(), "success response should convert to Ok");

        let exec_result = result.unwrap();
        assert_eq!(exec_result.output, "skill output");
        assert_eq!(exec_result.fuel_consumed, Some(12345));
        assert!(exec_result.memory_peak_bytes.is_none());
        assert_eq!(exec_result.duration, elapsed);
    }

    #[test]
    fn sandbox_response_into_result_success_no_output() {
        let resp = SandboxResponse {
            success: true,
            output: None,
            error: None,
            fuel_consumed: None,
            duration_ms: None,
        };

        let elapsed = std::time::Duration::from_millis(10);
        let result = resp.into_execution_result(elapsed).unwrap();
        assert_eq!(result.output, "", "None output should become empty string");
        assert!(result.fuel_consumed.is_none());
    }

    #[test]
    fn sandbox_response_into_result_failure() {
        let resp = SandboxResponse {
            success: false,
            output: None,
            error: Some("out of fuel".to_string()),
            fuel_consumed: Some(500_000),
            duration_ms: Some(10_000),
        };

        let elapsed = std::time::Duration::from_secs(10);
        let result = resp.into_execution_result(elapsed);
        assert!(result.is_err(), "failure response should convert to Err");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("sandboxed execution failed"),
            "error should contain prefix: {}",
            err_msg
        );
        assert!(
            err_msg.contains("out of fuel"),
            "error should contain subprocess message: {}",
            err_msg
        );
    }

    #[test]
    fn sandbox_response_into_result_failure_unknown_error() {
        let resp = SandboxResponse {
            success: false,
            output: None,
            error: None,
            fuel_consumed: None,
            duration_ms: None,
        };

        let elapsed = std::time::Duration::from_secs(1);
        let result = resp.into_execution_result(elapsed);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("unknown error"),
            "missing error field should produce 'unknown error': {}",
            err_msg
        );
    }
}
