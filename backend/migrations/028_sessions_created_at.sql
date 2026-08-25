-- Track session age so abandoned wallet prompts can expire from the pending cap.
ALTER TABLE sessions ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0;