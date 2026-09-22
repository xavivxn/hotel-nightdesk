CREATE TABLE backup_queue (
  backup_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('local_ready', 'pending_upload', 'uploaded', 'failed')),
  snapshot_path TEXT NOT NULL,
  enc_path TEXT,
  size_bytes INTEGER NOT NULL,
  checksum TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  app_version TEXT NOT NULL,
  motel_id TEXT NOT NULL,
  last_error TEXT,
  uploaded_at TEXT,
  remote_path TEXT
);
CREATE INDEX idx_backup_queue_status ON backup_queue(status);
CREATE TABLE backup_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
