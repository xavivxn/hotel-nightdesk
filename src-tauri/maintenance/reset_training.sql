-- Called inside the same transaction as seed + maintenance_resets marker.
-- Keep users (including hashes, IDs, roles and active flags) and schema metadata.
-- Keep the technical device role too: clearing it with users present would
-- make App.initialize silently turn a remote-admin PC into a reception.
-- Credentials live outside SQLite and are not touched.
DELETE FROM receipt_snapshots;
DELETE FROM payments;
DELETE FROM charges;
DELETE FROM stays;
DELETE FROM reservations;
DELETE FROM guests;
DELETE FROM stock_movements;
DELETE FROM product_stock;
DELETE FROM rooms;
DELETE FROM rate_plans;
DELETE FROM products;
DELETE FROM catalog_audit;
DELETE FROM catalog_setting_versions;
DELETE FROM login_attempts;
DELETE FROM sync_outbox;
DELETE FROM sync_state;
DELETE FROM backup_queue;
DELETE FROM backup_meta;
DELETE FROM settings WHERE key <> 'device_mode';
DELETE FROM sqlite_sequence WHERE name IN (
  'rooms', 'rate_plans', 'products', 'guests', 'reservations', 'stays',
  'charges', 'payments', 'stock_movements', 'sync_outbox'
);
