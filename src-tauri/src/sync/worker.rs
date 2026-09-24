use super::bootstrap;
use super::client::{SupabaseClient, SyncClient};
use super::push::{self, DrainResult};
use super::realtime;
use crate::error::{AppError, AppResult};
use crate::models::SyncStatus;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const POLL_EVERY: Duration = Duration::from_secs(300);
const PULL_DEBOUNCE: Duration = Duration::from_millis(500);
const PULL_NOW_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct SyncSnapshot {
    pub connected: bool,
    pub last_push_at: Option<String>,
    pub last_pull_at: Option<String>,
    pub last_error: Option<String>,
    pub realtime_connected: bool,
}

impl Default for SyncSnapshot {
    fn default() -> Self {
        Self {
            connected: false,
            last_push_at: None,
            last_pull_at: None,
            last_error: None,
            realtime_connected: false,
        }
    }
}

pub enum Wake {
    Push,
    Pull,
    Full,
    PullNow(mpsc::Sender<AppResult<()>>),
}

#[derive(Clone)]
pub struct SyncHandle {
    tx: mpsc::Sender<Wake>,
    snapshot: Arc<Mutex<SyncSnapshot>>,
}

impl SyncHandle {
    pub fn snapshot(&self) -> SyncSnapshot {
        self.snapshot.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn wake_push(&self) {
        let _ = self.tx.send(Wake::Push);
    }

    pub fn pull_now(&self) -> AppResult<()> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(Wake::PullNow(tx))
            .map_err(|_| AppError::storage("Sincronización no disponible"))?;
        rx.recv_timeout(PULL_NOW_TIMEOUT)
            .map_err(|_| AppError::storage("La sincronización no respondió a tiempo"))?
    }
}

pub fn should_start(device_mode: Option<&str>, configured: bool) -> bool {
    device_mode == Some("reception") && configured
}

pub fn start(app: AppHandle, db_path: PathBuf, data_dir: PathBuf) -> AppResult<SyncHandle> {
    let creds = crate::credentials::load_device(&data_dir)?;
    let client = Arc::new(SupabaseClient::from_device(creds)?);
    let (tx, rx) = mpsc::channel();
    let snapshot = Arc::new(Mutex::new(SyncSnapshot::default()));
    realtime::spawn(client.clone(), tx.clone(), snapshot.clone());
    let handle = SyncHandle {
        tx: tx.clone(),
        snapshot: snapshot.clone(),
    };
    let worker_app = app.clone();
    std::thread::Builder::new()
        .name("nightdesk-sync".into())
        .spawn(move || {
            if let Err(error) = run_worker(worker_app, db_path, client, rx, snapshot) {
                eprintln!("sync worker: {error}");
            }
        })
        .map_err(|_| AppError::storage("No se pudo iniciar la sincronización"))?;
    Ok(handle)
}

