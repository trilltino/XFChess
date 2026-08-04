-- Migration 014: client think-time telemetry.
-- Adds an audited per-move think time alongside the blur flag (migration 013).
-- think_ms is a client *claim* — the analysis enqueue audits it against the
-- server-observed game wall clock before it reaches scoring.

ALTER TABLE move_telemetry ADD COLUMN think_ms INTEGER;
