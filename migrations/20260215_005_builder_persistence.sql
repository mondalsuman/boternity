-- Boternity Phase 7: Builder draft persistence and memory
-- Enables auto-save of builder sessions (resume interrupted sessions)
-- and builder memory (recall past session choices for suggestions).

CREATE TABLE IF NOT EXISTS builder_drafts (
    session_id      TEXT PRIMARY KEY,
    state_json      TEXT NOT NULL,
    schema_version  INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS builder_memory (
    id                  TEXT PRIMARY KEY,
    purpose_category    TEXT NOT NULL,
    initial_description TEXT NOT NULL,
    chosen_tone         TEXT,
    chosen_model        TEXT,
    chosen_skills       TEXT NOT NULL DEFAULT '[]',
    bot_slug            TEXT,
    created_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_builder_memory_category ON builder_memory(purpose_category);
