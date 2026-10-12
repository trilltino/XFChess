-- Accounts. The wallet is the identity; email, password and social logins are
-- optional ways to reach it.

CREATE TABLE IF NOT EXISTS users_v2 (
    wallet               TEXT PRIMARY KEY,
    username             TEXT NOT NULL,
    email                TEXT,               -- for notifications / account recovery
    kyc_status           TEXT NOT NULL DEFAULT 'none',
    created_at           INTEGER NOT NULL DEFAULT 0,
    deleted_at           INTEGER,            -- set on GDPR erasure
    password_hash        TEXT,
    profile_sponsored_at INTEGER             -- one backend-sponsored on-chain profile per account
);
ALTER TABLE users_v2 ADD COLUMN password_hash TEXT;
ALTER TABLE users_v2 ADD COLUMN profile_sponsored_at INTEGER;

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_v2_username ON users_v2 (LOWER(username));

-- JWTs issued at or before a subject's valid_after (its last logout) are rejected.
CREATE TABLE IF NOT EXISTS jwt_revocations (
    subject     TEXT    PRIMARY KEY,
    valid_after INTEGER NOT NULL
);

-- Privy logins (Google, email, ...) mapped to the wallet they control. A DID is
-- a credential, not an account: Elo, KYC and on-chain state stay keyed by wallet.
CREATE TABLE IF NOT EXISTS social_identities (
    provider      TEXT    NOT NULL,            -- 'privy'
    subject       TEXT    NOT NULL,            -- Privy DID
    wallet        TEXT    NOT NULL,            -- users_v2.wallet
    login_method  TEXT    NOT NULL,            -- 'google' | 'email' | ...
    email         TEXT,                        -- as asserted by the provider
    embedded      INTEGER NOT NULL DEFAULT 1,  -- Privy-created embedded wallet
    created_at    INTEGER NOT NULL,
    last_login_at INTEGER NOT NULL,
    PRIMARY KEY (provider, subject)
);

CREATE INDEX IF NOT EXISTS idx_social_identities_wallet ON social_identities (wallet);

-- A provider email maps to at most one wallet.
CREATE UNIQUE INDEX IF NOT EXISTS idx_social_identities_email
    ON social_identities (provider, LOWER(email))
    WHERE email IS NOT NULL;
