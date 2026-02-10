//! Secret management CLI commands: set, list.

use anyhow::Result;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;
use dialoguer::Password;

use boternity_core::service::secret::SecretService;
use boternity_types::secret::SecretScope;

use crate::state::AppState;

/// Set a secret value with hidden input prompt.
///
/// # Examples
///
/// ```bash
/// # Secure prompt (recommended)
/// bnity set secret ANTHROPIC_API_KEY
///
/// # Script/automation mode
/// bnity set secret ANTHROPIC_API_KEY --value sk-...
/// ```
pub async fn set_secret(
    state: &AppState,
    key: &str,
    value: Option<&str>,
    json: bool,
) -> Result<()> {
    let secret_value = match value {
        Some(v) => v.to_string(),
        None => {
            Password::new()
                .with_prompt(format!("Enter value for {}", style(key).bold()))
                .interact()?
        }
    };

    state
        .secret_service
        .set_secret(key, &secret_value, &SecretScope::Global)
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({"set": true, "key": key, "masked": SecretService::mask_secret(&secret_value)})
        );
    } else {
        println!(
            "  {} Secret '{}' set ({})",
            style("✓").green().bold(),
            style(key).bold(),
            SecretService::mask_secret(&secret_value)
        );
    }

    Ok(())
}

/// List all secrets with masked values.
pub async fn list_secrets(state: &AppState, json: bool) -> Result<()> {
    let entries = state
        .secret_service
        .list_secrets(&SecretScope::Global)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if entries.is_empty() {
        println!();
        println!(
            "  {} No secrets stored. Add one with: {}",
            style("i").blue().bold(),
            style("bnity set secret ANTHROPIC_API_KEY").yellow()
        );
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_FULL_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Key").fg(Color::White),
        Cell::new("Provider").fg(Color::White),
        Cell::new("Scope").fg(Color::White),
        Cell::new("Updated").fg(Color::White),
    ]);

    for entry in &entries {
        // Try to get the actual value for masking
        let masked = match state
            .secret_service
            .get_secret(&entry.key.0, &SecretScope::Global)
            .await
        {
            Ok(Some(val)) => SecretService::mask_secret(&val),
            _ => "****".to_string(),
        };

        table.add_row(vec![
            Cell::new(format!("{}: {}", entry.key, masked)).fg(Color::Cyan),
            Cell::new(entry.provider.to_string()),
            Cell::new(entry.scope.to_string()),
            Cell::new(entry.updated_at.format("%Y-%m-%d").to_string()).fg(Color::DarkGrey),
        ]);
    }

    println!();
    println!("{table}");
    println!();
    println!(
        "  {} secret{}",
        style(entries.len()).bold(),
        if entries.len() == 1 { "" } else { "s" }
    );
    println!();

    Ok(())
}
