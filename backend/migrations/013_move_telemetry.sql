-- Migration 013: client-side anti-cheat telemetry (blur reporting).
-- One row per (game, ply); written by POST /telemetry/blur, first write wins.

CREATE TABLE IF NOT EXISTS move_telemetry (
    game_id     TEXT    NOT NULL,
    move_number INTEGER NOT NULL,
    color       TEXT    NOT NULL,            -- 'white' | 'black' (must match ply parity)
    blurred     INTEGER NOT NULL DEFAULT 0,  -- window lost focus since the player's previous move
    reported_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    PRIMARY KEY (game_id, move_number)
);

CREATE INDEX IF NOT EXISTS idx_move_telemetry_game ON move_telemetry(game_id);
