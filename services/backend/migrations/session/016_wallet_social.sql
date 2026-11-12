-- Wallet-keyed friendships, the last node seen for a wallet, and directional blocks.
-- A friendship between two verified wallets outlives either player's device; the
-- node-keyed `contacts` rows are re-derived from it (see FriendManager::sync_wallet_contacts).

CREATE TABLE IF NOT EXISTS wallet_friendships (
    wallet_lo  TEXT NOT NULL,
    wallet_hi  TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (wallet_lo, wallet_hi),
    CHECK (wallet_lo < wallet_hi)
);
CREATE INDEX IF NOT EXISTS idx_wallet_friendships_hi ON wallet_friendships (wallet_hi);

-- Updated from verified presence: which node a wallet most recently played from.
CREATE TABLE IF NOT EXISTS wallet_nodes (
    wallet     TEXT PRIMARY KEY,
    node_id    TEXT NOT NULL,
    display    TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_wallet_nodes_node ON wallet_nodes (node_id);

-- One row per blocker/blocked node. The wallets are recorded when known so a block
-- also holds when the blocked player returns from another device.
CREATE TABLE IF NOT EXISTS blocks (
    blocker_node_id TEXT NOT NULL,
    blocked_node_id TEXT NOT NULL,
    blocker_pubkey  TEXT,
    blocked_pubkey  TEXT,
    blocked_display TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL,
    PRIMARY KEY (blocker_node_id, blocked_node_id)
);
CREATE INDEX IF NOT EXISTS idx_blocks_blocker_pk ON blocks (blocker_pubkey);
CREATE INDEX IF NOT EXISTS idx_blocks_blocked_pk ON blocks (blocked_pubkey);
