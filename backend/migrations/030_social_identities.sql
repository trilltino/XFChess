-- Social-login credentials (Privy: Google / email) mapped onto wallets.
--
-- The Solana pubkey remains THE identity (users_v2.wallet is still the PK of a
-- user). A Privy DID is only a credential that resolves to one. That ordering is
-- deliberate: ELO, KYC, tournaments, game history and every on-chain PDA are all
-- keyed on the wallet, and dropping Privy later must not orphan any of them.
--
-- See docs/plans/social-login-embedded-wallet-plan.md §5 (D1, D3) and §9.1.

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

-- Enforces D3 ("one human, one account") in the database rather than in handler
-- logic: a given Google address can resolve to exactly one wallet. Without this,
-- a user who signs up with Google, later connects Phantom, and then signs in
-- with Google again ends up with two accounts, two ELOs, and funds split across
-- both. Partial (WHERE email IS NOT NULL) because email is optional on the
-- provider side and SQLite would otherwise treat multiple NULLs as duplicates.
CREATE UNIQUE INDEX IF NOT EXISTS idx_social_identities_email
    ON social_identities (provider, LOWER(email))
    WHERE email IS NOT NULL;
