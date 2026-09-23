//! I05/I08: consistent SQLite snapshots, AES-GCM encryption, local queue, Storage upload.
use crate::credentials;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::{BackupListItem, BackupRunResult, BackupStatus};
use crate::sync::client::SupabaseClient;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use chrono::{Datelike, Local, Timelike, Utc};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rand::RngCore;
use rusqlite::{params, Connection, DatabaseName, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::AppHandle;

const LOCAL_KEEP: usize = 7;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const SCHEDULE_HOUR: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub backup_id: String,
    pub motel_id: String,
    pub backuped_at: String,
    pub schema_version: String,
    pub app_version: String,
    pub size_bytes: u64,
    pub checksum: String,
    #[serde(default)]
    pub nonce_hex: Option<String>,
    #[serde(default)]
    pub storage_path: Option<String>,
}

#[derive(Clone)]
pub struct BackupHandle {
    tx: mpsc::Sender<BackupWake>,
}

enum BackupWake {
    Drain,
    RunNow(mpsc::Sender<AppResult<BackupRunResult>>),
}

impl BackupHandle {
    pub fn wake_drain(&self) {
        let _ = self.tx.send(BackupWake::Drain);
    }

    pub fn run_now(&self) -> AppResult<BackupRunResult> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(BackupWake::RunNow(tx))
            .map_err(|_| AppError::storage("Motor de respaldos no disponible"))?;
        rx.recv_timeout(Duration::from_secs(120))
            .map_err(|_| AppError::storage("El respaldo no respondió a tiempo"))?
    }
}

pub fn should_start(device_mode: Option<&str>) -> bool {
    device_mode == Some("reception")
}

pub fn start(app: AppHandle, db_path: PathBuf, data_dir: PathBuf) -> AppResult<BackupHandle> {
    let _ = credentials::ensure_backup_key(&data_dir)?;
    let (tx, rx) = mpsc::channel();
    let handle = BackupHandle { tx: tx.clone() };
    thread::Builder::new()
        .name("nightdesk-backup".into())
        .spawn(move || worker_loop(app, db_path, data_dir, rx))
        .map_err(|_| AppError::storage("No se pudo iniciar el motor de respaldos"))?;
    let _ = tx.send(BackupWake::Drain);
    Ok(handle)
}

