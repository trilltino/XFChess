-- Migration 020: casual (off-chain) game history + sponsored profile-creation guard
--
-- Casual games (vs bot / local P2P) played while logged into an Account are
-- recorded here for history only. They deliberately do NOT touch on-chain
-- elo_rating — writing to a Solana account costs a transaction per game, and
-- on-chain Elo stays driven only by real wagered/ranked on-chain settlement,
-- as today. See docs/plans/identity-implementation-plan.md.
CREATE TABLE IF NOT EXISTS casual_games (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id      TEXT    NOT NULL,   -- wallet pubkey, or the "email:<addr>" JWT subject
    opponent_type   TEXT    NOT NULL,   -- 'bot' | 'local_p2p'
    result          TEXT    NOT NULL,   -- 'win' | 'loss' | 'draw'
    pgn             TEXT,
    created_at      INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_casual_games_account ON casual_games (account_id);

-- Guards backend-sponsored on-chain profile creation to one grant per account.
ALTER TABLE users_v2 ADD COLUMN profile_sponsored_at INTEGER;
