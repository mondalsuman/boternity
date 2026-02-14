//! macOS Seatbelt sandbox implementation.
//!
//! Uses `sandbox-exec` with dynamically generated Seatbelt profiles to restrict
//! the WASM executor subprocess. The host process is never restricted -- the
//! Seatbelt profile applies only to the spawned child process.

use super::sandbox::{SandboxConfig, SandboxRequest, SandboxResponse};

/// Generate a Seatbelt profile string from the sandbox configuration.
///
/// The profile starts with `(deny default)` and selectively allows:
/// - Process execution (for the subprocess itself)
/// - System library access (dylibs, frameworks)
/// - WASM binary read access
/// - Configured readable/writable paths
/// - Network access (if allowed by config)
pub fn generate_seatbelt_profile(config: &SandboxConfig) -> String {
    let mut profile = String::new();

    // Version and default deny
    profile.push_str("(version 1)\n");
    profile.push_str("(deny default)\n");
    profile.push_str("\n");

    // Allow process execution (subprocess needs to run)
    profile.push_str("; Allow process execution\n");
    profile.push_str("(allow process-exec)\n");
    profile.push_str("(allow process-fork)\n");
    profile.push_str("\n");

    // Allow system libraries and frameworks
    profile.push_str("; Allow system libraries\n");
    profile.push_str("(allow file-read*\n");
    profile.push_str("  (subpath \"/usr/lib\")\n");
    profile.push_str("  (subpath \"/System/Library\")\n");
    profile.push_str("  (subpath \"/Library/Frameworks\")\n");
    profile.push_str("  (subpath \"/usr/local/lib\")\n");
    profile.push_str(")\n");
    profile.push_str("\n");

    // Allow reading the WASM binary
    profile.push_str("; Allow reading the WASM binary\n");
    profile.push_str(&format!(
        "(allow file-read* (literal \"{}\"))\n",
        config.wasm_path.display()
    ));
    profile.push_str("\n");

    // Allow temp directory (always readable/writable)
    profile.push_str("; Allow temp directory\n");
    profile.push_str(&format!(
        "(allow file-read* file-write* (subpath \"{}\"))\n",
        config.temp_dir.display()
    ));
    profile.push_str("\n");

    // Allow configured readable paths
    if !config.readable_paths.is_empty() {
        profile.push_str("; Configured readable paths\n");
        for path in &config.readable_paths {
            profile.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))\n",
                path.display()
            ));
        }
        profile.push_str("\n");
    }

    // Allow configured writable paths
    if !config.writable_paths.is_empty() {
        profile.push_str("; Configured writable paths\n");
        for path in &config.writable_paths {
            profile.push_str(&format!(
                "(allow file-read* file-write* (subpath \"{}\"))\n",
                path.display()
            ));
        }
        profile.push_str("\n");
    }

    // Conditional network access
    if config.allow_network {
        profile.push_str("; Allow network access\n");
        profile.push_str("(allow network*)\n");
        profile.push_str("\n");
    }

    // Allow sysctl (needed for some runtime queries)
    profile.push_str("; Allow sysctl for runtime queries\n");
    profile.push_str("(allow sysctl-read)\n");

    // Allow mach IPC (needed for basic process operation)
    profile.push_str("(allow mach-lookup)\n");

    profile
}

