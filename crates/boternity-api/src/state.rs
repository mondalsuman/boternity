//! Application state wiring all services together.
//!
//! AppState holds the concrete service instances used by both CLI and REST API.
//! Services are generic over repository/filesystem/hasher traits, but AppState
//! pins them to the concrete infra implementations.

use std::path::PathBuf;
use std::sync::Arc;

use boternity_core::service::bot::BotService;
use boternity_core::service::secret::SecretService;
use boternity_core::service::soul::SoulService;
use boternity_infra::crypto::hash::Sha256ContentHasher;
use boternity_infra::crypto::vault::VaultCrypto;
use boternity_infra::filesystem::{resolve_data_dir, LocalFileSystem};
use boternity_infra::keychain::KeychainProvider;
use boternity_infra::secret::chain::build_secret_chain;
use boternity_infra::secret::VaultSecretProvider;
use boternity_infra::sqlite::bot::SqliteBotRepository;
use boternity_infra::sqlite::pool::DatabasePool;
use boternity_infra::sqlite::secret::SqliteSecretRepository;
use boternity_infra::sqlite::soul::SqliteSoulRepository;

/// Concrete type aliases for the service generics pinned to infra implementations.
pub type ConcreteBotService = BotService<
    SqliteBotRepository,
    SqliteSoulRepository,
    LocalFileSystem,
    Sha256ContentHasher,
>;

pub type ConcreteSoulService =
    SoulService<SqliteSoulRepository, LocalFileSystem, Sha256ContentHasher>;

/// Shared application state holding all services.
///
/// Used by both CLI commands and REST API handlers.
#[derive(Clone)]
pub struct AppState {
    pub bot_service: Arc<ConcreteBotService>,
    pub soul_service: Arc<ConcreteSoulService>,
    pub secret_service: Arc<SecretService>,
    pub data_dir: PathBuf,
    pub db_pool: DatabasePool,
}

impl AppState {
    /// Initialize the application state: connect to DB, wire services.
    pub async fn init() -> anyhow::Result<Self> {
        let data_dir = resolve_data_dir();

        // Ensure data directory exists
        tokio::fs::create_dir_all(&data_dir).await?;

        // Initialize database
        let db_url = format!(
            "sqlite://{}?mode=rwc",
            data_dir.join("boternity.db").display()
        );
        let db_pool = DatabasePool::new(&db_url).await?;

        // Create repository instances
        let bot_repo = SqliteBotRepository::new(db_pool.clone());
        let secret_repo = SqliteSecretRepository::new(db_pool.clone());

        // Wire soul service (consumed by bot service)
        let soul_service = SoulService::new(
            SqliteSoulRepository::new(db_pool.clone()),
            LocalFileSystem::new(),
            Sha256ContentHasher::new(),
        );

        // Wire bot service
        let bot_service = BotService::new(bot_repo, soul_service, data_dir.clone());

        // Wire secret service with resolution chain
        let vault_crypto = match VaultCrypto::from_keychain() {
            Ok(crypto) => crypto,
            Err(e) => {
                tracing::warn!("Keychain unavailable ({e}), using fallback key for vault");
                // Fallback: use a deterministic key derived from the data dir
                // This is less secure than keychain but works on headless systems
                VaultCrypto::from_password(&format!(
                    "boternity-fallback-{}",
                    data_dir.display()
                ))?
            }
        };

        let vault_provider = VaultSecretProvider::new(secret_repo, vault_crypto);
        let keychain = KeychainProvider::new();
        let secret_chain = build_secret_chain(vault_provider, Some(keychain), true);
        let secret_service = SecretService::new(secret_chain);

        // Create a separate soul service for the API (bot_service owns one internally)
        let api_soul_service = SoulService::new(
            SqliteSoulRepository::new(db_pool.clone()),
            LocalFileSystem::new(),
            Sha256ContentHasher::new(),
        );

        Ok(Self {
            bot_service: Arc::new(bot_service),
            soul_service: Arc::new(api_soul_service),
            secret_service: Arc::new(secret_service),
            data_dir,
            db_pool,
        })
    }
}
