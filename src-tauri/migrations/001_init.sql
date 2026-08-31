PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_migrations (
  id TEXT PRIMARY KEY,
  applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rooms (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  number TEXT NOT NULL UNIQUE,
  room_type TEXT NOT NULL DEFAULT 'Estándar',
  floor INTEGER NOT NULL DEFAULT 1,
  status TEXT NOT NULL DEFAULT 'available',
  notes TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rate_plans (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  base_amount_cents INTEGER NOT NULL,
  extra_hour_cents INTEGER NOT NULL DEFAULT 0,
  included_hours INTEGER NOT NULL DEFAULT 1,
  grace_minutes INTEGER NOT NULL DEFAULT 10,
  night_cutoff_hour INTEGER NOT NULL DEFAULT 12,
  active INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS guests (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  document TEXT,
  phone TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS reservations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  guest_id INTEGER NOT NULL REFERENCES guests(id),
  room_id INTEGER NOT NULL REFERENCES rooms(id),
  rate_plan_id INTEGER NOT NULL REFERENCES rate_plans(id),
  expected_arrival_at TEXT NOT NULL,
  expected_nights INTEGER NOT NULL DEFAULT 1,
  status TEXT NOT NULL DEFAULT 'hold',
  notes TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS stays (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  room_id INTEGER NOT NULL REFERENCES rooms(id),
  guest_id INTEGER NOT NULL REFERENCES guests(id),
  rate_plan_id INTEGER NOT NULL REFERENCES rate_plans(id),
  reservation_id INTEGER REFERENCES reservations(id),
  check_in_at TEXT NOT NULL,
  expected_checkout_at TEXT,
  check_out_at TEXT,
  status TEXT NOT NULL DEFAULT 'open',
  converted_to_overnight INTEGER NOT NULL DEFAULT 0,
  overnight_rate_plan_id INTEGER REFERENCES rate_plans(id),
  notes TEXT
);

CREATE TABLE IF NOT EXISTS charges (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  stay_id INTEGER NOT NULL REFERENCES stays(id),
  kind TEXT NOT NULL,
  description TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS payments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  stay_id INTEGER NOT NULL REFERENCES stays(id),
  method TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_stays_room_open ON stays(room_id, status);
CREATE INDEX IF NOT EXISTS idx_reservations_room_status ON reservations(room_id, status);
CREATE INDEX IF NOT EXISTS idx_charges_stay ON charges(stay_id);
CREATE INDEX IF NOT EXISTS idx_payments_stay ON payments(stay_id);
