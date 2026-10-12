-- Backend-held session keys that co-sign a game's moves for a wallet.

CREATE TABLE IF NOT EXISTS sessions (
    game_id    INTEGER PRIMARY KEY,
    keypair    BLOB    NOT NULL,             -- encrypted with the identity vault
    wallet     TEXT    NOT NULL,
    active     INTEGER NOT NULL DEFAULT 0,
    move_count INTEGER NOT NULL DEFAULT 0,
    is_global  INTEGER NOT NULL DEFAULT 0,   -- global_record_move: no per-game SessionDelegation
    created_at INTEGER NOT NULL DEFAULT 0    -- expires abandoned wallet prompts from the pending cap
);
ALTER TABLE sessions ADD COLUMN move_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN is_global INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0;

-- Activation is idempotent per (game, wallet), so the host's activation does
-- not suppress the joiner's.
CREATE TABLE IF NOT EXISTS session_wallet_activations (
    game_id INTEGER NOT NULL,
    wallet  TEXT    NOT NULL,
    sig     TEXT    NOT NULL,
    PRIMARY KEY (game_id, wallet)
);
