-- Migration 006: game history tables + move counter
-- Adds persistent game records, full move log, and per-session move counter.
-- All tables live on the session pool (sessions.db).

-- Track move number per game inside the existing sessions row
ALTER TABLE sessions ADD COLUMN move_count INTEGER NOT NULL DEFAULT 0;

-- One row per game (created on first move, completed at finalize)
CREATE TABLE IF NOT EXISTS games (
    id              TEXT    PRIMARY KEY,
    player_white    TEXT,
    player_black    TEXT,
    white_username  TEXT,
    black_username  TEXT,
    stake_amount    REAL    NOT NULL DEFAULT 0.0,
    fee_lamports    INTEGER NOT NULL DEFAULT 0,
    start_time      INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    end_time        INTEGER,
    winner          TEXT,
    final_fen       TEXT,
    finalize_sig    TEXT,
    status          TEXT    NOT NULL DEFAULT 'playing',
    archived_at     INTEGER,
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

-- One row per move
CREATE TABLE IF NOT EXISTS moves (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL,
    move_number INTEGER NOT NULL,
    move_uci    TEXT    NOT NULL,
    fen_after   TEXT,
    player      TEXT    NOT NULL,
    timestamp   INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_games_white  ON games(player_white);
CREATE INDEX IF NOT EXISTS idx_games_black  ON games(player_black);
CREATE INDEX IF NOT EXISTS idx_games_status ON games(status, created_at);
CREATE INDEX IF NOT EXISTS idx_moves_game   ON moves(game_id, move_number);
