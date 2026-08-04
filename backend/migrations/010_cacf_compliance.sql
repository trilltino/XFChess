-- Migration 010: Persistent CACF compliance records.
--
-- Replaces the in-memory CacfComplianceManager HashMap so records survive
-- server restarts.  One row per (wallet, country) pair.
--
-- `kyc_completed`  mirrors whether full KYC has been submitted for this pair.
-- `status`         matches CacfComplianceStatus variants (snake_case text).
-- `details_json`   stores jurisdiction-specific fields (NI number verified,
--                  UTR verified, CPF verified, etc.) as a JSON object so we
--                  don't need separate columns per country.

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
