-- Identity is captured at sale time, independently of optional stock tracking.
-- Historical surcharges deliberately remain unclassified.
ALTER TABLE charges ADD COLUMN product_uid TEXT;
ALTER TABLE charges ADD COLUMN product_quantity INTEGER
  CHECK (product_quantity IS NULL OR product_quantity > 0);
ALTER TABLE stays ADD COLUMN product_tracking_since TEXT;
INSERT INTO settings(key, value) VALUES
  ('product_tracking_since', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
