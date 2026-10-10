-- Persist Move, Resign, and Chat in an append-only log. Ping/Pong and
-- SessionInfo remain ephemeral and are reconstructed on connection.

-- Store ChessMessage kind tags without a constraint so new variants need no migration.
CREATE TABLE IF NOT EXISTS game_event_log (
    game_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    kind TEXT NOT NULL,
    version_hash TEXT NOT NULL,
    parent_version TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (game_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_game_event_log_game ON game_event_log(game_id, seq);
