-- One compliance record per (wallet, country). status uses snake_case enum
-- variants; details_json holds jurisdiction-specific fields.

CREATE TABLE IF NOT EXISTS cacf_compliance (
    wallet          TEXT    NOT NULL,
    country         TEXT    NOT NULL,   -- ISO 3166-1 alpha-2 (GB, BR, DE, CA, …)
    status          TEXT    NOT NULL DEFAULT 'not_compliant',
    kyc_completed   INTEGER NOT NULL DEFAULT 0,
    details_json    TEXT,               -- JSON blob of country-specific booleans
    updated_at      INTEGER NOT NULL,   -- Unix timestamp
    PRIMARY KEY (wallet, country)
);

CREATE INDEX IF NOT EXISTS idx_cacf_wallet ON cacf_compliance (wallet);
