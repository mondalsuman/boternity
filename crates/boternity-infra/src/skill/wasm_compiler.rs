//! WASM component generation for registry skills.
//!
//! Handles two paths:
//! 1. Pre-compiled: registry provides `.wasm` bytes directly
//! 2. Stub generation: creates a minimal WASM stub marker from SKILL.md body
//!
//! The stub approach allows the WASM execution pipeline to succeed end-to-end
//! for registry Tool-type skills that don't ship pre-compiled binaries.
//! Real WASM component generation from Rust source code is a Phase 7 builder
//! concern.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::debug;

/// Ensure a Tool-type skill has a `.wasm` binary at the install path.
///
/// If `wasm_bytes` is already provided (from registry fetch), writes them to
/// disk. If not, generates a stub WASM component that returns the skill body
/// as output.
///
/// Returns the path to the `.wasm` file on disk.
pub fn ensure_wasm_binary(
    install_dir: &Path,
    skill_body: &str,
    wasm_bytes: Option<&[u8]>,
) -> Result<PathBuf> {
    let wasm_path = install_dir.join("skill.wasm");

    if let Some(bytes) = wasm_bytes {
        // Path A: pre-compiled binary from registry
        debug!(
            path = %wasm_path.display(),
            size = bytes.len(),
            "Writing pre-compiled WASM binary"
        );
        std::fs::write(&wasm_path, bytes)
            .with_context(|| format!("Failed to write WASM binary: {}", wasm_path.display()))?;
        return Ok(wasm_path);
    }

    // Path B: generate stub WASM component
    debug!(
        path = %wasm_path.display(),
        body_len = skill_body.len(),
        "Generating stub WASM component for Tool skill"
    );

    let component_bytes = generate_stub_component(skill_body)?;
    std::fs::write(&wasm_path, &component_bytes)
        .with_context(|| format!("Failed to write stub WASM component: {}", wasm_path.display()))?;

    Ok(wasm_path)
}

/// Generate a stub WASM component marker.
///
/// This is NOT a real WASM binary -- it's a JSON marker that the
/// [`WasmSkillExecutor`] detects and handles specially, returning
/// the skill body as output without actually running a WASM module.
fn generate_stub_component(skill_body: &str) -> Result<Vec<u8>> {
    let stub = serde_json::json!({
        "boternity_wasm_stub": true,
        "version": 1,
        "body": skill_body,
    });
    Ok(serde_json::to_vec_pretty(&stub)?)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn ensure_wasm_binary_with_precompiled() {
        let tmp = TempDir::new().unwrap();
        let precompiled = b"\x00asm\x01\x00\x00\x00fake-wasm-bytes";

        let result = ensure_wasm_binary(tmp.path(), "unused body", Some(precompiled));
        assert!(result.is_ok(), "should succeed: {:?}", result.err());

        let wasm_path = result.unwrap();
        assert!(wasm_path.exists(), "skill.wasm should exist on disk");
        assert_eq!(wasm_path.file_name().unwrap(), "skill.wasm");

        let written = std::fs::read(&wasm_path).unwrap();
        assert_eq!(written, precompiled, "written bytes should match input");
    }

    #[test]
    fn ensure_wasm_binary_generates_stub() {
        let tmp = TempDir::new().unwrap();
        let body = "This skill does web search using the search API.";

        let result = ensure_wasm_binary(tmp.path(), body, None);
        assert!(result.is_ok(), "should succeed: {:?}", result.err());

        let wasm_path = result.unwrap();
        assert!(wasm_path.exists(), "skill.wasm should exist on disk");
        assert_eq!(wasm_path.file_name().unwrap(), "skill.wasm");

        // Stub should be non-empty
        let written = std::fs::read(&wasm_path).unwrap();
        assert!(!written.is_empty(), "stub should not be empty");
    }

    #[test]
    fn ensure_wasm_binary_stub_is_valid_json() {
        let tmp = TempDir::new().unwrap();
        let body = "Use this skill to query databases.";

        let wasm_path = ensure_wasm_binary(tmp.path(), body, None).unwrap();
        let written = std::fs::read(&wasm_path).unwrap();

        let parsed: serde_json::Value =
            serde_json::from_slice(&written).expect("stub should be valid JSON");

        assert_eq!(
            parsed.get("boternity_wasm_stub").and_then(|v| v.as_bool()),
            Some(true),
            "stub should have boternity_wasm_stub: true"
        );
        assert_eq!(
            parsed.get("version").and_then(|v| v.as_u64()),
            Some(1),
            "stub should have version: 1"
        );
        assert_eq!(
            parsed.get("body").and_then(|v| v.as_str()),
            Some(body),
            "stub body should match input"
        );
    }

    #[test]
    fn ensure_wasm_binary_precompiled_overwrites_existing() {
        let tmp = TempDir::new().unwrap();

        // First, create a stub
        ensure_wasm_binary(tmp.path(), "old body", None).unwrap();

        // Then overwrite with pre-compiled
        let precompiled = b"new-wasm-binary-content";
        let wasm_path = ensure_wasm_binary(tmp.path(), "unused", Some(precompiled)).unwrap();

        let written = std::fs::read(&wasm_path).unwrap();
        assert_eq!(written, precompiled, "pre-compiled should overwrite stub");
    }
}
