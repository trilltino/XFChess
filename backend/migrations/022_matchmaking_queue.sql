
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
