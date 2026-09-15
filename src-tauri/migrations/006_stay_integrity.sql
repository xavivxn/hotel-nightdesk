PRAGMA foreign_keys = ON;

ALTER TABLE stays ADD COLUMN closed_applied_kind TEXT;
ALTER TABLE stays ADD COLUMN closed_tax_percent REAL;
ALTER TABLE stays ADD COLUMN closed_duration_label TEXT;

DROP INDEX IF EXISTS idx_stays_room_open;
CREATE UNIQUE INDEX IF NOT EXISTS idx_stays_one_open_per_room
ON stays(room_id) WHERE status = 'open';
