
CREATE TABLE IF NOT EXISTS player_bans (
    wallet         TEXT PRIMARY KEY,
    reason         TEXT NOT NULL,
    duration_days  INTEGER,
    banned_at      INTEGER NOT NULL,
    expires_at     INTEGER
);

CREATE INDEX IF NOT EXISTS idx_player_bans_expires ON player_bans(expires_at);
