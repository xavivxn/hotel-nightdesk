PRAGMA foreign_keys = ON;

ALTER TABLE stays ADD COLUMN closed_total_cents INTEGER;
ALTER TABLE stays ADD COLUMN closed_line_count INTEGER;

-- Older databases used `product` for a consumed item. It is the same
-- historical kind as a surcharge and Supabase intentionally accepts only the
-- canonical kind set.
UPDATE charges SET kind = 'surcharge' WHERE kind = 'product';

-- A marker is only backfilled when the old closure snapshot proves that the
-- account was closed through the current flow. Legacy rows without that
-- snapshot stay NULL and are reported as requiring review remotely.
UPDATE stays
SET closed_total_cents = (
      SELECT COALESCE(SUM(c.amount_cents), 0)
      FROM charges c
      WHERE c.stay_id = stays.id AND c.deleted_at IS NULL
    ),
    closed_line_count = (
      SELECT COUNT(*)
      FROM charges c
      WHERE c.stay_id = stays.id AND c.deleted_at IS NULL
    )
WHERE stays.status = 'closed'
  AND stays.closed_applied_kind IS NOT NULL
  AND stays.closed_total_cents IS NULL;
