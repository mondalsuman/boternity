# Phase 1: Foundation + Bot Identity - Research

**Researched:** 2026-02-10
**Domain:** Rust monorepo scaffold, SQLite storage, bot identity, secrets vault, CLI + REST API
**Confidence:** HIGH

## Summary

This phase establishes the foundational infrastructure for Boternity: a Turborepo + Cargo workspace monorepo, SQLite storage with repository abstraction, bot identity system (SOUL.md, IDENTITY.md, USER.md), an encrypted secrets vault with OS keychain integration, and CLI + REST API for bot lifecycle management.

The standard Rust web stack for this domain is well-established: **Axum 0.8** for HTTP, **sqlx 0.8** for async SQLite with compile-time checked queries and built-in migration support, **clap 4.5** for CLI, and the RustCrypto ecosystem (aes-gcm, sha2, argon2) for cryptographic operations. The **keyring** crate (v3.6) provides cross-platform OS keychain integration. The architecture follows clean/hexagonal patterns with trait-based repository abstractions in the core crate and implementations in the infra crate.

Key architectural insight: Turborepo manages only the TypeScript/JavaScript side of the monorepo (future web UI). Cargo workspace manages the Rust side independently. They coexist in the same repository but do not integrate their build graphs -- the root has both `turbo.json` and `Cargo.toml` (workspace).

**Primary recommendation:** Use sqlx 0.8 with SQLite (not rusqlite) because it provides compile-time query checking, built-in async support via tokio spawn_blocking, built-in migrations, and a direct migration path to PostgreSQL by changing the feature flag and connection string.

## Standard Stack

The established libraries/tools for this domain:

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.8.8 | HTTP framework / REST API | Tokio-native, tower ecosystem, macro-free routing, extractors, 0.8 removes async_trait requirement |
| sqlx | 0.8.6 | Async SQL toolkit (SQLite) | Compile-time checked queries, built-in migrations, supports SQLite + PostgreSQL with same API, async via spawn_blocking |
| clap | 4.5.57 | CLI argument parsing | Derive macro for subcommands, feature-rich, de facto standard (240k+ dependents) |
| tokio | 1.x | Async runtime | Full-featured async runtime, required by axum and sqlx |
| serde | 1.x | Serialization/deserialization | Universal Rust serialization, needed for JSON API, config files |
| uuid | 1.20.0 | Unique identifiers | UUID v7 for time-sortable IDs, guaranteed ordering within process |
| tracing | 0.1.x | Structured logging/diagnostics | Spans + events, integrates with tower-http TraceLayer, structured JSON output |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tower-http | 0.6.x | HTTP middleware | CORS, tracing, compression layers for axum |
| keyring | 3.6.3 | OS keychain integration | macOS Keychain (apple-native feature), Linux Secret Service (sync-secret-service feature) |
| aes-gcm | 0.10.3 | Authenticated encryption | Encrypting secrets in SQLite vault, AES-256-GCM with NCC Group audit |
| argon2 | 0.5.x | Key derivation | Deriving encryption key from master password for vault, OWASP recommended |
| sha2 | 0.10.9 | SHA-256 hashing | SOUL.md integrity verification at startup (SECU-05) |
| thiserror | 1.x (2.x available) | Error type definitions | Typed errors in library crates (boternity-core, boternity-infra) |
| anyhow | 1.x | Error propagation | Application-level error handling in boternity-api, CLI |
| serde_json | 1.x | JSON serialization | REST API request/response bodies |
| chrono | 0.4.x | Date/time handling | Timestamps for soul versions, bot creation dates |
| tracing-subscriber | 0.3.x | Tracing output configuration | EnvFilter, JSON formatting, log level control |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| sqlx | rusqlite + r2d2 | rusqlite is synchronous-only, no compile-time query checks, no built-in PostgreSQL migration path. Only advantage: slightly less overhead for SQLite-only workloads |
| sqlx | diesel | Diesel has its own DSL, heavier ORM, schema.rs generation step; sqlx is lighter and more flexible for this use case |
| clap | argh | argh is simpler but lacks features like shell completions, env var support, colored help |
| aes-gcm | ring | ring is excellent but aes-gcm is pure Rust, easier to cross-compile, has NCC audit |
| uuid v7 | ULID | uuid crate is more widely used, v7 provides same time-sortability as ULID, standard UUID format |

