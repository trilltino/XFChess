-- Admin audit trail and saved tournament templates.

-- Handler-specific entries, plus a generic record from the middleware for any
-- mutating admin request without one.
CREATE TABLE IF NOT EXISTS admin_audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    ts          INTEGER NOT NULL,
    actor       TEXT    NOT NULL,
    action      TEXT    NOT NULL,
    target      TEXT    NOT NULL DEFAULT '',
    result      TEXT    NOT NULL DEFAULT '',
    method      TEXT    NOT NULL DEFAULT '',
    path        TEXT    NOT NULL DEFAULT '',
    status      INTEGER
);

CREATE INDEX IF NOT EXISTS idx_admin_audit_log_target ON admin_audit_log (target);
CREATE INDEX IF NOT EXISTS idx_admin_audit_log_ts     ON admin_audit_log (ts);

CREATE TABLE IF NOT EXISTS tournament_templates (
    name        TEXT    PRIMARY KEY,
    data_json   TEXT    NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
