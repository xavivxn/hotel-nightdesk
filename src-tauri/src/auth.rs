use crate::{db, error::{AppError, AppResult}, models::*, AppState};
use argon2::{Argon2, PasswordHasher, PasswordVerifier, password_hash::{SaltString, PasswordHash}};
use rand_core::{OsRng, RngCore};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, time::{Duration, Instant}};
use tauri::State;

pub const SESSION_SECONDS: i64 = 8 * 60 * 60;
pub struct ActiveSession { pub user_id: i64, pub deadline: Instant, pub expires_at: i64 }
#[derive(Default)]
pub struct AuthState { pub sessions: HashMap<String, ActiveSession> }
fn error(code: &str, message: &str) -> AppError {
    match code {
        "FORBIDDEN" => AppError::forbidden(message),
        "SESSION_EXPIRED" => AppError::session_expired(message),
        "RATE_LIMITED" => AppError::rate_limited(message),
        "INVALID_CREDENTIALS" => AppError::invalid_credentials(message),
        _ => AppError::msg(message),
    }
}

fn map_user(id: i64, username: String, role: String) -> SessionUser {
    SessionUser {
        id,
        username,
        role: Role::parse(&role),
    }
}
fn key(token: &str) -> String { hex::encode(Sha256::digest(token.as_bytes())) }

pub fn require(state: &AppState, token: Option<&str>, admin: bool) -> AppResult<SessionUser> {
    let token = token.ok_or_else(|| error("SESSION_EXPIRED", "Iniciá sesión para continuar"))?;
    let conn = state.db.lock().map_err(|_| AppError::msg("Base no disponible"))?;
    let mut auth = state.auth.lock().map_err(|_| AppError::msg("Sesiones no disponibles"))?;
    let session_key = key(token);
    let session = auth.sessions.get(&session_key).ok_or_else(|| error("SESSION_EXPIRED", "La sesión terminó. Volvé a ingresar"))?;
    if Instant::now() >= session.deadline {
        auth.sessions.remove(&session_key);
        return Err(error("SESSION_EXPIRED", "La sesión venció. Volvé a ingresar"));
    }
    let user = conn.query_row("SELECT id, username, role FROM users WHERE id = ?1 AND active = 1", [session.user_id], |row| Ok(map_user(row.get(0)?, row.get(1)?, row.get(2)?))).optional()?;
    let Some(user) = user else {
        auth.sessions.remove(&session_key);
        return Err(error("SESSION_EXPIRED", "La cuenta ya no está habilitada"));
    };
    if admin && !user.role.is_admin() { return Err(error("FORBIDDEN", "Esta operación requiere administración")); }
    Ok(user)
}

fn insert_user(conn: &rusqlite::Connection, payload: &CreateUserPayload) -> AppResult<SessionUser> {
    let username = payload.username.trim().to_lowercase();
    if username.is_empty() || username.len() > 64 { return Err(AppError::msg("Ingresá un usuario de 1 a 64 caracteres")); }
    if payload.password.is_empty() || payload.password.len() > 128 { return Err(AppError::msg("La contraseña debe tener entre 1 y 128 bytes")); }
    if !matches!(payload.role.as_str(), "admin" | "recepcion") { return Err(AppError::msg("Rol inválido")); }
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM users WHERE username = ?1)", [&username], |r| r.get(0))?;
    if exists { return Err(AppError::msg("Ese usuario ya existe")); }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(payload.password.as_bytes(), &salt).map_err(|_| AppError::msg("No se pudo proteger la contraseña"))?.to_string();
    conn.execute("INSERT INTO users(username, password_hash, role) VALUES (?1, ?2, ?3)", params![username, hash, Role::parse(&payload.role).as_str()])?;
    Ok(SessionUser { id: conn.last_insert_rowid(), username, role: Role::parse(&payload.role) })
}

#[tauri::command]
pub fn auth_setup_required(state: State<AppState>) -> AppResult<bool> {
    let conn = state.db.lock().unwrap();
    Ok(conn.query_row("SELECT COUNT(*) = 0 FROM users", [], |r| r.get(0))?)
}

#[tauri::command]
pub fn auth_setup(state: State<AppState>, payload: LoginPayload, legacy_pin: Option<String>) -> AppResult<SessionUser> {
    let mut conn = state.db.lock().unwrap();
    let tx = conn.transaction()?;
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
    if count != 0 { return Err(error("FORBIDDEN", "La administración ya está configurada")); }
    let hash = db::get_setting(&tx, "pin_hash", "")?;
    if !hash.is_empty() && hash != db::hash_pin(legacy_pin.as_deref().unwrap_or("")) {
        return Err(AppError::msg("Ingresá el PIN anterior para configurar el administrador"));
    }
    let user = insert_user(&tx, &CreateUserPayload { username: payload.username, password: payload.password, role: "admin".into() })?;
    tx.commit()?;
    Ok(user)
}

