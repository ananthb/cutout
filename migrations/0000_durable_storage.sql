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
