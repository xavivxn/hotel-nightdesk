-- Shop stock at reception (local, not synced). A product without a row is not tracked.
-- No foreign keys: a catalog pull must never fail because of stock rows.
CREATE TABLE product_stock (
  product_id INTEGER PRIMARY KEY,
  quantity INTEGER NOT NULL,
  min_quantity INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL
);

CREATE TABLE stock_movements (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  product_id INTEGER NOT NULL,
  delta INTEGER NOT NULL,
  quantity_after INTEGER NOT NULL,
  reason TEXT NOT NULL CHECK (reason IN ('sale', 'void', 'restock', 'count')),
  charge_id INTEGER,
  username TEXT,
  note TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX idx_stock_movements_product ON stock_movements(product_id, id);
CREATE INDEX idx_stock_movements_charge ON stock_movements(charge_id);
