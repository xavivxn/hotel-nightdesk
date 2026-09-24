//! Supabase HTTP client for the reception sync worker (I07) and catalog write-through (I11).
use crate::credentials::DeviceCreds;
use crate::error::{AppError, AppResult};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::Duration;

const OFFLINE: &str = "Requiere conexión con administración.";

pub trait SyncClient {
    fn apply_ops(&self, device_id: &str, ops: &[Value]) -> AppResult<ApplyOutcome>;
    fn pull_rows(&self, table: &str, cursor: Option<&str>, limit: i64) -> AppResult<Vec<Value>>;
    fn bootstrap(&self, device_id: &str, catalog: &Value) -> AppResult<Value>;
    fn device_id(&self) -> AppResult<String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub applied: i64,
    pub skipped: i64,
}

#[derive(Clone)]
struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: chrono::DateTime<Utc>,
    device_id: String,
}

pub struct SupabaseClient {
    base: String,
    anon_key: String,
    email: String,
    password: String,
    session: Mutex<Option<Session>>,
}

impl SupabaseClient {
    pub fn from_device(creds: DeviceCreds) -> AppResult<Self> {
        Ok(Self {
            base: validate_base(&creds.project_url)?,
            anon_key: creds.anon_key,
            email: creds.device_email,
            password: creds.device_password,
            session: Mutex::new(None),
        })
    }

    pub fn project_url(&self) -> &str {
        &self.base
    }

    pub fn anon_key(&self) -> &str {
        &self.anon_key
    }

    pub fn access_token(&self) -> AppResult<String> {
        Ok(self.ensure_session()?.access_token)
    }

    fn ensure_session(&self) -> AppResult<Session> {
        {
            let guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(session) = guard.as_ref() {
                if session.expires_at > Utc::now() + chrono::Duration::seconds(60) {
                    return Ok(session.clone());
                }
            }
        }
        let existing = self.session.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let session = match existing {
            Some(prev) if !prev.refresh_token.is_empty() => self
                .login_refresh(&prev.refresh_token)
                .or_else(|_| self.login_password())?,
            _ => self.login_password()?,
        };
        *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(session.clone());
        Ok(session)
    }

    fn login_password(&self) -> AppResult<Session> {
        let client = http()?;
        let reply = client
            .post(format!("{}/auth/v1/token?grant_type=password", self.base))
            .header("apikey", &self.anon_key)
            .json(&json!({"email": self.email, "password": self.password}))
            .send()
            .map_err(|_| connect_failed())?;
        parse_auth_reply(reply, "No se pudo autenticar el dispositivo de recepción")
    }

    fn login_refresh(&self, refresh_token: &str) -> AppResult<Session> {
        let client = http()?;
        let reply = client
            .post(format!("{}/auth/v1/token?grant_type=refresh_token", self.base))
            .header("apikey", &self.anon_key)
            .json(&json!({"refresh_token": refresh_token}))
            .send()
            .map_err(|_| connect_failed())?;
        parse_auth_reply(reply, "No se pudo renovar la sesión del dispositivo")
    }

    pub fn rpc(&self, name: &str, body: &Value) -> AppResult<Value> {
        let token = self.access_token()?;
        let client = http()?;
        let reply = client
            .post(format!("{}/rest/v1/rpc/{name}", self.base))
            .header("apikey", &self.anon_key)
            .bearer_auth(&token)
            .json(body)
            .send()
            .map_err(|_| offline())?;
        parse_rest_reply(reply)
    }

    pub fn select(&self, table: &str, query: &[(&str, String)]) -> AppResult<Vec<Value>> {
        if !PULL_TABLES.contains(&table) {
            return Err(AppError::msg("Tabla de pull inválida"));
        }
        self.rest_select(table, query)
    }

