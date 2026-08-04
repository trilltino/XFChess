-- Tracks which wallet has successfully activated a session for a given game_id.
--
-- `sessions.active` is a single game-level flag that flips true once the
-- HOST's create_game (or global_create_game) setup TX lands — /session/activate
-- used that flag as an idempotency guard against retries. But the same
-- endpoint is also used by a JOINING player's join_game setup TX for the
-- SAME game_id, and the game-level flag can't distinguish "the host already
-- activated" from "this specific wallet already activated" — so the joiner's
-- transaction was silently short-circuited as a false "already active" retry
-- and never submitted. This table keys idempotency per (game_id, wallet)
-- instead, so each player's activation is tracked independently.
CREATE TABLE IF NOT EXISTS session_wallet_activations (
    game_id INTEGER NOT NULL,
    wallet  TEXT    NOT NULL,
    sig     TEXT    NOT NULL,
    PRIMARY KEY (game_id, wallet)
);
