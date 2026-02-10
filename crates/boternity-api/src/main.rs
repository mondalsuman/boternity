//! Boternity CLI and REST API entry point.
//!
//! Binary name: `bnity`
//!
//! Parses CLI arguments, initializes database and services, then dispatches
//! to the appropriate command handler or starts the REST API server.

mod cli;
mod state;

use clap::Parser;
use clap_complete::generate;
use tracing_subscriber::EnvFilter;

use cli::{Cli, CloneResource, Commands, CreateResource, DeleteResource, ListResource, SetResource};
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Set up tracing based on verbosity
    let filter = match cli.verbose {
        0 if cli.quiet => "error",
        0 => "warn",
        1 => "info,boternity=debug",
        _ => "trace",
    };

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .init();

    // Shell completions don't need app state
    if let Commands::Completions { shell } = &cli.command {
        let mut cmd = <Cli as clap::CommandFactory>::command();
        generate(*shell, &mut cmd, "bnity", &mut std::io::stdout());
        return Ok(());
    }

    // Initialize application state (DB, services)
    let state = AppState::init().await?;

    match cli.command {
        Commands::Create { resource } => match resource {
            CreateResource::Bot {
                name,
                description,
                category,
            } => {
                cli::bot::create_bot(&state, name, description, category, cli.json).await?;
            }
        },

        Commands::List { resource } => match resource {
            ListResource::Bots { status, sort } => {
                cli::bot::list_bots(&state, status, &sort, cli.json).await?;
            }
            ListResource::Secrets => {
                cli::secret::list_secrets(&state, cli.json).await?;
            }
        },

        Commands::Show { slug } => {
            cli::bot::show_bot(&state, &slug, cli.json).await?;
        }

        Commands::Delete { resource } => match resource {
            DeleteResource::Bot { slug, force } => {
                cli::bot::delete_bot(&state, &slug, force, cli.json).await?;
            }
        },

        Commands::Clone { resource } => match resource {
            CloneResource::Bot { slug } => {
                cli::bot::clone_bot(&state, &slug, cli.json).await?;
            }
        },

        Commands::Set { resource } => match resource {
            SetResource::Secret { key, value } => {
                cli::secret::set_secret(&state, &key, value.as_deref(), cli.json).await?;
            }
        },

        Commands::Check { slug } => {
            // Basic health check for a bot
            let bot = state.bot_service.get_bot_by_slug(&slug).await?;
            let soul_path = state.data_dir.join("bots").join(&bot.slug).join("SOUL.md");
            let has_soul = tokio::fs::try_exists(&soul_path).await.unwrap_or(false);
            let identity_path = state.data_dir.join("bots").join(&bot.slug).join("IDENTITY.md");
            let has_identity = tokio::fs::try_exists(&identity_path).await.unwrap_or(false);

            if cli.json {
                let check = serde_json::json!({
                    "slug": slug,
                    "status": bot.status.to_string(),
                    "soul_exists": has_soul,
                    "identity_exists": has_identity,
                    "healthy": has_soul && has_identity,
                });
                println!("{}", serde_json::to_string_pretty(&check)?);
            } else {
                println!();
                println!(
                    "  {} Health check for '{}'",
                    console::style("🔍").bold(),
                    console::style(&bot.name).cyan()
                );
                println!();
                let check_mark = |ok: bool| {
                    if ok {
                        format!("{}", console::style("✓").green())
                    } else {
                        format!("{}", console::style("✗").red())
                    }
                };
                println!("  {} SOUL.md exists", check_mark(has_soul));
                println!("  {} IDENTITY.md exists", check_mark(has_identity));
                println!(
                    "  {} Bot status: {}",
                    check_mark(bot.status == boternity_types::bot::BotStatus::Active),
                    bot.status
                );
                println!();
            }
        }

        Commands::Status => {
            cli::status::status(&state, cli.json).await?;
        }

        Commands::Serve { port, host } => {
            // Will be fully implemented in Task 2
            println!(
                "Starting server on {}:{}...",
                host, port
            );
            println!("REST API server not yet implemented. Use Task 2.");
        }

        Commands::Completions { .. } => unreachable!("handled above"),

        Commands::NewBot { name, description } => {
            // Alias for `create bot`
            cli::bot::create_bot(&state, name, description, None, cli.json).await?;
        }
    }

    Ok(())
}
