-- KYC records and GDPR erasure requests. Tax IDs are stored only as blind
-- SHA-256 hashes; erasure soft-deletes the record.

CREATE TABLE IF NOT EXISTS kyc_records (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    wallet_pubkey   TEXT    NOT NULL,
    country         TEXT    NOT NULL,
    full_name       TEXT    NOT NULL,
    dob             TEXT    NOT NULL,   -- YYYY-MM-DD
    residence       TEXT    NOT NULL,
    tax_id_hash     TEXT    NOT NULL,   -- blind hash, never the raw ID
    data_source     TEXT    NOT NULL DEFAULT 'self_submitted',
    created_at      INTEGER NOT NULL,
    deleted_at      INTEGER,            -- set on GDPR erasure
    UNIQUE(wallet_pubkey)
);

CREATE TABLE IF NOT EXISTS deletion_requests (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    wallet_pubkey   TEXT    NOT NULL,
    email           TEXT,
    reason          TEXT,
    requested_at    INTEGER NOT NULL,
    completed_at    INTEGER             -- NULL while pending
);

CREATE INDEX IF NOT EXISTS idx_deletion_wallet  ON deletion_requests (wallet_pubkey);
CREATE INDEX IF NOT EXISTS idx_deletion_pending ON deletion_requests (wallet_pubkey) WHERE completed_at IS NULL;

-- Covered by UNIQUE(wallet_pubkey).
DROP INDEX IF EXISTS idx_kyc_active;
