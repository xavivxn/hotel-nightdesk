CREATE TABLE receipt_snapshots (
  stay_id INTEGER PRIMARY KEY REFERENCES stays(id),
  bytes BLOB NOT NULL,
  created_at TEXT NOT NULL
);
