//! CLI skill management subcommands.
//!
//! Provides create, install, remove, list, inspect, attach, detach, enable,
//! disable, publish, browse, and update operations for skills.

use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;
use dialoguer::Confirm;

use boternity_core::skill::inheritance::inspect_resolved_capabilities;
use boternity_core::skill::registry::SkillRegistry;
use boternity_types::skill::{
    BotSkillConfig, BotSkillsFile, SkillManifest, SkillMeta, SkillSource, SkillType,
    TrustTier,
};

use crate::state::AppState;

/// Skill management subcommands.
#[derive(Subcommand)]
pub enum SkillCommand {
    /// Create a new local skill.
    Create {
        /// Skill name (slug format, e.g. "web-search").
        name: String,

        /// Skill type: prompt or tool.
        #[arg(long, default_value = "prompt")]
        r#type: String,
    },

    /// Install a skill from a registry.
    Install {
        /// Source identifier (owner/repo or skill name to search).
        source: String,

        /// Specific skill name within the repository.
        #[arg(long)]
        skill: Option<String>,

        /// Bot slug to attach the skill to after installation.
        #[arg(long)]
        bot: Option<String>,
    },

    /// Remove an installed skill.
    Remove {
        /// Skill name to remove.
        name: String,

        /// Bot slug to detach only (does not delete the skill files).
        #[arg(long)]
        bot: Option<String>,
    },

    /// List installed skills.
    List {
        /// Filter by bot slug (shows only skills attached to this bot).
        #[arg(long)]
        bot: Option<String>,
    },

    /// Inspect a skill, showing manifest and resolved capabilities.
    Inspect {
        /// Skill name to inspect.
        name: String,
    },

    /// Attach an installed skill to a bot.
    Attach {
        /// Skill name to attach.
        name: String,

        /// Bot slug to attach the skill to.
        #[arg(long)]
        bot: String,
    },

    /// Detach a skill from a bot (does not uninstall).
    Detach {
        /// Skill name to detach.
        name: String,

        /// Bot slug to detach from.
        #[arg(long)]
        bot: String,
    },

    /// Enable a skill for a bot.
    Enable {
        /// Skill name to enable.
        name: String,

        /// Bot slug.
        #[arg(long)]
        bot: String,
    },

    /// Disable a skill for a bot.
    Disable {
        /// Skill name to disable.
        name: String,

        /// Bot slug.
        #[arg(long)]
        bot: String,
    },

    /// Validate a skill for publishing (prints instructions).
    Publish {
        /// Skill name to validate.
        name: String,
    },

    /// Launch the interactive TUI skill browser.
    Browse,

    /// Interactive LLM-powered skill builder wizard.
    Generate,

    /// Check for skill updates.
    Update {
        /// Update all skills (otherwise specify a name).
        #[arg(long)]
        all: bool,

        /// Specific skill name to update.
        name: Option<String>,
    },
}

