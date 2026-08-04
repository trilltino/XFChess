-- Migration 009: External ELO linking (Lichess) tables.
-- All tables on the sessions pool (sessions.db).

-- Links between wallet pubkeys and external chess platform accounts.
CREATE TABLE IF NOT EXISTS external_elo_links (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    pubkey              TEXT    NOT NULL,
    platform            TEXT    NOT NULL CHECK(platform IN ('lichess')),
    username            TEXT    NOT NULL,
    verified            INTEGER NOT NULL DEFAULT 0,  -- bool
    blitz_rating        INTEGER,
    rapid_rating        INTEGER,
    bullet_rating       INTEGER,
    games_count         INTEGER,  -- rated games on platform (optional)
    account_created_at  INTEGER,  -- Unix timestamp (optional)
    linked_at           INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    last_sync_at        INTEGER,
    on_chain_tx         TEXT,  -- transaction signature
    UNIQUE(pubkey, platform),
    UNIQUE(platform, username)  -- one username per platform globally
);

CREATE INDEX IF NOT EXISTS idx_external_elo_pubkey ON external_elo_links(pubkey);
CREATE INDEX IF NOT EXISTS idx_external_elo_platform_username ON external_elo_links(platform, username);

-- Audit log of external ELO sync events.
CREATE TABLE IF NOT EXISTS external_elo_sync_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    pubkey      TEXT    NOT NULL,
    platform    TEXT    NOT NULL,
    old_rating  INTEGER,
    new_rating  INTEGER,
    sync_type   TEXT    NOT NULL CHECK(sync_type IN ('initial', 'weekly', 'manual')),
    synced_at   INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    on_chain_tx TEXT
);

CREATE INDEX IF NOT EXISTS idx_external_elo_sync_pubkey ON external_elo_sync_log(pubkey);
