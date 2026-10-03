-- Operational revisions are independent of Supabase's catalog versions.
ALTER TABLE rooms ADD COLUMN operational_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE stays ADD COLUMN operational_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE reservations ADD COLUMN operational_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE receipt_snapshots ADD COLUMN document_json TEXT;

CREATE TABLE operation_results (
  operation_id TEXT PRIMARY KEY,
  actor_uid TEXT NOT NULL,
  station_id TEXT NOT NULL,
  command TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE operational_audit (
  uid TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL UNIQUE,
  actor_uid TEXT NOT NULL,
  username TEXT NOT NULL,
  station_id TEXT NOT NULL,
  command TEXT NOT NULL,
  entity_id INTEGER,
  closed_total_cents INTEGER,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_operational_audit_time ON operational_audit(created_at, actor_uid);
CREATE TABLE lan_stations (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL
);
CREATE TABLE local_revision (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL);
INSERT INTO local_revision VALUES (1,0);

CREATE TRIGGER rooms_operational_revision AFTER UPDATE OF status ON rooms
WHEN OLD.status IS NOT NEW.status BEGIN
  UPDATE rooms SET operational_version=operational_version+1 WHERE id=NEW.id;
END;
CREATE TRIGGER stays_operational_revision AFTER UPDATE OF status, converted_to_overnight ON stays
BEGIN UPDATE stays SET operational_version=operational_version+1 WHERE id=NEW.id; END;
CREATE TRIGGER reservations_operational_revision AFTER UPDATE OF status ON reservations
BEGIN UPDATE reservations SET operational_version=operational_version+1 WHERE id=NEW.id; END;
CREATE TRIGGER charges_account_insert AFTER INSERT ON charges
BEGIN UPDATE stays SET operational_version=operational_version+1 WHERE id=NEW.stay_id; END;
CREATE TRIGGER charges_account_update AFTER UPDATE OF deleted_at, amount_cents ON charges
BEGIN UPDATE stays SET operational_version=operational_version+1 WHERE id=NEW.stay_id; END;

CREATE TRIGGER lan_revision_rooms_insert AFTER INSERT ON rooms
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_rooms_update AFTER UPDATE ON rooms
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_rooms_delete AFTER DELETE ON rooms
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_stays_insert AFTER INSERT ON stays
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_stays_update AFTER UPDATE ON stays
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_stays_delete AFTER DELETE ON stays
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_reservations_insert AFTER INSERT ON reservations
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_reservations_update AFTER UPDATE ON reservations
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_reservations_delete AFTER DELETE ON reservations
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_charges_insert AFTER INSERT ON charges
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_charges_update AFTER UPDATE ON charges
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_charges_delete AFTER DELETE ON charges
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_guests_insert AFTER INSERT ON guests
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_guests_update AFTER UPDATE ON guests
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_guests_delete AFTER DELETE ON guests
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_rate_plans_insert AFTER INSERT ON rate_plans
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_rate_plans_update AFTER UPDATE ON rate_plans
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_rate_plans_delete AFTER DELETE ON rate_plans
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_products_insert AFTER INSERT ON products
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_products_update AFTER UPDATE ON products
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_products_delete AFTER DELETE ON products
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_users_insert AFTER INSERT ON users
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_users_update AFTER UPDATE ON users
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_users_delete AFTER DELETE ON users
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_settings_insert AFTER INSERT ON settings
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_settings_update AFTER UPDATE ON settings
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_settings_delete AFTER DELETE ON settings
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_product_stock_insert AFTER INSERT ON product_stock
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_product_stock_update AFTER UPDATE ON product_stock
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_product_stock_delete AFTER DELETE ON product_stock
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_operational_audit_insert AFTER INSERT ON operational_audit
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_operational_audit_update AFTER UPDATE ON operational_audit
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;

CREATE TRIGGER lan_revision_operational_audit_delete AFTER DELETE ON operational_audit
BEGIN UPDATE local_revision SET revision=revision+1 WHERE id=1; END;