fn worker_loop(app: AppHandle, db_path: PathBuf, data_dir: PathBuf, rx: mpsc::Receiver<BackupWake>) {
    let last_tick = Arc::new(Mutex::new(Utc::now().date_naive()));
    loop {
        match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(BackupWake::Drain) => {
                let _ = drain_once(&db_path, &data_dir);
            }
            Ok(BackupWake::RunNow(reply)) => {
                let result = run_backup_now(&db_path, &data_dir);
                let _ = reply.send(result);
                let _ = drain_once(&db_path, &data_dir);
            }
            Err(RecvTimeoutError::Timeout) => {
                maybe_daily(&db_path, &data_dir, &last_tick);
                let _ = drain_once(&db_path, &data_dir);
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let _ = app;
    }
}

fn maybe_daily(db_path: &Path, data_dir: &Path, last_tick: &Mutex<chrono::NaiveDate>) {
    let now = Local::now();
    if now.hour() < SCHEDULE_HOUR {
        return;
    }
    let today = Utc::now().date_naive();
    let mut guard = last_tick.lock().unwrap_or_else(|e| e.into_inner());
    if *guard == today {
        return;
    }
    if let Ok(conn) = Connection::open(db_path) {
        if day_has_local(&conn, &today.to_string()).unwrap_or(true) {
            *guard = today;
            return;
        }
    }
    if run_backup_now(db_path, data_dir).is_ok() {
        *guard = today;
    }
}

fn day_has_local(conn: &Connection, day_prefix: &str) -> AppResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM backup_queue WHERE created_at LIKE ?1 || '%'",
        [day_prefix],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

pub fn status(conn: &Connection, data_dir: &Path) -> AppResult<BackupStatus> {
    let ready = credentials::backup_key_configured(data_dir)
        || credentials::ensure_backup_key(data_dir).is_ok();
    let last_local_at: Option<String> = conn
        .query_row(
            "SELECT created_at FROM backup_queue ORDER BY created_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let last_remote_at: Option<String> = conn
        .query_row(
            "SELECT uploaded_at FROM backup_queue WHERE status='uploaded' AND uploaded_at IS NOT NULL ORDER BY uploaded_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let pending: i64 = conn.query_row(
        "SELECT COUNT(*) FROM backup_queue WHERE status IN ('pending_upload','failed')",
        [],
        |r| r.get(0),
    )?;
    let last_error: Option<String> = conn
        .query_row(
            "SELECT last_error FROM backup_queue WHERE last_error IS NOT NULL AND last_error != '' ORDER BY created_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(BackupStatus {
        last_local_at,
        last_remote_at,
        pending,
        last_error,
        ready,
    })
}

pub fn list_local(conn: &Connection) -> AppResult<Vec<BackupListItem>> {
    let mut stmt = conn.prepare(
        "SELECT backup_id, created_at, status, size_bytes, checksum, schema_version, uploaded_at, remote_path
         FROM backup_queue ORDER BY created_at DESC LIMIT 30",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(BackupListItem {
                backup_id: r.get(0)?,
                created_at: r.get(1)?,
                status: r.get(2)?,
                size_bytes: r.get(3)?,
                checksum: r.get(4)?,
                schema_version: r.get(5)?,
                uploaded_at: r.get(6)?,
                remote_path: r.get(7)?,
                source: "local".into(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_backups(conn: &Connection, data_dir: &Path) -> AppResult<Vec<BackupListItem>> {
    let mut items = list_local(conn)?;
    if !credentials::device_configured(data_dir) {
        return Ok(items);
    }
    let client = match SupabaseClient::from_device(credentials::load_device(data_dir)?) {
        Ok(client) => client,
        Err(_) => return Ok(items),
    };
    let remotes = match client.list_backup_manifests() {
        Ok(rows) => rows,
        Err(_) => return Ok(items),
    };
    let local_ids: std::collections::HashSet<String> =
        items.iter().map(|item| item.backup_id.clone()).collect();
    for row in remotes {
        let backup_id = row
            .get("backup_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if backup_id.is_empty() || local_ids.contains(&backup_id) {
            continue;
        }
        items.push(BackupListItem {
            backup_id,
            created_at: row
                .get("backuped_at")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            status: "uploaded".into(),
            size_bytes: row.get("size_bytes").and_then(|v| v.as_i64()).unwrap_or(0),
            checksum: row
                .get("checksum")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            schema_version: row
                .get("schema_version")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            uploaded_at: row
                .get("backuped_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            remote_path: row
                .get("storage_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            source: "remote".into(),
        });
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

pub fn run_backup_now(db_path: &Path, data_dir: &Path) -> AppResult<BackupRunResult> {
    let key = credentials::ensure_backup_key(data_dir)?;
    let dir = backups_dir(data_dir);
    fs::create_dir_all(&dir)?;
    let backup_id = uuid::Uuid::new_v4().to_string();
    let snap_path = dir.join(format!("{backup_id}.db"));
    let live = Connection::open(db_path)?;
    live.execute_batch("PRAGMA foreign_keys = ON;")?;
    let manifest = create_snapshot(&live, &snap_path, &backup_id)?;
    let enc_path = dir.join(format!("{backup_id}.db.gz.enc"));
    let nonce = encrypt_file(&snap_path, &enc_path, &key)?;
    let mut manifest = manifest;
    manifest.nonce_hex = Some(hex::encode(nonce));
    let manifest_path = dir.join(format!("{backup_id}.manifest.json"));
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).map_err(|e| AppError::msg(e.to_string()))?)?;

    let queue_conn = Connection::open(db_path)?;
    queue_conn.execute(
        "INSERT INTO backup_queue(
            backup_id, created_at, status, snapshot_path, enc_path, size_bytes, checksum,
            schema_version, app_version, motel_id, last_error, uploaded_at, remote_path
         ) VALUES (?1,?2,'pending_upload',?3,?4,?5,?6,?7,?8,?9,NULL,NULL,NULL)",
        params![
            backup_id,
            manifest.backuped_at,
            snap_path.to_string_lossy(),
            enc_path.to_string_lossy(),
            manifest.size_bytes as i64,
            manifest.checksum,
            manifest.schema_version,
            manifest.app_version,
            manifest.motel_id,
        ],
    )?;
    retain_local(&queue_conn, &dir)?;
    Ok(BackupRunResult { backup_id })
}

pub fn create_snapshot(conn: &Connection, dest: &Path, backup_id: &str) -> AppResult<Manifest> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if dest.exists() {
        fs::remove_file(dest)?;
    }
    conn.backup(DatabaseName::Main, dest, None)?;
    verify_integrity(dest)?;
    let checksum = sha256_file(dest)?;
    let size_bytes = fs::metadata(dest)?.len();
    let schema_version = latest_schema(conn)?;
    let motel_id = db::get_setting(conn, "business_name", "motel")?;
    let motel_id = if motel_id.trim().is_empty() {
        "motel".into()
    } else {
        motel_id
    };
    Ok(Manifest {
        backup_id: backup_id.to_string(),
        motel_id,
        backuped_at: db::now_rfc3339(),
        schema_version,
        app_version: APP_VERSION.to_string(),
        size_bytes,
        checksum,
        nonce_hex: None,
        storage_path: None,
    })
}

pub fn verify_integrity(path: &Path) -> AppResult<()> {
    let conn = Connection::open(path)?;
    let ok: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if ok != "ok" {
        return Err(AppError::storage(format!("Integridad fallida: {ok}")));
    }
    Ok(())
}

fn latest_schema(conn: &Connection) -> AppResult<String> {
    let rows = db::list_applied_migrations(conn)?;
    Ok(rows.last().cloned().unwrap_or_else(|| "none".into()))
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn encrypt_file(src: &Path, dest: &Path, key: &[u8; 32]) -> AppResult<[u8; 12]> {
    let plain = fs::read(src)?;
    let mut compressed = Vec::new();
    {
        let mut enc = GzEncoder::new(&mut compressed, Compression::default());
        enc.write_all(&plain)?;
        enc.finish()?;
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AppError::storage("Clave AES inválida"))?;
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, compressed.as_ref())
        .map_err(|_| AppError::storage("No se pudo cifrar el respaldo"))?;
    fs::write(dest, ciphertext)?;
    Ok(nonce_bytes)
}

fn decrypt_file(src: &Path, dest: &Path, key: &[u8; 32], nonce_hex: &str) -> AppResult<()> {
    let ciphertext = fs::read(src)?;
    let nonce_raw = hex::decode(nonce_hex).map_err(|_| AppError::storage("Nonce inválido"))?;
    if nonce_raw.len() != 12 {
        return Err(AppError::storage("Nonce inválido"));
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| AppError::storage("Clave AES inválida"))?;
    let nonce = Nonce::from_slice(&nonce_raw);
    let compressed = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| AppError::storage("No se pudo descifrar el respaldo"))?;
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut plain = Vec::new();
    decoder.read_to_end(&mut plain)?;
    fs::write(dest, plain)?;
    Ok(())
}

fn drain_once(db_path: &Path, data_dir: &Path) -> AppResult<()> {
    if !credentials::device_configured(data_dir) {
        return Ok(());
    }
    let client = SupabaseClient::from_device(credentials::load_device(data_dir)?)?;
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT backup_id, enc_path, checksum, size_bytes, schema_version, app_version, motel_id, created_at
         FROM backup_queue WHERE status IN ('pending_upload','failed') ORDER BY created_at ASC LIMIT 3",
    )?;
    let rows: Vec<(String, String, String, i64, String, String, String, String)> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    for (backup_id, enc_path, checksum, size_bytes, schema_version, app_version, motel_id, created_at) in rows {
        let enc = PathBuf::from(&enc_path);
        if !enc.is_file() {
            conn.execute(
                "UPDATE backup_queue SET status='failed', last_error=?2 WHERE backup_id=?1",
                params![backup_id, "Falta el archivo cifrado local"],
            )?;
            continue;
        }
        let bytes = fs::read(&enc)?;
        let dt = chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let storage_path = format!(
            "{}/{}/{:02}/{}.db.gz.enc",
            sanitize_motel(&motel_id),
            dt.format("%Y"),
            dt.month(),
            backup_id
        );
        let mut manifest_path = enc.clone();
        manifest_path.set_file_name(format!("{backup_id}.manifest.json"));
        let nonce_hex = fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Manifest>(&raw).ok())
            .and_then(|m| m.nonce_hex);
        let Some(nonce_hex) = nonce_hex else {
            conn.execute(
                "UPDATE backup_queue SET status='failed', last_error=?2 WHERE backup_id=?1",
                params![backup_id, "Manifiesto local sin nonce de cifrado"],
            )?;
            continue;
        };
        match client.upload_backup_object(&storage_path, &bytes) {
            Ok(()) => {
                let row = serde_json::json!({
                    "motel_id": motel_id,
                    "backup_id": backup_id,
                    "backuped_at": created_at,
                    "schema_version": schema_version,
                    "app_version": app_version,
                    "size_bytes": size_bytes,
                    "checksum": checksum,
                    "storage_path": storage_path,
                    "nonce_hex": nonce_hex,
                });
                match client.insert_backup_manifest(&row) {
                    Ok(()) => {
                        conn.execute(
                            "UPDATE backup_queue SET status='uploaded', uploaded_at=?2, remote_path=?3, last_error=NULL WHERE backup_id=?1",
                            params![backup_id, db::now_rfc3339(), storage_path],
                        )?;
                        if let Ok(raw) = fs::read_to_string(&manifest_path) {
                            if let Ok(mut m) = serde_json::from_str::<Manifest>(&raw) {
                                m.storage_path = Some(storage_path);
                                let _ = fs::write(manifest_path, serde_json::to_string_pretty(&m).unwrap_or(raw));
                            }
                        }
                    }
                    Err(e) => {
                        conn.execute(
                            "UPDATE backup_queue SET status='failed', last_error=?2 WHERE backup_id=?1",
                            params![backup_id, e.to_string()],
                        )?;
                    }
                }
            }
            Err(e) => {
                conn.execute(
                    "UPDATE backup_queue SET status='failed', last_error=?2 WHERE backup_id=?1",
                    params![backup_id, e.to_string()],
                )?;
            }
        }
    }
    Ok(())
}

fn sanitize_motel(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    if s.is_empty() {
        "motel".into()
    } else {
        s.to_lowercase()
    }
}

fn retain_local(conn: &Connection, dir: &Path) -> AppResult<()> {
    let mut stmt = conn.prepare("SELECT backup_id FROM backup_queue ORDER BY created_at DESC")?;
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for (idx, id) in ids.into_iter().enumerate() {
        if idx < LOCAL_KEEP {
            continue;
        }
        let snap = dir.join(format!("{id}.db"));
        let enc = dir.join(format!("{id}.db.gz.enc"));
        let man = dir.join(format!("{id}.manifest.json"));
        let _ = fs::remove_file(snap);
        let _ = fs::remove_file(enc);
        let _ = fs::remove_file(man);
        conn.execute("DELETE FROM backup_queue WHERE backup_id=?1", [&id])?;
    }
    Ok(())
}

fn backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

/// Restore from a local queue entry or a remote Storage object.
pub fn restore_backup(
    live_db_path: &Path,
    data_dir: &Path,
    backup_id: &str,
    source: &str,
    auth: &mut crate::auth::AuthState,
) -> AppResult<()> {
    match source {
        "" | "local" => restore_local(live_db_path, data_dir, backup_id, auth),
        "remote" => restore_remote(live_db_path, data_dir, backup_id, auth),
        _ => Err(AppError::msg("Origen de respaldo inválido")),
    }
}

fn restore_local(
    live_db_path: &Path,
    data_dir: &Path,
    backup_id: &str,
    auth: &mut crate::auth::AuthState,
) -> AppResult<()> {
    let key = credentials::ensure_backup_key(data_dir)?;
    let conn = Connection::open(live_db_path)?;
    let (snapshot_path, enc_path, checksum): (String, Option<String>, String) = conn.query_row(
        "SELECT snapshot_path, enc_path, checksum FROM backup_queue WHERE backup_id=?1",
        [backup_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    drop(conn);

    let dir = backups_dir(data_dir);
    let staging = dir.join(format!("restore-{backup_id}.db"));
    let snap = PathBuf::from(&snapshot_path);
    if snap.is_file() {
        fs::copy(&snap, &staging)?;
    } else if let Some(enc) = enc_path {
        let enc_path = PathBuf::from(enc);
        let man_path = dir.join(format!("{backup_id}.manifest.json"));
        let man: Manifest = serde_json::from_str(&fs::read_to_string(man_path).map_err(|e| AppError::storage(e.to_string()))?)
            .map_err(|_| AppError::storage("Manifiesto inválido"))?;
        let nonce = man
            .nonce_hex
            .ok_or_else(|| AppError::storage("Manifiesto sin nonce de cifrado"))?;
        decrypt_file(&enc_path, &staging, &key, &nonce)?;
    } else {
        return Err(AppError::not_found("No se encontró el archivo del respaldo"));
    }
    apply_verified_snapshot(live_db_path, &staging, &checksum, auth)
}

fn restore_remote(
    live_db_path: &Path,
    data_dir: &Path,
    backup_id: &str,
    auth: &mut crate::auth::AuthState,
) -> AppResult<()> {
    if !credentials::device_configured(data_dir) {
        return Err(AppError::storage("Configurá el dispositivo de recepción para descargar copias remotas"));
    }
    if !credentials::backup_key_configured(data_dir) {
        return Err(AppError::storage("Importá la clave de cifrado del custodio antes de restaurar"));
    }
    let client = SupabaseClient::from_device(credentials::load_device(data_dir)?)?;
    let row = client.fetch_backup_manifest(backup_id)?;
    let storage_path = row
        .get("storage_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::storage("El manifiesto remoto no incluye la ruta del objeto"))?;
    let nonce = row
        .get("nonce_hex")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::storage("El manifiesto remoto no incluye nonce de cifrado"))?;
    let checksum = row
        .get("checksum")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::storage("El manifiesto remoto no incluye checksum"))?;
    let bytes = client.download_backup_object(storage_path)?;
    restore_from_encrypted(live_db_path, data_dir, backup_id, &bytes, nonce, checksum, auth)
}

/// Decrypt an encrypted artifact and swap it into the live DB. Used by remote restore and tests.
pub fn restore_from_encrypted(
    live_db_path: &Path,
    data_dir: &Path,
    backup_id: &str,
    enc_bytes: &[u8],
    nonce_hex: &str,
    checksum: &str,
    auth: &mut crate::auth::AuthState,
) -> AppResult<()> {
    if !credentials::backup_key_configured(data_dir) {
        return Err(AppError::storage("Importá la clave de cifrado del custodio antes de restaurar"));
    }
    let key = credentials::ensure_backup_key(data_dir)?;
    let dir = backups_dir(data_dir);
    fs::create_dir_all(&dir)?;
    let enc_staging = dir.join(format!("restore-{backup_id}.db.gz.enc"));
    let staging = dir.join(format!("restore-{backup_id}.db"));
    fs::write(&enc_staging, enc_bytes)?;
    let decrypt = decrypt_file(&enc_staging, &staging, &key, nonce_hex);
    let _ = fs::remove_file(&enc_staging);
    decrypt?;
    apply_verified_snapshot(live_db_path, &staging, checksum, auth)
}

fn apply_verified_snapshot(
    live_db_path: &Path,
    staging: &Path,
    checksum: &str,
    auth: &mut crate::auth::AuthState,
) -> AppResult<()> {
    verify_integrity(staging)?;
    let got = sha256_file(staging)?;
    if got != checksum {
        let _ = fs::remove_file(staging);
        return Err(AppError::storage("El checksum del respaldo no coincide"));
    }
    {
        let check = Connection::open(staging)?;
        let tip = latest_schema(&check)?;
        motel_compatible_schema(&check, &tip)?;
    }

    let pre = live_db_path.with_extension("pre-restore.bak");
    {
        let live = Connection::open(live_db_path)?;
        if pre.exists() {
            let _ = fs::remove_file(&pre);
        }
        live.backup(DatabaseName::Main, &pre, None)?;
    }

    {
        let mut live = Connection::open(live_db_path)?;
        live.restore(DatabaseName::Main, staging, None::<fn(_)>)?;
        live.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        clear_sync_state(&live)?;
    }
    auth.sessions.clear();
    let _ = fs::remove_file(staging);
    Ok(())
}

fn clear_sync_state(conn: &Connection) -> AppResult<()> {
    let _ = conn.execute("DELETE FROM sync_outbox", []);
    let _ = conn.execute("DELETE FROM sync_state", []);
    Ok(())
}

pub fn motel_compatible_schema(conn: &Connection, expected: &str) -> AppResult<()> {
    let current = latest_schema(conn)?;
    // Allow restore onto equal or newer migration set (same latest id).
    if current != expected {
        // Soft check: both must be known applied lists ending with compatible tip.
        let applied = db::list_applied_migrations(conn)?;
        if !applied.iter().any(|m| m == expected) && current != expected {
            return Err(AppError::conflict(format!(
                "Esquema incompatible (copia {expected}, local {current})"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nightdesk-backup-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn snapshot_integrity_and_checksum() {
        let dir = temp_dir();
        let db_path = dir.join("live.db");
        let conn = db::open(&db_path).unwrap();
        let snap = dir.join("snap.db");
        let man = create_snapshot(&conn, &snap, "test-id").unwrap();
        assert!(snap.is_file());
        assert_eq!(man.checksum, sha256_file(&snap).unwrap());
        verify_integrity(&snap).unwrap();
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let dir = temp_dir();
        let src = dir.join("plain.db");
        fs::write(&src, b"hello-backup-bytes").unwrap();
        let key = [7u8; 32];
        let enc = dir.join("x.db.gz.enc");
        let nonce = encrypt_file(&src, &enc, &key).unwrap();
        let out = dir.join("out.db");
        decrypt_file(&enc, &out, &key, &hex::encode(nonce)).unwrap();
        assert_eq!(fs::read(&src).unwrap(), fs::read(&out).unwrap());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn run_and_restore_roundtrip() {
        let dir = temp_dir();
        let db_path = dir.join("nightdesk.db");
        {
            let conn = db::open(&db_path).unwrap();
            conn.execute(
                "UPDATE rooms SET notes='before' WHERE id=1",
                [],
            )
            .ok();
        }
        let result = run_backup_now(&db_path, &dir).unwrap();
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute("UPDATE rooms SET notes='mutated' WHERE id=1", [])
                .unwrap();
            let notes: Option<String> = conn
                .query_row("SELECT notes FROM rooms WHERE id=1", [], |r| r.get(0))
                .ok();
            assert_eq!(notes.as_deref(), Some("mutated"));
        }
        let mut auth = crate::auth::AuthState::default();
        restore_backup(&db_path, &dir, &result.backup_id, "local", &mut auth).unwrap();
        let conn = Connection::open(&db_path).unwrap();
        let notes: Option<String> = conn
            .query_row("SELECT notes FROM rooms WHERE id=1", [], |r| r.get(0))
            .ok();
        assert_eq!(notes.as_deref(), Some("before"));
        let outbox: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_outbox", [], |r| r.get(0))
            .unwrap_or(0);
        assert_eq!(outbox, 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn status_ready_after_key() {
        let dir = temp_dir();
        let db_path = dir.join("nightdesk.db");
        let conn = db::open(&db_path).unwrap();
        let st = status(&conn, &dir).unwrap();
        assert!(st.ready);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn restore_encrypted_artifact_on_fresh_db() {
        let source = temp_dir();
        let source_db = source.join("nightdesk.db");
        {
            let conn = db::open(&source_db).unwrap();
            conn.execute("UPDATE rooms SET notes='from-source' WHERE id=1", [])
                .unwrap();
        }
        let result = run_backup_now(&source_db, &source).unwrap();
        let key = credentials::ensure_backup_key(&source).unwrap();
        let enc = fs::read(source.join("backups").join(format!("{}.db.gz.enc", result.backup_id))).unwrap();
        let man: Manifest = serde_json::from_str(
            &fs::read_to_string(source.join("backups").join(format!("{}.manifest.json", result.backup_id))).unwrap(),
        )
        .unwrap();

        let dest = temp_dir();
        let dest_db = dest.join("nightdesk.db");
        {
            let conn = db::open(&dest_db).unwrap();
            conn.execute("UPDATE rooms SET notes='other-machine' WHERE id=1", [])
                .unwrap();
            conn.execute(
                "INSERT INTO sync_state(key, value) VALUES('bootstrap_done','1')",
                [],
            )
            .ok();
        }
        credentials::import_backup_key(&dest, &hex::encode(key)).unwrap();
        let mut auth = crate::auth::AuthState::default();
        restore_from_encrypted(
            &dest_db,
            &dest,
            &result.backup_id,
            &enc,
            man.nonce_hex.as_deref().unwrap(),
            &man.checksum,
            &mut auth,
        )
        .unwrap();
        let conn = Connection::open(&dest_db).unwrap();
        let notes: Option<String> = conn
            .query_row("SELECT notes FROM rooms WHERE id=1", [], |r| r.get(0))
            .ok();
        assert_eq!(notes.as_deref(), Some("from-source"));
        let outbox: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_outbox", [], |r| r.get(0))
            .unwrap_or(-1);
        let state: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_state", [], |r| r.get(0))
            .unwrap_or(-1);
        assert_eq!(outbox, 0);
        assert_eq!(state, 0);
        let _ = fs::remove_dir_all(source);
        let _ = fs::remove_dir_all(dest);
    }

    #[test]
    fn restore_encrypted_wrong_key_keeps_live_db() {
        let source = temp_dir();
        let source_db = source.join("nightdesk.db");
        let conn = db::open(&source_db).unwrap();
        drop(conn);
        let result = run_backup_now(&source_db, &source).unwrap();
        let enc = fs::read(source.join("backups").join(format!("{}.db.gz.enc", result.backup_id))).unwrap();
        let man: Manifest = serde_json::from_str(
            &fs::read_to_string(source.join("backups").join(format!("{}.manifest.json", result.backup_id))).unwrap(),
        )
        .unwrap();

        let dest = temp_dir();
        let dest_db = dest.join("nightdesk.db");
        {
            let conn = db::open(&dest_db).unwrap();
            conn.execute("UPDATE rooms SET notes='keep-me' WHERE id=1", [])
                .unwrap();
        }
        credentials::import_backup_key(&dest, &hex::encode([9u8; 32])).unwrap();
        let mut auth = crate::auth::AuthState::default();
        let err = restore_from_encrypted(
            &dest_db,
            &dest,
            &result.backup_id,
            &enc,
            man.nonce_hex.as_deref().unwrap(),
            &man.checksum,
            &mut auth,
        )
        .unwrap_err();
        assert!(err.to_string().contains("descifrar") || err.to_string().contains("checksum"));
        let conn = Connection::open(&dest_db).unwrap();
        let notes: Option<String> = conn
            .query_row("SELECT notes FROM rooms WHERE id=1", [], |r| r.get(0))
            .ok();
        assert_eq!(notes.as_deref(), Some("keep-me"));
        let _ = fs::remove_dir_all(source);
        let _ = fs::remove_dir_all(dest);
    }
}
