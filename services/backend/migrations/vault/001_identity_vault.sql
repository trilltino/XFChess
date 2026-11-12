-- Encrypted identity records and the GDPR access/erasure trail.

CREATE TABLE IF NOT EXISTS vault_users (
    pubkey                  TEXT PRIMARY KEY,
    blind_index_hash        TEXT UNIQUE,
    encrypted_blob          BLOB,
    registered_at           INTEGER,
    consent_kyc             BOOLEAN NOT NULL DEFAULT 0,
    consent_retention_years INTEGER NOT NULL DEFAULT 7
);

CREATE TABLE IF NOT EXISTS audit_log (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    pubkey    TEXT    NOT NULL,
    action    TEXT    NOT NULL,
    timestamp INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_log_pubkey    ON audit_log (pubkey);
CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log (timestamp);
