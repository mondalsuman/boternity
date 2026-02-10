-- Boternity initial schema
-- SQLite with WAL mode, foreign keys enforced

-- Bots table: core entity for each bot in the fleet
CREATE TABLE IF NOT EXISTS bots (
    id TEXT PRIMARY KEY NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'disabled', 'archived')),
    category TEXT NOT NULL DEFAULT 'assistant' CHECK(category IN ('assistant', 'creative', 'research', 'utility')),
    tags TEXT NOT NULL DEFAULT '[]',
    user_id TEXT,
    conversation_count INTEGER NOT NULL DEFAULT 0,
    total_tokens_used INTEGER NOT NULL DEFAULT 0,
    version_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_active_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_bots_slug ON bots(slug);
CREATE INDEX IF NOT EXISTS idx_bots_status ON bots(status);
CREATE INDEX IF NOT EXISTS idx_bots_category ON bots(category);
CREATE INDEX IF NOT EXISTS idx_bots_created_at ON bots(created_at);

-- Soul versions table: immutable versioned SOUL.md content per bot
CREATE TABLE IF NOT EXISTS soul_versions (
    id TEXT PRIMARY KEY NOT NULL,
    bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    hash TEXT NOT NULL,
    version INTEGER NOT NULL,
    message TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(bot_id, version)
);

CREATE INDEX IF NOT EXISTS idx_soul_versions_bot_id ON soul_versions(bot_id);
CREATE INDEX IF NOT EXISTS idx_soul_versions_bot_id_version ON soul_versions(bot_id, version);

-- Secrets table: encrypted secret storage (vault provider)
CREATE TABLE IF NOT EXISTS secrets (
    id TEXT PRIMARY KEY NOT NULL,
    key TEXT NOT NULL,
    encrypted_value BLOB NOT NULL,
    scope TEXT NOT NULL DEFAULT 'global',
    provider TEXT NOT NULL DEFAULT 'vault' CHECK(provider IN ('vault', 'keychain', 'environment')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(key, scope)
);

CREATE INDEX IF NOT EXISTS idx_secrets_key_scope ON secrets(key, scope);

-- API keys table: authentication for REST API
CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY NOT NULL,
    key_hash TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT 'default',
    created_at TEXT NOT NULL,
    last_used_at TEXT
);
