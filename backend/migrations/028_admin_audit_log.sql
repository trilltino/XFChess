-- Migration 028: persistent admin audit log.
--
-- Previously the admin audit trail (admin.rs AUDIT_LOG) was a process-local
-- Vec capped at 500 entries, actor hardcoded to the literal string "admin",
-- and lost entirely on restart. This table backs a real, persistent,
-- per-actor audit trail. Two kinds of rows land here:
--   - "rich" entries from add_audit() calls inside specific handlers
--     (action/target/result carry a human-readable description)
--   - "generic" entries from the catch-all admin-request middleware, which
--     logs every mutating /admin/* request even if its handler never calls
--     add_audit() — this is what closes the "new endpoint forgot to log"
--     gap (method/path/status carry the description instead).

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
