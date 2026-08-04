-- Migration 007: Add password_hash column to users_v2.
-- Migration 003 created users_v2 without this column for the "wallet-first"
-- design, but the email/password auth code (register_email, login_email) still
-- references it. Re-add it as a nullable column so both flows work.

ALTER TABLE users_v2 ADD COLUMN password_hash TEXT;
