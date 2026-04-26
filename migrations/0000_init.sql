-- Initial schema for Cutout. All statements are idempotent so this file is
-- the single source of schema truth: the deploy workflow re-executes it on
-- every deploy via `wrangler d1 execute --file migrations/0000_init.sql`.

-- Durable reverse alias mappings
CREATE TABLE IF NOT EXISTS reverse_mappings (
    id TEXT PRIMARY KEY, -- The generated UUID in `reply+<id>@domain`
    alias_address TEXT NOT NULL, -- The address the original sender emailed (the alias)
    original_sender TEXT NOT NULL, -- The external sender's real address
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_reverse_mappings_id ON reverse_mappings(id);

-- Durable bot reply contexts
CREATE TABLE IF NOT EXISTS bot_reply_contexts (
    key TEXT PRIMARY KEY, -- 'tg:<chat_id>:<msg_id>' or 'dc:<channel_id>:<msg_id>'
    alias_address TEXT NOT NULL,
    original_sender TEXT NOT NULL,
    subject TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Stored emails (Action::Store with persist=true). Body lives in R2 at the
-- `r2_key` (typically `messages/<id>`); D1 keeps only metadata.
DROP TABLE IF EXISTS messages;
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    sender TEXT NOT NULL,
    recipient TEXT NOT NULL,
    subject TEXT NOT NULL,
    r2_key TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_messages_recipient ON messages(recipient);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at);

-- Failed dispatches awaiting retry. Raw bytes live in R2 at `r2_key`
-- (`pending/<id>`); `pending_actions` is a JSON-encoded list of the still-
-- failing destinations so the queue consumer knows what to re-run without
-- re-matching against rules. `dead_lettered = 1` means the platform has
-- moved the queue message to the DLQ and the operator must intervene.
CREATE TABLE IF NOT EXISTS pending_dispatches (
    id TEXT PRIMARY KEY,
    sender TEXT NOT NULL,
    recipient TEXT NOT NULL,
    rule_id TEXT,
    r2_key TEXT NOT NULL,
    pending_actions TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    dead_lettered INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_pending_dispatches_dead ON pending_dispatches(dead_lettered);
