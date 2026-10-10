-- Privy DIDs are credentials mapped to wallet identities, not user IDs.
-- Keep wallet keys stable for Elo, KYC, tournaments, and on-chain accounts.

CREATE TABLE IF NOT EXISTS social_identities (
    provider      TEXT    NOT NULL,            -- 'privy'
    subject       TEXT    NOT NULL,            -- Privy DID, e.g. did:privy:cl...
    wallet        TEXT    NOT NULL,            -- -> users_v2.wallet
    login_method  TEXT    NOT NULL,            -- 'google' | 'email' | ...
    email         TEXT,                        -- as asserted by the provider
    embedded      INTEGER NOT NULL DEFAULT 1,  -- 1 = Privy-created embedded wallet
    created_at    INTEGER NOT NULL,
    last_login_at INTEGER NOT NULL,
    PRIMARY KEY (provider, subject)
);

CREATE INDEX IF NOT EXISTS idx_social_identities_wallet
    ON social_identities (wallet);

-- A non-null provider email may map to only one wallet.
CREATE UNIQUE INDEX IF NOT EXISTS idx_social_identities_email
    ON social_identities (provider, LOWER(email))
    WHERE email IS NOT NULL;
