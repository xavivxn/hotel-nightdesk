-- Only records explicitly requested maintenance. Ordinary updates never reset data.
CREATE TABLE maintenance_resets (
  reset_id TEXT PRIMARY KEY,
  applied_at TEXT NOT NULL
);
