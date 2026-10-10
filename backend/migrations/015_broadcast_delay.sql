-- Expose moves only after broadcast_delay_secs for delayed spectator feeds.
-- Zero means live.

ALTER TABLE games ADD COLUMN broadcast_delay_secs INTEGER NOT NULL DEFAULT 0;
