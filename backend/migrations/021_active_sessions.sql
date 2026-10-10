
CREATE TABLE IF NOT EXISTS active_sessions (
    session_id TEXT PRIMARY KEY,
    game_id INTEGER NOT NULL,
    player_white TEXT NOT NULL,
    player_black TEXT NOT NULL,
    current_fen TEXT NOT NULL,
    move_history TEXT NOT NULL,
    white_time_ms INTEGER,
    black_time_ms INTEGER,
    last_activity INTEGER NOT NULL,
    grace_period_ends INTEGER,
    status TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_game ON active_sessions(game_id);
CREATE INDEX IF NOT EXISTS idx_sessions_player ON active_sessions(player_white, player_black);
CREATE INDEX IF NOT EXISTS idx_sessions_status ON active_sessions(status, grace_period_ends);
