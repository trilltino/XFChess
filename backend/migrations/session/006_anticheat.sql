-- Anti-cheat: the analysis queue, per-game verdicts, rolling per-player stats,
-- client move telemetry and multi-account linkage signals.

-- Finalized games waiting for engine analysis.
CREATE TABLE IF NOT EXISTS anticheat_queue (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id     TEXT    NOT NULL UNIQUE,
    queued_at   INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    attempts    INTEGER NOT NULL DEFAULT 0,
    last_error  TEXT
);

CREATE INDEX IF NOT EXISTS idx_acqueue_queued ON anticheat_queue (queued_at);

-- One verdict per (game, engine version).
CREATE TABLE IF NOT EXISTS anticheat_verdicts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    game_id         TEXT    NOT NULL,
    engine_version  TEXT    NOT NULL DEFAULT 'stockfish',
    analysis_depth  INTEGER NOT NULL DEFAULT 18,
    white_pubkey    TEXT    NOT NULL,
    black_pubkey    TEXT    NOT NULL,
    white_verdict   TEXT    NOT NULL,   -- Clean | Review | Flag
    black_verdict   TEXT    NOT NULL,
    white_score     REAL    NOT NULL,   -- weighted signal score, 0.0-1.0
    black_score     REAL    NOT NULL,
    white_signals   TEXT    NOT NULL,   -- JSON: timing_anomaly, cpl_vs_elo, t1_rate
    black_signals   TEXT    NOT NULL,
    report_path     TEXT,               -- report on disk, Review/Flag only
    analysed_at     INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    UNIQUE(game_id, engine_version)
);

CREATE INDEX IF NOT EXISTS idx_acverdicts_white ON anticheat_verdicts (white_pubkey, analysed_at);
CREATE INDEX IF NOT EXISTS idx_acverdicts_black ON anticheat_verdicts (black_pubkey, analysed_at);

-- Covered by UNIQUE(game_id, engine_version).
DROP INDEX IF EXISTS idx_acverdicts_game;

-- Cross-game statistics for the z-score model, written by xfchess-anticheat.
CREATE TABLE IF NOT EXISTS player_anticheat_stats (
    pubkey           TEXT    PRIMARY KEY,
    games_analysed   INTEGER NOT NULL DEFAULT 0,
    lifetime_cpl     REAL    NOT NULL DEFAULT 0.0,   -- running average
    last_30_cpls     TEXT    NOT NULL DEFAULT '[]',  -- JSON, newest first
    last_30_t1s      TEXT    NOT NULL DEFAULT '[]',  -- JSON t1_rate per game
    flags_received   INTEGER NOT NULL DEFAULT 0,
    reviews_received INTEGER NOT NULL DEFAULT 0,
    last_updated     INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

-- Client telemetry per ply, first write wins. think_ms is a client claim,
-- audited against server wall-clock time before scoring.
CREATE TABLE IF NOT EXISTS move_telemetry (
    game_id     TEXT    NOT NULL,
    move_number INTEGER NOT NULL,
    color       TEXT    NOT NULL,            -- 'white' | 'black', must match ply parity
    blurred     INTEGER NOT NULL DEFAULT 0,  -- window lost focus since the previous move
    think_ms    INTEGER,
    reported_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    PRIMARY KEY (game_id, move_number)
);
ALTER TABLE move_telemetry ADD COLUMN think_ms INTEGER;

-- Linkage signals per wallet across tournaments: flagged asks for manual
-- review, hard_blocked keeps the wallet out of prize events.
CREATE TABLE IF NOT EXISTS account_linkage (
    wallet        TEXT    PRIMARY KEY,
    funder        TEXT,
    device_hash   TEXT,
    ip_count      INTEGER NOT NULL DEFAULT 0,
    flagged       INTEGER NOT NULL DEFAULT 0,
    hard_blocked  INTEGER NOT NULL DEFAULT 0,
    first_seen    INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    last_seen     INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

CREATE INDEX IF NOT EXISTS idx_linkage_funder ON account_linkage (funder);
CREATE INDEX IF NOT EXISTS idx_linkage_device ON account_linkage (device_hash);
