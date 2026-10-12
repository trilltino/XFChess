-- Per-country wagering compliance (CACF), one record per (wallet, country).

CREATE TABLE IF NOT EXISTS cacf_compliance (
    wallet          TEXT    NOT NULL,
    country         TEXT    NOT NULL,   -- ISO 3166-1 alpha-2
    status          TEXT    NOT NULL DEFAULT 'not_compliant',
    kyc_completed   INTEGER NOT NULL DEFAULT 0,
    details_json    TEXT,               -- country-specific fields
    updated_at      INTEGER NOT NULL,
    PRIMARY KEY (wallet, country)
);

-- Covered by the primary key.
DROP INDEX IF EXISTS idx_cacf_wallet;
