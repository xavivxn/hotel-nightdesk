-- Jacuzzi and normal rooms have their own hour, adicional 30 min and dormida prices.
-- The jacuzzi plans themselves are seeded by `db::seed_jacuzzi_plans_if_missing` (fixed uids
-- shared with Supabase), so a fresh install and an upgrade end up with the same rows.
ALTER TABLE rate_plans ADD COLUMN room_category TEXT NOT NULL DEFAULT 'normal';

-- Username that opened (check-in) and charged (checkout) each stay, for the history.
ALTER TABLE stays ADD COLUMN checked_in_by TEXT;
ALTER TABLE stays ADD COLUMN checked_out_by TEXT;
