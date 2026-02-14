//! CLI workflow management subcommands.
//!
//! Provides create, trigger, list, status, logs, delete, approve, and cancel
//! operations for workflow definitions and runs.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use comfy_table::{presets, Cell, Color, ContentArrangement, Table};
use console::style;

use boternity_core::repository::workflow::WorkflowRepository;
use boternity_core::workflow::definition::{load_workflow_file, WorkflowError};
use boternity_types::workflow::{WorkflowOwner, WorkflowRunStatus};

use crate::state::AppState;

/// Workflow management subcommands.
#[derive(Subcommand)]
pub enum WorkflowCommand {
    /// Create (register) a workflow from a YAML file.
    Create {
        /// Path to the workflow YAML file.
        file: PathBuf,

        /// Bot slug to assign as owner (omit for global).
        #[arg(long)]
        bot: Option<String>,
    },

    /// Trigger a workflow run manually.
    Trigger {
        /// Workflow name.
        name: String,

        /// Bot slug that owns the workflow (omit for global).
        #[arg(long)]
        bot: Option<String>,

        /// Optional JSON payload for the trigger.
        #[arg(long)]
        payload: Option<String>,
    },

    /// List registered workflows.
    List {
        /// Filter by bot slug (omit for all).
        #[arg(long)]
        bot: Option<String>,
    },

    /// Show recent runs for a workflow (by name or run ID).
    Status {
        /// Workflow name or run UUID.
        target: String,

        /// Maximum number of runs to display.
        #[arg(long, default_value = "10")]
        limit: u32,
    },

    /// Show step logs for a specific workflow run.
    Logs {
        /// Workflow run UUID.
        run_id: String,
    },

    /// Delete a registered workflow.
    Delete {
        /// Workflow name.
        name: String,

        /// Bot slug that owns the workflow (omit for global).
        #[arg(long)]
        bot: Option<String>,
    },

    /// Approve a paused approval step in a workflow run.
    Approve {
        /// Workflow run UUID.
        run_id: String,
    },

    /// Cancel a running workflow.
    Cancel {
        /// Workflow run UUID.
        run_id: String,
    },
}