/// Handle a skill subcommand.
pub async fn handle_skill_command(
    cmd: SkillCommand,
    state: &AppState,
    json: bool,
) -> Result<()> {
    match cmd {
        SkillCommand::Create { name, r#type } => {
            handle_create(&name, &r#type, state, json)?;
        }
        SkillCommand::Install { source, skill, bot } => {
            handle_install(&source, skill.as_deref(), bot.as_deref(), state, json).await?;
        }
        SkillCommand::Remove { name, bot } => {
            handle_remove(&name, bot.as_deref(), state, json)?;
        }
        SkillCommand::List { bot } => {
            handle_list(bot.as_deref(), state, json)?;
        }
        SkillCommand::Inspect { name } => {
            handle_inspect(&name, state, json)?;
        }
        SkillCommand::Attach { name, bot } => {
            handle_attach(&name, &bot, state, json)?;
        }
        SkillCommand::Detach { name, bot } => {
            handle_detach(&name, &bot, state, json)?;
        }
        SkillCommand::Enable { name, bot } => {
            handle_enable_disable(&name, &bot, true, state, json)?;
        }
        SkillCommand::Disable { name, bot } => {
            handle_enable_disable(&name, &bot, false, state, json)?;
        }
        SkillCommand::Publish { name } => {
            handle_publish(&name, state, json)?;
        }
        SkillCommand::Generate => {
            super::skill_create::run_skill_create(state).await?;
        }
        SkillCommand::Browse => {
            handle_browse(state).await?;
        }
        SkillCommand::Update { all, name } => {
            handle_update(all, name.as_deref(), state, json)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

fn handle_create(name: &str, skill_type: &str, state: &AppState, json: bool) -> Result<()> {
    // Validate name
    validate_skill_name(name)?;

    // Parse skill type
    let st = match skill_type {
        "prompt" => SkillType::Prompt,
        "tool" => SkillType::Tool,
        other => bail!("Invalid skill type '{}'. Use 'prompt' or 'tool'.", other),
    };

    // Check if already exists
    if state.skill_store.skill_exists(name) {
        bail!("Skill '{}' already exists.", name);
    }

    // Generate SKILL.md content
    let skill_md = generate_skill_md(name, &st);

    // Install to disk (local, no meta, no WASM)
    let install_path = state.skill_store.install_skill(name, &skill_md, None, None)?;

    // For tool skills, create a scripts/ directory with a placeholder
    if matches!(st, SkillType::Tool) {
        let scripts_dir = install_path.join("scripts");
        std::fs::create_dir_all(&scripts_dir)
            .with_context(|| format!("Failed to create scripts dir: {}", scripts_dir.display()))?;

        let placeholder = scripts_dir.join("run.sh");
        std::fs::write(
            &placeholder,
            "#!/bin/bash\n# Tool execution script for skill\necho '{\"result\": \"not implemented\"}'\n",
        )?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&placeholder, std::fs::Permissions::from_mode(0o755))?;
        }
    }

    if json {
        let out = serde_json::json!({
            "name": name,
            "type": skill_type,
            "path": install_path.display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!();
        println!(
            "  {} Created skill '{}'",
            style("*").green().bold(),
            style(name).cyan()
        );
        println!("  Type: {skill_type}");
        println!("  Path: {}", install_path.display());
        println!();
        println!("  Edit {} to define your skill.", style("SKILL.md").yellow());
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

async fn handle_install(
    source: &str,
    skill_name: Option<&str>,
    bot_slug: Option<&str>,
    state: &AppState,
    json: bool,
) -> Result<()> {
    use boternity_infra::skill::registry_client::{default_registry_configs, GitHubRegistryClient};

    let cache_dir = state.data_dir.join("cache").join("registries");

    // Build registry clients from default configs
    let configs = default_registry_configs();
    let mut clients: Vec<GitHubRegistryClient> = Vec::new();

    for config in &configs {
        if !config.enabled {
            continue;
        }
        if let boternity_core::skill::registry::RegistryType::GitHub { ref owner, ref repo } =
            config.registry_type
        {
            clients.push(GitHubRegistryClient::new(
                owner.clone(),
                repo.clone(),
                config.name.clone(),
                cache_dir.clone(),
            ));
        }
    }

    // Search across registries
    let query = skill_name.unwrap_or(source);

    if !json {
        println!();
        println!(
            "  {} Searching registries for '{}'...",
            style("*").cyan(),
            style(query).yellow()
        );
    }

    let mut all_results = Vec::new();
    for client in &clients {
        match client.search(query, 10).await {
            Ok(results) => all_results.extend(results),
            Err(e) => {
                if !json {
                    println!(
                        "  {} Registry '{}' unavailable: {}",
                        style("!").yellow(),
                        client.name(),
                        e
                    );
                }
            }
        }
    }

    if all_results.is_empty() {
        if json {
            println!("{{\"error\": \"No skills found matching query\"}}");
        } else {
            println!("  No skills found matching '{query}'.");
        }
        return Ok(());
    }

    // If only one result, use it; otherwise let user pick
    let selected = if all_results.len() == 1 {
        all_results.remove(0)
    } else {
        if json {
            // In JSON mode, list results and let caller pick
            let out: Vec<_> = all_results
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    serde_json::json!({
                        "index": i,
                        "name": s.name,
                        "description": s.description,
                        "source": s.source,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(());
        }

        // Interactive selection
        println!();
        for (i, skill) in all_results.iter().enumerate() {
            println!(
                "  [{}] {} - {} ({})",
                style(i + 1).cyan(),
                style(&skill.name).bold(),
                &skill.description,
                style(&skill.source).dim(),
            );
        }
        println!();

        let selection: usize = dialoguer::Input::new()
            .with_prompt("  Select skill (number)")
            .interact_text()?;

        if selection == 0 || selection > all_results.len() {
            bail!("Invalid selection");
        }

        all_results.remove(selection - 1)
    };

    // Show capabilities and ask for approval
    let capabilities = selected
        .manifest
        .metadata
        .as_ref()
        .and_then(|m| m.capabilities.as_ref())
        .cloned()
        .unwrap_or_default();

    if !capabilities.is_empty() && !json {
        println!();
        println!(
            "  {} Skill '{}' requests the following capabilities:",
            style("!").yellow(),
            style(&selected.name).cyan()
        );
        println!();
        for cap in &capabilities {
            println!("    - {:?}", cap);
        }
        println!();

        let approved = Confirm::new()
            .with_prompt("  Approve these capabilities?")
            .default(false)
            .interact()?;

        if !approved {
            println!("  Installation cancelled.");
            return Ok(());
        }
    }

    // Fetch skill content
    if !json {
        println!("  Fetching skill content...");
    }

    // Find the registry client that matches the source
    let mut content_and_wasm = None;
    for client in &clients {
        if client.name() == selected.source {
            content_and_wasm = Some(client.fetch_skill(&selected).await?);
            break;
        }
    }

    let (content, wasm_bytes) = content_and_wasm
        .ok_or_else(|| anyhow::anyhow!("Could not find registry for source: {}", selected.source))?;

    // Build metadata
    let meta = SkillMeta {
        source: SkillSource::Registry {
            registry_name: selected.source.clone(),
            repo: format!("{}/{}", selected.source, selected.name),
            path: selected.path.clone(),
        },
        installed_at: chrono::Utc::now(),
        version: selected
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.version.as_ref())
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| "0.1.0".parse().unwrap()),
        checksum: "none".to_string(),
        trust_tier: TrustTier::Untrusted,
    };

    // Install to disk
    let install_path = state.skill_store.install_skill(
        &selected.name,
        &content,
        Some(meta),
        wasm_bytes.as_deref(),
    )?;

    // Ensure Tool-type skills have a WASM binary (pre-compiled or stub)
    let skill_type = selected
        .manifest
        .metadata
        .as_ref()
        .and_then(|m| m.skill_type.as_ref());

    if matches!(skill_type, Some(SkillType::Tool)) {
        use boternity_infra::skill::wasm_compiler;

        // Parse the skill body from content for stub generation
        let (_manifest, body) = boternity_core::skill::manifest::parse_skill_md(&content)
            .context("Failed to re-parse skill for WASM compilation")?;

        let wasm_path = wasm_compiler::ensure_wasm_binary(
            &install_path,
            &body,
            wasm_bytes.as_deref(),
        )
        .context("Failed to compile/generate WASM for Tool skill")?;

        if !json {
            println!(
                "  {} WASM binary at {}",
                style("*").green(),
                wasm_path.display()
            );
        }
    }

    if json {
        let out = serde_json::json!({
            "name": selected.name,
            "source": selected.source,
            "path": install_path.display().to_string(),
            "capabilities": capabilities,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!(
            "  {} Installed skill '{}' to {}",
            style("*").green().bold(),
            style(&selected.name).cyan(),
            install_path.display()
        );
    }

    // Auto-attach to bot if specified
    if let Some(slug) = bot_slug {
        handle_attach(&selected.name, slug, state, json)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

fn handle_remove(name: &str, bot_slug: Option<&str>, state: &AppState, json: bool) -> Result<()> {
    if let Some(slug) = bot_slug {
        // Detach only
        handle_detach(name, slug, state, json)?;
        return Ok(());
    }

    if !state.skill_store.skill_exists(name) {
        bail!("Skill '{}' not found.", name);
    }

    state.skill_store.remove_skill(name)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"removed": name})
        );
    } else {
        println!();
        println!(
            "  {} Removed skill '{}'",
            style("*").green().bold(),
            style(name).cyan()
        );
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

fn handle_list(bot_slug: Option<&str>, state: &AppState, json: bool) -> Result<()> {
    let all_skills = state.skill_store.list_skills()?;

    // If bot filter, get the bot's skill config
    let bot_skills: Option<BotSkillsFile> = if let Some(slug) = bot_slug {
        let bot_dir = state.data_dir.join("bots").join(slug);
        Some(state.skill_store.get_bot_skills_config(&bot_dir)?)
    } else {
        None
    };

    let skills_to_show: Vec<_> = if let Some(ref bot_config) = bot_skills {
        all_skills
            .iter()
            .filter(|s| bot_config.skills.contains_key(&s.manifest.name))
            .collect()
    } else {
        all_skills.iter().collect()
    };

    if json {
        let out: Vec<_> = skills_to_show
            .iter()
            .map(|s| {
                let tier = s
                    .manifest
                    .metadata
                    .as_ref()
                    .and_then(|m| m.trust_tier.as_ref())
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "local".to_string());

                let skill_type = s
                    .manifest
                    .metadata
                    .as_ref()
                    .and_then(|m| m.skill_type.as_ref())
                    .map(|t| format!("{:?}", t).to_lowercase())
                    .unwrap_or_else(|| "prompt".to_string());

                let version = s
                    .manifest
                    .metadata
                    .as_ref()
                    .and_then(|m| m.version.as_ref())
                    .cloned()
                    .unwrap_or_else(|| "-".to_string());

                let enabled = bot_skills
                    .as_ref()
                    .and_then(|bc| bc.skills.get(&s.manifest.name))
                    .map(|c| c.enabled)
                    .unwrap_or(true);

                serde_json::json!({
                    "name": s.manifest.name,
                    "type": skill_type,
                    "tier": tier,
                    "version": version,
                    "source": format!("{:?}", s.source),
                    "enabled": enabled,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if skills_to_show.is_empty() {
        println!();
        if bot_slug.is_some() {
            println!("  No skills attached to this bot.");
        } else {
            println!("  No skills installed. Use 'bnity skill install' to add skills.");
        }
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Name").fg(Color::Cyan),
            Cell::new("Type"),
            Cell::new("Tier"),
            Cell::new("Version"),
            Cell::new("Source"),
            Cell::new("Enabled"),
        ]);

    for s in &skills_to_show {
        let tier = s
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.trust_tier.as_ref())
            .map(|t| t.to_string())
            .unwrap_or_else(|| "local".to_string());

        let tier_color = match tier.as_str() {
            "local" => Color::Green,
            "verified" => Color::Yellow,
            _ => Color::Red,
        };

        let skill_type = s
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.skill_type.as_ref())
            .map(|t| format!("{:?}", t).to_lowercase())
            .unwrap_or_else(|| "prompt".to_string());

        let version = s
            .manifest
            .metadata
            .as_ref()
            .and_then(|m| m.version.as_ref())
            .cloned()
            .unwrap_or_else(|| "-".to_string());

        let source_str = match &s.source {
            SkillSource::Local => "local".to_string(),
            SkillSource::Registry { registry_name, .. } => registry_name.clone(),
        };

        let enabled = bot_skills
            .as_ref()
            .and_then(|bc| bc.skills.get(&s.manifest.name))
            .map(|c| c.enabled)
            .unwrap_or(true);

        table.add_row(vec![
            Cell::new(&s.manifest.name),
            Cell::new(&skill_type),
            Cell::new(&tier).fg(tier_color),
            Cell::new(&version),
            Cell::new(&source_str),
            Cell::new(if enabled { "yes" } else { "no" }),
        ]);
    }

    println!();
    println!("{table}");
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Inspect
// ---------------------------------------------------------------------------

fn handle_inspect(name: &str, state: &AppState, json: bool) -> Result<()> {
    let skill = state.skill_store.get_skill(name)?;

    // Build a manifest map for inheritance resolution
    let all_skills_list = state.skill_store.list_skills()?;
    let mut all_manifests: HashMap<String, SkillManifest> = HashMap::new();
    for s in &all_skills_list {
        all_manifests.insert(s.manifest.name.clone(), s.manifest.clone());
    }

    let inspected = inspect_resolved_capabilities(name, &all_manifests)?;

    if json {
        let out = serde_json::json!({
            "name": inspected.name,
            "description": skill.manifest.description,
            "own_capabilities": inspected.own_capabilities,
            "inherited_capabilities": inspected.inherited_capabilities,
            "combined_capabilities": inspected.combined_capabilities,
            "parent_chain": inspected.parent_chain,
            "conflicts_with": inspected.conflicts_with,
            "depth": inspected.depth,
            "source": format!("{:?}", skill.source),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!();
    println!(
        "  {} {}",
        style("Skill:").bold(),
        style(&inspected.name).cyan().bold()
    );
    println!("  {}", skill.manifest.description);
    println!();

    // Source info
    let source_str = match &skill.source {
        SkillSource::Local => "local".to_string(),
        SkillSource::Registry {
            registry_name,
            repo,
            ..
        } => format!("{registry_name} ({repo})"),
    };
    println!("  Source: {source_str}");

    // Tier
    let tier = skill
        .manifest
        .metadata
        .as_ref()
        .and_then(|m| m.trust_tier.as_ref())
        .map(|t| t.to_string())
        .unwrap_or_else(|| "local".to_string());
    println!("  Trust Tier: {tier}");

    // Type
    let skill_type = skill
        .manifest
        .metadata
        .as_ref()
        .and_then(|m| m.skill_type.as_ref())
        .map(|t| format!("{:?}", t).to_lowercase())
        .unwrap_or_else(|| "prompt".to_string());
    println!("  Type: {skill_type}");

    // Version
    let version = skill
        .manifest
        .metadata
        .as_ref()
        .and_then(|m| m.version.as_ref())
        .cloned()
        .unwrap_or_else(|| "-".to_string());
    println!("  Version: {version}");
    println!();

    // Capabilities
    if !inspected.own_capabilities.is_empty() {
        println!("  {} Own Capabilities:", style("*").green());
        for cap in &inspected.own_capabilities {
            println!("    - {:?}", cap);
        }
    }

    if !inspected.inherited_capabilities.is_empty() {
        println!("  {} Inherited Capabilities:", style("*").yellow());
        for cap in &inspected.inherited_capabilities {
            println!("    - {:?}", cap);
        }
    }

    if !inspected.combined_capabilities.is_empty() {
        println!("  {} Combined Capabilities:", style("*").cyan());
        for cap in &inspected.combined_capabilities {
            println!("    - {:?}", cap);
        }
    } else {
        println!("  No capabilities declared.");
    }

    // Parent chain
    if !inspected.parent_chain.is_empty() {
        println!();
        println!(
            "  Parent chain: {} (depth {})",
            inspected.parent_chain.join(" -> "),
            inspected.depth
        );
    }

    // Conflicts
    if !inspected.conflicts_with.is_empty() {
        println!();
        println!(
            "  {} Conflicts with: {}",
            style("!").red(),
            inspected.conflicts_with.join(", ")
        );
    }

    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Attach / Detach
// ---------------------------------------------------------------------------

fn handle_attach(name: &str, bot_slug: &str, state: &AppState, json: bool) -> Result<()> {
    if !state.skill_store.skill_exists(name) {
        bail!("Skill '{}' not found. Install it first.", name);
    }

    let bot_dir = state.data_dir.join("bots").join(bot_slug);
    let mut config = state.skill_store.get_bot_skills_config(&bot_dir)?;

    if config.skills.contains_key(name) {
        if !json {
            println!(
                "  Skill '{}' is already attached to bot '{}'.",
                name, bot_slug
            );
        }
        return Ok(());
    }

    let skill = state.skill_store.get_skill(name)?;
    let capabilities = skill
        .manifest
        .metadata
        .as_ref()
        .and_then(|m| m.capabilities.clone());

    let tier = skill
        .manifest
        .metadata
        .as_ref()
        .and_then(|m| m.trust_tier.clone());

    let version = skill
        .manifest
        .metadata
        .as_ref()
        .and_then(|m| m.version.clone());

    config.skills.insert(
        name.to_string(),
        BotSkillConfig {
            skill_name: name.to_string(),
            enabled: true,
            trust_tier: tier,
            version,
            overrides: HashMap::new(),
            capabilities,
        },
    );

    state.skill_store.save_bot_skills_config(&bot_dir, &config)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"attached": name, "bot": bot_slug})
        );
    } else {
        println!();
        println!(
            "  {} Attached skill '{}' to bot '{}'",
            style("*").green().bold(),
            style(name).cyan(),
            style(bot_slug).cyan()
        );
        println!();
    }

    Ok(())
}

fn handle_detach(name: &str, bot_slug: &str, state: &AppState, json: bool) -> Result<()> {
    let bot_dir = state.data_dir.join("bots").join(bot_slug);
    let mut config = state.skill_store.get_bot_skills_config(&bot_dir)?;

    if config.skills.remove(name).is_none() {
        if !json {
            println!(
                "  Skill '{}' was not attached to bot '{}'.",
                name, bot_slug
            );
        }
        return Ok(());
    }

    state.skill_store.save_bot_skills_config(&bot_dir, &config)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"detached": name, "bot": bot_slug})
        );
    } else {
        println!();
        println!(
            "  {} Detached skill '{}' from bot '{}'",
            style("*").green().bold(),
            style(name).cyan(),
            style(bot_slug).cyan()
        );
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Enable / Disable
// ---------------------------------------------------------------------------

fn handle_enable_disable(
    name: &str,
    bot_slug: &str,
    enable: bool,
    state: &AppState,
    json: bool,
) -> Result<()> {
    let bot_dir = state.data_dir.join("bots").join(bot_slug);
    let mut config = state.skill_store.get_bot_skills_config(&bot_dir)?;

    let entry = config.skills.get_mut(name).ok_or_else(|| {
        anyhow::anyhow!(
            "Skill '{}' is not attached to bot '{}'. Use 'bnity skill attach' first.",
            name,
            bot_slug
        )
    })?;

    entry.enabled = enable;
    state.skill_store.save_bot_skills_config(&bot_dir, &config)?;

    let action = if enable { "Enabled" } else { "Disabled" };

    if json {
        println!(
            "{}",
            serde_json::json!({"skill": name, "bot": bot_slug, "enabled": enable})
        );
    } else {
        println!();
        println!(
            "  {} {} skill '{}' for bot '{}'",
            style("*").green().bold(),
            action,
            style(name).cyan(),
            style(bot_slug).cyan()
        );
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Publish
// ---------------------------------------------------------------------------

fn handle_publish(name: &str, state: &AppState, json: bool) -> Result<()> {
    let skill = state.skill_store.get_skill(name)?;

    // Validate the skill manifest
    let manifest = &skill.manifest;
    let mut issues: Vec<String> = Vec::new();

    if manifest.description.is_empty() {
        issues.push("Missing description".to_string());
    }

    let meta = manifest.metadata.as_ref();
    if meta.is_none() {
        issues.push("Missing metadata section".to_string());
    } else {
        let m = meta.unwrap();
        if m.author.is_none() {
            issues.push("Missing metadata.author".to_string());
        }
        if m.version.is_none() {
            issues.push("Missing metadata.version".to_string());
        }
    }

    if json {
        let out = serde_json::json!({
            "name": name,
            "valid": issues.is_empty(),
            "issues": issues,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!();
    if issues.is_empty() {
        println!(
            "  {} Skill '{}' passes validation.",
            style("*").green().bold(),
            style(name).cyan()
        );
        println!();
        println!("  To publish:");
        println!("  1. Create a GitHub repository with your skill directory.");
        println!("  2. Ensure SKILL.md is in the root (or a subdirectory).");
        println!("  3. Submit a PR to a skill registry (e.g., ComposioHQ/awesome-claude-skills).");
        println!(
            "  4. Or publish to skills.sh: https://skills.sh/publish"
        );
    } else {
        println!(
            "  {} Skill '{}' has validation issues:",
            style("!").yellow(),
            style(name).cyan()
        );
        println!();
        for issue in &issues {
            println!("    - {issue}");
        }
        println!();
        println!("  Fix these issues before publishing.");
    }
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Browse
// ---------------------------------------------------------------------------

async fn handle_browse(state: &AppState) -> Result<()> {
    use boternity_infra::skill::registry_client::{default_registry_configs, GitHubRegistryClient};

    let cache_dir = state.data_dir.join("cache").join("registries");

    let configs = default_registry_configs();
    let mut clients: Vec<GitHubRegistryClient> = Vec::new();

    for config in &configs {
        if !config.enabled {
            continue;
        }
        if let boternity_core::skill::registry::RegistryType::GitHub { ref owner, ref repo } =
            config.registry_type
        {
            clients.push(GitHubRegistryClient::new(
                owner.clone(),
                repo.clone(),
                config.name.clone(),
                cache_dir.clone(),
            ));
        }
    }

    let selected = super::skill_browser::run_browser(&clients, &cache_dir).await?;

    if let Some(skill) = selected {
        println!();
        println!(
            "  Selected: {} (from {})",
            style(&skill.name).cyan().bold(),
            style(&skill.source).dim()
        );
        println!();

        let install = Confirm::new()
            .with_prompt("  Install this skill?")
            .default(true)
            .interact()?;

        if install {
            handle_install(&skill.name, Some(&skill.name), None, state, false).await?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

fn handle_update(all: bool, name: Option<&str>, state: &AppState, json: bool) -> Result<()> {
    let skills = state.skill_store.list_skills()?;

    let to_check: Vec<_> = if all {
        skills
            .iter()
            .filter(|s| matches!(s.source, SkillSource::Registry { .. }))
            .collect()
    } else if let Some(n) = name {
        skills.iter().filter(|s| s.manifest.name == n).collect()
    } else {
        bail!("Specify --all or a skill name to update.");
    };

    if to_check.is_empty() {
        if json {
            println!("[]");
        } else {
            println!();
            println!("  No registry-installed skills to update.");
            println!();
        }
        return Ok(());
    }

    // For now, report current versions (actual remote check would require async)
    if json {
        let out: Vec<_> = to_check
            .iter()
            .map(|s| {
                let version = s
                    .manifest
                    .metadata
                    .as_ref()
                    .and_then(|m| m.version.as_ref())
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());

                serde_json::json!({
                    "name": s.manifest.name,
                    "current_version": version,
                    "status": "up-to-date",
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!();
        for s in &to_check {
            let version = s
                .manifest
                .metadata
                .as_ref()
                .and_then(|m| m.version.as_ref())
                .cloned()
                .unwrap_or_else(|| "?".to_string());

            println!(
                "  {} {} v{} - up to date",
                style("*").green(),
                style(&s.manifest.name).cyan(),
                version
            );
        }
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Validate skill name: lowercase alphanumeric + hyphens, no leading/trailing hyphens.
fn validate_skill_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Skill name cannot be empty.");
    }
    if name.starts_with('-') || name.ends_with('-') {
        bail!("Skill name cannot start or end with a hyphen.");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        bail!(
            "Skill name must contain only lowercase letters, digits, and hyphens."
        );
    }
    Ok(())
}

/// Generate SKILL.md content for a new local skill.
fn generate_skill_md(name: &str, skill_type: &SkillType) -> String {
    let type_str = match skill_type {
        SkillType::Prompt => "prompt",
        SkillType::Tool => "tool",
    };

    format!(
        r#"---
name: {name}
description: A new {type_str} skill
metadata:
  author: local
  version: "0.1.0"
  skill-type: {type_str}
  capabilities: []
---

# {name}

Describe what this skill does and how the agent should use it.
"#
    )
}