    fn rest_select(&self, table: &str, query: &[(&str, String)]) -> AppResult<Vec<Value>> {
        let token = self.access_token()?;
        let client = http()?;
        let mut url = reqwest::Url::parse(&format!("{}/rest/v1/{table}", self.base))
            .map_err(|_| AppError::msg("URL de administración inválida"))?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key, value);
            }
        }
        let reply = client
            .get(url)
            .header("apikey", &self.anon_key)
            .bearer_auth(&token)
            .header("Accept", "application/json")
            .send()
            .map_err(|_| offline())?;
        let value = parse_rest_reply(reply)?;
        value
            .as_array()
            .cloned()
            .ok_or_else(|| AppError::storage(OFFLINE))
    }

    /// Upload bytes to the private `backups` Storage bucket. Returns after HTTP success.
    pub fn upload_backup_object(&self, storage_path: &str, bytes: &[u8]) -> AppResult<()> {
        let token = self.access_token()?;
        let client = http()?;
        let url = format!("{}/storage/v1/object/backups/{}", self.base, storage_path.trim_start_matches('/'));
        let reply = client
            .post(url)
            .header("apikey", &self.anon_key)
            .bearer_auth(&token)
            .header("Content-Type", "application/octet-stream")
            .header("x-upsert", "false")
            .body(bytes.to_vec())
            .send()
            .map_err(|_| offline())?;
        let status = reply.status();
        if status.is_success() {
            return Ok(());
        }
        let body: Value = reply.json().unwrap_or(Value::Null);
        Err(map_http(status.as_u16(), &body))
    }

    pub fn insert_backup_manifest(&self, row: &Value) -> AppResult<()> {
        let token = self.access_token()?;
        let client = http()?;
        let reply = client
            .post(format!("{}/rest/v1/backups", self.base))
            .header("apikey", &self.anon_key)
            .bearer_auth(&token)
            .header("Prefer", "return=minimal")
            .header("Content-Type", "application/json")
            .json(row)
            .send()
            .map_err(|_| offline())?;
        let status = reply.status();
        if status.is_success() || status.as_u16() == 201 {
            return Ok(());
        }
        let body: Value = reply.json().unwrap_or(Value::Null);
        Err(map_http(status.as_u16(), &body))
    }

    pub fn list_backup_manifests(&self) -> AppResult<Vec<Value>> {
        self.rest_select(
            "backups",
            &[
                (
                    "select",
                    "backup_id,backuped_at,schema_version,size_bytes,checksum,storage_path,nonce_hex".into(),
                ),
                ("order", "backuped_at.desc".into()),
                ("limit", "30".into()),
            ],
        )
    }

    pub fn fetch_backup_manifest(&self, backup_id: &str) -> AppResult<Value> {
        let rows = self.rest_select(
            "backups",
            &[
                (
                    "select",
                    "backup_id,backuped_at,schema_version,size_bytes,checksum,storage_path,nonce_hex".into(),
                ),
                ("backup_id", format!("eq.{backup_id}")),
                ("limit", "1".into()),
            ],
        )?;
        rows.into_iter()
            .next()
            .ok_or_else(|| AppError::not_found("No se encontró el manifiesto remoto"))
    }

    pub fn download_backup_object(&self, storage_path: &str) -> AppResult<Vec<u8>> {
        let token = self.access_token()?;
        let client = http()?;
        let url = format!("{}/storage/v1/object/backups/{}", self.base, storage_path.trim_start_matches('/'));
        let reply = client
            .get(url)
            .header("apikey", &self.anon_key)
            .bearer_auth(&token)
            .send()
            .map_err(|_| offline())?;
        if !reply.status().is_success() {
            let status = reply.status().as_u16();
            let body: Value = reply.json().unwrap_or(Value::Null);
            return Err(map_http(status, &body));
        }
        Ok(reply.bytes().map_err(|_| offline())?.to_vec())
    }
}

const PULL_TABLES: &[&str] = &[
    "rooms",
    "rate_plans",
    "products",
    "app_users",
    "business_settings",
    "catalog_deletes",
];

impl SyncClient for SupabaseClient {
    fn apply_ops(&self, device_id: &str, ops: &[Value]) -> AppResult<ApplyOutcome> {
        let reply = self.rpc(
            "sync_apply_ops",
            &json!({"device_id": device_id, "ops": ops}),
        )?;
        Ok(ApplyOutcome {
            applied: reply["applied"].as_i64().unwrap_or(0),
            skipped: reply["skipped"].as_i64().unwrap_or(0),
        })
    }

    fn pull_rows(&self, table: &str, cursor: Option<&str>, limit: i64) -> AppResult<Vec<Value>> {
        let cursor = cursor.unwrap_or("1970-01-01T00:00:00Z");
        let (stamp, order) = if table == "catalog_deletes" {
            ("deleted_at", "deleted_at.asc,uid.asc")
        } else if table == "business_settings" {
            ("updated_at", "updated_at.asc,key.asc")
        } else {
            ("updated_at", "updated_at.asc,uid.asc")
        };
        self.select(
            table,
            &[
                ("select", "*".into()),
                (stamp, format!("gte.{cursor}")),
                ("order", order.into()),
                ("limit", limit.to_string()),
            ],
        )
    }

    fn bootstrap(&self, device_id: &str, catalog: &Value) -> AppResult<Value> {
        let mut body = catalog.clone();
        if let Some(obj) = body.as_object_mut() {
            obj.insert("device_id".into(), json!(device_id));
        }
        self.rpc("sync_bootstrap_catalog", &body)
    }

