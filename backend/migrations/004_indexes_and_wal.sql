-- Migration 004: Performance indexes
-- WAL mode is set at connection time (not via SQL), but we add the
-- remaining indexes here so queries on large audit/deletion tables stay fast.

-- Fast lookup of all audit events for a wallet
CREATE INDEX IF NOT EXISTS idx_audit_log_pubkey     ON audit_log (pubkey);
CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp  ON audit_log (timestamp);

-- Fast lookup of pending deletion requests
CREATE INDEX IF NOT EXISTS idx_deletion_wallet      ON deletion_requests (wallet_pubkey);
CREATE INDEX IF NOT EXISTS idx_deletion_pending     ON deletion_requests (wallet_pubkey) WHERE completed_at IS NULL;

-- Fast lookup of active KYC records (already has UNIQUE, but explicit for clarity)
CREATE INDEX IF NOT EXISTS idx_kyc_active           ON kyc_records (wallet_pubkey) WHERE deleted_at IS NULL;
