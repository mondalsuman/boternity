//! Application state wiring all services together.
//!
//! AppState holds the concrete service instances used by both CLI and REST API.
//! Services are generic over repository/filesystem/hasher traits, but AppState
//! pins them to the concrete infra implementations.

use std::path::PathBuf;
use std::sync::Arc;

use boternity_core::chat::service::ChatService;
use boternity_core::service::bot::BotService;
use boternity_core::service::secret::SecretService;
use boternity_core::service::soul::SoulService;
use boternity_infra::crypto::hash::Sha256ContentHasher;
use boternity_infra::crypto::vault::VaultCrypto;
use boternity_infra::filesystem::{resolve_data_dir, LocalFileSystem};
use boternity_infra::secret::chain::build_secret_chain;
use boternity_infra::secret::VaultSecretProvider;
use boternity_infra::sqlite::bot::SqliteBotRepository;
use boternity_infra::sqlite::chat::SqliteChatRepository;
use boternity_infra::sqlite::memory::SqliteMemoryRepository;
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

pub type ConcreteChatService = ChatService<SqliteChatRepository, SqliteMemoryRepository>;

/// Shared application state holding all services.
///
/// Used by both CLI commands and REST API handlers.
#[derive(Clone)]
pub struct AppState {
    pub bot_service: Arc<ConcreteBotService>,
    pub soul_service: Arc<ConcreteSoulService>,
    pub chat_service: Arc<ConcreteChatService>,
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

        // Wire secret service with resolution chain.
        // The vault master key is stored in a file (vault.key) rather than the
        // OS keychain to avoid repeated password prompts on every CLI invocation.
        let vault_key_path = data_dir.join("vault.key");
        let vault_crypto = VaultCrypto::from_key_file(&vault_key_path)?;

        let vault_provider = VaultSecretProvider::new(secret_repo, vault_crypto);
        // KeychainProvider is not included in the secret chain. Each keychain
        // entry triggers a separate macOS authorization prompt, causing multiple
        // password dialogs per command. The keychain is used only for the vault
        // master key (VaultCrypto::from_keychain above), not for individual secrets.
        let secret_chain = build_secret_chain(vault_provider, None, true);
        let secret_service = SecretService::new(secret_chain);

        // Create a separate soul service for the API (bot_service owns one internally)
        let api_soul_service = SoulService::new(
            SqliteSoulRepository::new(db_pool.clone()),
            LocalFileSystem::new(),
            Sha256ContentHasher::new(),
        );

        // Wire chat service with its repositories
        let chat_repo = SqliteChatRepository::new(db_pool.clone());
        let memory_repo = SqliteMemoryRepository::new(db_pool.clone());
        let chat_service = ChatService::new(chat_repo, memory_repo);

        Ok(Self {
            bot_service: Arc::new(bot_service),
            soul_service: Arc::new(api_soul_service),
            chat_service: Arc::new(chat_service),
            secret_service: Arc::new(secret_service),
            data_dir,
            db_pool,
        })
    }
}
