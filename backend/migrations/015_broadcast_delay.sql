-- Migration 015: per-game broadcast delay for esports integrity.
-- The public spectator feed for a game is gated to moves at least
-- broadcast_delay_secs old, defeating live-stream ghosting. 0 = live
-- (today's behavior for casual/ranked games); tournament games set a delay.

ALTER TABLE games ADD COLUMN broadcast_delay_secs INTEGER NOT NULL DEFAULT 0;
