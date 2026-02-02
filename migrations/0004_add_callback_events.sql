-- Add per-job callback event filters.
ALTER TABLE jobs ADD COLUMN callback_events TEXT[];
