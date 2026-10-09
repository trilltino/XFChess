-- Migration 033: durable P2P relay rooms
--
-- The lobby relay (signing/p2p_relay) kept announcements, the JOIN_ACK
-- handshake and relayed messages only in memory, so a backend restart lost
-- every open lobby and any in-flight join handshake. Each room is written
-- through as one JSON row and hydrated on startup; the TTL sweep still
-- removes stale rooms, so a long outage expires them exactly as before.

CREATE TABLE IF NOT EXISTS p2p_relay_rooms (
    game_id TEXT PRIMARY KEY,
    room_json TEXT NOT NULL,
    last_activity INTEGER NOT NULL
);
