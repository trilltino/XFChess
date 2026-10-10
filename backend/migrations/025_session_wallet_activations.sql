-- Activation idempotency is per (game_id, wallet), so host activation
-- does not suppress the joining wallet transaction.
CREATE TABLE IF NOT EXISTS session_wallet_activations (
    game_id INTEGER NOT NULL,
    wallet  TEXT    NOT NULL,
    sig     TEXT    NOT NULL,
    PRIMARY KEY (game_id, wallet)
);
