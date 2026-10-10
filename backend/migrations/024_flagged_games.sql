-- Moderation flags and reviewer assignments; separate from formal on-chain disputes.

CREATE TABLE IF NOT EXISTS flagged_games (
    game_id     INTEGER PRIMARY KEY,
    reason      TEXT    NOT NULL,
    flagged_at  INTEGER NOT NULL,
    assigned_to TEXT
);
