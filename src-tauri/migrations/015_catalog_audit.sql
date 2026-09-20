CREATE TABLE catalog_audit (
  operation_id TEXT PRIMARY KEY,
  entity TEXT NOT NULL,
  actor TEXT NOT NULL,
  created_at TEXT NOT NULL,
  before_json TEXT NOT NULL,
  after_json TEXT NOT NULL
);
CREATE TABLE catalog_setting_versions (key TEXT PRIMARY KEY, version INTEGER NOT NULL);