    fn device_id(&self) -> AppResult<String> {
        Ok(self.ensure_session()?.device_id)
    }
}

fn http() -> AppResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(4))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| offline())
}

fn offline() -> AppError {
    AppError::storage(OFFLINE)
}

fn connect_failed() -> AppError {
    AppError::storage("No se pudo conectar con administración. Revisá la URL y que Supabase esté en marcha.")
}

pub fn validate_base(url: &str) -> AppResult<String> {
    let parsed = reqwest::Url::parse(url.trim()).map_err(|_| AppError::msg("URL de administración inválida"))?;
    let local = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if parsed.scheme() != "https" && !(parsed.scheme() == "http" && local) {
        return Err(AppError::msg("Administración requiere HTTPS"));
    }
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

fn parse_auth_reply(reply: reqwest::blocking::Response, forbidden: &str) -> AppResult<Session> {
    let status = reply.status();
    let body: Value = reply.json().map_err(|_| {
        AppError::storage("Administración no respondió con una sesión válida. Revisá la URL y la clave anónima.")
    })?;
    if !status.is_success() {
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(AppError::forbidden(forbidden));
        }
        let detail = body["error_description"]
            .as_str()
            .or_else(|| body["msg"].as_str())
            .or_else(|| body["message"].as_str())
            .unwrap_or("Administración rechazó el acceso del dispositivo");
        return Err(AppError::storage(detail));
    }
    let access = body["access_token"]
        .as_str()
        .ok_or_else(offline)?;
    let refresh = body["refresh_token"].as_str().unwrap_or("").to_string();
    let claims = decode_jwt_claims(access)?;
    let device_id = claims["app_metadata"]["device_id"]
        .as_str()
        .ok_or_else(|| AppError::forbidden("El dispositivo no tiene device_id en la sesión"))?
        .to_string();
    uuid::Uuid::parse_str(&device_id)
        .map_err(|_| AppError::forbidden("El dispositivo no tiene device_id en la sesión"))?;
    let expires_at = claims["exp"]
        .as_i64()
        .and_then(|exp| Utc.timestamp_opt(exp, 0).single())
        .or_else(|| {
            body["expires_in"]
                .as_i64()
                .map(|secs| Utc::now() + chrono::Duration::seconds(secs))
        })
        .unwrap_or_else(|| Utc::now() + chrono::Duration::seconds(3600));
    Ok(Session {
        access_token: access.to_string(),
        refresh_token: refresh,
        expires_at,
        device_id,
    })
}

fn parse_rest_reply(reply: reqwest::blocking::Response) -> AppResult<Value> {
    let status = reply.status();
    if status.as_u16() == 204 {
        return Ok(Value::Null);
    }
    let body: Value = reply.json().map_err(|_| offline())?;
    if status.is_success() {
        return Ok(body);
    }
    Err(map_http(status.as_u16(), &body))
}

pub fn map_http(status: u16, body: &Value) -> AppError {
    let msg = body["message"].as_str().unwrap_or("").to_string();
    let code = body["code"].as_str().unwrap_or("");
    if msg.contains("conflict:") {
        return AppError::conflict("La ficha cambió; recargá antes de guardar");
    }
    if status == 401 || status == 403 || code == "42501" || msg.starts_with("forbidden:") {
        return AppError::forbidden("Sin permiso para sincronizar");
    }
    if msg.starts_with("validation:") || code == "P0001" {
        return AppError::msg(if msg.is_empty() {
            "Administración rechazó los datos".into()
        } else {
            msg
        });
    }
    if status >= 500 {
        return offline();
    }
    if msg.is_empty() {
        AppError::msg("Administración rechazó los datos")
    } else {
        AppError::msg(msg)
    }
}

pub fn decode_jwt_claims(token: &str) -> AppResult<Value> {
    use base64::Engine;
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(offline)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .map_err(|_| offline())?;
    serde_json::from_slice(&bytes).map_err(|_| offline())
}

pub fn realtime_ws_url(base: &str, anon_key: &str) -> String {
    let rest = base
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let scheme = if base.starts_with("https://") { "wss" } else { "ws" };
    let mut url = reqwest::Url::parse(&format!("{scheme}://{rest}/realtime/v1/websocket"))
        .unwrap_or_else(|_| reqwest::Url::parse("ws://127.0.0.1/realtime/v1/websocket").expect("fallback"));
    url.query_pairs_mut()
        .append_pair("apikey", anon_key)
        .append_pair("vsn", "1.0.0");
    url.to_string()
}

