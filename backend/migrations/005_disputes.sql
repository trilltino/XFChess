-- Dispute tracking table: one row per disputed game.
-- The on-chain DisputeRecord PDA is the authoritative source -
-- this table drives email notifications and admin review.

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

CREATE INDEX IF NOT EXISTS idx_disputes_status ON disputes (status);
CREATE INDEX IF NOT EXISTS idx_disputes_notified ON disputes (notified_at DESC);
