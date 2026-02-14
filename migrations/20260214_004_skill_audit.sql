-- Boternity Phase 6: Skill audit log
-- Tracks every skill invocation for security auditing and debugging.

CREATE TABLE IF NOT EXISTS skill_audit_log (
    invocation_id     TEXT PRIMARY KEY,
    skill_name        TEXT NOT NULL,
    skill_version     TEXT NOT NULL,
    trust_tier        TEXT NOT NULL,
    capabilities_used TEXT NOT NULL,      -- JSON array of capability strings
    input_hash        TEXT NOT NULL,      -- SHA-256 hash of input
    output_hash       TEXT NOT NULL,      -- SHA-256 hash of output
    fuel_consumed     INTEGER,
    memory_peak_bytes INTEGER,
    duration_ms       INTEGER NOT NULL,
    success           INTEGER NOT NULL,   -- 0 or 1
    error             TEXT,
    timestamp         TEXT NOT NULL,      -- ISO 8601
    bot_id            TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_skill_audit_bot ON skill_audit_log(bot_id);
CREATE INDEX IF NOT EXISTS idx_skill_audit_skill ON skill_audit_log(skill_name);
CREATE INDEX IF NOT EXISTS idx_skill_audit_timestamp ON skill_audit_log(timestamp);
