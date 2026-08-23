-- Migration 029: persistent tournament templates.
--
-- Previously "SAVE" in the admin panel's TournamentDetail overview wrote to
-- localStorage only (tauri/tournament-admin's own browser storage) — invisible
-- across machines, invisible in the audit log, and gone if the operator
-- clears app data. This table backs a real, shared, audited store.

CREATE TABLE IF NOT EXISTS tournament_templates (
    name        TEXT PRIMARY KEY,
    data_json   TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