### Installation

```toml
# Root Cargo.toml [workspace.dependencies]
axum = { version = "0.8", features = ["macros"] }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
clap = { version = "4.5", features = ["derive", "env"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1.20", features = ["v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip"] }
keyring = { version = "3.6", features = ["apple-native", "sync-secret-service", "crypto-rust"] }
aes-gcm = "0.10"
argon2 = "0.5"
sha2 = "0.10"
thiserror = "1"
anyhow = "1"
```

## Architecture Patterns

### Recommended Project Structure

```
boternity/
├── Cargo.toml                  # Workspace root: [workspace] members
├── Cargo.lock                  # Single lockfile for all Rust crates
├── turbo.json                  # Turborepo config (JS/TS packages only)
├── package.json                # Root package.json for Turborepo
├── pnpm-workspace.yaml         # pnpm workspace config (future web UI)
├── crates/
│   ├── boternity-types/        # Shared domain types, zero external deps beyond serde/uuid/chrono
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── bot.rs          # Bot, BotId, BotStatus
│   │       ├── soul.rs         # Soul, SoulVersion, SoulHash
│   │       ├── identity.rs     # Identity (display name, avatar, description)
│   │       ├── secret.rs       # SecretEntry, SecretProvider
│   │       └── error.rs        # Domain error types
│   ├── boternity-core/         # Business logic, repository TRAITS, zero infra deps
│   │   ├── Cargo.toml          # depends on: boternity-types ONLY
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── repository/     # Trait definitions (ports)
│   │       │   ├── mod.rs
│   │       │   ├── bot.rs      # BotRepository trait
│   │       │   ├── soul.rs     # SoulRepository trait
│   │       │   └── secret.rs   # SecretRepository trait
│   │       └── service/        # Business logic (use cases)
│   │           ├── mod.rs
│   │           ├── bot.rs      # BotService
│   │           ├── soul.rs     # SoulService (versioning, immutability)
│   │           └── secret.rs   # SecretService (vault, keychain, env)
│   ├── boternity-infra/        # SQLite implementations, file I/O
│   │   ├── Cargo.toml          # depends on: boternity-types, boternity-core
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sqlite/         # SQLite repository implementations (adapters)
│   │       │   ├── mod.rs
│   │       │   ├── pool.rs     # Pool configuration, WAL mode, migrations
│   │       │   ├── bot.rs      # SqliteBotRepository impl BotRepository
│   │       │   ├── soul.rs     # SqliteSoulRepository impl SoulRepository
│   │       │   └── secret.rs   # SqliteSecretRepository impl SecretRepository
│   │       ├── crypto/         # Encryption, hashing implementations
│   │       │   ├── mod.rs
│   │       │   ├── vault.rs    # AES-256-GCM vault encryption
│   │       │   └── hash.rs     # SHA-256 file hashing
│   │       ├── keychain/       # OS keychain adapter
│   │       │   └── mod.rs
│   │       └── filesystem/     # SOUL.md, IDENTITY.md, USER.md file I/O
│   │           └── mod.rs
│   └── boternity-api/          # CLI + REST API (application layer)
│       ├── Cargo.toml          # depends on: boternity-types, boternity-core, boternity-infra
│       └── src/
│           ├── main.rs         # Entry point, wiring, startup
│           ├── cli/            # Clap CLI commands
│           │   ├── mod.rs
│           │   ├── bot.rs      # bot create/list/configure/delete/start/stop
│           │   └── secret.rs   # secret set/get/delete
│           ├── http/           # Axum REST API
│           │   ├── mod.rs
│           │   ├── router.rs   # Route definitions
│           │   ├── handlers/   # Request handlers
│           │   ├── extractors/ # Custom extractors
│           │   └── error.rs    # API error responses (JSON)
│           └── state.rs        # AppState with Arc<dyn Repository> fields
├── migrations/                 # sqlx SQL migration files
│   └── 20260210_001_initial.sql
├── apps/                       # Future: web UI (Turborepo manages this)
└── packages/                   # Future: shared TS packages
```

