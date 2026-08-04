-- Migration 018: puzzle pool + per-user solve records + admin-funded bounties.
--
-- See docs/PUZZLES.md. The earn model is admin-prefunded bounties only:
-- an operator funds a puzzle (or band / daily) with the VPS authority key, and
-- a server-verified solve pays the posted reward. No staking, no ladder.

-- The pool. `line` is SERVER-ONLY and must never be serialised to a client
-- response. Treat it like a secret.
CREATE TABLE IF NOT EXISTS puzzles (
    id          TEXT PRIMARY KEY,        -- Lichess puzzle id (stable)
    name        TEXT,                    -- admin-assignable label (nullable);
                                         -- imported puzzles have none until named
    fen         TEXT NOT NULL,           -- starting position
    line        TEXT NOT NULL,           -- space-separated UCI, incl. setup move
    rating      INTEGER NOT NULL,        -- difficulty (Glicko rating / "ELO")
    rating_dev  INTEGER NOT NULL DEFAULT 80,
    themes      TEXT NOT NULL DEFAULT '',-- space-separated theme tags
    plays       INTEGER NOT NULL DEFAULT 0,
    nb_wins     INTEGER NOT NULL DEFAULT 0,
    featured    INTEGER NOT NULL DEFAULT 0,  -- admin "selected" flag
    enabled     INTEGER NOT NULL DEFAULT 1
);

-- Rating-band index drives ELO selection and the admin ELO browser.
CREATE INDEX IF NOT EXISTS idx_puzzles_rating ON puzzles(rating) WHERE enabled = 1;
-- Name index drives the admin name search.
CREATE INDEX IF NOT EXISTS idx_puzzles_name ON puzzles(name) WHERE name IS NOT NULL;

-- One row per (user, puzzle) attempt. Prevents re-solving the same puzzle for
-- another payout, and feeds rating + anti-cheat.
CREATE TABLE IF NOT EXISTS puzzle_rounds (
    wallet      TEXT NOT NULL,
    puzzle_id   TEXT NOT NULL,
    win         INTEGER NOT NULL,        -- 1 = solved cleanly, 0 = failed
    rated       INTEGER NOT NULL DEFAULT 1,
    solve_ms    INTEGER,                 -- total think time, client claim (audited)
    paid_sig    TEXT,                    -- Solana tx signature if a payout fired
    created_at  INTEGER NOT NULL,
    PRIMARY KEY (wallet, puzzle_id)
);
CREATE INDEX IF NOT EXISTS idx_puzzle_rounds_wallet ON puzzle_rounds(wallet, created_at);

-- Each user's puzzle rating (mirrors Lichess's per-perf Glicko).
CREATE TABLE IF NOT EXISTS puzzle_ratings (
    wallet      TEXT PRIMARY KEY,
    rating      INTEGER NOT NULL DEFAULT 1500,
    rating_dev  INTEGER NOT NULL DEFAULT 350,
    nb          INTEGER NOT NULL DEFAULT 0,
    updated_at  INTEGER NOT NULL
);

-- An in-progress solve the server has committed to. Makes server-side
-- verification possible: we record which puzzle we served and a nonce, so the
-- solve submission can't be forged or replayed.
CREATE TABLE IF NOT EXISTS puzzle_challenges (
    nonce       TEXT PRIMARY KEY,        -- random, returned to client
    wallet      TEXT NOT NULL,
    puzzle_id   TEXT NOT NULL,
    mode        TEXT NOT NULL,           -- 'solve' | 'earn' | 'daily'
    issued_at   INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    consumed    INTEGER NOT NULL DEFAULT 0,
    progress    INTEGER NOT NULL DEFAULT 0  -- verified player moves so far (interactive /puzzle/move)
);

-- Admin-funded bounties. An admin selects a puzzle (or rating band) in the
-- tournament-admin app and locks SOL against it, drawn from and signed by the
-- VPS authority key. A server-verified solve pays from this budget.
CREATE TABLE IF NOT EXISTS puzzle_bounties (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    scope           TEXT NOT NULL,           -- 'puzzle' | 'band' | 'daily'
    puzzle_id       TEXT,                    -- set when scope='puzzle'
    band_lo         INTEGER,                 -- ELO band when scope='band'
    band_hi         INTEGER,
    reward_lamports INTEGER NOT NULL,        -- payout per successful solve
    budget_lamports INTEGER NOT NULL,        -- total SOL locked for this bounty
    spent_lamports  INTEGER NOT NULL DEFAULT 0,
    max_per_wallet  INTEGER NOT NULL DEFAULT 1,
    fund_sig        TEXT,                    -- VPS-signed funding tx signature
    vault_pubkey    TEXT,                    -- on-chain prize vault PDA (if used)
    created_by      TEXT NOT NULL,           -- admin token / authority pubkey
    status          TEXT NOT NULL DEFAULT 'active', -- 'active'|'exhausted'|'closed'
    created_at      INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_puzzle_bounties_active ON puzzle_bounties(status, scope);
