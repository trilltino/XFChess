-- think_ms is a client claim audited against server wall-clock duration
-- before scoring.

ALTER TABLE move_telemetry ADD COLUMN think_ms INTEGER;