### Pattern 1: Repository Trait with Dependency Inversion

**What:** Define repository traits in boternity-core, implement them in boternity-infra. The core crate never imports infra.
**When to use:** Always -- this is the foundational architecture pattern for the entire project.
**Example:**

```rust
// crates/boternity-types/src/bot.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotId(pub Uuid);

impl BotId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bot {
    pub id: BotId,
    pub name: String,
    pub status: BotStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BotStatus {
    Created,
    Running,
    Stopped,
}

// crates/boternity-core/src/repository/bot.rs
use boternity_types::bot::{Bot, BotId};
use std::future::Future;

pub trait BotRepository: Send + Sync {
    fn create(&self, bot: &Bot) -> impl Future<Output = Result<Bot, RepositoryError>> + Send;
    fn get_by_id(&self, id: &BotId) -> impl Future<Output = Result<Option<Bot>, RepositoryError>> + Send;
    fn list(&self) -> impl Future<Output = Result<Vec<Bot>, RepositoryError>> + Send;
    fn delete(&self, id: &BotId) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}

// crates/boternity-infra/src/sqlite/bot.rs
use boternity_core::repository::bot::BotRepository;
use sqlx::SqlitePool;

pub struct SqliteBotRepository {
    pool: SqlitePool,
}

impl BotRepository for SqliteBotRepository {
    async fn create(&self, bot: &Bot) -> Result<Bot, RepositoryError> {
        sqlx::query!(
            "INSERT INTO bots (id, name, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
            bot.id.0.to_string(),
            bot.name,
            bot.status.as_str(),
            bot.created_at,
            bot.updated_at
        )
        .execute(&self.pool)
        .await?;
        Ok(bot.clone())
    }
    // ... other methods
}
```

### Pattern 2: AppState with Trait Objects for DI

**What:** Wire repositories as trait objects in AppState, injected into axum handlers via State extractor.
**When to use:** In the API layer (boternity-api) for connecting handlers to business logic.
**Example:**

```rust
// crates/boternity-api/src/state.rs
use boternity_core::repository::bot::BotRepository;
use boternity_core::service::bot::BotService;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub bot_service: Arc<BotService>,
    // ... other services
}

// In main.rs, wire everything:
let pool = create_sqlite_pool("sqlite://boternity.db").await?;
let bot_repo = Arc::new(SqliteBotRepository::new(pool.clone()));
let bot_service = Arc::new(BotService::new(bot_repo));
let state = AppState { bot_service };

let app = Router::new()
    .route("/api/bots", get(list_bots).post(create_bot))
    .route("/api/bots/{id}", get(get_bot).delete(delete_bot))
    .with_state(state);
```

### Pattern 3: Split Read/Write SQLite Pools

**What:** Use separate connection pools for reads and writes with SQLite WAL mode.
**When to use:** When you need concurrent read/write safety (INFR-07, INFR-08).
**Example:**

```rust
// crates/boternity-infra/src/sqlite/pool.rs
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::str::FromStr;

pub struct DatabasePool {
    pub reader: SqlitePool,  // Multiple connections for concurrent reads
    pub writer: SqlitePool,  // Single connection for serialized writes
}

impl DatabasePool {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let read_opts = SqliteConnectOptions::from_str(database_url)?
            .journal_mode(SqliteJournalMode::Wal)
            .read_only(true)
            .create_if_missing(true)
            .busy_timeout(std::time::Duration::from_secs(5));

        let write_opts = SqliteConnectOptions::from_str(database_url)?
            .journal_mode(SqliteJournalMode::Wal)
            .create_if_missing(true)
            .busy_timeout(std::time::Duration::from_secs(5));

        let reader = SqlitePoolOptions::new()
            .max_connections(num_cpus::get() as u32 * 2)
            .connect_with(read_opts)
            .await?;

        let writer = SqlitePoolOptions::new()
            .max_connections(1)  // SQLite allows only one writer at a time
            .connect_with(write_opts)
            .await?;

        // Run migrations on the writer connection
        sqlx::migrate!("./migrations")
            .run(&writer)
            .await?;

        Ok(Self { reader, writer })
    }
}
```

