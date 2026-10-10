-- Select record_move or global_record_move using is_global; global games
-- do not create the per-game SessionDelegation required by record_move.
ALTER TABLE sessions ADD COLUMN is_global INTEGER NOT NULL DEFAULT 0;
