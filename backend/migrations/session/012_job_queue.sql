-- Durable queue for off-chain work (email, anti-cheat analysis). Chain work is
-- not queued: its workers re-derive it from on-chain state.
-- status: pending -> running -> done, or back to pending with backoff, or dead.

CREATE TABLE IF NOT EXISTS jobs (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    kind         TEXT    NOT NULL,                    -- e.g. 'email.confirmation'
    payload      TEXT    NOT NULL,                    -- JSON
    dedupe_key   TEXT,                                -- NULL = no dedupe
    status       TEXT    NOT NULL DEFAULT 'pending',  -- pending | running | done | dead
    attempts     INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    run_at       INTEGER NOT NULL,                    -- backoff pushes this forward
    claimed_at   INTEGER,                             -- for stale-claim recovery
    last_error   TEXT,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

-- NULLs never conflict, so jobs without a dedupe_key are always enqueued.
CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_dedupe ON jobs (dedupe_key);
CREATE INDEX IF NOT EXISTS idx_jobs_due  ON jobs (status, run_at) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_jobs_dead ON jobs (status) WHERE status = 'dead';