### Pattern 4: Secrets Resolution Chain

**What:** Try multiple secret providers in priority order: OS keychain -> encrypted vault -> environment variables.
**When to use:** For SECU-01, SECU-02, SECU-03 -- resolving API keys and credentials.
**Example:**

```rust
// crates/boternity-core/src/service/secret.rs
pub struct SecretService {
    providers: Vec<Arc<dyn SecretProvider>>,
}

impl SecretService {
    pub async fn get_secret(&self, key: &str) -> Result<Option<String>, SecretError> {
        for provider in &self.providers {
            if let Some(value) = provider.get(key).await? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }
}

// Provider trait in boternity-core
pub trait SecretProvider: Send + Sync {
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<String>, SecretError>> + Send;
    fn set(&self, key: &str, value: &str) -> impl Future<Output = Result<(), SecretError>> + Send;
    fn delete(&self, key: &str) -> impl Future<Output = Result<(), SecretError>> + Send;
}
```

### Anti-Patterns to Avoid

- **Core depending on Infra:** boternity-core must NEVER have boternity-infra in its Cargo.toml dependencies. This breaks the entire clean architecture. The dependency direction is: api -> core, infra -> core (implements core traits).
- **Putting sqlx types in core/types:** Domain types should not contain sqlx::FromRow derives. Use separate mapper functions in the infra layer to convert between domain types and database rows.
- **Single SQLite connection without WAL:** Without WAL mode and a write-serialization strategy, concurrent requests will cause "database is locked" errors immediately.
- **Storing secrets in plaintext in SQLite:** Even for a local-only application, secrets must be encrypted at rest. AES-256-GCM with a key derived from a master password or OS keychain-stored key.
- **Using `async_trait` macro with Axum 0.8:** Axum 0.8 removed the need for `#[async_trait]`. Use native `impl Future<Output = ...> + Send` in traits or `async fn` in impl blocks directly (Rust 2024 edition).

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CLI argument parsing | Custom arg parsing | clap 4.5 with derive | Subcommands, help text, shell completions, env var support, validation -- hundreds of edge cases |
| SQL migrations | Manual CREATE TABLE scripts | sqlx::migrate!() | Tracks applied migrations, reversible, compile-time embedded, consistent across environments |
| SHA-256 hashing | Manual implementation | sha2 0.10 crate | Constant-time, hardware-accelerated (SHA-NI), audited by RustCrypto team |
| AES encryption | Custom encryption scheme | aes-gcm 0.10 | Authenticated encryption (prevents tampering), NCC Group audited, constant-time |
| Key derivation | Simple hash of password | argon2 0.5 | Memory-hard KDF, OWASP recommended, resistant to GPU attacks |
| OS keychain access | Platform-specific FFI | keyring 3.6 | Cross-platform (macOS Keychain, Linux Secret Service, Windows Credential Store) |
| UUID generation | Random string IDs | uuid 1.20 with v7 | Time-sortable, guaranteed ordering, standard format, database-index friendly |
| HTTP middleware (CORS, tracing) | Custom middleware | tower-http 0.6 | Battle-tested, axum-native, handles edge cases (preflight, vary headers) |
| Connection pooling | Manual connection management | sqlx built-in Pool | Health checks, idle timeout, connection limits, async-safe |
| JSON error responses | Ad-hoc error formatting | IntoResponse impl + thiserror | Consistent error shapes, proper HTTP status codes, no information leakage |

**Key insight:** The Rust ecosystem has mature, audited solutions for every cryptographic and infrastructure concern in this phase. Hand-rolling any crypto or connection management is a security and reliability risk with zero upside.

## Common Pitfalls

### Pitfall 1: SQLite "Database is Locked" Errors

