-- Puzzles, attempts, ratings and admin-funded bounties, which pay only for
-- server-verified solves. See docs/PUZZLES.md.

-- `line` is the solution: server-only, never serialised to a client.
CREATE TABLE IF NOT EXISTS puzzles (
    id          TEXT    PRIMARY KEY,           -- Lichess puzzle id
    name        TEXT,                          -- admin label, NULL until named
    fen         TEXT    NOT NULL,              -- starting position
    line        TEXT    NOT NULL,              -- space-separated UCI, incl. setup move
    rating      INTEGER NOT NULL,
    rating_dev  INTEGER NOT NULL DEFAULT 80,
    themes      TEXT    NOT NULL DEFAULT '',   -- space-separated tags
    plays       INTEGER NOT NULL DEFAULT 0,
    nb_wins     INTEGER NOT NULL DEFAULT 0,
    featured    INTEGER NOT NULL DEFAULT 0,
    enabled     INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_puzzles_rating ON puzzles (rating) WHERE enabled = 1;
CREATE INDEX IF NOT EXISTS idx_puzzles_name   ON puzzles (name) WHERE name IS NOT NULL;

-- One attempt per (wallet, puzzle): a puzzle cannot be re-solved for another payout.
CREATE TABLE IF NOT EXISTS puzzle_rounds (
    wallet      TEXT    NOT NULL,
    puzzle_id   TEXT    NOT NULL,
    win         INTEGER NOT NULL,
    rated       INTEGER NOT NULL DEFAULT 1,
    solve_ms    INTEGER,                       -- client claim, audited
    paid_sig    TEXT,                          -- payout transaction, if any
    created_at  INTEGER NOT NULL,
    PRIMARY KEY (wallet, puzzle_id)
);

CREATE INDEX IF NOT EXISTS idx_puzzle_rounds_wallet ON puzzle_rounds (wallet, created_at);

CREATE TABLE IF NOT EXISTS puzzle_ratings (
    wallet      TEXT    PRIMARY KEY,
    rating      INTEGER NOT NULL DEFAULT 1500,
    rating_dev  INTEGER NOT NULL DEFAULT 350,
    nb          INTEGER NOT NULL DEFAULT 0,
    updated_at  INTEGER NOT NULL
);

-- Binds each solve to the puzzle served under a nonce, against forgery and replay.
CREATE TABLE IF NOT EXISTS puzzle_challenges (
    nonce       TEXT    PRIMARY KEY,
    wallet      TEXT    NOT NULL,
    puzzle_id   TEXT    NOT NULL,
    mode        TEXT    NOT NULL,               -- 'solve' | 'earn' | 'daily'
    issued_at   INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    consumed    INTEGER NOT NULL DEFAULT 0,
    progress    INTEGER NOT NULL DEFAULT 0,     -- verified player moves so far
    hinted      INTEGER NOT NULL DEFAULT 0,     -- a hint makes the round unrated and unpaid
    source      TEXT    NOT NULL DEFAULT 'local' -- 'local' dump or live 'lichess'
);
ALTER TABLE puzzle_challenges ADD COLUMN hinted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE puzzle_challenges ADD COLUMN source TEXT NOT NULL DEFAULT 'local';

-- Bounties are funded by the VPS authority; verified solves spend the budget.
CREATE TABLE IF NOT EXISTS puzzle_bounties (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    scope           TEXT    NOT NULL,           -- 'puzzle' | 'band' | 'daily'
    puzzle_id       TEXT,                       -- scope = 'puzzle'
    band_lo         INTEGER,                    -- scope = 'band'
    band_hi         INTEGER,
    reward_lamports INTEGER NOT NULL,           -- per solve
    budget_lamports INTEGER NOT NULL,
    spent_lamports  INTEGER NOT NULL DEFAULT 0,
    max_per_wallet  INTEGER NOT NULL DEFAULT 1,
    fund_sig        TEXT,
    vault_pubkey    TEXT,
    created_by      TEXT    NOT NULL,
    status          TEXT    NOT NULL DEFAULT 'active',  -- active | exhausted | closed
    created_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_puzzle_bounties_active ON puzzle_bounties (status, scope);
