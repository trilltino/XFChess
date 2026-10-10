-- Persist and restore relay rooms, including JOIN_ACK state; the TTL sweep
-- still expires rooms after prolonged outages.

CREATE TABLE IF NOT EXISTS p2p_relay_rooms (
    game_id TEXT PRIMARY KEY,
    room_json TEXT NOT NULL,
    last_activity INTEGER NOT NULL
);