**What goes wrong:** Concurrent write attempts cause SQLITE_BUSY errors even with WAL mode enabled.
**Why it happens:** SQLite allows only one writer at a time. Without proper write serialization, multiple async tasks try to write simultaneously. Also, using DEFERRED transactions (the default) for writes causes upgrade deadlocks that bypass the busy timeout entirely.
**How to avoid:**
  - Use a single-connection write pool (`max_connections = 1`) alongside a multi-connection read pool
  - Always use `BEGIN IMMEDIATE` for transactions that perform writes
  - Set `busy_timeout` to at least 5 seconds on all connections
  - sqlx handles spawn_blocking internally -- do NOT double-wrap with tokio::task::spawn_blocking
**Warning signs:** Sporadic 500 errors under concurrent API requests, "database is locked" in logs.

### Pitfall 2: WAL Mode Reset on Reconnection

**What goes wrong:** Opening a connection with a different journal_mode to a WAL-mode database silently resets WAL mode, requiring an exclusive lock.
**Why it happens:** sqlx does not set journal_mode by default. If any connection opens without explicitly setting WAL, it can reset the database to rollback journal mode.
**How to avoid:** Always set `.journal_mode(SqliteJournalMode::Wal)` on ALL SqliteConnectOptions (both reader and writer pools). Never rely on the database already being in WAL mode.
**Warning signs:** Intermittent lock errors after connection pool recycling.

### Pitfall 3: Axum 0.8 Path Syntax Change

**What goes wrong:** Routes with parameters don't match, returning 404.
**Why it happens:** Axum 0.8 changed path parameter syntax from `/:param` to `/{param}` and `/*catch` to `/{*catch}`.
**How to avoid:** Always use `{param}` syntax: `.route("/api/bots/{id}", get(get_bot))`. This aligns with OpenAPI and format!() style.
**Warning signs:** All parameterized routes return 404 while static routes work.

### Pitfall 4: Core Crate Accidentally Depending on Infra

**What goes wrong:** boternity-core gains a dependency on sqlx or other infrastructure crates, breaking the clean architecture boundary.
**Why it happens:** Developer convenience -- it's tempting to put sqlx::FromRow on domain types or use sqlx error types in core.
**How to avoid:**
  - Add a CI check: `cargo tree -p boternity-core | grep -q boternity-infra && exit 1`
  - Domain types in boternity-types should only derive serde traits, never database traits
  - Repository traits in core return domain error types, not sqlx errors
**Warning signs:** boternity-core's Cargo.toml has sqlx, diesel, or any database crate in dependencies.

### Pitfall 5: Secrets Appearing in Logs or API Responses

**What goes wrong:** API keys or credentials end up in structured logs, error messages, or JSON responses.
**Why it happens:** Debug derives on types containing secrets, tracing instrumentation on handlers that receive secrets, error messages including secret values.
**How to avoid:**
  - Implement custom Debug for any type holding secrets (redact the value)
  - Use `#[instrument(skip(secret))]` on handlers that receive secrets
  - Never include secret values in error types -- use "secret not found" not "secret 'sk-abc123' not found"
  - Create a `Secret<T>` wrapper type that redacts on Debug/Display
**Warning signs:** Secrets visible in development logs, error responses containing credential fragments.

### Pitfall 6: Missing PRAGMA foreign_keys

**What goes wrong:** Foreign key constraints are silently ignored, leading to orphaned records.
**Why it happens:** SQLite does not enforce foreign keys by default. Each connection must execute `PRAGMA foreign_keys = ON`.
**How to avoid:** sqlx 0.8's SqliteConnectOptions has `.foreign_keys(true)` which is enabled by default. Verify this is not accidentally disabled.
**Warning signs:** Deleting a bot leaves orphaned soul_versions or secret entries in the database.

### Pitfall 7: Turborepo Trying to Build Rust

**What goes wrong:** Turborepo fails or behaves unexpectedly when encountering Rust crates.
**Why it happens:** Turborepo only supports JavaScript/TypeScript packages. If the crates/ directory or Cargo.toml is somehow included in Turborepo's task graph, it will fail.
**How to avoid:** Keep Rust crates in `crates/` directory (not `packages/` or `apps/`). Turborepo's `turbo.json` should only reference JS/TS packages. The two build systems are independent -- use separate npm scripts for Rust builds if needed (`"build:rust": "cargo build --release"`).
**Warning signs:** Turborepo errors about missing package.json in crates/ directories.

