-- Durable job queue (WS-A, Production Reality Plan).
--
-- For work with NO durable backing of its own (email sends, anti-cheat analyses).
-- Chain-derived work (settlement, prize distribution) deliberately does NOT use
-- this table — those workers re-derive their work from on-chain state every tick,
-- which is already durable and idempotent.
--
-- Lifecycle: pending → running → done | pending (retry, run_at pushed back)
--                                     | dead    (attempts exhausted → DLQ)

CREATE TABLE IF NOT EXISTS jobs (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    kind         TEXT    NOT NULL,              -- e.g. 'email.confirmation', 'anticheat.analyze'
    payload      TEXT    NOT NULL,              -- JSON
    dedupe_key   TEXT,                          -- NULL = no dedupe; UNIQUE enforces idempotent enqueue
    status       TEXT    NOT NULL DEFAULT 'pending',  -- pending | running | done | dead
    attempts     INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    run_at       INTEGER NOT NULL,              -- unix seconds (backoff pushes this forward)
    claimed_at   INTEGER,                       -- when a worker took it (stale-claim recovery)
    last_error   TEXT,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

-- SQLite UNIQUE treats NULLs as distinct, so rows without a dedupe_key never conflict.
CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_dedupe ON jobs(dedupe_key);

-- The poller's hot path: due pending jobs, oldest first.
CREATE INDEX IF NOT EXISTS idx_jobs_due
    ON jobs(status, run_at) WHERE status = 'pending';

-- DLQ review: SELECT * FROM jobs WHERE status='dead' ORDER BY updated_at DESC;
CREATE INDEX IF NOT EXISTS idx_jobs_dead
    ON jobs(status) WHERE status = 'dead';
