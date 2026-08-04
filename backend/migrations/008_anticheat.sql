-- Migration 008: Anti-cheat analysis queue, verdicts, and cross-game player stats.
-- All tables on the sessions pool (sessions.db).

-- Games queued for Stockfish analysis after finalization.
CREATE TABLE IF NOT EXISTS anticheat_queue (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL UNIQUE,
    queued_at   INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    attempts    INTEGER NOT NULL DEFAULT 0,
    last_error  TEXT
);

CREATE INDEX IF NOT EXISTS idx_acqueue_queued ON anticheat_queue(queued_at);

-- One row per completed analysis (keyed by game_id + engine_version for idempotency).
CREATE TABLE IF NOT EXISTS anticheat_verdicts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id         TEXT    NOT NULL,
    engine_version  TEXT    NOT NULL DEFAULT 'stockfish',
    analysis_depth  INTEGER NOT NULL DEFAULT 18,
    white_pubkey    TEXT    NOT NULL,
    black_pubkey    TEXT    NOT NULL,
    white_verdict   TEXT    NOT NULL,   -- Clean | Review | Flag
    black_verdict   TEXT    NOT NULL,
    white_score     REAL    NOT NULL,   -- weighted signal score 0.0-1.0
    black_score     REAL    NOT NULL,
    white_signals   TEXT    NOT NULL,   -- JSON: {timing_anomaly, cpl_vs_elo, t1_rate}
    black_signals   TEXT    NOT NULL,
    report_path     TEXT,               -- path to .txt report on disk (Review/Flag only)
    analysed_at     INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    UNIQUE(game_id, engine_version)
);

CREATE INDEX IF NOT EXISTS idx_acverdicts_white  ON anticheat_verdicts(white_pubkey, analysed_at);
CREATE INDEX IF NOT EXISTS idx_acverdicts_black  ON anticheat_verdicts(black_pubkey, analysed_at);
CREATE INDEX IF NOT EXISTS idx_acverdicts_game   ON anticheat_verdicts(game_id);

-- Rolling per-player cross-game statistics for z-score model.
-- Updated after each analysed game; last_30_cpls is a JSON array (ring buffer).
CREATE TABLE IF NOT EXISTS player_anticheat_stats (
    pubkey          TEXT    PRIMARY KEY,
    games_analysed  INTEGER NOT NULL DEFAULT 0,
    lifetime_cpl    REAL    NOT NULL DEFAULT 0.0,   -- running average
    last_30_cpls    TEXT    NOT NULL DEFAULT '[]',  -- JSON array, newest first, max 30
    last_30_t1s     TEXT    NOT NULL DEFAULT '[]',  -- JSON array of t1_rate per game
    flags_received  INTEGER NOT NULL DEFAULT 0,
    reviews_received INTEGER NOT NULL DEFAULT 0,
    last_updated    INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);
