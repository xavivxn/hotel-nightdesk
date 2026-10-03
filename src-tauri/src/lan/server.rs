use super::*;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        DefaultBodyLimit, State, WebSocketUpgrade,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use rusqlite::{params, OptionalExtension};
use std::{net::SocketAddr, time::Duration};

#[derive(Serialize, Deserialize)]
struct IdentitySecret {
    certificate: String,
    key: String,
}
pub fn identity(app: &AppHandle) -> AppResult<HostIdentity> {
    let state = app.state::<AppState>();
    let config = state.device.lock().unwrap().clone();
    let secret = identity_secret(&state.data_dir)?;
    Ok(HostIdentity {
        station_id: config.station_id,
        address: config.bind_address,
        port: config.port,
        fingerprint: fingerprint(&secret.certificate)?,
        certificate: secret.certificate,
        name: config.name,
    })
}
fn identity_secret(dir: &std::path::Path) -> AppResult<IdentitySecret> {
    if let Some(bytes) = crate::credentials::read_lan_secret(dir, "lan-identity")? {
        return serde_json::from_slice(&bytes)
            .map_err(|_| AppError::storage("No se pudo leer la identidad TLS"));
    }
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec![TLS_NAME.into()])
            .map_err(|_| AppError::storage("No se pudo crear la identidad TLS"))?;
    let secret = IdentitySecret {
        certificate: cert.pem(),
        key: signing_key.serialize_pem(),
    };
    crate::credentials::write_lan_secret(
        dir,
        "lan-identity",
        &serde_json::to_vec(&secret).map_err(|_| AppError::storage("Identidad inválida"))?,
    )?;
    Ok(secret)
}
pub async fn start(app: AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let _lifecycle = state.lan.start_lock.lock().await;
    let config = state.device.lock().unwrap().clone();
    if config.mode.as_deref() != Some("reception") || !config.lan_enabled {
        return Ok(());
    }
    if state.lan.host.lock().unwrap().is_some() {
        return Ok(());
    }
    let ip = local_ip(&config.bind_address)?;
    if config.port < 1024 {
        return Err(AppError::msg("El puerto debe ser mayor o igual a 1024"));
    }
    let secret = identity_secret(&state.data_dir)?;
    let tls = axum_server::tls_rustls::RustlsConfig::from_pem(
        secret.certificate.into_bytes(),
        secret.key.into_bytes(),
    )
    .await
    .map_err(|_| AppError::storage("No se pudo iniciar TLS"))?;
    {
        let current = state.device.lock().unwrap();
        if !current.lan_enabled
            || current.bind_address != config.bind_address
            || current.port != config.port
        {
            return Ok(());
        }
    }
    let listener =
        std::net::TcpListener::bind(SocketAddr::from((ip, config.port))).map_err(|_| {
            AppError::storage(
                "No se pudo abrir la conexión local. Revisá dirección, puerto y firewall",
            )
        })?;
    listener.set_nonblocking(true)?;
    let handle = axum_server::Handle::new();
    let (mdns, discovery_error) = match super::discovery::advertise(&identity(&app)?) {
        Ok(daemon) => (Some(daemon), None),
        Err(error) => (
            None,
            Some(format!(
                "Descubrimiento automático no disponible: {error}. Usá conexión manual"
            )),
        ),
    };
    let router = Router::new()
        .route("/lan/v1/call", post(call))
        .route("/lan/v1/pair", post(pair))
        .route("/lan/v1/pair/status", post(pair_status))
        .route("/lan/v1/events", get(events))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .with_state(app.clone());
    let server = axum_server::from_tcp_rustls(listener, tls)
        .map_err(|_| AppError::storage("No se pudo iniciar recepción"))?
        .handle(handle.clone());
    state.lan.connection.lock().unwrap().last_error = discovery_error;
    let instance = uuid::Uuid::new_v4().to_string();
    let finished = Arc::new(tokio::sync::Notify::new());
    *state.lan.host.lock().unwrap() = Some(HostRuntime {
        id: instance.clone(),
        handle,
        mdns,
        finished: finished.clone(),
    });
    let server_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let app = server_app;
        if server.serve(router.into_make_service()).await.is_err() {
            let state = app.state::<AppState>();
            state.lan.connection.lock().unwrap().last_error =
                Some("La conexión local se detuvo".into());
        }
        let state = app.state::<AppState>();
        let mut host = state.lan.host.lock().unwrap();
        if host.as_ref().is_some_and(|h| h.id == instance) {
            if let Some(old) = host.take() {
                if let Some(mdns) = old.mdns {
                    let _ = mdns.shutdown();
                }
            }
        }
        finished.notify_one();
    });
    Ok(())
}
fn response(result: AppResult<Value>) -> Response {
    match result {
        Ok(value) => (StatusCode::OK, Json(json!({"ok":true,"value":value}))).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok":false,"error":error})),
        )
            .into_response(),
    }
}
fn station(app: &AppHandle, headers: &HeaderMap) -> AppResult<String> {
    let key = headers
        .get("x-nightdesk-station")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::forbidden("Puesto no vinculado"))?;
    let state = app.state::<AppState>();
    if !state.device.lock().unwrap().lan_enabled {
        return Err(AppError::forbidden("Conexión local desactivada"));
    }
    let id = state
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT id FROM lan_stations WHERE token_hash=?1 AND active=1",
            [crate::auth::key(key)],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    id.ok_or_else(|| AppError::forbidden("El puesto ya no está autorizado. Volvé a vincularlo"))
}
fn check_session(app: &AppHandle, station: &str, token: Option<&str>) -> AppResult<()> {
    let token = token.ok_or_else(|| AppError::session_expired("Iniciá sesión"))?;
    let state = app.state::<AppState>();
    if state
        .lan
        .session_stations
        .lock()
        .unwrap()
        .get(&crate::auth::key(token))
        .map(String::as_str)
        != Some(station)
    {
        return Err(AppError::session_expired("Iniciá sesión desde este puesto"));
    }
    crate::auth::require(&state, Some(token), false)?;
    Ok(())
}
fn validate_protocol(request: &Request) -> AppResult<()> {
    if request.protocol_version != PROTOCOL
        || request.contract_version != crate::service::CONTRACT_VERSION
    {
        return Err(AppError::new(
            crate::error::ErrorCode::IncompatibleVersion,
            "Actualizá ambos puestos a una versión compatible",
        ));
    }
    Ok(())
}
async fn call(
    State(app): State<AppHandle>,
    headers: HeaderMap,
    Json(request): Json<Request>,
) -> Response {
    let result = async {
        let station = station(&app, &headers)?;
        validate_protocol(&request)?;
        if !matches!(
            request.command.as_str(),
            "auth_login" | "auth_setup_required"
        ) {
            check_session(&app, &station, request.session_token.as_deref())?;
        }
        let result = crate::backend::dispatch(
            &app,
            &station,
            &request.command,
            request.args,
            request.session_token.clone(),
        )
        .await?;
        if request.command == "auth_login" {
            let token = result["token"]
                .as_str()
                .ok_or_else(|| AppError::storage("Sesión inválida"))?;
            app.state::<AppState>()
                .lan
                .session_stations
                .lock()
                .unwrap()
                .insert(crate::auth::key(token), station);
        } else if request.command == "auth_logout" {
            if let Some(token) = request.session_token {
                app.state::<AppState>()
                    .lan
                    .session_stations
                    .lock()
                    .unwrap()
                    .remove(&crate::auth::key(&token));
            }
        }
        Ok(result)
    }
    .await;
    response(result)
}
#[derive(Deserialize)]
struct PairRequest {
    station_id: String,
    name: String,
}
async fn pair(
    State(app): State<AppHandle>,
    headers: HeaderMap,
    Json(req): Json<PairRequest>,
) -> Response {
    response((|| {
        uuid::Uuid::parse_str(&req.station_id).map_err(|_| AppError::msg("Puesto inválido"))?;
        let key = headers
            .get("x-nightdesk-station")
            .and_then(|v| v.to_str().ok())
            .filter(|v| v.len() == 64 && hex::decode(v).is_ok())
            .ok_or_else(|| AppError::msg("Solicitud inválida"))?;
        let state = app.state::<AppState>();
        if !state
            .lan
            .pairing_until
            .lock()
            .unwrap()
            .is_some_and(|t| Instant::now() < t)
        {
            return Err(AppError::forbidden(
                "Habilitá la vinculación desde la recepción principal",
            ));
        }
        let mut pending = state.lan.pending.lock().unwrap();
        pending.retain(|_, p| p.expires > Instant::now());
        if pending.len() >= 8 {
            return Err(AppError::new(
                crate::error::ErrorCode::RateLimited,
                "Esperá antes de solicitar otra vinculación",
            ));
        }
        if req.name.trim().is_empty() || req.name.len() > 64 {
            return Err(AppError::msg("Nombre de puesto inválido"));
        }
        if pending.contains_key(&req.station_id) {
            return Err(AppError::conflict(
                "Ya existe una solicitud para este puesto",
            ));
        }
        pending.insert(
            req.station_id.clone(),
            PendingPair {
                station_id: req.station_id,
                name: req.name.trim().into(),
                token_hash: crate::auth::key(key),
                expires: Instant::now() + Duration::from_secs(120),
            },
        );
        Ok(json!({"pending":true}))
    })())
}
async fn pair_status(State(app): State<AppHandle>, headers: HeaderMap) -> Response {
    response(station(&app, &headers).map(|id| json!({"paired":true,"station_id":id})))
}
async fn events(
    State(app): State<AppHandle>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let access = (|| {
        let station = station(&app, &headers)?;
        let token = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| AppError::session_expired("Iniciá sesión"))?
            .to_string();
        check_session(&app, &station, Some(&token))?;
        Ok((station, token))
    })();
    match access {
        Ok((station, token)) => ws
            .max_message_size(4096)
            .on_upgrade(move |socket| stream(app, socket, headers, station, token))
            .into_response(),
        Err(e) => response(Err(e)),
    }
}
async fn stream(
    app: AppHandle,
    socket: WebSocket,
    headers: HeaderMap,
    station_id: String,
    token: String,
) {
    let mut events = app.state::<AppState>().lan.changes.subscribe();
    let (mut send, mut receive) = socket.split();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            _=heartbeat.tick()=>{},
            event=events.recv()=>{if matches!(event,Err(tokio::sync::broadcast::error::RecvError::Closed)){break;}},
            incoming=receive.next()=>{match incoming{Some(Ok(Message::Ping(p)))=>{if send.send(Message::Pong(p)).await.is_err(){break;}continue;},Some(Ok(Message::Close(_)))|None|Some(Err(_))=>break,_=>continue}}
        }
        if station(&app, &headers).is_err()
            || check_session(&app, &station_id, Some(&token)).is_err()
        {
            break;
        }
        let value = {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            json!({"epoch":state.lan.epoch.lock().unwrap().clone(),"revision":crate::operations::revision(&conn).unwrap_or(0)})
        };
        if send
            .send(Message::Text(value.to_string().into()))
            .await
            .is_err()
        {
            break;
        }
    }
}
pub fn approve(app: &AppHandle, id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    let pending = state
        .lan
        .pending
        .lock()
        .unwrap()
        .remove(id)
        .ok_or_else(|| AppError::not_found("La solicitud ya no existe"))?;
    if pending.expires < Instant::now() {
        return Err(AppError::msg("La solicitud venció. Volvé a vincular"));
    }
    let conn = state.db.lock().unwrap();
    let other: i64 = conn.query_row(
        "SELECT COUNT(*) FROM lan_stations WHERE active=1 AND id<>?1",
        [id],
        |r| r.get(0),
    )?;
    if other > 0 {
        return Err(AppError::conflict("Esta instalación admite una recepción adicional. Revocá el puesto anterior antes de vincular otro"));
    }
    conn.execute("INSERT INTO lan_stations(id,name,token_hash,active,created_at) VALUES (?1,?2,?3,1,?4) ON CONFLICT(id) DO UPDATE SET name=excluded.name,token_hash=excluded.token_hash,active=1",params![id,pending.name,pending.token_hash,crate::db::now_rfc3339()])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn https_accepts_only_pinned_identity_and_negotiated_protocol() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec![TLS_NAME.into()]).unwrap();
        let certificate = cert.pem();
        let tls = axum_server::tls_rustls::RustlsConfig::from_pem(
            certificate.as_bytes().to_vec(),
            signing_key.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback TLS listener");
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        async fn negotiate(Json(request): Json<Request>) -> Response {
            response(validate_protocol(&request).map(|_| json!({"accepted":true})))
        }
        let router = Router::new().route("/lan/v1/call", post(negotiate));
        let handle = axum_server::Handle::new();
        let finished = Arc::new(tokio::sync::Notify::new());
        let shutdown = HostRuntime {
            id: "test".into(),
            handle: handle.clone(),
            mdns: None,
            finished: finished.clone(),
        };
        let task = tokio::spawn(async move {
            axum_server::from_tcp_rustls(listener, tls)
                .unwrap()
                .handle(handle)
                .serve(router.into_make_service())
                .await
                .unwrap();
            finished.notify_one();
        });
        let host = HostIdentity {
            station_id: uuid::Uuid::new_v4().to_string(),
            address: "127.0.0.1".into(),
            port,
            certificate: certificate.clone(),
            fingerprint: fingerprint(&certificate).unwrap(),
            name: "Test".into(),
        };
        let request = crate::lan::client::request("contract_info", json!({}), None);
        let url = format!("https://{TLS_NAME}:{port}/lan/v1/call");
        let reply: Value = crate::lan::client::http(&host)
            .unwrap()
            .post(&url)
            .json(&request)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(reply["ok"], true);
        let mut incompatible = request;
        incompatible.contract_version = 2;
        let reply: Value = crate::lan::client::http(&host)
            .unwrap()
            .post(&url)
            .json(&incompatible)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(reply["error"]["code"], "incompatible_version");
        let other = rcgen::generate_simple_self_signed(vec![TLS_NAME.into()])
            .unwrap()
            .cert
            .pem();
        let mut impostor = host.clone();
        impostor.fingerprint = fingerprint(&other).unwrap();
        impostor.certificate = other;
        assert!(crate::lan::client::http(&impostor)
            .unwrap()
            .post(&url)
            .json(&incompatible)
            .send()
            .await
            .is_err());
        impostor = host;
        impostor.address = "8.8.8.8".into();
        assert!(crate::lan::client::http(&impostor).is_err());
        shutdown.shutdown().await.unwrap();
        // Reconfiguration must be able to bind the same address immediately after shutdown.
        let replacement =
            std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
        drop(replacement);
        task.await.unwrap();
    }
}
