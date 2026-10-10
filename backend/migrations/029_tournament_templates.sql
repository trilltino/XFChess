
CREATE TABLE IF NOT EXISTS tournament_templates (
    name        TEXT PRIMARY KEY,
    data_json   TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
