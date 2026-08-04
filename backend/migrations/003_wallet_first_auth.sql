-- Migration 003: Wallet-first auth — no passwords
-- The wallet signature proves identity. Email is optional (notifications only).
-- Migrates existing users into the new schema and drops password_hash.

CREATE TABLE IF NOT EXISTS users_v2 (
    wallet      TEXT PRIMARY KEY,   -- Solana pubkey is the identity
    username    TEXT NOT NULL,
    email       TEXT,               -- Optional, for notifications / account recovery
    kyc_status  TEXT NOT NULL DEFAULT 'none',
    created_at  INTEGER NOT NULL DEFAULT 0,
    deleted_at  INTEGER             -- NULL = active, set on GDPR erasure
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_v2_username ON users_v2 (LOWER(username));

-- Migrate any existing rows that have a wallet linked
INSERT OR IGNORE INTO users_v2 (wallet, username, email, created_at)
SELECT wallet, username, email, COALESCE(created_at, 0)
FROM   users
WHERE  wallet IS NOT NULL AND wallet != '';
