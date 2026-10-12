-- Disputes, moderation flags and bans. The on-chain DisputeRecord is
-- authoritative; `disputes` drives notifications and review.

CREATE TABLE IF NOT EXISTS disputes (
    game_id         INTEGER PRIMARY KEY,
    challenger      TEXT    NOT NULL,
    reason          TEXT    NOT NULL,
    status          TEXT    NOT NULL DEFAULT 'pending',  -- pending | resolved | dismissed
    anticheat_score REAL,
    report_path     TEXT,
    decision        TEXT,
    resolution_text TEXT,
    tx_sig          TEXT,
    notified_at     INTEGER NOT NULL,
    analysed_at     INTEGER,
    resolved_at     INTEGER
);

CREATE INDEX IF NOT EXISTS idx_disputes_status   ON disputes (status);
CREATE INDEX IF NOT EXISTS idx_disputes_notified ON disputes (notified_at DESC);

-- Games flagged for a reviewer, separate from formal disputes.
CREATE TABLE IF NOT EXISTS flagged_games (
    game_id     INTEGER PRIMARY KEY,
    reason      TEXT    NOT NULL,
    flagged_at  INTEGER NOT NULL,
    assigned_to TEXT
);

CREATE TABLE IF NOT EXISTS player_bans (
    wallet         TEXT    PRIMARY KEY,
    reason         TEXT    NOT NULL,
    duration_days  INTEGER,
    banned_at      INTEGER NOT NULL,
    expires_at     INTEGER
);

CREATE INDEX IF NOT EXISTS idx_player_bans_expires ON player_bans (expires_at);
