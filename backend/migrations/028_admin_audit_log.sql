-- Persist handler-specific audit entries and generic middleware records
-- for mutating admin requests that have no handler-specific entry.

CREATE TABLE IF NOT EXISTS admin_audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    ts          INTEGER NOT NULL,
    actor       TEXT NOT NULL,
    action      TEXT NOT NULL,
    target      TEXT NOT NULL DEFAULT '',
    result      TEXT NOT NULL DEFAULT '',
    method      TEXT NOT NULL DEFAULT '',
    path        TEXT NOT NULL DEFAULT '',
    status      INTEGER
);

CREATE INDEX IF NOT EXISTS idx_admin_audit_log_target ON admin_audit_log(target);
CREATE INDEX IF NOT EXISTS idx_admin_audit_log_ts ON admin_audit_log(ts);
