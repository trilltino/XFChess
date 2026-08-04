-- Migration 016: persisted, cross-tournament account-linkage signals for
-- Sybil / multi-accounting defense. Replaces the in-memory per-tournament IP
-- counter as the source of truth; the IP count becomes one input among several.
--
-- One row per wallet, accumulating linkage signals across all events:
--   funder      — on-chain SOL funding source (common funder => likely linked)
--   device_hash — coarse client/device fingerprint (hashed, PII-light)
--   ip_count    — registrations observed from the same IP (demoted signal)
--   flagged     — surfaced for manual review (soft)
--   hard_blocked— blocked from prize entry (hard, KYC-driven)

CREATE TABLE IF NOT EXISTS account_linkage (
    wallet        TEXT    PRIMARY KEY,
    funder        TEXT,
    device_hash   TEXT,
    ip_count      INTEGER NOT NULL DEFAULT 0,
    flagged       INTEGER NOT NULL DEFAULT 0,
    hard_blocked  INTEGER NOT NULL DEFAULT 0,
    first_seen    INTEGER NOT NULL DEFAULT (strftime('%s','now')),
    last_seen     INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

-- Wallets sharing a funder or device are the cluster query's hot paths.
CREATE INDEX IF NOT EXISTS idx_linkage_funder ON account_linkage(funder);
CREATE INDEX IF NOT EXISTS idx_linkage_device ON account_linkage(device_hash);
