//! Interactive TUI skill browser.
//!
//! Provides a ratatui-based 3-pane browser for discovering and installing
//! skills from configured registries. Stub for Task 1 compilation; Task 2
//! implements the full TUI.

use std::path::Path;

use anyhow::Result;
use boternity_core::skill::registry::DiscoveredSkill;
use boternity_infra::skill::registry_client::GitHubRegistryClient;

/// Launch the interactive TUI skill browser.
///
/// Returns `Some(DiscoveredSkill)` if the user selected a skill, or `None`
/// if they quit without selecting.
pub async fn run_browser(
    _registries: &[GitHubRegistryClient],
    _cache_dir: &Path,
) -> Result<Option<DiscoveredSkill>> {
    // Stub: will be implemented in Task 2
    anyhow::bail!("TUI browser not yet implemented. Use 'bnity skill install <name>' instead.")
}