## Code Examples

Verified patterns from official sources:

### SQLite Pool Setup with WAL Mode

```rust
// Source: docs.rs/sqlx/0.8/sqlx/sqlite/struct.SqliteConnectOptions.html
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::str::FromStr;

pub async fn create_pool(db_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(db_url)?
        .journal_mode(SqliteJournalMode::Wal)
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
}
```

### Axum 0.8 Router with State

```rust
// Source: docs.rs/axum/0.8/axum + tokio.rs/blog/2025-01-01-announcing-axum-0-8-0
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, delete},
    Router,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub bot_service: Arc<BotService>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/bots", get(list_bots).post(create_bot))
        .route("/api/bots/{id}", get(get_bot).delete(delete_bot))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state)
}

async fn get_bot(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Bot>, StatusCode> {
    let bot_id = BotId(Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?);
    state.bot_service
        .get_bot(&bot_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
```

### Clap CLI with Subcommands

```rust
// Source: docs.rs/clap/4.5/clap
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "boternity", version, about = "Manage your AI bot fleet")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new bot
    Create {
        /// Bot name
        #[arg(short, long)]
        name: String,
        /// Path to SOUL.md file
        #[arg(short, long)]
        soul: Option<std::path::PathBuf>,
    },
    /// List all bots
    List,
    /// Start a bot
    Start {
        /// Bot ID or name
        id: String,
    },
    /// Stop a bot
    Stop {
        /// Bot ID or name
        id: String,
    },
    /// Delete a bot
    Delete {
        /// Bot ID or name
        id: String,
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Manage secrets
    Secret {
        #[command(subcommand)]
        action: SecretCommands,
    },
}

#[derive(Subcommand)]
pub enum SecretCommands {
    /// Set a secret
    Set { key: String, value: String },
    /// Get a secret
    Get { key: String },
    /// Delete a secret
    Delete { key: String },
    /// List all secret keys
    List,
}
```

### SHA-256 File Verification

```rust
// Source: docs.rs/sha2/0.10/sha2 + rust-lang-nursery.github.io/rust-cookbook
use sha2::{Sha256, Digest};
use std::path::Path;
use tokio::fs;

pub async fn compute_file_hash(path: &Path) -> Result<String, std::io::Error> {
    let content = fs::read(path).await?;
    let hash = Sha256::digest(&content);
    Ok(format!("{:x}", hash))
}

pub async fn verify_soul_integrity(
    soul_path: &Path,
    expected_hash: &str,
) -> Result<bool, std::io::Error> {
    let actual_hash = compute_file_hash(soul_path).await?;
    Ok(actual_hash == expected_hash)
}
```

### AES-256-GCM Vault Encryption

```rust
// Source: docs.rs/aes-gcm/0.10/aes_gcm
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use aes_gcm::aead::rand_core::RngCore;

pub struct VaultCrypto {
    cipher: Aes256Gcm,
}

impl VaultCrypto {
    /// Create from a 32-byte key (derived via argon2 from master password)
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new(key.into());
        Self { cipher }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self.cipher.encrypt(nonce, plaintext)?;
        // Prepend nonce to ciphertext for storage
        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        self.cipher.decrypt(nonce, ciphertext)
    }
}
```

### Keyring OS Keychain Integration

```rust
// Source: docs.rs/keyring/3.6
use keyring::Entry;

pub struct KeychainProvider;

impl KeychainProvider {
    const SERVICE: &'static str = "boternity";

    pub fn get_secret(key: &str) -> Result<Option<String>, keyring::Error> {
        let entry = Entry::new(Self::SERVICE, key)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn set_secret(key: &str, value: &str) -> Result<(), keyring::Error> {
        let entry = Entry::new(Self::SERVICE, key)?;
        entry.set_password(value)
    }

    pub fn delete_secret(key: &str) -> Result<(), keyring::Error> {
        let entry = Entry::new(Self::SERVICE, key)?;
        entry.delete_credential()
    }
}
```

