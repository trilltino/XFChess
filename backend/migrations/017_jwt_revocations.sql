-- Reject JWTs issued at or before the subject's valid_after logout cutoff.
CREATE TABLE IF NOT EXISTS jwt_revocations (
    subject     TEXT    PRIMARY KEY,
    valid_after INTEGER NOT NULL
);
