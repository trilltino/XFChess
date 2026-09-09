-- Migration 031: off-chain tournament events.
-- Deliberately separate from Solana-backed tournaments: no escrow, fees, or chain IDs.

CREATE TABLE IF NOT EXISTS offline_tournaments (
    tournament_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    format TEXT NOT NULL CHECK (format IN ('single_elimination', 'swiss')),
    status TEXT NOT NULL CHECK (status IN ('draft', 'published', 'active', 'completed', 'cancelled')),
    state_json TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_offline_tournaments_status ON offline_tournaments(status);
