-- Finished and in-progress games with their moves, plus casual (bot / local)
-- results, which count towards history only.

CREATE TABLE IF NOT EXISTS games (
    id                   TEXT    PRIMARY KEY,
    player_white         TEXT,
    player_black         TEXT,
    white_username       TEXT,
    black_username       TEXT,
    stake_amount         REAL    NOT NULL DEFAULT 0.0,
    fee_lamports         INTEGER NOT NULL DEFAULT 0,
    start_time           INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    end_time             INTEGER,
    winner               TEXT,
    final_fen            TEXT,
    finalize_sig         TEXT,
    status               TEXT    NOT NULL DEFAULT 'playing',
    archived_at          INTEGER,
    created_at           INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    broadcast_delay_secs INTEGER NOT NULL DEFAULT 0,  -- spectators see moves this late; 0 = live
    pgn_text             TEXT
);
ALTER TABLE games ADD COLUMN broadcast_delay_secs INTEGER NOT NULL DEFAULT 0;
ALTER TABLE games ADD COLUMN pgn_text TEXT;

CREATE INDEX IF NOT EXISTS idx_games_status       ON games (status, created_at);
CREATE INDEX IF NOT EXISTS idx_games_start_time   ON games (start_time DESC);
CREATE INDEX IF NOT EXISTS idx_games_white_start  ON games (player_white, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_games_black_start  ON games (player_black, start_time DESC);

-- Covered by idx_games_white_start / idx_games_black_start.
DROP INDEX IF EXISTS idx_games_white;
DROP INDEX IF EXISTS idx_games_black;

CREATE TABLE IF NOT EXISTS moves (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL,
    move_number INTEGER NOT NULL,
    move_uci    TEXT    NOT NULL,
    fen_after   TEXT,
    player      TEXT    NOT NULL,
    timestamp   INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    move_san    TEXT,
    fen_before  TEXT,
    FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
);
ALTER TABLE moves ADD COLUMN move_san TEXT;
ALTER TABLE moves ADD COLUMN fen_before TEXT;

CREATE INDEX IF NOT EXISTS idx_moves_game      ON moves (game_id, move_number);
CREATE INDEX IF NOT EXISTS idx_moves_timestamp ON moves (game_id, timestamp ASC);

CREATE TABLE IF NOT EXISTS casual_games (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id      TEXT    NOT NULL,   -- wallet, or the "email:<addr>" JWT subject
    opponent_type   TEXT    NOT NULL,   -- 'bot' | 'local_p2p'
    result          TEXT    NOT NULL,   -- 'win' | 'loss' | 'draw'
    pgn             TEXT,
    created_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_casual_games_account ON casual_games (account_id);
