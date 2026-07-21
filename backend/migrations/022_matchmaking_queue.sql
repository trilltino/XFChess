-- Migration 022: matchmaking_queue + matchmaking_matches tables
--
-- SharedMatchmakingState (backend/src/signing/routes/matchmaking/state.rs)
-- previously lived only in Arc<Mutex<Vec/HashMap>> — a backend restart
-- silently dropped every queued player and pending match. These tables let
-- it hydrate from disk on startup instead of losing state.

CREATE TABLE IF NOT EXISTS matchmaking_queue (
    pubkey TEXT PRIMARY KEY,
    elo INTEGER NOT NULL,
    joined_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS matchmaking_matches (
    pubkey TEXT PRIMARY KEY,
    game_id INTEGER NOT NULL,
    opponent TEXT NOT NULL,
    is_white INTEGER NOT NULL,
    matched_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_matchmaking_queue_joined ON matchmaking_queue(joined_at);
CREATE INDEX IF NOT EXISTS idx_matchmaking_matches_matched ON matchmaking_matches(matched_at);
