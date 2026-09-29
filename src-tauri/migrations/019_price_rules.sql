-- Promotion price snapshotted at check-in (or at conversion to dormida).
-- It applies only while price_plan_id is the plan being billed. Rules live in settings.price_rules.
ALTER TABLE stays ADD COLUMN price_plan_id INTEGER;
ALTER TABLE stays ADD COLUMN price_base_cents INTEGER;
ALTER TABLE stays ADD COLUMN price_extra_cents INTEGER;
ALTER TABLE stays ADD COLUMN price_rule TEXT;
