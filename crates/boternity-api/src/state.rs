//! Application state wiring all services together.
//!
//! AppState holds the concrete service instances used by both CLI and REST API.
//! Services are generic over repository/filesystem/hasher traits, but AppState
//! pins them to the concrete infra implementations.
//!
//! Phase 3 additions: vector store, embedder, vector memory, shared memory,
//! file store, file indexer, KV store, audit log, and provider health store.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use boternity_core::chat::service::ChatService;
use boternity_core::event::EventBus;
use boternity_core::llm::fallback::FallbackChain;
use boternity_core::llm::provider::LlmProvider;
use boternity_core::memory::box_embedder::BoxEmbedder;
use boternity_core::memory::embedder::Embedder;
use boternity_core::service::bot::BotService;
use boternity_core::service::secret::SecretService;
use boternity_core::service::soul::SoulService;
use boternity_types::config::GlobalConfig;
use dashmap::DashMap;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use boternity_infra::crypto::hash::Sha256ContentHasher;
use boternity_infra::crypto::vault::VaultCrypto;
use boternity_infra::filesystem::{resolve_data_dir, LocalFileSystem};
use boternity_infra::llm::openai_compat::config::default_cost_table;
use boternity_infra::secret::chain::build_secret_chain;
use boternity_infra::secret::VaultSecretProvider;
use boternity_infra::skill::skill_store::SkillStore;
use boternity_infra::skill::wasm_runtime::WasmRuntime;
use boternity_infra::sqlite::audit::SqliteAuditLog;
use boternity_infra::sqlite::bot::SqliteBotRepository;
use boternity_infra::sqlite::chat::SqliteChatRepository;
use boternity_infra::sqlite::file_metadata::SqliteFileMetadataStore;
use boternity_infra::sqlite::kv::SqliteKvStore;
use boternity_infra::sqlite::memory::SqliteMemoryRepository;
use boternity_infra::sqlite::pool::DatabasePool;
use boternity_infra::sqlite::provider_health::SqliteProviderHealthStore;
use boternity_infra::sqlite::secret::SqliteSecretRepository;
use boternity_infra::builder::sqlite_draft_store::SqliteBuilderDraftStore;
use boternity_infra::builder::sqlite_memory_store::SqliteBuilderMemoryStore;
use boternity_infra::sqlite::skill_audit::SqliteSkillAuditLog;
use boternity_infra::sqlite::soul::SqliteSoulRepository;
use boternity_infra::storage::filesystem::LocalFileStore;
use boternity_infra::storage::indexer::FileIndexer;
use boternity_infra::vector::embedder::FastEmbedEmbedder;
use boternity_infra::vector::lance::LanceVectorStore;
use boternity_infra::vector::memory::LanceVectorMemoryStore;
use boternity_infra::vector::shared::LanceSharedMemoryStore;
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

/// Concrete type alias for the file indexer pinned to FastEmbedEmbedder.
pub type ConcreteFileIndexer = FileIndexer<FastEmbedEmbedder>;

/// Shared application state holding all services.
///
/// Used by both CLI commands and REST API handlers.
///
/// Phase 3 additions: vector_store, embedder, vector_memory, shared_memory,
/// file_store, file_indexer, kv_store, audit_log, provider_health_store.
///
/// Phase 6 additions: skill_store, wasm_runtime, skill_audit_log.
///
/// Phase 7 additions: builder_draft_store, builder_memory_store.
#[derive(Clone)]
pub struct AppState {
    pub bot_service: Arc<ConcreteBotService>,
    pub soul_service: Arc<ConcreteSoulService>,
    pub chat_service: Arc<ConcreteChatService>,
    pub secret_service: Arc<SecretService>,
    pub data_dir: PathBuf,
    pub db_pool: DatabasePool,

