-- Puzzle hints and Lichess-sourced puzzles.
--
-- hinted: the player asked for a hint, so the round is unrated and unpaid.
-- source: 'local' (the imported Lichess dump) or 'lichess' (served live from the
--         player's Lichess account; the result is synced back to Lichess).
ALTER TABLE puzzle_challenges ADD COLUMN hinted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE puzzle_challenges ADD COLUMN source TEXT NOT NULL DEFAULT 'local';

-- Lichess OAuth access tokens, encrypted with the identity vault. Kept only so
-- the backend can fetch unseen puzzles and record results for the player; the
-- token never leaves the server.
CREATE TABLE IF NOT EXISTS lichess_tokens (
    pubkey     TEXT PRIMARY KEY,
    token_enc  BLOB NOT NULL,
    scope      TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
