-- Workflow definitions (canonical IR stored as JSON).
CREATE TABLE IF NOT EXISTS workflows (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    owner_type  TEXT NOT NULL,
    owner_bot_id TEXT,
    definition  TEXT NOT NULL, -- Full WorkflowDefinition as JSON
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_workflows_name_owner
    ON workflows(name, owner_type, COALESCE(owner_bot_id, ''));

-- Workflow execution runs.
CREATE TABLE IF NOT EXISTS workflow_runs (
    id              TEXT PRIMARY KEY,
    workflow_id     TEXT NOT NULL REFERENCES workflows(id),
    workflow_name   TEXT NOT NULL,
    status          TEXT NOT NULL CHECK(status IN ('pending','running','paused','completed','failed','crashed','cancelled')),
    trigger_type    TEXT NOT NULL,
    trigger_payload TEXT,
    context         TEXT NOT NULL DEFAULT '{}',
    started_at      TEXT NOT NULL,
    completed_at    TEXT,
    error           TEXT,
    concurrency_key TEXT
);

CREATE INDEX IF NOT EXISTS idx_workflow_runs_workflow_id ON workflow_runs(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflow_runs_status ON workflow_runs(status);

-- Workflow step execution logs.
CREATE TABLE IF NOT EXISTS workflow_steps (
    id              TEXT PRIMARY KEY,
    run_id          TEXT NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
    step_id         TEXT NOT NULL,
    step_name       TEXT NOT NULL,
    status          TEXT NOT NULL,
    attempt         INTEGER NOT NULL DEFAULT 1,
    idempotency_key TEXT,
    input           TEXT,
    output          TEXT,
    error           TEXT,
    started_at      TEXT,
    completed_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_workflow_steps_run_id ON workflow_steps(run_id);

-- Bot-to-bot messages (audit trail).
CREATE TABLE IF NOT EXISTS bot_messages (
    id                TEXT PRIMARY KEY,
    sender_bot_id     TEXT NOT NULL,
    sender_bot_name   TEXT NOT NULL,
    recipient_type    TEXT NOT NULL,  -- 'direct' or 'channel'
    recipient_bot_id  TEXT,           -- set for direct messages
    recipient_channel TEXT,           -- set for channel messages
    message_type      TEXT NOT NULL,
    body              TEXT NOT NULL,  -- JSON
    reply_to          TEXT,
    timestamp         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_bot_messages_pair ON bot_messages(sender_bot_id, recipient_bot_id);
CREATE INDEX IF NOT EXISTS idx_bot_messages_channel ON bot_messages(recipient_channel);
CREATE INDEX IF NOT EXISTS idx_bot_messages_timestamp ON bot_messages(timestamp);

-- Pub/sub channels for bot communication.
CREATE TABLE IF NOT EXISTS bot_channels (
    name              TEXT PRIMARY KEY,
    created_at        TEXT NOT NULL,
    created_by_bot_id TEXT NOT NULL
);

-- Bot subscriptions to channels.
CREATE TABLE IF NOT EXISTS bot_subscriptions (
    bot_id        TEXT NOT NULL,
    channel_name  TEXT NOT NULL REFERENCES bot_channels(name) ON DELETE CASCADE,
    subscribed_at TEXT NOT NULL,
    PRIMARY KEY (bot_id, channel_name)
);