#[cfg(test)]
#[derive(Default)]
pub struct FakeClient {
    pub offline: std::sync::atomic::AtomicBool,
    pub apply_calls: Mutex<Vec<Vec<Value>>>,
    pub sent_ids: Mutex<std::collections::HashSet<String>>,
    pub reject_op: Mutex<Option<String>>,
    pub pull: Mutex<std::collections::HashMap<String, Vec<Value>>>,
    pub bootstrap_reply: Mutex<Value>,
    pub bootstrap_calls: Mutex<Vec<Value>>,
    pub device: Mutex<String>,
}

#[cfg(test)]
impl FakeClient {
    pub fn new() -> Self {
        Self {
            bootstrap_reply: Mutex::new(json!({"accepted": true})),
            device: Mutex::new("33333333-3333-3333-3333-333333333333".into()),
            ..Self::default()
        }
    }

    pub fn set_offline(&self, value: bool) {
        self.offline.store(value, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
impl SyncClient for FakeClient {
    fn apply_ops(&self, _device_id: &str, ops: &[Value]) -> AppResult<ApplyOutcome> {
        if self.offline.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(offline());
        }
        self.apply_calls.lock().expect("fake").push(ops.to_vec());
        if let Some(bad) = self.reject_op.lock().expect("fake").as_ref() {
            if ops.iter().any(|op| op["operation_id"].as_str() == Some(bad.as_str())) {
                return Err(AppError::msg(format!("validation: bad op {bad}")));
            }
        }
        let mut applied = 0;
        let mut skipped = 0;
        let mut sent = self.sent_ids.lock().expect("fake");
        for op in ops {
            let id = op["operation_id"].as_str().unwrap_or("").to_string();
            if sent.contains(&id) {
                skipped += 1;
            } else {
                sent.insert(id);
                applied += 1;
            }
        }
        Ok(ApplyOutcome { applied, skipped })
    }

    fn pull_rows(&self, table: &str, cursor: Option<&str>, limit: i64) -> AppResult<Vec<Value>> {
        if self.offline.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(offline());
        }
        let cursor = cursor.unwrap_or("1970-01-01T00:00:00Z");
        let rows = self.pull.lock().expect("fake").get(table).cloned().unwrap_or_default();
        let stamp = if table == "catalog_deletes" { "deleted_at" } else { "updated_at" };
        let filtered: Vec<Value> = rows
            .into_iter()
            .filter(|row| row[stamp].as_str().unwrap_or("").cmp(cursor) != std::cmp::Ordering::Less)
            .take(limit as usize)
            .collect();
        Ok(filtered)
    }

    fn bootstrap(&self, _device_id: &str, catalog: &Value) -> AppResult<Value> {
        if self.offline.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(offline());
        }
        self.bootstrap_calls.lock().expect("fake").push(catalog.clone());
        Ok(self.bootstrap_reply.lock().expect("fake").clone())
    }

    fn device_id(&self) -> AppResult<String> {
        if self.offline.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(offline());
        }
        Ok(self.device.lock().expect("fake").clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use base64::Engine;

    fn jwt(device_id: &str) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!(
            r#"{{"app_metadata":{{"role":"device","device_id":"{device_id}"}},"exp":9999999999}}"#
        ));
        format!("{header}.{payload}.sig")
    }

    #[test]
    fn jwt_exposes_device_id() {
        let claims = decode_jwt_claims(&jwt("33333333-3333-3333-3333-333333333333")).unwrap();
        assert_eq!(
            claims["app_metadata"]["device_id"],
            "33333333-3333-3333-3333-333333333333"
        );
    }

    #[test]
    fn http_errors_map_to_domain_codes() {
        assert_eq!(
            map_http(409, &json!({"message":"conflict: expected_version"})).code(),
            ErrorCode::Conflict
        );
        assert_eq!(
            map_http(403, &json!({"code":"42501","message":"forbidden: device"})).code(),
            ErrorCode::Forbidden
        );
        assert_eq!(
            map_http(400, &json!({"code":"P0001","message":"validation: ops must be a JSON array"})).code(),
            ErrorCode::Validation
        );
        assert_eq!(
            map_http(503, &json!({"message":"boom"})).code(),
            ErrorCode::Storage
        );
    }

    #[test]
    fn https_required_except_localhost() {
        assert!(validate_base("http://example.supabase.co").is_err());
        assert!(validate_base("https://xxxx.supabase.co").is_ok());
        assert!(validate_base("http://127.0.0.1:54321").is_ok());
    }
}
