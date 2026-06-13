-- JWT revocation cut-offs: a per-subject "valid_after" timestamp.
-- A logout records the current time for the subject; any token issued at or
-- before that time is then rejected, giving the otherwise stateless JWTs a
-- server-side kill switch. (Also created idempotently by SessionStore::init.)
CREATE TABLE IF NOT EXISTS jwt_revocations (
    subject     TEXT    PRIMARY KEY,
    valid_after INTEGER NOT NULL
);
