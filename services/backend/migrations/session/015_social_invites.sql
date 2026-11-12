-- Profile-owned invitations survive client/backend restarts. Stable IDs replace
-- indexes into a capped in-memory list; dismissal is scoped to the recipient.
CREATE TABLE IF NOT EXISTS social_invites (
    id TEXT PRIMARY KEY,
    recipient TEXT NOT NULL,
    sender TEXT NOT NULL,
    game_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    UNIQUE(recipient, sender, game_id)
);
CREATE INDEX IF NOT EXISTS idx_social_invites_recipient ON social_invites(recipient, expires_at);
CREATE INDEX IF NOT EXISTS idx_social_invites_expiry ON social_invites(expires_at);