/// Run a WASM skill in a macOS Seatbelt sandbox.
///
/// Spawns `sandbox-exec -p {profile} {exe} --wasm-sandbox-exec` as a subprocess.
/// The sandbox configuration is serialized as JSON to stdin, and the result
/// is read from stdout.
///
/// # Errors
///
/// Returns an error if:
/// - The current executable path cannot be determined
/// - `sandbox-exec` fails to spawn
/// - The subprocess exits with a non-zero status
/// - The subprocess output cannot be parsed as JSON
pub async fn run_sandboxed_macos(config: &SandboxConfig) -> anyhow::Result<String> {
    let profile = generate_seatbelt_profile(config);

    // Get the current executable path for subprocess spawning
    let exe_path = std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("failed to get current executable path: {}", e))?;

    // Build the subprocess request
    let request = SandboxRequest {
        wasm_path: config.wasm_path.clone(),
        input: config.input.clone(),
        trust_tier: config.trust_tier.clone(),
        resource_limits: config.resource_limits.clone(),
    };

    let request_json =
        serde_json::to_string(&request).map_err(|e| anyhow::anyhow!("failed to serialize sandbox request: {}", e))?;

    // Spawn sandbox-exec with the generated profile
    let timeout_ms = config.resource_limits.max_duration_ms;

    let mut child = tokio::process::Command::new("sandbox-exec")
        .arg("-p")
        .arg(&profile)
        .arg(&exe_path)
        .arg("--wasm-sandbox-exec")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn sandbox-exec: {}", e))?;

    // Write request to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(request_json.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("failed to write to sandbox subprocess stdin: {}", e))?;
        // Drop stdin to signal EOF
    }

    // Wait for completion with timeout
    let output = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "sandbox subprocess timed out after {}ms",
            timeout_ms
        )
    })?
    .map_err(|e| anyhow::anyhow!("sandbox subprocess failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "sandbox subprocess exited with status {}: {}",
            output.status,
            stderr.trim()
        );
    }

    // Parse the JSON response from stdout
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: SandboxResponse = serde_json::from_str(&stdout).map_err(|e| {
        anyhow::anyhow!(
            "failed to parse sandbox response: {} (raw: {})",
            e,
            stdout.chars().take(200).collect::<String>()
        )
    })?;

    if response.success {
        response
            .output
            .ok_or_else(|| anyhow::anyhow!("sandbox response marked success but no output"))
    } else {
        anyhow::bail!(
            "sandboxed skill execution failed: {}",
            response.error.unwrap_or_else(|| "unknown error".to_string())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boternity_types::skill::{ResourceLimits, TrustTier};
    use std::path::PathBuf;

    fn test_config() -> SandboxConfig {
        SandboxConfig {
            wasm_path: PathBuf::from("/tmp/test-skill.wasm"),
            input: "test input".to_string(),
            readable_paths: vec![PathBuf::from("/tmp/readable")],
            writable_paths: vec![PathBuf::from("/tmp/writable")],
            allow_network: false,
            temp_dir: PathBuf::from("/tmp/sandbox-tmp"),
            trust_tier: TrustTier::Untrusted,
            resource_limits: ResourceLimits::default(),
        }
    }

    #[test]
    fn seatbelt_profile_includes_deny_default() {
        let config = test_config();
        let profile = generate_seatbelt_profile(&config);
        assert!(
            profile.contains("(deny default)"),
            "profile must include deny default"
        );
    }

    #[test]
    fn seatbelt_profile_includes_version() {
        let config = test_config();
        let profile = generate_seatbelt_profile(&config);
        assert!(
            profile.contains("(version 1)"),
            "profile must include version 1"
        );
    }

    #[test]
    fn seatbelt_profile_includes_wasm_path() {
        let config = test_config();
        let profile = generate_seatbelt_profile(&config);
        assert!(
            profile.contains("/tmp/test-skill.wasm"),
            "profile must allow reading the WASM binary"
        );
    }

    #[test]
    fn seatbelt_profile_includes_readable_paths() {
        let config = test_config();
        let profile = generate_seatbelt_profile(&config);
        assert!(
            profile.contains("/tmp/readable"),
            "profile must include configured readable paths"
        );
    }

    #[test]
    fn seatbelt_profile_includes_writable_paths() {
        let config = test_config();
        let profile = generate_seatbelt_profile(&config);
        assert!(
            profile.contains("/tmp/writable"),
            "profile must include configured writable paths"
        );
    }

    #[test]
    fn seatbelt_profile_includes_temp_dir() {
        let config = test_config();
        let profile = generate_seatbelt_profile(&config);
        assert!(
            profile.contains("/tmp/sandbox-tmp"),
            "profile must include temp directory"
        );
    }

    #[test]
    fn seatbelt_profile_excludes_network_when_disabled() {
        let config = test_config();
        let profile = generate_seatbelt_profile(&config);
        assert!(
            !profile.contains("(allow network"),
            "profile must NOT include network access when disabled"
        );
    }

    #[test]
    fn seatbelt_profile_includes_network_when_enabled() {
        let mut config = test_config();
        config.allow_network = true;
        let profile = generate_seatbelt_profile(&config);
        assert!(
            profile.contains("(allow network"),
            "profile must include network access when enabled"
        );
    }

    #[test]
    fn seatbelt_profile_allows_system_libraries() {
        let config = test_config();
        let profile = generate_seatbelt_profile(&config);
        assert!(
            profile.contains("/usr/lib"),
            "profile must allow /usr/lib"
        );
        assert!(
            profile.contains("/System/Library"),
            "profile must allow /System/Library"
        );
    }
}
