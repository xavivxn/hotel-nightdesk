//! LAN v1. Only the principal owns business data; stations send commands.
pub mod client;
mod discovery;
mod server;
pub use server::start;
pub mod ipc;

use crate::{
    device::HostIdentity,
    error::{AppError, AppResult},
    AppState,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tauri::{AppHandle, Emitter, Manager};

pub const PROTOCOL: u32 = 1;
pub const TLS_NAME: &str = "nightdesk.local";
pub const SERVICE_TYPE: &str = "_nightdesk._tcp.local.";
pub const CLIENT_SERVICE_TYPE: &str = "_nightdesk-peer._udp.local.";

#[derive(Clone, Serialize, Deserialize)]
pub struct Request {
    pub protocol_version: u32,
    pub contract_version: u32,
    pub command: String,
    #[serde(default)]
    pub args: Value,
    pub session_token: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Change {
    pub epoch: String,
    pub revision: i64,
}
pub struct PendingPair {
    pub station_id: String,
    pub name: String,
    pub token_hash: String,
    pub expires: Instant,
}
pub struct HostRuntime {
    pub id: String,
    pub handle: axum_server::Handle<std::net::SocketAddr>,
    pub mdns: Option<mdns_sd::ServiceDaemon>,
    pub finished: Arc<tokio::sync::Notify>,
}
impl HostRuntime {
    pub async fn shutdown(self) -> AppResult<()> {
        self.handle
            .graceful_shutdown(Some(std::time::Duration::from_secs(2)));
        if let Some(mdns) = self.mdns {
            let _ = mdns.shutdown();
        }
        tokio::time::timeout(std::time::Duration::from_secs(3), self.finished.notified())
            .await
            .map_err(|_| {
                AppError::storage("La conexión anterior todavía se está cerrando. Volvé a intentar")
            })
    }
}
#[derive(Clone, Serialize, Default)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub last_seen_at: Option<String>,
    pub last_error: Option<String>,
    pub events_connected: bool,
}
pub struct Runtime {
    pub host: Mutex<Option<HostRuntime>>,
    pub client_advertisement: Mutex<Option<(String, mdns_sd::ServiceDaemon)>>,
    pub start_lock: tokio::sync::Mutex<()>,
    pub epoch: Mutex<String>,
    pub changes: tokio::sync::broadcast::Sender<Change>,
    pub pairing_until: Mutex<Option<Instant>>,
    pub pending: Mutex<HashMap<String, PendingPair>>,
    pub session_stations: Mutex<HashMap<String, String>>,
    pub client_actor: Mutex<Option<String>>,
    pub client_generation: Mutex<Option<String>>,
    pub client_session: Mutex<Option<(String, String)>>,
    pub connection: Mutex<ConnectionStatus>,
    pub events_started: AtomicBool,
    pub stopping: AtomicBool,
}
impl Default for Runtime {
    fn default() -> Self {
        let (changes, _) = tokio::sync::broadcast::channel(64);
        Self {
            host: Mutex::new(None),
            client_advertisement: Mutex::new(None),
            start_lock: tokio::sync::Mutex::new(()),
            epoch: Mutex::new(uuid::Uuid::new_v4().to_string()),
            changes,
            pairing_until: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            session_stations: Mutex::new(HashMap::new()),
            client_actor: Mutex::new(None),
            client_generation: Mutex::new(None),
            client_session: Mutex::new(None),
            connection: Mutex::new(ConnectionStatus::default()),
            events_started: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
        }
    }
}
pub fn changed(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(conn) = state.db.lock() else { return };
    let Ok(revision) = crate::operations::revision(&conn) else {
        return;
    };
    let event = Change {
        epoch: state.lan.epoch.lock().unwrap().clone(),
        revision,
    };
    drop(conn);
    let _ = state.lan.changes.send(event.clone());
    let _ = app.emit("reception:changed", event);
}
pub fn local_ip(value: &str) -> AppResult<std::net::Ipv4Addr> {
    let ip: std::net::Ipv4Addr = value
        .parse()
        .map_err(|_| AppError::msg("Ingresá una dirección IPv4 de la red local"))?;
    if !(ip.is_private() || ip.is_link_local() || ip.is_loopback()) {
        return Err(AppError::msg(
            "La conexión solo admite direcciones de la red local",
        ));
    }
    Ok(ip)
}
pub fn status(app: &AppHandle) -> Value {
    let state = app.state::<AppState>();
    let config = state.device.lock().unwrap().clone();
    json!({"mode":config.mode,"station_id":config.station_id,"name":config.name,"enabled":config.lan_enabled,
        "running":state.lan.host.lock().unwrap().is_some(),"paired":config.paired,"host_name":config.host.as_ref().map(|h|&h.name),
        "connection":state.lan.connection.lock().unwrap().clone(),"print_target":config.print_target,
        "bind_address":config.bind_address,"port":config.port})
}
pub fn start_observer(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut previous = None;
        let mut ticks = 0u32;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let state = app.state::<AppState>();
            if state.lan.stopping.load(Ordering::Relaxed) {
                break;
            }
            let config = state.device.lock().unwrap().clone();
            if config.mode.as_deref() == Some("reception_client") {
                ticks += 1;
                if ticks % 10 == 0 {
                    let addresses = discovery::client_addresses();
                    let key = format!("{}:{}:{}", config.station_id, config.name, addresses.join(","));
                    let previous_key = state.lan.client_advertisement.lock().unwrap().as_ref().map(|(key, _)| key.clone());
                    if config.paired || addresses.is_empty() {
                        if let Some((_, daemon)) = state.lan.client_advertisement.lock().unwrap().take() {
                            let _ = daemon.shutdown();
                        }
                    } else if previous_key.as_deref() != Some(&key) {
                        if let Some((_, daemon)) = state.lan.client_advertisement.lock().unwrap().take() {
                            let _ = daemon.shutdown();
                        }
                        let station_id = config.station_id.clone();
                        let name = config.name.clone();
                        let result = tauri::async_runtime::spawn_blocking(move || discovery::advertise_client(&station_id, &name, &addresses)).await;
                        match result {
                            Ok(Ok(daemon)) => *state.lan.client_advertisement.lock().unwrap() = Some((key, daemon)),
                            Ok(Err(error)) => state.lan.connection.lock().unwrap().last_error = Some(error.to_string()),
                            Err(_) => {}
                        }
                    }
                }
                continue;
            }
            if config.mode.as_deref() != Some("reception") {
                continue;
            }
            let now = state
                .db
                .lock()
                .ok()
                .and_then(|conn| crate::operations::revision(&conn).ok());
            if now != previous {
                previous = now;
                changed(&app);
            }
            ticks += 1;
            if ticks % 10 == 0 {
                let config = state.device.lock().unwrap().clone();
                if config.lan_enabled && !config.bind_interface.is_empty() {
                    let interfaces = if_addrs::get_if_addrs().unwrap_or_default();
                    if !interfaces
                        .iter()
                        .any(|i| i.ip().to_string() == config.bind_address)
                    {
                        if let Some(interface)=interfaces.into_iter().find(|i|i.name==config.bind_interface && matches!(i.ip(),std::net::IpAddr::V4(ip) if !ip.is_loopback() && local_ip(&ip.to_string()).is_ok())) {
                            if let Err(error) = stop_and_wait(&app).await {
                                state.lan.connection.lock().unwrap().last_error = Some(error.to_string());
                                continue;
                            }
                            let mut next=config; next.bind_address=interface.ip().to_string();
                            if crate::device::save(&state.data_dir,&next).is_ok(){
                                *state.device.lock().unwrap()=next;
                                if let Err(error)=start(app.clone()).await{state.lan.connection.lock().unwrap().last_error=Some(error.to_string());}
                            }
                        }
                    } else if state.lan.host.lock().unwrap().is_none() {
                        if let Err(error) = start(app.clone()).await {
                            state.lan.connection.lock().unwrap().last_error =
                                Some(error.to_string());
                        }
                    }
                }
            }
        }
    });
}
pub fn stop(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(host) = state.lan.host.lock().unwrap().take() {
        host.handle
            .graceful_shutdown(Some(std::time::Duration::from_secs(2)));
        if let Some(mdns) = host.mdns {
            let _ = mdns.shutdown();
        }
    }
    *state.lan.pairing_until.lock().unwrap() = None;
    if let Some((_, daemon)) = state.lan.client_advertisement.lock().unwrap().take() {
        let _ = daemon.shutdown();
    };
}
pub async fn stop_and_wait(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let _lifecycle = state.lan.start_lock.lock().await;
    let host = state.lan.host.lock().unwrap().take();
    *state.lan.pairing_until.lock().unwrap() = None;
    if let Some(host) = host {
        host.shutdown().await?;
    }
    Ok(())
}
pub fn fingerprint(cert: &str) -> AppResult<String> {
    use sha2::{Digest, Sha256};
    let der = rustls_pemfile::certs(&mut cert.as_bytes())
        .next()
        .transpose()
        .map_err(|_| AppError::msg("Certificado inválido"))?
        .ok_or_else(|| AppError::msg("Falta el certificado"))?;
    Ok(hex::encode(Sha256::digest(der.as_ref())))
}
pub fn validate_identity(host: &HostIdentity) -> AppResult<()> {
    local_ip(&host.address)?;
    uuid::Uuid::parse_str(&host.station_id)
        .map_err(|_| AppError::msg("Identidad de recepción inválida"))?;
    if host.port < 1024
        || host.certificate.len() > 4096
        || fingerprint(&host.certificate)? != host.fingerprint
    {
        return Err(AppError::msg(
            "Los datos de conexión no coinciden con su huella de seguridad",
        ));
    }
    Ok(())
}