### Workspace Cargo.toml

```toml
# Root Cargo.toml
[workspace]
resolver = "3"
members = [
    "crates/boternity-types",
    "crates/boternity-core",
    "crates/boternity-infra",
    "crates/boternity-api",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/user/boternity"

[workspace.dependencies]
# All shared dependencies defined here, consumed via { workspace = true }
axum = { version = "0.8", features = ["macros"] }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
clap = { version = "4.5", features = ["derive", "env"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1.20", features = ["v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip"] }
keyring = { version = "3.6", features = ["apple-native", "sync-secret-service", "crypto-rust"] }
aes-gcm = "0.10"
argon2 = "0.5"
sha2 = "0.10"
thiserror = "1"
anyhow = "1"
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `#[async_trait]` on trait impls | Native async fn in traits (RPITIT) | Rust 1.75 (Dec 2023), Axum 0.8 (Jan 2025) | Remove async_trait dependency, cleaner trait definitions, no hidden allocations |
| Axum `/:param` path syntax | Axum `/{param}` syntax | Axum 0.8.0 (Jan 2025) | Aligns with OpenAPI, allows literal `:` in paths |
| UUID v4 (random) for DB keys | UUID v7 (time-sorted) | uuid 1.9.0 (2024) | Natural ordering, better index performance, guaranteed process-local ordering |
| Cargo resolver "2" | Cargo resolver "3" | Cargo 1.84 / Rust 2024 Edition | Improved dependency resolution algorithm |
| rusqlite + r2d2 for SQLite | sqlx with built-in async | sqlx 0.5+ (2021+) | Compile-time query checks, built-in migrations, PostgreSQL migration path |
| Manual PRAGMA setup | sqlx SqliteConnectOptions builder | sqlx 0.6+ | Type-safe configuration, no raw SQL PRAGMAs needed |

**Deprecated/outdated:**
- **`#[async_trait]` macro**: No longer needed with Rust 2024 edition and Axum 0.8. Remove from all trait definitions and implementations.
- **Axum `/:param` syntax**: Will not compile in Axum 0.8. Must use `/{param}`.
- **`axum::Server`**: Removed. Use `axum::serve(listener, app)` instead.
- **`into_make_service()`**: No longer needed for basic server setup in Axum 0.8.

## Open Questions

Things that couldn't be fully resolved:

1. **Vault Master Key Source**
   - What we know: Argon2id should derive the encryption key from a master password. The keyring crate can store the derived key in OS keychain.
   - What's unclear: Should the user set a master password on first run? Or should we auto-generate a key and store it in OS keychain (simpler UX, less portable)?
   - Recommendation: Default to auto-generated key stored in OS keychain. Offer `--master-password` flag for users who want portable vault encryption. Implement both paths.

2. **sqlx Compile-Time Check Database**
   - What we know: sqlx query!() macros require a live database at compile time (or a cached sqlx-data.json / .sqlx directory).
   - What's unclear: How to handle this in CI and fresh clones where no database exists.
   - Recommendation: Use `sqlx prepare` to generate offline query data (`.sqlx/` directory). Commit this to the repo. CI verifies with `cargo sqlx prepare --check`.

3. **Turborepo + Cargo Workspace Integration Depth**
   - What we know: Turborepo only manages JS/TS packages. Cargo workspace is independent.
   - What's unclear: Whether to use Turborepo task definitions to also trigger Cargo builds (via npm scripts) or keep them entirely separate.
   - Recommendation: Keep them separate for Phase 1. Root package.json can have convenience scripts (`"build:rust": "cargo build --release"`) but Turborepo's turbo.json should only define pipelines for JS/TS packages. Revisit when web UI is added in Phase 4.

4. **SOUL.md File Storage Location**
   - What we know: Each bot has SOUL.md, IDENTITY.md, USER.md files. They need to be on the filesystem for easy editing.
   - What's unclear: Store under a configurable data directory (`~/.boternity/bots/{id}/`)? Or alongside the project? Database stores metadata + hash, filesystem stores the actual markdown.
   - Recommendation: Use `~/.boternity/` as default data directory (configurable via env var `BOTERNITY_DATA_DIR`). Structure: `~/.boternity/bots/{bot-id}/SOUL.md`, `IDENTITY.md`, `USER.md`. Database stores the soul version history with content snapshots for versioning.

