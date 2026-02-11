-- Boternity chat and memory schema
-- Chat sessions, messages, session memories, pending extractions, context summaries

-- Chat sessions table
CREATE TABLE IF NOT EXISTS chat_sessions (
    id              TEXT PRIMARY KEY NOT NULL,           -- UUIDv7
    bot_id          TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    title           TEXT,                                -- Auto-generated from first exchange
    started_at      TEXT NOT NULL,                       -- ISO 8601
    ended_at        TEXT,                                -- NULL if session is active
    total_input_tokens  INTEGER NOT NULL DEFAULT 0,
    total_output_tokens INTEGER NOT NULL DEFAULT 0,
    message_count   INTEGER NOT NULL DEFAULT 0,
    model           TEXT NOT NULL,                       -- Model used for this session
    status          TEXT NOT NULL DEFAULT 'active'       -- active|completed|crashed
        CHECK (status IN ('active', 'completed', 'crashed'))
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_bot_id ON chat_sessions(bot_id);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_started_at ON chat_sessions(started_at DESC);

-- Chat messages table
CREATE TABLE IF NOT EXISTS chat_messages (
    id              TEXT PRIMARY KEY NOT NULL,           -- UUIDv7
    session_id      TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role            TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content         TEXT NOT NULL,                       -- Message text
    created_at      TEXT NOT NULL,                       -- ISO 8601
    input_tokens    INTEGER,                             -- For assistant messages
    output_tokens   INTEGER,                             -- For assistant messages
    model           TEXT,                                -- Which model generated this
    stop_reason     TEXT,                                -- end_turn|tool_use|max_tokens
    response_ms     INTEGER                              -- Response time in milliseconds
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at);

-- Session memories table
CREATE TABLE IF NOT EXISTS session_memories (
    id              TEXT PRIMARY KEY NOT NULL,           -- UUIDv7
    bot_id          TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
    session_id      TEXT NOT NULL REFERENCES chat_sessions(id),
    fact            TEXT NOT NULL,                       -- The extracted key point
    category        TEXT NOT NULL                        -- preference|fact|decision|context|correction
        CHECK (category IN ('preference', 'fact', 'decision', 'context', 'correction')),
    importance      INTEGER NOT NULL CHECK (importance BETWEEN 1 AND 5),
    source_message_id TEXT,                              -- Message that triggered this memory
    superseded_by   TEXT,                                -- FK to session_memories.id (for corrections)
    created_at      TEXT NOT NULL,                       -- ISO 8601
    is_manual       INTEGER NOT NULL DEFAULT 0           -- 1 if injected via /remember or bnity remember
);

CREATE INDEX IF NOT EXISTS idx_memories_bot_importance
    ON session_memories(bot_id, importance DESC, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memories_session
    ON session_memories(session_id);

-- Pending memory extractions queue (retry on failure)
CREATE TABLE IF NOT EXISTS pending_memory_extractions (
    id              TEXT PRIMARY KEY NOT NULL,
    session_id      TEXT NOT NULL REFERENCES chat_sessions(id),
    bot_id          TEXT NOT NULL REFERENCES bots(id),
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TEXT,
    next_attempt_at TEXT NOT NULL,
    error_message   TEXT,
    created_at      TEXT NOT NULL
);

-- Context summaries for sliding window
CREATE TABLE IF NOT EXISTS context_summaries (
    id              TEXT PRIMARY KEY NOT NULL,           -- UUIDv7
    session_id      TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    summary         TEXT NOT NULL,                       -- LLM-generated summary of older messages
    messages_start  INTEGER NOT NULL,                    -- First message index summarized
    messages_end    INTEGER NOT NULL,                    -- Last message index summarized
    token_count     INTEGER NOT NULL,                    -- Estimated tokens in this summary
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_context_summaries_session
    ON context_summaries(session_id, created_at DESC);
