-- I06: identity, catalog versions, logical charge deletes, local outbox.
-- SQLite cannot ADD COLUMN with UNIQUE or a non-constant default, so uid is
-- filled, then indexed. AFTER INSERT triggers cover seed, auth and tests.

-- rooms
ALTER TABLE rooms ADD COLUMN uid TEXT;
ALTER TABLE rooms ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE rooms ADD COLUMN updated_at TEXT;
UPDATE rooms SET uid = (
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
  substr(lower(hex(randomblob(2))), 2) || '-' ||
  substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
  lower(hex(randomblob(6)))
) WHERE uid IS NULL;
UPDATE rooms SET updated_at = datetime('now') WHERE updated_at IS NULL;
CREATE UNIQUE INDEX idx_rooms_uid ON rooms(uid);
CREATE TRIGGER rooms_uid_ai AFTER INSERT ON rooms WHEN NEW.uid IS NULL
BEGIN
  UPDATE rooms SET uid = (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-' ||
    substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
    lower(hex(randomblob(6)))
  ) WHERE id = NEW.id AND uid IS NULL;
END;

-- rate_plans
ALTER TABLE rate_plans ADD COLUMN uid TEXT;
ALTER TABLE rate_plans ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE rate_plans ADD COLUMN updated_at TEXT;
UPDATE rate_plans SET uid = (
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
  substr(lower(hex(randomblob(2))), 2) || '-' ||
  substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
  lower(hex(randomblob(6)))
) WHERE uid IS NULL;
UPDATE rate_plans SET updated_at = datetime('now') WHERE updated_at IS NULL;
CREATE UNIQUE INDEX idx_rate_plans_uid ON rate_plans(uid);
CREATE TRIGGER rate_plans_uid_ai AFTER INSERT ON rate_plans WHEN NEW.uid IS NULL
BEGIN
  UPDATE rate_plans SET uid = (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-' ||
    substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
    lower(hex(randomblob(6)))
  ) WHERE id = NEW.id AND uid IS NULL;
END;

-- products
ALTER TABLE products ADD COLUMN uid TEXT;
ALTER TABLE products ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE products ADD COLUMN updated_at TEXT;
UPDATE products SET uid = (
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
  substr(lower(hex(randomblob(2))), 2) || '-' ||
  substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
  lower(hex(randomblob(6)))
) WHERE uid IS NULL;
UPDATE products SET updated_at = datetime('now') WHERE updated_at IS NULL;
CREATE UNIQUE INDEX idx_products_uid ON products(uid);
CREATE TRIGGER products_uid_ai AFTER INSERT ON products WHEN NEW.uid IS NULL
BEGIN
  UPDATE products SET uid = (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-' ||
    substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
    lower(hex(randomblob(6)))
  ) WHERE id = NEW.id AND uid IS NULL;
END;

-- guests
ALTER TABLE guests ADD COLUMN uid TEXT;
UPDATE guests SET uid = (
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
  substr(lower(hex(randomblob(2))), 2) || '-' ||
  substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
  lower(hex(randomblob(6)))
) WHERE uid IS NULL;
CREATE UNIQUE INDEX idx_guests_uid ON guests(uid);
CREATE TRIGGER guests_uid_ai AFTER INSERT ON guests WHEN NEW.uid IS NULL
BEGIN
  UPDATE guests SET uid = (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-' ||
    substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
    lower(hex(randomblob(6)))
  ) WHERE id = NEW.id AND uid IS NULL;
END;

-- reservations
ALTER TABLE reservations ADD COLUMN uid TEXT;
UPDATE reservations SET uid = (
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
  substr(lower(hex(randomblob(2))), 2) || '-' ||
  substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
  lower(hex(randomblob(6)))
) WHERE uid IS NULL;
CREATE UNIQUE INDEX idx_reservations_uid ON reservations(uid);
CREATE TRIGGER reservations_uid_ai AFTER INSERT ON reservations WHEN NEW.uid IS NULL
BEGIN
  UPDATE reservations SET uid = (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-' ||
    substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
    lower(hex(randomblob(6)))
  ) WHERE id = NEW.id AND uid IS NULL;
END;

-- stays
ALTER TABLE stays ADD COLUMN uid TEXT;
UPDATE stays SET uid = (
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
  substr(lower(hex(randomblob(2))), 2) || '-' ||
  substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
  lower(hex(randomblob(6)))
) WHERE uid IS NULL;
CREATE UNIQUE INDEX idx_stays_uid ON stays(uid);
CREATE TRIGGER stays_uid_ai AFTER INSERT ON stays WHEN NEW.uid IS NULL
BEGIN
  UPDATE stays SET uid = (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-' ||
    substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
    lower(hex(randomblob(6)))
  ) WHERE id = NEW.id AND uid IS NULL;
END;

-- charges
ALTER TABLE charges ADD COLUMN uid TEXT;
ALTER TABLE charges ADD COLUMN deleted_at TEXT;
UPDATE charges SET uid = (
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
  substr(lower(hex(randomblob(2))), 2) || '-' ||
  substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
  lower(hex(randomblob(6)))
) WHERE uid IS NULL;
CREATE UNIQUE INDEX idx_charges_uid ON charges(uid);
CREATE TRIGGER charges_uid_ai AFTER INSERT ON charges WHEN NEW.uid IS NULL
BEGIN
  UPDATE charges SET uid = (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-' ||
    substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
    lower(hex(randomblob(6)))
  ) WHERE id = NEW.id AND uid IS NULL;
END;

-- payments
ALTER TABLE payments ADD COLUMN uid TEXT;
UPDATE payments SET uid = (
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
  substr(lower(hex(randomblob(2))), 2) || '-' ||
  substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
  lower(hex(randomblob(6)))
) WHERE uid IS NULL;
CREATE UNIQUE INDEX idx_payments_uid ON payments(uid);
CREATE TRIGGER payments_uid_ai AFTER INSERT ON payments WHEN NEW.uid IS NULL
BEGIN
  UPDATE payments SET uid = (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-' ||
    substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
    lower(hex(randomblob(6)))
  ) WHERE id = NEW.id AND uid IS NULL;
END;

-- users
ALTER TABLE users ADD COLUMN uid TEXT;
ALTER TABLE users ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE users ADD COLUMN updated_at TEXT;
UPDATE users SET uid = (
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
  substr(lower(hex(randomblob(2))), 2) || '-' ||
  substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
  lower(hex(randomblob(6)))
) WHERE uid IS NULL;
UPDATE users SET updated_at = datetime('now') WHERE updated_at IS NULL;
CREATE UNIQUE INDEX idx_users_uid ON users(uid);
CREATE TRIGGER users_uid_ai AFTER INSERT ON users WHEN NEW.uid IS NULL
BEGIN
  UPDATE users SET uid = (
    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-' ||
    substr('89ab', abs(random()) % 4 + 1, 1) || substr(lower(hex(randomblob(2))), 2) || '-' ||
    lower(hex(randomblob(6)))
  ) WHERE id = NEW.id AND uid IS NULL;
END;

CREATE TABLE sync_outbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  operation_id TEXT NOT NULL UNIQUE,
  root_operation_id TEXT NOT NULL,
  entity TEXT NOT NULL,
  entity_uid TEXT NOT NULL,
  op TEXT NOT NULL CHECK (op IN ('upsert', 'delete')),
  payload TEXT NOT NULL,
  created_at TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'sent', 'rejected'))
);
CREATE INDEX idx_sync_outbox_status_id ON sync_outbox (status, id);
CREATE INDEX idx_sync_outbox_root ON sync_outbox (root_operation_id);

CREATE TABLE sync_state (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