    // --- Phase 3 services ---
    /// LanceDB vector store for bot memories, shared memories, and file chunks.
    pub vector_store: Arc<LanceVectorStore>,
    /// Type-erased embedding generator (FastEmbedEmbedder in production).
    pub embedder: Arc<BoxEmbedder>,
    /// Per-bot vector memory store backed by LanceDB.
    pub vector_memory: Arc<LanceVectorMemoryStore>,
    /// Cross-bot shared memory store backed by LanceDB.
    pub shared_memory: Arc<LanceSharedMemoryStore>,
    /// Local filesystem file store with version history.
    pub file_store: Arc<LocalFileStore>,
    /// File indexer for chunking, embedding, and semantic search.
    pub file_indexer: Arc<ConcreteFileIndexer>,
    /// Per-bot key-value store backed by SQLite.
    pub kv_store: Arc<SqliteKvStore>,
    /// Memory audit log for tracking add/delete/share/revoke operations.
    pub audit_log: Arc<SqliteAuditLog>,
    /// Provider health persistence for circuit breaker state across restarts.
    pub provider_health_store: Arc<SqliteProviderHealthStore>,

    // --- Phase 5 services ---
    /// Event bus for agent lifecycle events (broadcast to WebSocket + CLI).
    pub event_bus: EventBus,
    /// Global configuration from `~/.boternity/config.toml`.
    pub global_config: GlobalConfig,
    /// Active agent cancellation tokens, keyed by agent_id.
    /// Inserted by orchestrator when spawning, removed on completion.
    pub agent_cancellations: Arc<DashMap<Uuid, CancellationToken>>,
    /// Budget pause channels, keyed by request_id.
    /// Orchestrator inserts sender; WebSocket/CLI sends continue/stop decision.
    pub budget_responses: Arc<DashMap<Uuid, oneshot::Sender<bool>>>,

    // --- Phase 6 services ---
    /// Filesystem-based skill store managing installed skills.
    pub skill_store: Arc<SkillStore>,
    /// Wasmtime WASM runtime with per-trust-tier engine configurations.
    pub wasm_runtime: Arc<WasmRuntime>,
    /// SQLite-backed audit log for skill invocations.
    pub skill_audit_log: Arc<SqliteSkillAuditLog>,

    // --- Phase 7 services ---
    /// SQLite-backed builder draft persistence for auto-save/resume.
    pub builder_draft_store: Arc<SqliteBuilderDraftStore>,
    /// SQLite-backed builder memory for cross-session suggestion recall.
    pub builder_memory_store: Arc<SqliteBuilderMemoryStore>,
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

        // --- Phase 3 services ---

        // Initialize embedding model (downloads on first run, cached after)
        let embedder = FastEmbedEmbedder::new()?;
        tracing::info!(
            model = embedder.model_name(),
            dimension = embedder.dimension(),
            "Embedding model loaded"
        );
        let embedder_arc = Arc::new(embedder);

