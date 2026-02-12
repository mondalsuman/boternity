//! Application state wiring all services together.
//!
//! AppState holds the concrete service instances used by both CLI and REST API.
//! Services are generic over repository/filesystem/hasher traits, but AppState
//! pins them to the concrete infra implementations.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use boternity_core::chat::service::ChatService;
use boternity_core::llm::fallback::FallbackChain;
use boternity_core::llm::provider::LlmProvider;
use boternity_core::service::bot::BotService;
use boternity_core::service::secret::SecretService;
use boternity_core::service::soul::SoulService;
use boternity_infra::crypto::hash::Sha256ContentHasher;
use boternity_infra::crypto::vault::VaultCrypto;
use boternity_infra::filesystem::{resolve_data_dir, LocalFileSystem};
use boternity_infra::llm::openai_compat::config::default_cost_table;
use boternity_infra::secret::chain::build_secret_chain;
use boternity_infra::secret::VaultSecretProvider;
use boternity_infra::sqlite::bot::SqliteBotRepository;
use boternity_infra::sqlite::chat::SqliteChatRepository;
use boternity_infra::sqlite::memory::SqliteMemoryRepository;
use boternity_infra::sqlite::pool::DatabasePool;
use boternity_infra::sqlite::secret::SqliteSecretRepository;
use boternity_infra::sqlite::soul::SqliteSoulRepository;
use boternity_types::llm::{FallbackChainConfig, ProviderConfig, ProviderType};
use boternity_types::secret::SecretScope;

use boternity_core::llm::box_provider::BoxLlmProvider;
use boternity_infra::llm::anthropic::AnthropicProvider;
use boternity_infra::llm::bedrock::BedrockProvider;
use secrecy::SecretString;

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

    /// Build a [`FallbackChain`] for a bot using the configured providers.
    ///
    /// Currently builds a single-provider chain using the ANTHROPIC_API_KEY from
    /// the secret store. When additional providers are configured (via `bnity provider add`),
    /// they will be included in the chain with their priorities.
    ///
    /// The chain uses the default cost table for failover cost warnings.
    ///
    /// # Arguments
    ///
    /// * `model` - The model to use for the primary Anthropic provider
    ///
    /// # Errors
    ///
    /// Returns an error if no ANTHROPIC_API_KEY is found in the secret store.
    pub async fn build_fallback_chain(&self, model: &str) -> anyhow::Result<FallbackChain> {
        let api_key_value = self
            .secret_service
            .get_secret("ANTHROPIC_API_KEY", &SecretScope::Global)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ANTHROPIC_API_KEY not found. Set it with: bnity set secret ANTHROPIC_API_KEY"
                )
            })?;

        // Build provider based on key format (same auto-detection as before)
        let (provider, provider_config) = if api_key_value.starts_with("bedrock-api-key-") {
            let region =
                std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
            let api_key = SecretString::from(api_key_value);
            let bedrock = BedrockProvider::new(api_key, model.to_string(), region);
            let caps = bedrock.capabilities().clone();
            (
                BoxLlmProvider::new(bedrock),
                ProviderConfig {
                    name: "bedrock".to_string(),
                    provider_type: ProviderType::Bedrock,
                    api_key_secret_name: Some("ANTHROPIC_API_KEY".to_string()),
                    base_url: None,
                    model: model.to_string(),
                    priority: 0,
                    enabled: true,
                    capabilities: caps,
                },
            )
        } else {
            let api_key = SecretString::from(api_key_value);
            let anthropic = AnthropicProvider::new(api_key, model.to_string());
            let caps = anthropic.capabilities().clone();
            (
                BoxLlmProvider::new(anthropic),
                ProviderConfig {
                    name: "anthropic".to_string(),
                    provider_type: ProviderType::Anthropic,
                    api_key_secret_name: Some("ANTHROPIC_API_KEY".to_string()),
                    base_url: None,
                    model: model.to_string(),
                    priority: 0,
                    enabled: true,
                    capabilities: caps,
                },
            )
        };

        let chain_config = FallbackChainConfig {
            providers: vec![provider_config],
            rate_limit_queue_timeout_ms: 5000,
            cost_warning_multiplier: 3.0,
        };

        let cost_table = default_cost_table();

        // Build the chain keyed by provider name for cost lookups
        let mut keyed_cost_table = HashMap::new();
        for (key, cost) in &cost_table {
            // Map "provider:model" keyed entries to just "provider" for fallback chain lookup
            let provider_name = key.split(':').next().unwrap_or(key);
            if !keyed_cost_table.contains_key(provider_name) {
                keyed_cost_table.insert(provider_name.to_string(), cost.clone());
            }
        }

        let chain = FallbackChain::new(chain_config, vec![provider], keyed_cost_table);

        Ok(chain)
    }

    /// Create a single [`BoxLlmProvider`] from the secret store.
    ///
    /// This is a backward-compatible helper for non-chat uses (title generation,
    /// memory extraction, etc.) that need a standalone provider without the
    /// full fallback chain machinery.
    ///
    /// Auto-detects provider based on key format:
    /// - Keys starting with `bedrock-api-key-` -> AWS Bedrock provider
    /// - All other keys -> Anthropic direct API provider
    pub async fn create_single_provider(&self, model: &str) -> anyhow::Result<BoxLlmProvider> {
        let api_key_value = self
            .secret_service
            .get_secret("ANTHROPIC_API_KEY", &SecretScope::Global)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ANTHROPIC_API_KEY not found. Set it with: bnity set secret ANTHROPIC_API_KEY"
                )
            })?;

        if api_key_value.starts_with("bedrock-api-key-") {
            let region =
                std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
            let api_key = SecretString::from(api_key_value);
            let bedrock = BedrockProvider::new(api_key, model.to_string(), region);
            Ok(BoxLlmProvider::new(bedrock))
        } else {
            let api_key = SecretString::from(api_key_value);
            let anthropic = AnthropicProvider::new(api_key, model.to_string());
            Ok(BoxLlmProvider::new(anthropic))
        }
    }
}