#[tauri::command]
pub async fn auth_create_user(state: State<'_, AppState>, session_token: Option<String>, app: tauri::AppHandle, payload: CreateUserPayload, operation_id: Option<String>) -> AppResult<SessionUser> {
    let user = require(&state, session_token.as_deref(), true)?;
    crate::service::authorize(&crate::service::Actor::from(&user), crate::service::Operation::CreateUser)?;
    if payload.username.trim().is_empty() || payload.username.len()>64 || !matches!(payload.role.as_str(), "admin"|"recepcion") { return Err(AppError::msg("Usuario o rol inválido")); }
    let operation_id=Some(operation_id.unwrap_or_else(||uuid::Uuid::new_v4().to_string()));
    let hash = crate::service::hash_password_with_operation(&payload.password, operation_id.as_deref())?.hash;
    let data = serde_json::json!({"username":payload.username.trim().to_lowercase(),"password_hash":hash,"role":payload.role,"active":true});
    if let Some(id) = crate::commands::try_catalog(&state, &app, &crate::service::Actor::from(&user), "app_users", data, operation_id).await? {
        return state.db.lock().unwrap().query_row("SELECT id,username,role FROM users WHERE id=?1",[id],|r|Ok(map_user(r.get(0)?,r.get(1)?,r.get(2)?))).map_err(Into::into);
    }
    crate::sync::catalog::local(&state.db.lock().unwrap(), &crate::service::Actor::from(&user), "app_users", |tx| insert_user(tx, &payload))
}

#[tauri::command]
pub fn auth_login(state: State<AppState>, payload: LoginPayload) -> AppResult<SessionInfo> {
    login(&state, payload)
}

fn login(state: &AppState, payload: LoginPayload) -> AppResult<SessionInfo> {
    let conn = state.db.lock().unwrap();
    let username = payload.username.trim().to_lowercase();
    if username.len() > 64 || payload.password.len() > 128 { return Err(error("INVALID_CREDENTIALS", "Usuario o contraseña incorrectos")); }
    let now = chrono::Utc::now().timestamp();
    let (failures, blocked_until): (i64, i64) = conn.query_row("SELECT failures, blocked_until FROM login_attempts WHERE username = ?1", [&username], |r| Ok((r.get(0)?, r.get(1)?))).optional()?.unwrap_or((0, 0));
    if now < blocked_until { return Err(error("RATE_LIMITED", "Demasiados intentos. Esperá 5 minutos")); }
    let row: Option<(SessionUser, String, bool)> = conn.query_row("SELECT id, username, role, password_hash, active FROM users WHERE username = ?1", [&username], |r| Ok((map_user(r.get(0)?, r.get(1)?, r.get(2)?), r.get(3)?, r.get(4)?))).optional()?;
    let valid = row.as_ref().map(|(_, hash, active)| *active && PasswordHash::new(hash).map(|parsed| Argon2::default().verify_password(payload.password.as_bytes(), &parsed).is_ok()).unwrap_or(false)).unwrap_or(false);
    if !valid {
        let failures = if blocked_until != 0 { 1 } else { failures + 1 };
        let blocked = if failures >= 5 { now + 300 } else { 0 };
        conn.execute("INSERT INTO login_attempts VALUES (?1, ?2, ?3) ON CONFLICT(username) DO UPDATE SET failures=excluded.failures, blocked_until=excluded.blocked_until", params![username, failures, blocked])?;
        return Err(error("INVALID_CREDENTIALS", "Usuario o contraseña incorrectos"));
    }
    conn.execute("DELETE FROM login_attempts WHERE username = ?1", [&username])?;
    let user = row.unwrap().0;
    let mut random = [0u8; 32];
    OsRng.fill_bytes(&mut random);
    let token = hex::encode(random);
    let expires_at = now + SESSION_SECONDS;
    let mut auth = state.auth.lock().unwrap();
    auth.sessions.retain(|_, s| s.deadline > Instant::now());
    auth.sessions.insert(key(&token), ActiveSession { user_id: user.id, deadline: Instant::now() + Duration::from_secs(SESSION_SECONDS as u64), expires_at });
    Ok(SessionInfo { token, user, expires_at })
}

#[tauri::command]
pub fn auth_session(state: State<AppState>, session_token: Option<String>) -> AppResult<SessionInfo> {
    let user = require(&state, session_token.as_deref(), false)?;
    let token = session_token.unwrap();
    let auth = state.auth.lock().unwrap();
    let session = auth.sessions.get(&key(&token)).ok_or_else(|| error("SESSION_EXPIRED", "La sesión terminó"))?;
    Ok(SessionInfo { token, user, expires_at: session.expires_at })
}

