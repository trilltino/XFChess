-- Migration 002: GDPR-compliant KYC storage in vault database
-- Replaces flat-file kyc.jsonl + subscribers.jsonl with proper SQLite tables.
-- Tax IDs are NEVER stored raw; only SHA-256 blind hash is persisted.
-- All PII fields support soft-delete (deleted_at) for GDPR right-to-erasure.

-- KYC records table (vault database)
CREATE TABLE IF NOT EXISTS kyc_records (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    wallet_pubkey   TEXT    NOT NULL,
    country         TEXT    NOT NULL,
    full_name       TEXT    NOT NULL,
    dob             TEXT    NOT NULL,    -- YYYY-MM-DD
    residence       TEXT    NOT NULL,
    tax_id_hash     TEXT    NOT NULL,   -- SHA-256 blind hash, never raw
    data_source     TEXT    NOT NULL DEFAULT 'self_submitted',
    created_at      INTEGER NOT NULL,   -- Unix timestamp
    deleted_at      INTEGER,            -- NULL = active; set on GDPR erasure
    UNIQUE(wallet_pubkey)
);

-- GDPR deletion requests table (vault database)
CREATE TABLE IF NOT EXISTS deletion_requests (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    wallet_pubkey   TEXT    NOT NULL,
    email           TEXT,
    reason          TEXT,
    requested_at    INTEGER NOT NULL,
    completed_at    INTEGER             -- NULL = pending
);

-- Extend users table: add kyc_status, created_at, deleted_at
-- SQLite ALTER TABLE only supports ADD COLUMN; use separate statements.
ALTER TABLE users ADD COLUMN kyc_status  TEXT    NOT NULL DEFAULT 'none';
ALTER TABLE users ADD COLUMN created_at  INTEGER;
ALTER TABLE users ADD COLUMN deleted_at  INTEGER;
