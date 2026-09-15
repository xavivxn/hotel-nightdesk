CREATE TABLE users (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 username TEXT NOT NULL UNIQUE COLLATE NOCASE,
 password_hash TEXT NOT NULL,
 role TEXT NOT NULL CHECK(role IN ('admin', 'recepcion')),
 active INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE login_attempts (
 username TEXT PRIMARY KEY,
 failures INTEGER NOT NULL,
 blocked_until INTEGER NOT NULL
);
