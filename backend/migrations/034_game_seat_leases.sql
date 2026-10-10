-- Each (game_id, wallet) has one writing device. A new claim increments epoch
-- and makes the previous device view-only.

CREATE TABLE IF NOT EXISTS game_seat_leases (
    game_id TEXT NOT NULL,
    wallet TEXT NOT NULL,
    device_id TEXT NOT NULL,
    epoch INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (game_id, wallet)
);
