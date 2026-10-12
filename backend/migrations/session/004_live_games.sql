-- State that keeps a game in progress going across reconnects and restarts.

-- Append-only Move/Resign/Chat log served over Braid. Ping/Pong and
-- SessionInfo are not stored. `kind` is unconstrained so new message kinds
-- need no schema change.
CREATE TABLE IF NOT EXISTS game_event_log (
    game_id        TEXT    NOT NULL,
    seq            INTEGER NOT NULL,
    kind           TEXT    NOT NULL,
    version_hash   TEXT    NOT NULL,
    parent_version TEXT    NOT NULL,
    payload_json   TEXT    NOT NULL,
    created_at     INTEGER NOT NULL,
    PRIMARY KEY (game_id, seq)
);

-- Covered by the primary key.
DROP INDEX IF EXISTS idx_game_event_log_game;

-- A casual (non-wagered) game's host and joiner; player claims on its log are
-- checked against them, as wagered games are checked against the chain.
CREATE TABLE IF NOT EXISTS casual_game_participants (
    game_id        TEXT PRIMARY KEY,
    host_node_id   TEXT NOT NULL,
    joiner_node_id TEXT NOT NULL
);

-- One writing device per (game, wallet); a new claim bumps the epoch and turns
-- the previous device view-only.
CREATE TABLE IF NOT EXISTS game_seat_leases (
    game_id    TEXT    NOT NULL,
    wallet     TEXT    NOT NULL,
    device_id  TEXT    NOT NULL,
    epoch      INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (game_id, wallet)
);

-- Lobby relay rooms (incl. JOIN_ACK mailboxes), restored on restart; the TTL
-- sweep still expires them.
CREATE TABLE IF NOT EXISTS p2p_relay_rooms (
    game_id       TEXT    PRIMARY KEY,
    room_json     TEXT    NOT NULL,
    last_activity INTEGER NOT NULL
);

-- Disconnect-recovery snapshots, listed by /admin/active-sessions.
CREATE TABLE IF NOT EXISTS active_sessions (
    session_id        TEXT    PRIMARY KEY,
    game_id           INTEGER NOT NULL,
    player_white      TEXT    NOT NULL,
    player_black      TEXT    NOT NULL,
    current_fen       TEXT    NOT NULL,
    move_history      TEXT    NOT NULL,
    white_time_ms     INTEGER,
    black_time_ms     INTEGER,
    last_activity     INTEGER NOT NULL,
    grace_period_ends INTEGER,
    status            TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_game   ON active_sessions (game_id);
CREATE INDEX IF NOT EXISTS idx_sessions_player ON active_sessions (player_white, player_black);
CREATE INDEX IF NOT EXISTS idx_sessions_status ON active_sessions (status, grace_period_ends);
