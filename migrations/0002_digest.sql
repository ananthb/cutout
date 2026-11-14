-- Per-destination pending digest entries and the last-sent watermark.
CREATE TABLE IF NOT EXISTS digest_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id TEXT NOT NULL,
    destination TEXT NOT NULL,
    message_id TEXT NOT NULL,
    sender TEXT NOT NULL,
    recipient TEXT NOT NULL,
    subject TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_digest_items_dest
    ON digest_items(rule_id, destination, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_digest_items_unique
    ON digest_items(rule_id, destination, message_id);

CREATE TABLE IF NOT EXISTS digest_state (
    rule_id TEXT NOT NULL,
    destination TEXT NOT NULL,
    last_sent_at INTEGER NOT NULL,
    PRIMARY KEY (rule_id, destination)
);
