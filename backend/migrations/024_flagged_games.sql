-- Migration 024: flagged_games table.
--
-- Backs the Dashboard MODERATION tab's flag/assign-reviewer flow. Previously
-- both lived in process-local HashMaps in admin.rs (FLAGGED_GAMES,
-- DISPUTE_ASSIGNMENTS) that reset on every backend restart. This is
-- deliberately separate from the `disputes` table (migration 005) — that one
-- tracks the formal on-chain dispute/resolve flow; this one is the lighter
-- ad-hoc anti-cheat flag + reviewer-assignment queue.

CREATE TABLE IF NOT EXISTS flagged_games (
    game_id     INTEGER PRIMARY KEY,
    reason      TEXT    NOT NULL,
    flagged_at  INTEGER NOT NULL,
    assigned_to TEXT
);
