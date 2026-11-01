-- Move rules state and live events from KV into D1, and optimize existing
-- indexes. Applied automatically on deploy via `wrangler d1 migrations apply`.

-- Single-row table holding the serialized rule list and monotonic CAS version.
CREATE TABLE IF NOT EXISTS rules_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    version INTEGER NOT NULL DEFAULT 0,
    rules_json TEXT NOT NULL DEFAULT '[]',
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Ring-buffer table for recent live-feed events (bounded to MAX_EVENTS rows).
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts INTEGER NOT NULL,
    kind TEXT NOT NULL,
    sender TEXT NOT NULL,
    recipient TEXT NOT NULL,
    rule_id TEXT,
    channels_json TEXT NOT NULL DEFAULT '[]',
    size_bytes INTEGER NOT NULL DEFAULT 0,
    error TEXT
);
CREATE INDEX IF NOT EXISTS idx_events_ts ON events(ts DESC, id DESC);

-- Drop redundant index on reverse_mappings(id) (PRIMARY KEY already indexes id)
-- and add a lookup index on (alias_address, original_sender) to reuse reverse
-- aliases across repeat emails in the same sender/alias pair. Non-unique so
-- existing databases with historical duplicate pairs migrate cleanly.
DROP INDEX IF EXISTS idx_reverse_mappings_id;
CREATE INDEX IF NOT EXISTS idx_reverse_mappings_pair
    ON reverse_mappings(alias_address, original_sender);

-- Replace single-column pending_dispatches index with composite index matching
-- `ORDER BY dead_lettered DESC, updated_at DESC`.
DROP INDEX IF EXISTS idx_pending_dispatches_dead;
CREATE INDEX IF NOT EXISTS idx_pending_dispatches_dead_updated
    ON pending_dispatches(dead_lettered DESC, updated_at DESC);

-- Add `id DESC` tie-breaker to messages pagination indexes.
DROP INDEX IF EXISTS idx_messages_created_at;
CREATE INDEX IF NOT EXISTS idx_messages_created_at
    ON messages(created_at DESC, id DESC);
DROP INDEX IF EXISTS idx_messages_rule_created;
CREATE INDEX IF NOT EXISTS idx_messages_rule_created
    ON messages(rule_id, created_at DESC, id DESC);
