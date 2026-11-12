-- Linked Lichess accounts: imported ratings and the OAuth token used for puzzles.

CREATE TABLE IF NOT EXISTS external_elo_links (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    pubkey              TEXT    NOT NULL,
    platform            TEXT    NOT NULL CHECK(platform IN ('lichess')),
    username            TEXT    NOT NULL,
    verified            INTEGER NOT NULL DEFAULT 0,
    blitz_rating        INTEGER,
    rapid_rating        INTEGER,
    bullet_rating       INTEGER,
    games_count         INTEGER,            -- rated games on the platform
    account_created_at  INTEGER,
    linked_at           INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    last_sync_at        INTEGER,
    on_chain_tx         TEXT,
    UNIQUE(pubkey, platform),
    UNIQUE(platform, username)              -- one wallet per platform account
);

-- Covered by UNIQUE(pubkey, platform) and UNIQUE(platform, username).
DROP INDEX IF EXISTS idx_external_elo_pubkey;
DROP INDEX IF EXISTS idx_external_elo_platform_username;

-- Encrypted with the identity vault; used server-side only, to fetch unseen
-- puzzles and report results back.
CREATE TABLE IF NOT EXISTS lichess_tokens (
    pubkey     TEXT    PRIMARY KEY,
    token_enc  BLOB    NOT NULL,
    scope      TEXT    NOT NULL,
    created_at INTEGER NOT NULL
);
