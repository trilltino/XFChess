-- Migration 012: Performance indexes for hot query paths
--
-- games.start_time is used by ORDER BY in /games/history and /ratings/history.
-- games.status is already indexed (with created_at) but start_time is not.
-- moves.timestamp is used for per-game move reconstruction ordering.

CREATE INDEX IF NOT EXISTS idx_games_start_time
    ON games(start_time DESC);

-- Composite for history queries: find games for a player sorted by time
CREATE INDEX IF NOT EXISTS idx_games_white_start
    ON games(player_white, start_time DESC);

CREATE INDEX IF NOT EXISTS idx_games_black_start
    ON games(player_black, start_time DESC);

-- Moves by timestamp for ordered reconstruction
CREATE INDEX IF NOT EXISTS idx_moves_timestamp
    ON moves(game_id, timestamp ASC);
