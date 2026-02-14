//! Boternity CLI and REST API entry point.
//!
//! Binary name: `bnity`
//!
//! Parses CLI arguments, initializes database and services, then dispatches
//! to the appropriate command handler or starts the REST API server.

mod cli;
mod http;
mod state;

use clap::Parser;
use clap_complete::generate;
use tracing_subscriber::EnvFilter;

use cli::{Cli, CloneResource, Commands, CreateResource, DeleteResource, ExportResource, ListResource, SetResource, SoulCommand};
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
            DeleteResource::Session { id, force } => {
                let session_id = id.parse::<uuid::Uuid>().map_err(|_| anyhow::anyhow!("Invalid session ID: {id}"))?;
                cli::session::delete_session(&state, session_id, force, cli.json).await?;
            }
            DeleteResource::Memory { id, force } => {
                let memory_id = id.parse::<uuid::Uuid>().map_err(|_| anyhow::anyhow!("Invalid memory ID: {id}"))?;
                cli::memory::delete_memory(&state, memory_id, force, None, None, cli.json).await?;
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

        Commands::Soul { action } => match action {
            SoulCommand::Edit { slug } => {
                cli::soul::edit_soul(&state, &slug, cli.json).await?;
            }
            SoulCommand::History { slug } => {
                cli::soul::soul_history(&state, &slug, cli.json).await?;
            }
            SoulCommand::Diff { slug, from, to } => {
                cli::soul::soul_diff(&state, &slug, from, to, cli.json).await?;
            }
            SoulCommand::Rollback {
                slug,
                version,
                force,
            } => {
                cli::soul::soul_rollback(&state, &slug, version, force, cli.json).await?;
            }
            SoulCommand::Verify { slug } => {
                cli::soul::soul_verify(&state, &slug, cli.json).await?;
            }
        },

        Commands::Check { slug } => {
            // Health check for a bot including soul integrity verification
            let bot = state.bot_service.get_bot_by_slug(&slug).await?;
            let soul_path = state.data_dir.join("bots").join(&bot.slug).join("SOUL.md");
            let has_soul = tokio::fs::try_exists(&soul_path).await.unwrap_or(false);
            let identity_path = state.data_dir.join("bots").join(&bot.slug).join("IDENTITY.md");
            let has_identity = tokio::fs::try_exists(&identity_path).await.unwrap_or(false);

            // Run soul integrity check if soul exists
            let soul_integrity = if has_soul {
                match state.soul_service.verify_soul_integrity(&bot.id, &soul_path).await {
                    Ok(result) => Some(result),
                    Err(_) => None,
                }
            } else {
                None
            };

            let integrity_ok = soul_integrity.as_ref().is_some_and(|r| r.valid);

            if cli.json {
                let check = serde_json::json!({
                    "slug": slug,
                    "status": bot.status.to_string(),
                    "soul_exists": has_soul,
                    "identity_exists": has_identity,
                    "soul_integrity": soul_integrity.as_ref().map(|r| r.valid),
                    "healthy": has_soul && has_identity && integrity_ok,
                });
                println!("{}", serde_json::to_string_pretty(&check)?);
            } else {
                println!();
                println!(
                    "  Health check for '{}'",
                    console::style(&bot.name).cyan()
                );
                println!();
                let check_mark = |ok: bool| {
                    if ok {
                        format!("{}", console::style("ok").green())
                    } else {
                        format!("{}", console::style("FAIL").red())
                    }
                };
                println!("  {} SOUL.md exists", check_mark(has_soul));
                println!("  {} IDENTITY.md exists", check_mark(has_identity));
                println!(
                    "  {} Bot status: {}",
                    check_mark(bot.status == boternity_types::bot::BotStatus::Active),
                    bot.status
                );
                if let Some(integrity) = &soul_integrity {
                    println!(
                        "  {} Soul integrity",
                        check_mark(integrity.valid)
                    );
                    if !integrity.valid {
                        println!(
                            "     Expected: {}",
                            &integrity.expected_hash[..8.min(integrity.expected_hash.len())]
                        );
                        println!(
                            "     Actual:   {}",
                            &integrity.actual_hash[..8.min(integrity.actual_hash.len())]
                        );
                    }
                }
                println!();
            }
        }

        Commands::Status => {
            cli::status::status(&state, cli.json).await?;
        }

        Commands::Provider { action } => {
            cli::provider::handle_provider_command(action, &state, cli.json).await?;
        }

        Commands::Storage { action } => {
            cli::storage::handle_storage_command(action, &state, cli.json).await?;
        }

        Commands::Kv { action } => {
            cli::kv::handle_kv_command(action, &state, cli.json).await?;
        }

        Commands::SharedMemory { action } => {
            cli::shared_memory::handle_shared_memory_command(
                action,
                &state,
                &state.shared_memory,
                &state.embedder,
                &state.audit_log,
                cli.json,
            )
            .await?;
        }

        Commands::Skill { action } => {
            cli::skill::handle_skill_command(action, &state, cli.json).await?;
        }

        Commands::Serve { port, host } => {
            // Ensure an API key exists, print it if new
            let api_key = http::extractors::auth::ensure_api_key(&state).await?;
            if api_key.starts_with("bnity_") {
                println!();
                println!(
                    "  {} API key generated (save this -- it won't be shown again):",
                    console::style("🔑").bold()
                );
                println!();
                println!("  {}", console::style(&api_key).yellow().bold());
                println!();
            }

            let addr = format!("{host}:{port}");
            let listener = tokio::net::TcpListener::bind(&addr).await?;

            println!(
                "  {} Boternity API listening on {}",
                console::style("⚡").bold(),
                console::style(format!("http://{addr}")).cyan()
            );
            println!(
                "  {}",
                console::style("Press Ctrl+C to stop").dim()
            );

            let router = http::router::build_router(state);

            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown_signal())
                .await?;

            println!("\n  Server stopped.");
        }

        Commands::Export { resource } => match resource {
            ExportResource::Session { id } => {
                let session_id = id.parse::<uuid::Uuid>().map_err(|_| anyhow::anyhow!("Invalid session ID: {id}"))?;
                cli::session::export_session(&state, session_id, cli.json).await?;
            }
        },

        Commands::Sessions { slug } => {
            cli::session::list_sessions(&state, &slug, cli.json).await?;
        }

        Commands::Memories { slug } => {
            cli::memory::list_memories(&state, &slug, cli.json).await?;
        }

        Commands::Remember { slug, fact } => {
            cli::memory::remember(&state, &slug, &fact, None, None, None, cli.json).await?;
        }

        Commands::Forget { slug, force } => {
            cli::memory::forget(&state, &slug, force, cli.json).await?;
        }

        Commands::Chat { slug, resume, verbose, quiet } => {
            cli::chat::loop_runner::run_chat_loop(&state, &slug, resume, verbose, quiet).await?;
        }

        Commands::Completions { .. } => unreachable!("handled above"),

        Commands::NewBot { name, description } => {
            // Alias for `create bot`
            cli::bot::create_bot(&state, name, description, None, cli.json).await?;
        }
    }

    Ok(())
}

/// Wait for Ctrl+C or SIGTERM for graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
