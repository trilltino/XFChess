-- Friends and contacts social graph
-- Primary key is the Iroh node ID; Solana pubkey is optional.

CREATE TABLE IF NOT EXISTS friend_requests (
    id           TEXT PRIMARY KEY,
    from_node_id TEXT NOT NULL,
    from_pubkey  TEXT,
    from_display TEXT NOT NULL,
    to_node_id   TEXT,
    to_pubkey    TEXT,
    message      TEXT,
    status       TEXT NOT NULL DEFAULT 'pending',
    created_at   TEXT NOT NULL,
    responded_at TEXT,
    UNIQUE(from_node_id, to_node_id),
    UNIQUE(from_node_id, to_pubkey)
);

CREATE TABLE IF NOT EXISTS contacts (
    id               TEXT PRIMARY KEY,
    owner_node_id    TEXT NOT NULL,
    contact_node_id  TEXT NOT NULL,
    contact_pubkey   TEXT,
    contact_display  TEXT NOT NULL,
    contact_elo      INTEGER,
    created_at       TEXT NOT NULL,
    UNIQUE(owner_node_id, contact_node_id)
);

CREATE INDEX IF NOT EXISTS idx_friend_requests_to_node  ON friend_requests(to_node_id);
CREATE INDEX IF NOT EXISTS idx_friend_requests_to_pk    ON friend_requests(to_pubkey);
CREATE INDEX IF NOT EXISTS idx_contacts_owner           ON contacts(owner_node_id);