## Sources

### Primary (HIGH confidence)
- [docs.rs/axum/0.8.8](https://docs.rs/axum/latest/axum/) - Version, Router API, State management, extractors
- [tokio.rs/blog/2025-01-01-announcing-axum-0-8-0](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0) - Axum 0.8 breaking changes, migration guide
- [docs.rs/sqlx/0.8.6](https://docs.rs/sqlx/latest/sqlx/) - Version, SqliteConnectOptions, WAL mode, Pool, migrations
- [docs.rs/clap/4.5.57](https://docs.rs/clap/latest/clap/) - Version, derive API, features
- [docs.rs/keyring/3.6.3](https://docs.rs/keyring) - Version, platform support, feature flags, API
- [docs.rs/aes-gcm/0.10.3](https://docs.rs/aes-gcm/latest/aes_gcm/) - Version, API, security audit status
- [docs.rs/sha2/0.10.9](https://docs.rs/sha2) - Version, Digest trait, hashing API
- [docs.rs/uuid/1.20.0](https://docs.rs/uuid/latest/uuid/struct.Uuid.html) - Version, v7 support, ordering guarantees
- [doc.rust-lang.org/cargo/reference/workspaces.html](https://doc.rust-lang.org/cargo/reference/workspaces.html) - Workspace configuration, resolver, dependency sharing

### Secondary (MEDIUM confidence)
- [github.com/spa5k/monorepo-typescript-rust](https://github.com/spa5k/monorepo-typescript-rust) - Turborepo + Cargo workspace coexistence pattern (verified via WebFetch)
- [Axum DDD article (Medium, 2026)](https://medium.com/@qkpiot/building-a-robust-rust-backend-with-axum-diesel-postgresql-and-ddd-from-concept-to-deployment-b25cf5c65bc8) - Clean architecture with Axum, verified against official docs
- [Hexagonal Architecture in Rust (howtocodeit.com)](https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust) - Repository trait patterns
- [OneUptime Axum REST API (2026)](https://oneuptime.com/blog/post/2026-01-07-rust-axum-rest-api/view) - Production Axum 0.8 patterns
- [thiserror/anyhow best practices (2026)](https://oneuptime.com/blog/post/2026-01-25-error-types-thiserror-anyhow-rust/view) - Error handling patterns
- [SQLite concurrent writes (tenthousandmeters.com)](https://tenthousandmeters.com/blog/sqlite-concurrent-writes-and-database-is-locked-errors/) - WAL mode pitfalls, BEGIN IMMEDIATE
- [kodraus.github.io uuid v7 counters](https://kodraus.github.io/rust/2024/06/24/uuid-v7-counters.html) - UUID v7 ordering guarantees

### Tertiary (LOW confidence)
- [Turborepo Rust support issue #683](https://github.com/vercel/turborepo/issues/683) - Turborepo does not support Rust natively (multiple community reports, not officially documented as limitation)
- [Argon2 OWASP recommendation](https://github.com/RustCrypto/password-hashes/tree/master/argon2) - OWASP parameters (19 MiB, 2 iterations, 1 parallelism) -- needs validation against current OWASP docs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All library versions verified via docs.rs, APIs confirmed via official documentation
- Architecture: HIGH - Clean architecture / repository pattern is well-established in Rust, multiple recent (2026) production examples confirm patterns
- Pitfalls: HIGH - SQLite locking issues are extensively documented; Axum 0.8 breaking changes verified via official announcement
- Crypto: HIGH - All cryptographic crates are from RustCrypto ecosystem with security audits
- Turborepo integration: MEDIUM - Coexistence pattern confirmed but limited examples of production Turborepo + Cargo workspace monorepos

**Research date:** 2026-02-10
**Valid until:** 2026-03-12 (30 days -- stable ecosystem, all libraries are mature)