fn run_worker(
    app: AppHandle,
    db_path: PathBuf,
    client: Arc<SupabaseClient>,
    rx: mpsc::Receiver<Wake>,
    snapshot: Arc<Mutex<SyncSnapshot>>,
) -> AppResult<()> {
    let mut conn = open_worker_conn(&db_path)?;
    let device_id = loop {
        match client.device_id() {
            Ok(id) => break id,
            Err(error) => {
                remember_error(&snapshot, &error);
                eprintln!("sync worker: {error}");
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    };
    let _ = run_cycle(&mut conn, client.as_ref(), &device_id, true, true, &app, &snapshot);
    let mut next_poll = Instant::now() + POLL_EVERY;
    let mut next_push = Instant::now() + Duration::from_secs(60);
    let mut last_pull = Instant::now() - Duration::from_secs(10);
    loop {
        let now = Instant::now();
        let wait = next_deadline(now, next_poll, next_push);
        match rx.recv_timeout(wait) {
            Ok(Wake::Push) => {
                next_push = after_drain(
                    drain_cycle(&mut conn, client.as_ref(), &device_id, &app, &snapshot),
                    &conn,
                );
            }
            Ok(Wake::Pull) => {
                if last_pull.elapsed() >= PULL_DEBOUNCE {
                    last_pull = Instant::now();
                    pull_cycle(&mut conn, client.as_ref(), &app, &snapshot);
                    next_poll = Instant::now() + POLL_EVERY;
                }
            }
            Ok(Wake::Full) => {
                next_push = after_drain(
                    drain_cycle(&mut conn, client.as_ref(), &device_id, &app, &snapshot),
                    &conn,
                );
                last_pull = Instant::now();
                pull_cycle(&mut conn, client.as_ref(), &app, &snapshot);
                next_poll = Instant::now() + POLL_EVERY;
            }
            Ok(Wake::PullNow(reply)) => {
                let result = run_cycle(&mut conn, client.as_ref(), &device_id, true, true, &app, &snapshot);
                last_pull = Instant::now();
                next_poll = Instant::now() + POLL_EVERY;
                next_push = after_drain(result.as_ref().ok().copied().unwrap_or(DrainResult::Retry), &conn);
                let _ = reply.send(result.map(|_| ()));
            }
            Err(RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                if now >= next_poll {
                    pull_cycle(&mut conn, client.as_ref(), &app, &snapshot);
                    next_poll = Instant::now() + POLL_EVERY;
                    last_pull = Instant::now();
                }
                if now >= next_push {
                    next_push = after_drain(
                        drain_cycle(&mut conn, client.as_ref(), &device_id, &app, &snapshot),
                        &conn,
                    );
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

fn next_deadline(now: Instant, poll: Instant, push: Instant) -> Duration {
    let target = if poll < push { poll } else { push };
    target.saturating_duration_since(now)
}

fn after_drain(result: DrainResult, conn: &Connection) -> Instant {
    match result {
        DrainResult::Done => Instant::now() + Duration::from_secs(3600),
        DrainResult::Retry => {
            let attempt = push::pending_attempts(conn).ok().flatten().unwrap_or(1).max(1);
            Instant::now() + push::backoff(attempt)
        }
    }
}

fn run_cycle(
    conn: &mut Connection,
    client: &SupabaseClient,
    device_id: &str,
    do_drain: bool,
    do_pull: bool,
    app: &AppHandle,
    snapshot: &Arc<Mutex<SyncSnapshot>>,
) -> AppResult<DrainResult> {
    if let Err(error) = bootstrap::run(conn, client, device_id) {
        remember_error(snapshot, &error);
        return Err(error);
    }
    let mut drain_result = DrainResult::Done;
    if do_drain {
        drain_result = drain_cycle(conn, client, device_id, app, snapshot);
    }
    if do_pull {
        pull_cycle(conn, client, app, snapshot);
    }
    Ok(drain_result)
}

fn drain_cycle(
    conn: &mut Connection,
    client: &SupabaseClient,
    device_id: &str,
    _app: &AppHandle,
    snapshot: &Arc<Mutex<SyncSnapshot>>,
) -> DrainResult {
    match push::drain(conn, client, device_id) {
        Ok(result) => {
            remember_ok(snapshot, true, false);
            result
        }
        Err(error) => {
            remember_error(snapshot, &error);
            DrainResult::Retry
        }
    }
}

fn pull_cycle(conn: &mut Connection, client: &SupabaseClient, app: &AppHandle, snapshot: &Arc<Mutex<SyncSnapshot>>) {
    match super::pull::pull_all(conn, client) {
        Ok(changed) => {
            remember_ok(snapshot, false, true);
            if changed {
                let _ = app.emit("sync:catalog-updated", ());
            }
        }
        Err(error) => remember_error(snapshot, &error),
    }
}

fn remember_ok(snapshot: &Arc<Mutex<SyncSnapshot>>, push: bool, pull: bool) {
    if let Ok(mut guard) = snapshot.lock() {
        guard.connected = true;
        guard.last_error = None;
        if push {
            guard.last_push_at = Some(crate::db::now_rfc3339());
        }
        if pull {
            guard.last_pull_at = Some(crate::db::now_rfc3339());
        }
    }
}

fn remember_error(snapshot: &Arc<Mutex<SyncSnapshot>>, error: &AppError) {
    if let Ok(mut guard) = snapshot.lock() {
        guard.connected = false;
        guard.last_error = Some(error.to_string());
    }
}

pub fn open_worker_conn(db_path: &Path) -> AppResult<Connection> {
    let conn = crate::db::open(db_path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(conn)
}

pub fn status_from(snapshot: &SyncSnapshot, pending_outbox: i64, configured: bool, embedded: bool) -> SyncStatus {
    SyncStatus {
        connected: snapshot.connected,
        pending_outbox,
        last_push_at: snapshot.last_push_at.clone(),
        last_pull_at: snapshot.last_pull_at.clone(),
        last_error: snapshot.last_error.clone(),
        configured,
        realtime_connected: snapshot.realtime_connected,
        embedded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_only_for_configured_reception() {
        assert!(should_start(Some("reception"), true));
        assert!(!should_start(Some("reception"), false));
        assert!(!should_start(Some("remote"), true));
        assert!(!should_start(None, true));
    }
}