#[tauri::command]
pub fn auth_logout(state: State<AppState>, session_token: Option<String>) -> AppResult<()> {
    if let Some(token) = session_token { state.auth.lock().unwrap().sessions.remove(&key(&token)); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use std::{path::Path, sync::Mutex};
    fn fixture() -> AppState {
        let conn = db::open(Path::new(":memory:")).unwrap();
        insert_user(&conn, &CreateUserPayload { username: "admin".into(), password: "Prueba-segura-123".into(), role: "admin".into() }).unwrap();
        insert_user(&conn, &CreateUserPayload { username: "recepcion".into(), password: "Prueba-segura-456".into(), role: "recepcion".into() }).unwrap();
        AppState { db: Mutex::new(conn), auth: Mutex::new(AuthState::default()), sync: Mutex::new(None), backup: Mutex::new(None) }
    }
    fn credentials(user: &str, password: &str) -> LoginPayload { LoginPayload { username: user.into(), password: password.into() } }

    #[test]
    fn login_role_and_revocation_are_enforced() {
        let state = fixture();
        assert_eq!(require(&state, None, false).unwrap_err().code(), ErrorCode::SessionExpired);
        let session = login(&state, credentials("RECEPCION", "Prueba-segura-456")).unwrap();
        assert_eq!(require(&state, Some(&session.token), false).unwrap().role, Role::Recepcion);
        assert_eq!(require(&state, Some(&session.token), true).unwrap_err().code(), ErrorCode::Forbidden);
        state.auth.lock().unwrap().sessions.remove(&key(&session.token));
        assert!(require(&state, Some(&session.token), false).is_err());
        let admin = login(&state, credentials("admin", "Prueba-segura-123")).unwrap();
        assert!(require(&state, Some(&admin.token), true).is_ok());
    }

    #[test]
    fn expired_and_disabled_sessions_are_rejected() {
        let state = fixture();
        let session = login(&state, credentials("admin", "Prueba-segura-123")).unwrap();
        state.auth.lock().unwrap().sessions.get_mut(&key(&session.token)).unwrap().deadline = Instant::now();
        assert!(require(&state, Some(&session.token), true).is_err());
        let session = login(&state, credentials("admin", "Prueba-segura-123")).unwrap();
        state.db.lock().unwrap().execute("UPDATE users SET active=0 WHERE username='admin'", []).unwrap();
        assert!(require(&state, Some(&session.token), true).is_err());
    }

    #[test]
    fn bad_passwords_lock_temporarily_and_hashes_are_argon2id() {
        let state = fixture();
        for _ in 0..5 { assert!(login(&state, credentials("admin", "incorrecta")).is_err()); }
        assert_eq!(login(&state, credentials("admin", "Prueba-segura-123")).err().unwrap().code(), ErrorCode::RateLimited);
        let conn = state.db.lock().unwrap();
        let hash: String = conn.query_row("SELECT password_hash FROM users WHERE username='admin'", [], |r| r.get(0)).unwrap();
        assert!(hash.starts_with("$argon2id$"));
        conn.execute("UPDATE login_attempts SET blocked_until=1", []).unwrap();
        drop(conn);
        assert!(login(&state, credentials("admin", "Prueba-segura-123")).is_ok());
    }

    #[test]
    fn every_business_command_checks_session_and_admin_mutations_check_role() {
        let source = include_str!("commands.rs");
        let admins = ["save_room", "save_rate_plan", "save_product", "set_product_active", "add_charge", "delete_charge", "save_settings", "print_test", "backup_run_now", "backup_list", "backup_import_key", "backup_restore", "analytics_summary", "save_analytics_pdf"];
        let public = [
            "device_mode_get",
            "device_mode_set",
            "remote_configure",
            "remote_configured",
            "remote_get_config",
            "hash_password",
            "sync_status",
            "sync_pull_now",
            "sync_configure_device",
        ];
        for section in source.split("#[tauri::command]").skip(1) {
            let name = section.split("pub ").nth(1).unwrap().trim_start_matches("async ").trim_start_matches("fn ").split('(').next().unwrap();
            if public.contains(&name) {
                continue;
            }
            let guard = section.find("crate::auth::require").expect("missing authorization");
            if let Some(lock) = section.find("conn(&state)") {
                assert!(guard < lock, "{name} accesses data before authorization");
            } else {
                let effect = match name {
                    "save_daily_pdf" | "save_analytics_pdf" => "std::fs::",
                    "list_printers" => "printer::list_printers",
                    _ => panic!("Unexpected command without a database access check: {name}"),
                };
                assert!(guard < section.find(effect).unwrap());
            }
            if admins.contains(&name) { assert!(section[guard..].starts_with("crate::auth::require(&state, session_token.as_deref(), true)?;"), "{name} allows reception"); }
        }
    }
}
