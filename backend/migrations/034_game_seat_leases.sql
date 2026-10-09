-- Migration 034: one playing device per player per game
--
-- A wallet signed in on two devices could submit moves for the same seat from
-- both. A seat lease names the single device allowed to write moves for
-- (game_id, wallet). Claiming increments `epoch`, so the newest claim takes
-- over and the previous device becomes view-only.

CREATE TABLE IF NOT EXISTS game_seat_leases (
    game_id TEXT NOT NULL,
    wallet TEXT NOT NULL,
    device_id TEXT NOT NULL,
    epoch INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (game_id, wallet)
);
