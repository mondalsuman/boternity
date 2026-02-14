//! Standalone skill creation command (`bnity skill create-wizard`).
//!
//! Interactive flow: description prompt, skill type selection, generate via
//! SkillBuilder, show capabilities, confirm, write to disk.
//!
//! Populated in Task 2 of Plan 07-07.

use anyhow::Result;

use crate::state::AppState;

/// Run the standalone interactive skill creation wizard.
pub async fn run_skill_create(_state: &AppState) -> Result<()> {
    // Placeholder -- implemented in Task 2
    println!("Skill creation wizard coming soon.");
    Ok(())
}
