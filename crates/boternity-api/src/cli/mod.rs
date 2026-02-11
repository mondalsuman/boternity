//! CLI command definitions and dispatch for the `bnity` binary.
//!
//! Uses clap derive macros for argument parsing. The CLI follows a verb-noun
//! pattern (e.g., `bnity create bot`, `bnity list bots`).

pub mod bot;
pub mod chat;
pub mod secret;
pub mod session;
pub mod soul;
pub mod status;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

/// Manage your AI bot fleet.
#[derive(Parser)]
#[command(name = "bnity", version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Output machine-readable JSON instead of styled text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress all output except errors.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Detailed output (-v for verbose, -vv for debug/trace).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new resource.
    Create {
        #[command(subcommand)]
        resource: CreateResource,
    },

    /// List resources.
    #[command(alias = "ls")]
    List {
        #[command(subcommand)]
        resource: ListResource,
    },

    /// Show details of a bot.
    Show {
        /// Bot slug to display.
        slug: String,
    },

    /// Delete a resource.
    #[command(alias = "rm")]
    Delete {
        #[command(subcommand)]
        resource: DeleteResource,
    },

    /// Clone an existing bot (copies soul + config, not history).
    Clone {
        #[command(subcommand)]
        resource: CloneResource,
    },

    /// Set a secret value.
    Set {
        #[command(subcommand)]
        resource: SetResource,
    },

    /// Soul management (edit, history, diff, rollback, verify).
    Soul {
        #[command(subcommand)]
        action: SoulCommand,
    },

    /// System health check for a bot.
    Check {
        /// Bot slug to check.
        slug: String,
    },

    /// System status dashboard.
    Status,

    /// Start the REST API server.
    Serve {
        /// Port to listen on.
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Host to bind to.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Generate shell completions.
    Completions {
        /// Shell to generate completions for.
        shell: Shell,
    },

    // --- Short aliases ---
    /// Create a new bot (alias for `create bot`).
    #[command(name = "new", hide = true)]
    NewBot {
        /// Bot name.
        #[arg(long)]
        name: Option<String>,

        /// Short description.
        #[arg(long)]
        description: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum CreateResource {
    /// Create a new bot.
    Bot {
        /// Bot name (skips interactive wizard if provided).
        #[arg(long)]
        name: Option<String>,

        /// Short description.
        #[arg(long)]
        description: Option<String>,

        /// Category (assistant, creative, research, utility).
        #[arg(long)]
        category: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ListResource {
    /// List all bots.
    Bots {
        /// Filter by status (active, disabled, archived).
        #[arg(long)]
        status: Option<String>,

        /// Sort by field (name, created_at, last_active_at).
        #[arg(long, default_value = "created_at")]
        sort: String,
    },

    /// List stored secrets (masked).
    Secrets,
}

#[derive(Subcommand)]
pub enum DeleteResource {
    /// Delete a bot permanently.
    Bot {
        /// Bot slug to delete.
        slug: String,

        /// Skip confirmation prompt.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum CloneResource {
    /// Clone an existing bot.
    Bot {
        /// Slug of the bot to clone.
        slug: String,
    },
}

#[derive(Subcommand)]
pub enum SetResource {
    /// Set a secret value (prompted securely).
    Secret {
        /// Secret key name (e.g., ANTHROPIC_API_KEY).
        key: String,

        /// Secret value (optional; prompts if omitted for security).
        #[arg(long)]
        value: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum SoulCommand {
    /// Open SOUL.md in $EDITOR for editing (creates a new version).
    Edit {
        /// Bot slug.
        slug: String,
    },

    /// Show version history of a bot's soul.
    History {
        /// Bot slug.
        slug: String,
    },

    /// Show line-by-line diff between soul versions.
    Diff {
        /// Bot slug.
        slug: String,

        /// Starting version (default: previous version).
        #[arg(long)]
        from: Option<i32>,

        /// Ending version (default: current version).
        #[arg(long)]
        to: Option<i32>,
    },

    /// Rollback soul to a previous version (creates a new version).
    Rollback {
        /// Bot slug.
        slug: String,

        /// Target version number to rollback to.
        version: i32,

        /// Skip confirmation prompt.
        #[arg(long)]
        force: bool,
    },

    /// Verify soul integrity (SHA-256 hash check).
    Verify {
        /// Bot slug.
        slug: String,
    },
}
