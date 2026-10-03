use super::*;
use crate::{device, operations};
use futures_util::{SinkExt, StreamExt};
use rusqlite::{params, OptionalExtension};
use std::{sync::Arc, time::Duration};
use tokio_tungstenite::{
    tungstenite::{client::IntoClientRequest, Message},
    Connector,
};

pub fn http(host: &HostIdentity) -> AppResult<reqwest::Client> {
    validate_identity(host)?;
    let cert = reqwest::Certificate::from_pem(host.certificate.as_bytes())
        .map_err(|_| AppError::msg("Certificado inválido"))?;
    reqwest::Client::builder()
        .tls_certs_only([cert])
        .no_proxy()
        .resolve(TLS_NAME, (local_ip(&host.address)?, host.port).into())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AppError::storage("No se pudo preparar TLS"))
}
pub fn secret(app: &AppHandle) -> AppResult<String> {
    let state = app.state::<AppState>();
    let raw = crate::credentials::read_lan_secret(&state.data_dir, "lan-client")?
        .ok_or_else(|| AppError::forbidden("Vinculá este puesto con la recepción principal"))?;
    String::from_utf8(raw).map_err(|_| AppError::storage("Credencial local inválida"))
}
async fn unpack(response: reqwest::Response) -> AppResult<Value> {
    let body: Value = response
        .json()
        .await
        .map_err(|_| AppError::storage("La principal devolvió una respuesta inválida"))?;
    if body["ok"] == true {
        return Ok(body["value"].clone());
    }
    let code = serde_json::from_value(body["error"]["code"].clone())
        .unwrap_or(crate::error::ErrorCode::Storage);
    Err(AppError::new(
        code,
        body["error"]["message"]
            .as_str()
            .unwrap_or("La recepción principal rechazó la solicitud"),
    ))
}
fn network_error() -> AppError {
    AppError::new(
        crate::error::ErrorCode::HostUnavailable,
        "No hay conexión con recepción principal. Las operaciones están detenidas hasta reconectar",
    )
}
pub async fn send(app: &AppHandle, request: &Request) -> AppResult<Value> {
    let host = app
        .state::<AppState>()
        .device
        .lock()
        .unwrap()
        .host
        .clone()
        .ok_or_else(|| AppError::forbidden("Vinculá este puesto"))?;
    let response = http(&host)?
        .post(format!("https://{TLS_NAME}:{}/lan/v1/call", host.port))
        .header("x-nightdesk-station", secret(app)?)
        .json(request)
        .send()
        .await;
    let state = app.state::<AppState>();
    match response {
        Ok(reply) => {
            {
                let mut status = state.lan.connection.lock().unwrap();
                status.connected = true;
                status.last_seen_at = Some(crate::db::now_rfc3339());
                status.last_error = None;
            }
            unpack(reply).await
        }
        Err(_) => {
            let mut status = state.lan.connection.lock().unwrap();
            status.connected = false;
            status.last_error = Some(network_error().to_string());
            Err(network_error())
        }
    }
}
pub fn request(command: &str, args: Value, token: Option<String>) -> Request {
    Request {
        protocol_version: PROTOCOL,
        contract_version: crate::service::CONTRACT_VERSION,
        command: command.into(),
        args,
        session_token: token,
    }
}
pub async fn call(
    app: &AppHandle,
    command: &str,
    mut args: Value,
    token: Option<String>,
) -> AppResult<Value> {
    let mutation = operations::MUTATIONS.contains(&command);
    let state = app.state::<AppState>();
    if mutation {
        if !state.lan.connection.lock().unwrap().connected {
            return Err(network_error());
        }
        if args.get("_database_generation").is_none() {
            args["_database_generation"] = json!(state
                .lan
                .client_generation
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| AppError::session_expired("Volvé a ingresar"))?);
        }
        if args.get("_actor_uid").is_none() {
            args["_actor_uid"] = json!(state
                .lan
                .client_actor
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| AppError::session_expired("Volvé a ingresar"))?);
        }
        let id = args["operation_id"]
            .as_str()
            .ok_or_else(|| AppError::msg("Falta el identificador de operación"))?;
        let username = state
            .lan
            .client_session
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.1.clone())
            .ok_or_else(|| AppError::session_expired("Iniciá sesión"))?;
        let host = state
            .device
            .lock()
            .unwrap()
            .host
            .clone()
            .ok_or_else(|| AppError::forbidden("Vinculá el puesto"))?;
        let mut journal = device::journal(&state.data_dir)?;
        let conn = journal.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let pending: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pending_requests WHERE host_id=?1 AND operation_id<>?2",
            params![host.station_id, id],
            |r| r.get(0),
        )?;
        if pending > 0 {
            return Err(AppError::new(
                crate::error::ErrorCode::OperationPending,
                "Hay una operación sin confirmar. Resolvela antes de continuar",
            ));
        }
        let prior: Option<String> = conn
            .query_row(
                "SELECT args_json FROM pending_requests WHERE operation_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if prior.as_ref().is_some_and(|raw| raw != &args.to_string()) {
            return Err(AppError::conflict(
                "La solicitud pendiente tiene otros datos",
            ));
        }
        conn.execute(
            "INSERT OR IGNORE INTO pending_requests VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                id,
                host.station_id,
                username,
                command,
                args.to_string(),
                crate::db::now_rfc3339()
            ],
        )?;
        conn.commit()?;
    }
    let request = request(command, args.clone(), token);
    let mut result = send(app, &request).await;
    if command == "auth_login"
        && matches!(&result,Err(e) if e.code()==crate::error::ErrorCode::HostUnavailable)
    {
        rediscover(app).await;
        result = send(app, &request).await;
    }
    if mutation {
        match &result {
            Ok(_) => {
                device::journal(&state.data_dir)?.execute(
                    "DELETE FROM pending_requests WHERE operation_id=?1",
                    [args["operation_id"].as_str().unwrap()],
                )?;
            }
            Err(e)
                if !matches!(
                    e.code(),
                    crate::error::ErrorCode::HostUnavailable
                        | crate::error::ErrorCode::Storage
                        | crate::error::ErrorCode::SessionExpired
                        | crate::error::ErrorCode::RecoveryRequired
                ) =>
            {
                device::journal(&state.data_dir)?.execute(
                    "DELETE FROM pending_requests WHERE operation_id=?1",
                    [args["operation_id"].as_str().unwrap()],
                )?;
            }
            _ => {}
        }
    }
    if let Ok(value) = &result {
        if command == "auth_login" {
            *state.lan.client_actor.lock().unwrap() =
                value["actor_uid"].as_str().map(str::to_owned);
            *state.lan.client_generation.lock().unwrap() =
                value["database_generation"].as_str().map(str::to_owned);
            *state.lan.client_session.lock().unwrap() = Some((
                value["token"].as_str().unwrap_or("").into(),
                value["user"]["username"].as_str().unwrap_or("").into(),
            ));
            start_events(app.clone());
        } else if command == "auth_logout" {
            *state.lan.client_session.lock().unwrap() = None;
        }
    }
    result
}
pub async fn pair(app: &AppHandle, host: HostIdentity, name: String) -> AppResult<Value> {
    validate_identity(&host)?;
    let state = app.state::<AppState>();
    let key = hex::encode(rand::random::<[u8; 32]>());
    let mut config = state.device.lock().unwrap().clone();
    if config.mode.as_deref() != Some("reception_client") {
        return Err(AppError::forbidden(
            "Este equipo no es una recepción adicional",
        ));
    }
    if name.trim().is_empty() || name.len() > 64 {
        return Err(AppError::msg("Ingresá un nombre de puesto"));
    }
    config.name = name;
    config.host = Some(host.clone());
    config.paired = false;
    crate::credentials::write_lan_secret(&state.data_dir, "lan-client", key.as_bytes())?;
    device::save(&state.data_dir, &config)?;
    *state.device.lock().unwrap() = config.clone();
    let response = http(&host)?
        .post(format!("https://{TLS_NAME}:{}/lan/v1/pair", host.port))
        .header("x-nightdesk-station", &key)
        .json(&json!({"station_id":config.station_id,"name":config.name}))
        .send()
        .await
        .map_err(|_| network_error())?;
    unpack(response).await?;
    Ok(json!({"pending":true,"verification_code":&crate::auth::key(&key)[..12]}))
}
pub async fn pair_status(app: &AppHandle) -> AppResult<Value> {
    let state = app.state::<AppState>();
    let mut config = state.device.lock().unwrap().clone();
    let host = config
        .host
        .as_ref()
        .ok_or_else(|| AppError::msg("Seleccioná recepción principal"))?;
    let response = http(host)?
        .post(format!(
            "https://{TLS_NAME}:{}/lan/v1/pair/status",
            host.port
        ))
        .header("x-nightdesk-station", secret(app)?)
        .send()
        .await
        .map_err(|_| network_error())?;
    let value = unpack(response).await?;
    config.paired = true;
    device::save(&state.data_dir, &config)?;
    *state.device.lock().unwrap() = config;
    Ok(value)
}
pub async fn pending(
    app: &AppHandle,
    token: Option<String>,
    retry: Option<String>,
) -> AppResult<Value> {
    let state = app.state::<AppState>();
    let username = state
        .lan
        .client_session
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.1.clone())
        .ok_or_else(|| AppError::session_expired("Iniciá sesión"))?;
    let host = state
        .device
        .lock()
        .unwrap()
        .host
        .clone()
        .ok_or_else(|| AppError::forbidden("Vinculá el puesto"))?;
    let rows = {
        let conn = device::journal(&state.data_dir)?;
        let mut stmt=conn.prepare("SELECT operation_id,command,args_json,created_at,username FROM pending_requests WHERE host_id=?1 ORDER BY created_at")?;
        let rows = stmt
            .query_map([&host.station_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let mut unresolved = vec![];
    for (id, command, raw, created, owner) in rows {
        let args: Value = serde_json::from_str(&raw)
            .map_err(|_| AppError::storage("Solicitud pendiente inválida"))?;
        let same_actor =
            state.lan.client_actor.lock().unwrap().as_deref() == args["_actor_uid"].as_str();
        let same_generation = state.lan.client_generation.lock().unwrap().as_deref()
            == args["_database_generation"].as_str();
        if owner != username || !same_actor || !same_generation {
            unresolved.push(json!({"operation_id":id,"command":command,"created_at":created,"username":owner,"can_retry":false}));
            continue;
        }
        let found = send(
            app,
            &request(
                "operation_result",
                json!({"operation_id":id,"_database_generation":args["_database_generation"]}),
                token.clone(),
            ),
        )
        .await?;
        if !found.is_null() {
            device::journal(&state.data_dir)?
                .execute("DELETE FROM pending_requests WHERE operation_id=?1", [&id])?;
            let _ = app.emit("reception:changed", json!({}));
            continue;
        }
        if retry.as_deref() == Some(&id) {
            call(app, &command, args, token.clone()).await?;
            let _ = app.emit("reception:changed", json!({}));
        } else {
            unresolved.push(json!({"operation_id":id,"command":command,"created_at":created,"username":owner,"can_retry":true}));
        }
    }
    Ok(json!(unresolved))
}
/// Explicit administrator review preserves the abandoned request for support; it never replays it.
pub async fn review_pending(
    app: &AppHandle,
    token: Option<String>,
    id: String,
    note: String,
) -> AppResult<Value> {
    if note.trim().len() < 12 {
        return Err(AppError::msg(
            "Explicá qué verificaste en el historial y cómo resolviste la operación",
        ));
    }
    let session = send(app, &request("auth_session", json!({}), token)).await?;
    if session["user"]["role"] != "admin" {
        return Err(AppError::forbidden(
            "Un administrador debe revisar esta operación",
        ));
    }
    let state = app.state::<AppState>();
    let mut conn = device::journal(&state.data_dir)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS reviewed_requests(operation_id TEXT PRIMARY KEY,request_json TEXT NOT NULL,note TEXT NOT NULL,reviewed_by TEXT NOT NULL,reviewed_at TEXT NOT NULL)")?;
    let raw: String = tx.query_row(
        "SELECT args_json FROM pending_requests WHERE operation_id=?1",
        [&id],
        |r| r.get(0),
    )?;
    tx.execute(
        "INSERT INTO reviewed_requests VALUES (?1,?2,?3,?4,?5)",
        params![
            id,
            raw,
            note,
            session["user"]["username"].as_str().unwrap_or("admin"),
            crate::db::now_rfc3339()
        ],
    )?;
    tx.execute("DELETE FROM pending_requests WHERE operation_id=?1", [&id])?;
    tx.commit()?;
    Ok(Value::Null)
}

fn start_events(app: AppHandle) {
    if app
        .state::<AppState>()
        .lan
        .events_started
        .swap(true, Ordering::SeqCst)
    {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            if app.state::<AppState>().lan.stopping.load(Ordering::Relaxed) {
                break;
            }
            if app
                .state::<AppState>()
                .lan
                .client_session
                .lock()
                .unwrap()
                .is_some()
            {
                let _ = event_connection(&app).await;
            }
            {
                let state = app.state::<AppState>();
                state.lan.connection.lock().unwrap().events_connected = false;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            rediscover(&app).await;
        }
    });
}
async fn rediscover(app: &AppHandle) {
    let found = tauri::async_runtime::spawn_blocking(super::discovery::discover).await;
    if let Ok(Ok(hosts)) = found {
        let state = app.state::<AppState>();
        let mut config = state.device.lock().unwrap();
        if let Some(saved) = config.host.as_mut() {
            if let Some(current) = hosts
                .into_iter()
                .find(|h| h.station_id == saved.station_id && h.fingerprint == saved.fingerprint)
            {
                if current.address != saved.address || current.port != saved.port {
                    saved.address = current.address;
                    saved.port = current.port;
                    let _ = device::save(&state.data_dir, &config);
                }
            }
        }
    }
}
async fn event_connection(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let host = state
        .device
        .lock()
        .unwrap()
        .host
        .clone()
        .ok_or_else(|| AppError::msg("Falta recepción"))?;
    let token = state
        .lan
        .client_session
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.0.clone())
        .ok_or_else(|| AppError::session_expired("Iniciá sesión"))?;
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_pemfile::certs(&mut host.certificate.as_bytes()) {
        roots
            .add(cert.map_err(|_| AppError::msg("Certificado inválido"))?)
            .map_err(|_| AppError::msg("Certificado inválido"))?;
    }
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut request = format!("wss://{TLS_NAME}:{}/lan/v1/events", host.port)
        .into_client_request()
        .map_err(|_| AppError::msg("Dirección inválida"))?;
    request.headers_mut().insert(
        "x-nightdesk-station",
        secret(app)?
            .parse()
            .map_err(|_| AppError::msg("Credencial inválida"))?,
    );
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {token}")
            .parse()
            .map_err(|_| AppError::msg("Sesión inválida"))?,
    );
    let stream = tokio::time::timeout(
        Duration::from_secs(4),
        tokio::net::TcpStream::connect((host.address.as_str(), host.port)),
    )
    .await
    .map_err(|_| network_error())?
    .map_err(|_| network_error())?;
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::client_async_tls_with_config(
            request,
            stream,
            None,
            Some(Connector::Rustls(Arc::new(tls))),
        ),
    )
    .await
    .map_err(|_| network_error())?
    .map_err(|_| network_error())?;
    state.lan.connection.lock().unwrap().events_connected = true;
    let mut last = String::new();
    loop {
        if state
            .lan
            .client_session
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.0.as_str())
            != Some(token.as_str())
        {
            break;
        }
        let next = tokio::time::timeout(Duration::from_secs(12), socket.next())
            .await
            .map_err(|_| network_error())?;
        match next {
            Some(Ok(Message::Text(text))) => {
                {
                    let mut status = state.lan.connection.lock().unwrap();
                    status.connected = true;
                    status.last_seen_at = Some(crate::db::now_rfc3339());
                    status.last_error = None;
                }
                if text.as_str() != last {
                    last = text.to_string();
                    if let Ok(event) = serde_json::from_str::<Change>(&text) {
                        let _ = app.emit("reception:changed", event);
                    }
                }
            }
            Some(Ok(Message::Ping(p))) => {
                socket
                    .send(Message::Pong(p))
                    .await
                    .map_err(|_| network_error())?;
            }
            Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
            _ => {}
        }
    }
    Ok(())
}
