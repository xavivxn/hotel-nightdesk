-- Tarifas Love Nestt: 1 hora 45.000 + adicional 30 min 15.000, o dormida 120.000.
UPDATE rate_plans SET active = 0 WHERE kind = 'night';

UPDATE rate_plans
SET name = '1 hora',
    base_amount_cents = 45000,
    extra_hour_cents = 15000,
    included_hours = 1,
    grace_minutes = 5,
    night_cutoff_hour = 10,
    active = 1
WHERE kind = 'hourly';

UPDATE rate_plans
SET name = 'Dormida',
    base_amount_cents = 120000,
    extra_hour_cents = 15000,
    included_hours = 12,
    grace_minutes = 5,
    night_cutoff_hour = 10,
    active = 1
WHERE kind = 'overnight';
