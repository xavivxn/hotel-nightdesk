//! Per-PC configuration and recovery journal. Never an operational replica.
use crate::{
    db,
    error::{AppError, AppResult},
    models::AppSettings,
};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
pub struct HostIdentity {
    pub station_id: String,
    pub address: String,
    pub port: u16,
    pub certificate: String,
    pub fingerprint: String,
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceConfig {
    pub mode: Option<String>,
    pub station_id: String,
    pub database_generation: String,
    pub name: String,
    pub lan_enabled: bool,
    pub bind_address: String,
    pub bind_interface: String,
    pub port: u16,
    pub host: Option<HostIdentity>,
    pub paired: bool,
    pub printer: Option<AppSettings>,
    pub print_target: String,
}
impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            mode: None,
            station_id: uuid::Uuid::new_v4().to_string(),
            database_generation: uuid::Uuid::new_v4().to_string(),
            name: "Recepción".into(),
            lan_enabled: false,
            bind_address: "".into(),
            bind_interface: "".into(),
            port: 17443,
            host: None,
            paired: false,
            printer: None,
            print_target: "local".into(),
        }
    }
}
pub fn load(dir: &Path) -> AppResult<DeviceConfig> {
    let path = dir.join("device.json");
    if path.exists() {
        return serde_json::from_slice(&std::fs::read(path)?)
            .map_err(|_| AppError::storage("No se puede leer la configuración de este puesto"));
    }
    let mut config = DeviceConfig::default();
    let legacy = dir.join("nightdesk.db");
    if legacy.exists() {
        let conn = Connection::open_with_flags(legacy, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        config.mode = crate::service::device_mode_get(&conn)?;
        if config.mode.is_none() {
            let users: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
            if users > 0 {
                config.mode = Some("reception".into());
            }
        }
        let mut preferences = db::load_settings(&conn)?;
        preferences.pin_hash.clear();
        config.printer = Some(preferences);
    }
    save(dir, &config)?;
    Ok(config)
}
pub fn save(dir: &Path, config: &DeviceConfig) -> AppResult<()> {
    std::fs::create_dir_all(dir)?;
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|_| AppError::storage("No se pudo guardar el puesto"))?;
    let temp = dir.join(format!("device-{}.tmp", uuid::Uuid::new_v4()));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temp, dir.join("device.json"))?;
    Ok(())
}
pub fn operational_connection(dir: &Path, mode: Option<&str>) -> AppResult<Connection> {
    match mode {
        Some("reception" | "remote") => db::open(&dir.join("nightdesk.db")),
        _ => Ok(Connection::open_in_memory()?),
    }
}
pub fn journal(dir: &Path) -> AppResult<Connection> {
    std::fs::create_dir_all(dir)?;
    let conn = Connection::open(dir.join("station.db"))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS pending_requests(operation_id TEXT PRIMARY KEY, host_id TEXT NOT NULL, username TEXT NOT NULL, command TEXT NOT NULL, args_json TEXT NOT NULL, created_at TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS print_jobs(id TEXT PRIMARY KEY, request_hash TEXT NOT NULL, status TEXT NOT NULL, error TEXT, created_at TEXT NOT NULL);")?;
    Ok(conn)
}
pub fn local_settings(config: &DeviceConfig, mut business: AppSettings) -> AppSettings {
    if let Some(local) = &config.printer {
        business.printer_name = local.printer_name.clone();
        business.printer_path = local.printer_path.clone();
        business.printer_enabled = local.printer_enabled;
        business.paper_width = local.paper_width;
        business.auto_print_on_checkout = local.auto_print_on_checkout;
        business.theme = local.theme.clone();
    }
    business.pin_hash.clear();
    business
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additional_station_has_no_operational_database_or_seed() {
        let dir = std::env::temp_dir().join(format!("nightdesk-client-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = operational_connection(&dir, Some("reception_client")).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        assert!(!dir.join("nightdesk.db").exists());
        assert!(!crate::sync::worker::should_start(
            Some("reception_client"),
            true
        ));
        assert!(!crate::backup::should_start(Some("reception_client")));
        journal(&dir).unwrap();
        assert!(!dir.join("nightdesk.db").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
