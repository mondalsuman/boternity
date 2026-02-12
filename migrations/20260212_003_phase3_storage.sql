-- Boternity Phase 3: KV store, memory audit log, provider health, file metadata
-- SQLite with WAL mode, foreign keys enforced

-- Bot key-value store: arbitrary JSON values per bot
CREATE TABLE IF NOT EXISTS bot_kv_store (
    bot_id      TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,      -- JSON-encoded value
    created_at  TEXT NOT NULL,      -- ISO 8601
    updated_at  TEXT NOT NULL,      -- ISO 8601
    PRIMARY KEY (bot_id, key)
);

CREATE INDEX IF NOT EXISTS idx_kv_store_bot_id ON bot_kv_store(bot_id);

-- Memory audit log: tracks add/delete/share/revoke/merge actions
CREATE TABLE IF NOT EXISTS memory_audit_log (
    id          TEXT PRIMARY KEY NOT NULL,   -- UUIDv7
    bot_id      TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    memory_id   TEXT NOT NULL,               -- References a memory entry (may be deleted)
    action      TEXT NOT NULL CHECK (action IN ('add', 'delete', 'share', 'revoke', 'merge')),
    actor       TEXT NOT NULL,               -- 'system', 'user', or a bot slug
    details     TEXT,                         -- Optional JSON context about the action
    created_at  TEXT NOT NULL                 -- ISO 8601
);

CREATE INDEX IF NOT EXISTS idx_audit_log_bot_id ON memory_audit_log(bot_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_memory_id ON memory_audit_log(memory_id, created_at DESC);

-- Provider health: persists circuit breaker state across restarts
CREATE TABLE IF NOT EXISTS provider_health (
    name            TEXT PRIMARY KEY NOT NULL,   -- Provider name (matches ProviderConfig.name)
    priority        INTEGER NOT NULL DEFAULT 0,
    circuit_state   TEXT NOT NULL DEFAULT 'closed' CHECK (circuit_state IN ('closed', 'open', 'half_open')),
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT,
    last_latency_ms INTEGER,
    total_calls     INTEGER NOT NULL DEFAULT 0,
    total_failures  INTEGER NOT NULL DEFAULT 0,
    uptime_since    TEXT,                        -- ISO 8601 (NULL if circuit is open)
    updated_at      TEXT NOT NULL                -- ISO 8601
);

-- Bot files: metadata for files stored in bot file storage
CREATE TABLE IF NOT EXISTS bot_files (
    id          TEXT PRIMARY KEY NOT NULL,   -- UUIDv7
    bot_id      TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    filename    TEXT NOT NULL,
    mime_type   TEXT NOT NULL DEFAULT 'application/octet-stream',
    size_bytes  INTEGER NOT NULL DEFAULT 0,
    version     INTEGER NOT NULL DEFAULT 1,
    is_indexed  INTEGER NOT NULL DEFAULT 0, -- 1 if text content has been chunked and embedded
    created_at  TEXT NOT NULL,              -- ISO 8601
    updated_at  TEXT NOT NULL,              -- ISO 8601
    UNIQUE(bot_id, filename)
);

CREATE INDEX IF NOT EXISTS idx_bot_files_bot_id ON bot_files(bot_id);

-- Bot file versions: versioned snapshots of files
CREATE TABLE IF NOT EXISTS bot_file_versions (
    id          TEXT PRIMARY KEY NOT NULL,   -- UUIDv7
    file_id     TEXT NOT NULL REFERENCES bot_files(id) ON DELETE CASCADE,
    version     INTEGER NOT NULL,
    size_bytes  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,              -- ISO 8601
    UNIQUE(file_id, version)
);

CREATE INDEX IF NOT EXISTS idx_file_versions_file_id ON bot_file_versions(file_id);
