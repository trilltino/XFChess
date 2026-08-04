-- Migration 023: player_bans table.
--
-- Previously POST /admin/players/{wallet}/ban wrote to a process-local
-- HashMap (admin.rs PLAYER_BANS) that nothing outside admin.rs ever read —
-- bans didn't survive a restart and never actually blocked login,
-- matchmaking, or tournament registration. This table backs a real,
-- persistent, enforced ban.

CREATE TABLE IF NOT EXISTS player_bans (
    wallet         TEXT PRIMARY KEY,
    reason         TEXT NOT NULL,
    duration_days  INTEGER,
    banned_at      INTEGER NOT NULL,
    expires_at     INTEGER
);

CREATE INDEX IF NOT EXISTS idx_player_bans_expires ON player_bans(expires_at);