        // Initialize LanceDB vector store at {data_dir}/vector_store
        let vector_store_path = data_dir.join("vector_store");
        let vector_store = LanceVectorStore::new(vector_store_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize vector store: {e}"))?;
        let vector_store = Arc::new(vector_store);

        // Per-bot vector memory store (uses its own LanceVectorStore instance
        // since LanceVectorMemoryStore takes ownership, not Arc)
        let vector_memory_store_path = data_dir.join("vector_store");
        let vector_memory_lance = LanceVectorStore::new(vector_memory_store_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize vector memory store: {e}"))?;
        let vector_memory = Arc::new(LanceVectorMemoryStore::new(vector_memory_lance));

        // Cross-bot shared memory store
        let shared_memory_store_path = data_dir.join("vector_store");
        let shared_memory_lance = LanceVectorStore::new(shared_memory_store_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize shared memory store: {e}"))?;
        let shared_memory = Arc::new(LanceSharedMemoryStore::new(shared_memory_lance));

        // File metadata store (SQLite)
        let file_metadata_store = SqliteFileMetadataStore::new(db_pool.clone());

        // Local filesystem file store with version history
        let file_store = Arc::new(LocalFileStore::new(data_dir.clone(), file_metadata_store));

        // File indexer for chunking and embedding text files
        let file_indexer = Arc::new(FileIndexer::new(
            Arc::clone(&vector_store),
            embedder_arc.clone(),
        ));

        // Type-erase the embedder for dynamic dispatch
        let box_embedder = Arc::new(BoxEmbedder::new(FastEmbedEmbedder::new()?));

        // KV store (SQLite)
        let kv_store = Arc::new(SqliteKvStore::new(db_pool.clone()));

        // Audit log (SQLite)
        let audit_log = Arc::new(SqliteAuditLog::new(db_pool.clone()));

        // Provider health persistence (SQLite)
        let provider_health_store = Arc::new(SqliteProviderHealthStore::new(db_pool.clone()));

        // --- Phase 5 services ---
        let global_config = boternity_infra::config::load_global_config(&data_dir).await;
        let event_bus = EventBus::new(1024);
        let agent_cancellations = Arc::new(DashMap::new());
        let budget_responses = Arc::new(DashMap::new());

        // --- Phase 6 services ---

        // Ensure skills directory exists
        let skills_dir = data_dir.join("skills");
        tokio::fs::create_dir_all(&skills_dir).await?;

        // Filesystem-based skill store
        let skill_store = Arc::new(SkillStore::new(data_dir.clone()));

        // WASM runtime with per-tier engines
        let wasm_runtime = Arc::new(WasmRuntime::new()?);

        // Skill audit log (SQLite)
        let skill_audit_log = Arc::new(SqliteSkillAuditLog::new(db_pool.clone()));

        // --- Phase 7 services ---

        // Builder draft persistence (auto-save/resume)
        let builder_draft_store = Arc::new(SqliteBuilderDraftStore::new(db_pool.clone()));

        // Builder memory (cross-session suggestion recall)
        let builder_memory_store = Arc::new(SqliteBuilderMemoryStore::new(db_pool.clone()));

        Ok(Self {
            bot_service: Arc::new(bot_service),
            soul_service: Arc::new(api_soul_service),
            chat_service: Arc::new(chat_service),
            secret_service: Arc::new(secret_service),
            data_dir,
            db_pool,
            vector_store,
            embedder: box_embedder,
            vector_memory,
            shared_memory,
            file_store,
            file_indexer,
            kv_store,
            audit_log,
            provider_health_store,
            event_bus,
            global_config,
            agent_cancellations,
            budget_responses,
            skill_store,
            wasm_runtime,
            skill_audit_log,
            builder_draft_store,
            builder_memory_store,
        })
    }

    /// Build a [`FallbackChain`] for a bot using the configured providers.
    ///
    /// Loads additional providers from `providers.json` and includes them
    /// in the chain alongside the primary ANTHROPIC_API_KEY provider.
    /// Providers are ordered by priority (lower = higher priority).
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

        // Build primary provider based on key format (same auto-detection as before)
        let (primary_provider, primary_config) = if api_key_value.starts_with("bedrock-api-key-") {
            let region =
                std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
            let api_key = SecretString::from(api_key_value.clone());
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
            let api_key = SecretString::from(api_key_value.clone());
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

        let mut all_configs = vec![primary_config];
        let mut all_providers = vec![primary_provider];

        // Load additional providers from providers.json
        let extra_configs =
            crate::cli::provider::load_provider_configs(&self.data_dir).await.unwrap_or_default();

        for extra_config in &extra_configs {
            if !extra_config.enabled {
                continue;
            }
            // Skip if same name as primary (avoid duplicate)
            if extra_config.name == all_configs[0].name {
                continue;
            }

            // Resolve API key for this provider
            let api_key = if let Some(ref secret_name) = extra_config.api_key_secret_name {
                self.secret_service
                    .get_secret(secret_name, &SecretScope::Global)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };

            match boternity_infra::llm::create_provider(extra_config, api_key.as_deref()) {
                Ok(provider) => {
                    all_configs.push(extra_config.clone());
                    all_providers.push(provider);
                }
                Err(e) => {
                    tracing::warn!(
                        provider = %extra_config.name,
                        error = %e,
                        "Failed to create provider from config, skipping"
                    );
                }
            }
        }

        let chain_config = FallbackChainConfig {
            providers: all_configs,
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

        let chain = FallbackChain::new(chain_config, all_providers, keyed_cost_table);

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

    /// Return the path to the skills directory (`{data_dir}/skills`).
    pub fn skills_dir(&self) -> PathBuf {
        self.data_dir.join("skills")
    }
}