/// Handle a workflow subcommand.
pub async fn handle_workflow_command(
    cmd: WorkflowCommand,
    state: &AppState,
    json: bool,
) -> Result<()> {
    // Lazily create the workflow repository from the database pool.
    let repo = boternity_infra::sqlite::workflow::SqliteWorkflowRepository::new(
        state.db_pool.clone(),
    );

    match cmd {
        WorkflowCommand::Create { file, bot } => {
            handle_create(&file, bot.as_deref(), state, &repo, json).await
        }
        WorkflowCommand::Trigger { name, bot, payload } => {
            handle_trigger(&name, bot.as_deref(), payload.as_deref(), state, &repo, json).await
        }
        WorkflowCommand::List { bot } => handle_list(bot.as_deref(), state, &repo, json).await,
        WorkflowCommand::Status { target, limit } => {
            handle_status(&target, limit, &repo, json).await
        }
        WorkflowCommand::Logs { run_id } => handle_logs(&run_id, &repo, json).await,
        WorkflowCommand::Delete { name, bot } => {
            handle_delete(&name, bot.as_deref(), state, &repo, json).await
        }
        WorkflowCommand::Approve { run_id } => handle_approve(&run_id, &repo, json).await,
        WorkflowCommand::Cancel { run_id } => handle_cancel(&run_id, &repo, json).await,
    }
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

async fn handle_create(
    file: &PathBuf,
    bot_slug: Option<&str>,
    state: &AppState,
    repo: &impl WorkflowRepository,
    json: bool,
) -> Result<()> {
    // Parse and validate the YAML file
    let mut def = load_workflow_file(file).map_err(|e| match e {
        WorkflowError::ParseError(msg) => anyhow::anyhow!("Failed to parse workflow YAML: {msg}"),
        WorkflowError::ValidationError(msg) => anyhow::anyhow!("Workflow validation failed: {msg}"),
        other => anyhow::anyhow!("Failed to load workflow: {other}"),
    })?;

    // Set owner
    if let Some(slug) = bot_slug {
        let bot = state
            .bot_service
            .get_bot_by_slug(slug)
            .await
            .with_context(|| format!("Bot '{slug}' not found"))?;
        def.owner = WorkflowOwner::Bot {
            bot_id: bot.id.0,
            slug: bot.slug.clone(),
        };
    } else {
        def.owner = WorkflowOwner::Global;
    }

    // Save to repository
    repo.save_definition(&def)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to save workflow: {e}"))?;

    if json {
        let out = serde_json::json!({
            "id": def.id.to_string(),
            "name": def.name,
            "steps": def.steps.len(),
            "triggers": def.triggers.len(),
            "owner": format!("{:?}", def.owner),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!();
        println!(
            "  {} Created workflow '{}'",
            style("*").green().bold(),
            style(&def.name).cyan()
        );
        println!("  ID: {}", def.id);
        println!("  Steps: {}", def.steps.len());
        println!("  Triggers: {}", def.triggers.len());
        if let Some(slug) = bot_slug {
            println!("  Owner: bot '{}'", style(slug).cyan());
        } else {
            println!("  Owner: global");
        }
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Trigger
// ---------------------------------------------------------------------------

async fn handle_trigger(
    name: &str,
    bot_slug: Option<&str>,
    payload_str: Option<&str>,
    state: &AppState,
    repo: &impl WorkflowRepository,
    json: bool,
) -> Result<()> {
    // Resolve owner
    let owner = resolve_owner(bot_slug, state).await?;

    // Find the workflow definition
    let def = repo
        .get_definition_by_name(name, &owner)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to look up workflow: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("Workflow '{}' not found", name))?;

    // Parse optional payload
    let trigger_payload = if let Some(raw) = payload_str {
        Some(
            serde_json::from_str::<serde_json::Value>(raw)
                .with_context(|| "Invalid JSON payload")?,
        )
    } else {
        None
    };

    // Create a pending workflow run record
    let run = boternity_types::workflow::WorkflowRun {
        id: uuid::Uuid::now_v7(),
        workflow_id: def.id,
        workflow_name: def.name.clone(),
        status: WorkflowRunStatus::Pending,
        trigger_type: "manual".to_string(),
        trigger_payload,
        context: serde_json::json!({}),
        started_at: chrono::Utc::now(),
        completed_at: None,
        error: None,
        concurrency_key: Some(def.name.clone()),
    };

    repo.create_run(&run)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create workflow run: {e}"))?;

    if json {
        let out = serde_json::json!({
            "run_id": run.id.to_string(),
            "workflow_name": def.name,
            "status": "pending",
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!();
        println!(
            "  {} Triggered workflow '{}'",
            style("*").green().bold(),
            style(&def.name).cyan()
        );
        println!("  Run ID: {}", run.id);
        println!("  Status: pending");
        println!();
        println!(
            "  Check progress: {}",
            style(format!("bnity workflow status {}", run.id)).dim()
        );
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

async fn handle_list(
    bot_slug: Option<&str>,
    state: &AppState,
    repo: &impl WorkflowRepository,
    json: bool,
) -> Result<()> {
    let owner_filter = if let Some(slug) = bot_slug {
        Some(resolve_owner(Some(slug), state).await?)
    } else {
        None
    };

    let defs = repo
        .list_definitions(owner_filter.as_ref())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list workflows: {e}"))?;

    if json {
        let out: Vec<_> = defs
            .iter()
            .map(|d| {
                serde_json::json!({
                    "id": d.id.to_string(),
                    "name": d.name,
                    "version": d.version,
                    "steps": d.steps.len(),
                    "triggers": d.triggers.len(),
                    "owner": format!("{:?}", d.owner),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if defs.is_empty() {
        println!();
        println!("  No workflows registered.");
        println!(
            "  Create one with: {}",
            style("bnity workflow create <file.yaml>").dim()
        );
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Name").fg(Color::Cyan),
            Cell::new("Version"),
            Cell::new("Steps"),
            Cell::new("Triggers"),
            Cell::new("Owner"),
        ]);

    for d in &defs {
        let owner_str = match &d.owner {
            WorkflowOwner::Bot { slug, .. } => format!("bot:{slug}"),
            WorkflowOwner::Global => "global".to_string(),
        };

        table.add_row(vec![
            Cell::new(&d.name),
            Cell::new(&d.version),
            Cell::new(d.steps.len()),
            Cell::new(d.triggers.len()),
            Cell::new(&owner_str),
        ]);
    }

    println!();
    println!("{table}");
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

async fn handle_status(
    target: &str,
    limit: u32,
    repo: &impl WorkflowRepository,
    json: bool,
) -> Result<()> {
    // Try parsing as a UUID first (specific run)
    if let Ok(run_id) = target.parse::<uuid::Uuid>() {
        if let Some(run) = repo
            .get_run(&run_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get run: {e}"))?
        {
            return display_single_run(&run, json);
        }
    }

    // Otherwise treat as workflow name -- look up definition by name (global first)
    let owner = WorkflowOwner::Global;
    let def = repo
        .get_definition_by_name(target, &owner)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to look up workflow: {e}"))?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No workflow or run found for '{}'. Try a UUID or workflow name.",
                target
            )
        })?;

    let runs = repo
        .list_runs(&def.id, limit)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list runs: {e}"))?;

    if json {
        let out: Vec<_> = runs
            .iter()
            .map(|r| {
                serde_json::json!({
                    "run_id": r.id.to_string(),
                    "status": format!("{:?}", r.status),
                    "trigger": r.trigger_type,
                    "started_at": r.started_at.to_rfc3339(),
                    "completed_at": r.completed_at.map(|t| t.to_rfc3339()),
                    "error": r.error,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if runs.is_empty() {
        println!();
        println!("  No runs for workflow '{target}'.");
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Run ID").fg(Color::Cyan),
            Cell::new("Status"),
            Cell::new("Trigger"),
            Cell::new("Started"),
            Cell::new("Completed"),
        ]);

    for r in &runs {
        let status_cell = format_status(r.status);
        let completed = r
            .completed_at
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "-".to_string());

        table.add_row(vec![
            Cell::new(r.id.to_string().chars().take(8).collect::<String>()),
            status_cell,
            Cell::new(&r.trigger_type),
            Cell::new(r.started_at.format("%Y-%m-%d %H:%M").to_string()),
            Cell::new(completed),
        ]);
    }

    println!();
    println!(
        "  Runs for workflow '{}'",
        style(target).cyan()
    );
    println!();
    println!("{table}");
    println!();

    Ok(())
}

fn display_single_run(
    run: &boternity_types::workflow::WorkflowRun,
    json: bool,
) -> Result<()> {
    if json {
        let out = serde_json::json!({
            "run_id": run.id.to_string(),
            "workflow_id": run.workflow_id.to_string(),
            "workflow_name": run.workflow_name,
            "status": format!("{:?}", run.status),
            "trigger": run.trigger_type,
            "started_at": run.started_at.to_rfc3339(),
            "completed_at": run.completed_at.map(|t| t.to_rfc3339()),
            "error": run.error,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!();
    println!(
        "  {} Run {}",
        style("Workflow:").bold(),
        style(run.id.to_string().chars().take(8).collect::<String>()).cyan()
    );
    println!("  Workflow: {}", style(&run.workflow_name).cyan());
    println!("  Status: {:?}", run.status);
    println!("  Trigger: {}", run.trigger_type);
    println!("  Started: {}", run.started_at.format("%Y-%m-%d %H:%M:%S"));
    if let Some(completed) = run.completed_at {
        println!("  Completed: {}", completed.format("%Y-%m-%d %H:%M:%S"));
    }
    if let Some(ref err) = run.error {
        println!("  Error: {}", style(err).red());
    }
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

async fn handle_logs(
    run_id_str: &str,
    repo: &impl WorkflowRepository,
    json: bool,
) -> Result<()> {
    let run_id: uuid::Uuid = run_id_str
        .parse()
        .with_context(|| format!("Invalid run ID: '{run_id_str}'"))?;

    let steps = repo
        .list_step_logs(&run_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list step logs: {e}"))?;

    if json {
        let out: Vec<_> = steps
            .iter()
            .map(|s| {
                serde_json::json!({
                    "step_id": s.step_id,
                    "step_name": s.step_name,
                    "status": format!("{:?}", s.status),
                    "attempt": s.attempt,
                    "started_at": s.started_at.map(|t| t.to_rfc3339()),
                    "completed_at": s.completed_at.map(|t| t.to_rfc3339()),
                    "error": s.error,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if steps.is_empty() {
        println!();
        println!("  No step logs for run '{}'.", &run_id_str[..8.min(run_id_str.len())]);
        println!();
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Step").fg(Color::Cyan),
            Cell::new("Name"),
            Cell::new("Status"),
            Cell::new("Attempt"),
            Cell::new("Started"),
            Cell::new("Error"),
        ]);

    for s in &steps {
        let started = s
            .started_at
            .map(|t| t.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "-".to_string());
        let error = s
            .error
            .as_ref()
            .map(|e| e.chars().take(40).collect::<String>())
            .unwrap_or_else(|| "-".to_string());

        table.add_row(vec![
            Cell::new(&s.step_id),
            Cell::new(&s.step_name),
            Cell::new(format!("{:?}", s.status)),
            Cell::new(s.attempt),
            Cell::new(started),
            Cell::new(error),
        ]);
    }

    println!();
    println!(
        "  Step logs for run '{}'",
        style(&run_id_str[..8.min(run_id_str.len())]).cyan()
    );
    println!();
    println!("{table}");
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

async fn handle_delete(
    name: &str,
    bot_slug: Option<&str>,
    state: &AppState,
    repo: &impl WorkflowRepository,
    json: bool,
) -> Result<()> {
    let owner = resolve_owner(bot_slug, state).await?;

    let def = repo
        .get_definition_by_name(name, &owner)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to look up workflow: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("Workflow '{}' not found", name))?;

    let deleted = repo
        .delete_definition(&def.id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete workflow: {e}"))?;

    if !deleted {
        bail!("Workflow '{}' could not be deleted (not found in repository)", name);
    }

    if json {
        println!(
            "{}",
            serde_json::json!({"deleted": name, "id": def.id.to_string()})
        );
    } else {
        println!();
        println!(
            "  {} Deleted workflow '{}'",
            style("*").green().bold(),
            style(name).cyan()
        );
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Approve
// ---------------------------------------------------------------------------

async fn handle_approve(
    run_id_str: &str,
    repo: &impl WorkflowRepository,
    json: bool,
) -> Result<()> {
    let run_id: uuid::Uuid = run_id_str
        .parse()
        .with_context(|| format!("Invalid run ID: '{run_id_str}'"))?;

    let run = repo
        .get_run(&run_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get run: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("Run '{}' not found", run_id_str))?;

    if run.status != WorkflowRunStatus::Paused {
        bail!(
            "Run is not paused (current status: {:?}). Only paused runs can be approved.",
            run.status
        );
    }

    // Resume by setting status back to Running
    repo.update_run_status(&run_id, WorkflowRunStatus::Running, None, None)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to approve run: {e}"))?;

    if json {
        println!(
            "{}",
            serde_json::json!({"approved": run_id_str, "status": "running"})
        );
    } else {
        println!();
        println!(
            "  {} Approved run '{}' -- status changed to running",
            style("*").green().bold(),
            style(&run_id_str[..8.min(run_id_str.len())]).cyan()
        );
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Cancel
// ---------------------------------------------------------------------------

async fn handle_cancel(
    run_id_str: &str,
    repo: &impl WorkflowRepository,
    json: bool,
) -> Result<()> {
    let run_id: uuid::Uuid = run_id_str
        .parse()
        .with_context(|| format!("Invalid run ID: '{run_id_str}'"))?;

    let run = repo
        .get_run(&run_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get run: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("Run '{}' not found", run_id_str))?;

    if run.status == WorkflowRunStatus::Completed
        || run.status == WorkflowRunStatus::Cancelled
        || run.status == WorkflowRunStatus::Failed
    {
        bail!(
            "Run has already finished (status: {:?}). Cannot cancel.",
            run.status
        );
    }

    repo.update_run_status(
        &run_id,
        WorkflowRunStatus::Cancelled,
        Some("Cancelled via CLI"),
        None,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to cancel run: {e}"))?;

    if json {
        println!(
            "{}",
            serde_json::json!({"cancelled": run_id_str, "status": "cancelled"})
        );
    } else {
        println!();
        println!(
            "  {} Cancelled run '{}'",
            style("*").green().bold(),
            style(&run_id_str[..8.min(run_id_str.len())]).cyan()
        );
        println!();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn resolve_owner(bot_slug: Option<&str>, state: &AppState) -> Result<WorkflowOwner> {
    if let Some(slug) = bot_slug {
        let bot = state
            .bot_service
            .get_bot_by_slug(slug)
            .await
            .with_context(|| format!("Bot '{slug}' not found"))?;
        Ok(WorkflowOwner::Bot {
            bot_id: bot.id.0,
            slug: bot.slug.clone(),
        })
    } else {
        Ok(WorkflowOwner::Global)
    }
}

fn format_status(status: WorkflowRunStatus) -> Cell {
    match status {
        WorkflowRunStatus::Pending => Cell::new("pending").fg(Color::Yellow),
        WorkflowRunStatus::Running => Cell::new("running").fg(Color::Blue),
        WorkflowRunStatus::Paused => Cell::new("paused").fg(Color::Magenta),
        WorkflowRunStatus::Completed => Cell::new("completed").fg(Color::Green),
        WorkflowRunStatus::Failed => Cell::new("failed").fg(Color::Red),
        WorkflowRunStatus::Crashed => Cell::new("crashed").fg(Color::Red),
        WorkflowRunStatus::Cancelled => Cell::new("cancelled").fg(Color::DarkYellow),
    }
}
