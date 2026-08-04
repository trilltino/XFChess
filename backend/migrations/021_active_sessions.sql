-- Migration 021: active_sessions table for disconnect recovery
--
-- Backs GET /admin/active-sessions (backend/src/db/repository.rs
-- list_active_sessions) and backend/src/db/sessions.rs's session-recovery
-- queries. Previously only defined in db/schema.rs::init_db, which is never
-- called from the real startup path (infrastructure/database.rs::run_migrations
-- is what actually runs against the live pool) — the table never existed,
-- so every read of it failed with "no such table".

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
