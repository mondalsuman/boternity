//! Linux Landlock sandbox implementation.
//!
//! Uses Landlock (ABI v3+) to restrict filesystem access in the WASM executor
//! subprocess. The host process is never restricted -- Landlock rules apply
//! only after the child process self-restricts.
//!
//! Falls back gracefully on older kernels that lack Landlock support.

use super::sandbox::{SandboxConfig, SandboxRequest, SandboxResponse};

use landlock::{
    path_beneath_rules, Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr,
    RulesetStatus, ABI,
};

/// Apply Landlock filesystem restrictions to the current process.
///
/// This function is called inside the sandbox subprocess BEFORE executing
/// the WASM component. It restricts the calling process to only the paths
/// specified in the sandbox configuration.
///
/// Uses ABI::V3 with best-effort fallback for older kernels -- if Landlock
/// is unsupported, the function logs a warning and returns Ok (defense-in-depth
/// layer is additive, not required for correctness).
///
/// # Errors
///
/// Returns an error if Landlock setup fails for reasons other than kernel
/// unsupport (e.g., invalid path, permission issues).
pub fn apply_landlock(config: &SandboxConfig) -> anyhow::Result<()> {
    let abi = ABI::V3;

    let mut ruleset = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))?
        .create()?;

    // Allow read access to configured readable paths
    let read_access = AccessFs::from_read(abi);
    for path in &config.readable_paths {
        if path.exists() {
            let rules = path_beneath_rules([path], read_access)
                .collect::<Vec<_>>();
            for rule in rules {
                ruleset = ruleset.add_rule(rule?)?;
            }
        }
    }

    // Allow read+write access to configured writable paths
    let write_access = AccessFs::from_all(abi);
    for path in &config.writable_paths {
        if path.exists() {
            let rules = path_beneath_rules([path], write_access)
                .collect::<Vec<_>>();
            for rule in rules {
                ruleset = ruleset.add_rule(rule?)?;
            }
        }
    }

    // Allow read access to the WASM binary
    if config.wasm_path.exists() {
        let parent = config
            .wasm_path
            .parent()
            .unwrap_or(&config.wasm_path);
        let rules = path_beneath_rules([parent], read_access)
            .collect::<Vec<_>>();
        for rule in rules {
            ruleset = ruleset.add_rule(rule?)?;
        }
    }

    // Allow read+write access to temp directory
    if config.temp_dir.exists() {
        let rules = path_beneath_rules([&config.temp_dir], write_access)
            .collect::<Vec<_>>();
        for rule in rules {
            ruleset = ruleset.add_rule(rule?)?;
        }
    }

    // Allow reading system libraries
    let system_paths = ["/usr/lib", "/lib", "/lib64"];
    for sys_path in &system_paths {
        let p = std::path::Path::new(sys_path);
        if p.exists() {
            let rules = path_beneath_rules([p], read_access)
                .collect::<Vec<_>>();
            for rule in rules {
                ruleset = ruleset.add_rule(rule?)?;
            }
        }
    }

    let status = ruleset.restrict_self()?;
    match status.ruleset {
        RulesetStatus::FullyEnforced => {
            tracing::info!("landlock sandbox fully enforced");
        }
        RulesetStatus::PartiallyEnforced => {
            tracing::warn!("landlock sandbox partially enforced (older kernel ABI)");
        }
        RulesetStatus::NotEnforced => {
            tracing::warn!(
                "landlock sandbox not enforced (kernel may lack Landlock support)"
            );
        }
    }

    Ok(())
}

/// Run a WASM skill in a Linux Landlock sandbox.
///
/// Spawns the current executable as a subprocess with `--wasm-sandbox-exec` flag.
/// The child process applies Landlock restrictions before executing the WASM
/// component.
///
/// # Errors
///
/// Returns an error if:
/// - The current executable path cannot be determined
/// - The subprocess fails to spawn
/// - The subprocess exits with a non-zero status
/// - The subprocess output cannot be parsed as JSON
pub async fn run_sandboxed_linux(config: &SandboxConfig) -> anyhow::Result<String> {
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

    let timeout_ms = config.resource_limits.max_duration_ms;

    let mut child = tokio::process::Command::new(&exe_path)
        .arg("--wasm-sandbox-exec")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn sandbox subprocess: {}", e))?;

    // Write request to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(request_json.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("failed to write to sandbox subprocess stdin: {}", e))?;
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
