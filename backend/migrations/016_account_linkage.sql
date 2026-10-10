-- Accumulate linkage signals per wallet across tournaments. flagged requests
-- manual review; hard_blocked prevents prize entry.

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
