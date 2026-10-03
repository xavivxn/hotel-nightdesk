use crate::db;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::models::*;
use crate::printer;
use crate::service::{self, Actor};
use crate::AppState;
use rusqlite::Connection;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};


fn conn(state: &AppState) -> std::sync::MutexGuard<'_, Connection> {
    state.db.lock().expect("db lock")
}

fn actor_from(user: &SessionUser) -> Actor {
    Actor::from(user)
}

fn wake_push(state: &AppState) {
    if let Ok(guard) = state.sync.lock() {
        if let Some(handle) = guard.as_ref() {
            handle.wake_push();
        }
    }
}

fn maybe_start_worker(state: &AppState, app: &AppHandle, data_dir: &std::path::Path) -> AppResult<()> {
    let mode = service::device_mode_get(&conn(state))?;
    if !crate::sync::worker::should_start(mode.as_deref(), true) {
        return Ok(());
    }
    let mut slot = state.sync.lock().expect("sync lock");
    // Replacing the handle drops the previous channel so a dead worker can start again
    // after Supabase comes up or the credentials change.
    *slot = Some(crate::sync::worker::start(
        app.clone(),
        data_dir.join("nightdesk.db"),
        data_dir.to_path_buf(),
    )?);
    Ok(())
}

pub(crate) async fn try_catalog(state: &AppState, app: &AppHandle, actor: &Actor, entity: &str, payload: serde_json::Value, operation_id: Option<String>) -> AppResult<Option<i64>> {
    let dir = app_data_dir(app)?;
    if !crate::sync::catalog::configured(&dir)? { return Ok(None); }
    let request = crate::sync::catalog::prepare(&conn(state), actor, entity, payload, operation_id)?;
    if entity == "settings" && request["p_payload"]["values"].as_object().is_some_and(|v| v.is_empty()) { return Ok(Some(0)); }
    let remote = crate::sync::remote::SupabaseRemote::load(&dir)?;
    // Do not retain the database mutex during authentication/network IO.
    let remote_request=request.clone();
    let result = tauri::async_runtime::spawn_blocking(move || crate::sync::catalog::execute(&remote, &remote_request)).await.map_err(|_| AppError::storage("No se pudo completar la operación remota"))??;
    let id = crate::sync::catalog::apply_result(&mut conn(state), &request, &result)?;
    Ok(Some(id))
}

pub(crate) async fn try_catalog_delete(
    state: &AppState,
    app: &AppHandle,
    actor: &Actor,
    user_id: i64,
    expected_version: i64,
    operation_id: Option<String>,
) -> AppResult<bool> {
    let dir = app_data_dir(app)?;
    if !crate::sync::catalog::configured(&dir)? {
        return Ok(false);
    }
    let request = crate::sync::catalog::prepare_delete(&conn(state), actor, user_id, expected_version, operation_id)?;
    let remote = crate::sync::remote::SupabaseRemote::load(&dir)?;
    let remote_request = request.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::sync::catalog::execute_delete(&remote, &remote_request)
    })
    .await
    .map_err(|_| AppError::storage("No se pudo completar la operación remota"))??;
    crate::sync::catalog::apply_delete(&mut conn(state), &request, &result)?;
    Ok(true)
}

fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::msg(e.to_string()))
}

#[tauri::command]
pub fn contract_info(state: State<AppState>, session_token: Option<String>) -> AppResult<ContractInfo> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::contract_info(&conn)
}

#[tauri::command]
pub fn list_board(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<BoardRoom>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::list_board(&conn)
}

#[tauri::command]
pub fn list_rooms(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<Room>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::list_rooms(&conn)
}

#[tauri::command]
pub async fn save_room(state: State<'_, AppState>, session_token: Option<String>, app: AppHandle, payload: SaveRoomPayload) -> AppResult<Room> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    if let Some(id) = try_catalog(&state, &app, &actor_from(&user), "rooms", serde_json::to_value(&payload).unwrap(), None).await? { return db::get_room(&conn(&state), id); }
    let conn = conn(&state);
    crate::sync::catalog::local(&conn, &actor_from(&user), "rooms", |tx| service::save_room(tx, &actor_from(&user), payload))
}

#[tauri::command]
pub fn list_rate_plans(state: State<AppState>, session_token: Option<String>, active_only: Option<bool>) -> AppResult<Vec<RatePlan>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    db::list_rate_plans(&conn, active_only.unwrap_or(false))
}

#[tauri::command]
pub async fn save_rate_plan(state: State<'_, AppState>, session_token: Option<String>, app: AppHandle, payload: SaveRatePlanPayload) -> AppResult<RatePlan> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    if let Some(id) = try_catalog(&state, &app, &actor_from(&user), "rate_plans", serde_json::to_value(&payload).unwrap(), None).await? { return db::get_rate_plan(&conn(&state), id); }
    let conn = conn(&state);
    crate::sync::catalog::local(&conn, &actor_from(&user), "rate_plans", |tx| service::save_rate_plan(tx, &actor_from(&user), payload))
}

#[tauri::command]
pub fn preview_bill(state: State<AppState>, session_token: Option<String>, stay_id: i64) -> AppResult<BillPreview> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::preview_bill(&conn, stay_id)
}

#[tauri::command]
pub fn get_stay_detail(
    state: State<AppState>,
    session_token: Option<String>,
    stay_id: i64,
) -> AppResult<(Stay, BillPreview, Vec<Charge>, Vec<Payment>)> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::get_stay_detail(&conn, stay_id)
}

#[tauri::command]
pub fn list_products(state: State<AppState>, session_token: Option<String>, active_only: Option<bool>) -> AppResult<Vec<Product>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    db::list_products(&conn, active_only.unwrap_or(true))
}

#[tauri::command]
pub async fn save_product(state: State<'_, AppState>, session_token: Option<String>, app: AppHandle, payload: SaveProductPayload) -> AppResult<Product> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    if let Some(id) = try_catalog(&state, &app, &actor_from(&user), "products", serde_json::to_value(&payload).unwrap(), None).await? { return db::get_product(&conn(&state), id); }
    let conn = conn(&state);
    crate::sync::catalog::local(&conn, &actor_from(&user), "products", |tx| service::save_product(tx, &actor_from(&user), payload))
}

#[tauri::command]
pub async fn set_product_active(state: State<'_, AppState>, session_token: Option<String>, app: AppHandle, product_id: i64, active: bool, operation_id: Option<String>, expected_version: Option<i64>) -> AppResult<Product> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    let mut payload = serde_json::to_value(db::get_product(&conn(&state), product_id)?).unwrap();
    payload["active"] = serde_json::json!(active);
    payload["expected_version"] = serde_json::json!(expected_version);
    if let Some(id) = try_catalog(&state, &app, &actor_from(&user), "products", payload, operation_id).await? { return db::get_product(&conn(&state), id); }
    let conn = conn(&state);
    crate::sync::catalog::local(&conn, &actor_from(&user), "products", |tx| service::set_product_active(tx, &actor_from(&user), product_id, active))
}

#[tauri::command]
pub fn list_users(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<ManagedUser>> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    let conn = conn(&state);
    service::list_users(&conn, &actor_from(&user))
}

#[tauri::command]
pub async fn set_user_active(
    state: State<'_, AppState>,
    session_token: Option<String>,
    app: AppHandle,
    user_id: i64,
    active: bool,
    operation_id: Option<String>,
    expected_version: Option<i64>,
) -> AppResult<ManagedUser> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    let actor = actor_from(&user);
    let (current, hash) = {
        let conn = conn(&state);
        service::validate_set_user_active(&conn, &actor, user_id, active, expected_version)?
    };
    let payload = |expected: i64| {
        serde_json::json!({
            "id": current.id,
            "username": current.username,
            "password_hash": hash,
            "role": current.role.as_str(),
            "active": active,
            "expected_version": expected,
        })
    };
    // A locally created user (first boot, or created offline) has version >= 1 but
    // no remote row yet. catalog_upsert_user treats a missing uid as insert and
    // requires expected_version 0; retry once after a conflict.
    let catalog_id = match try_catalog(&state, &app, &actor, "app_users", payload(current.version), operation_id.clone()).await {
        Ok(id) => id,
        Err(error) if error.code() == ErrorCode::Conflict && current.version != 0 => {
            try_catalog(&state, &app, &actor, "app_users", payload(0), operation_id).await?
        }
        Err(error) => return Err(error),
    };
    if let Some(id) = catalog_id {
        if !active {
            crate::auth::revoke_sessions_for(&state, user_id);
        }
        return service::get_managed_user(&conn(&state), id);
    }
    let conn = conn(&state);
    let updated = crate::sync::catalog::local(&conn, &actor, "app_users", |tx| {
        service::set_user_active(tx, &actor, user_id, active, Some(current.version))
    })?;
    if !active {
        crate::auth::revoke_sessions_for(&state, user_id);
    }
    Ok(updated)
}

#[tauri::command]
pub async fn delete_user(
    state: State<'_, AppState>,
    session_token: Option<String>,
    app: AppHandle,
    user_id: i64,
    operation_id: Option<String>,
    expected_version: Option<i64>,
) -> AppResult<()> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    let actor = actor_from(&user);
    let current = {
        let conn = conn(&state);
        service::validate_delete_user(&conn, &actor, user_id, expected_version)?
    };
    let expected = expected_version.unwrap_or(current.version);
    let deleted = match try_catalog_delete(&state, &app, &actor, current.id, expected, operation_id.clone()).await {
        Ok(done) => done,
        Err(error) if error.code() == ErrorCode::Conflict && expected != 0 => {
            try_catalog_delete(&state, &app, &actor, current.id, 0, operation_id).await?
        }
        Err(error) => return Err(error),
    };
    if deleted {
        crate::auth::revoke_sessions_for(&state, user_id);
        return Ok(());
    }
    let conn = conn(&state);
    crate::sync::catalog::local(&conn, &actor, "app_users", |tx| {
        service::delete_user(tx, &actor, user_id, Some(current.version))
    })?;
    crate::auth::revoke_sessions_for(&state, user_id);
    Ok(())
}

#[tauri::command]
pub fn list_reservations(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<Reservation>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::list_reservations(&conn)
}

#[tauri::command]
pub fn list_history(state: State<AppState>, session_token: Option<String>, date: Option<String>) -> AppResult<Vec<HistoryStay>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::list_history(&conn, date)
}

#[tauri::command]
pub fn daily_report(state: State<AppState>, session_token: Option<String>, date: String) -> AppResult<DailyReport> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    crate::reports::daily_report(&conn(&state), &date)
}

#[tauri::command]
pub fn analytics_summary(
    state: State<AppState>,
    session_token: Option<String>,
    from: String,
    to: String,
    room_type: Option<String>,
) -> AppResult<AnalyticsSummary> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    crate::analytics::summary(&conn(&state), &from, &to, room_type.as_deref())
}

#[tauri::command]
pub fn save_analytics_pdf(
    state: State<AppState>,
    session_token: Option<String>,
    app: AppHandle,
    from: String,
    to: String,
    bytes: Vec<u8>,
) -> AppResult<String> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    crate::analytics::validate_range(&from, &to)?;
    if bytes.len() > 20_000_000 || !bytes.starts_with(b"%PDF-") || !bytes.ends_with(b"%%EOF\n") {
        return Err(AppError::msg("El archivo PDF no es válido o supera 20 MB"));
    }
    let dir = app_data_dir(&app)?.join("informes");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("analisis-{from}-a-{to}-{}.pdf", chrono::Local::now().format("%H%M%S-%f")));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
    if let Err(e) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(&path);
        return Err(e.into());
    }
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn save_daily_pdf(state: State<AppState>, session_token: Option<String>, app: AppHandle, date: String, bytes: Vec<u8>) -> AppResult<String> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    crate::reports::validate_date(&date)?;
    if bytes.len() > 20_000_000 || !bytes.starts_with(b"%PDF-") || !bytes.ends_with(b"%%EOF\n") {
        return Err(AppError::msg("El archivo PDF no es válido o supera 20 MB"));
    }
    let dir = app_data_dir(&app)?.join("informes");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("resumen-{date}-{}.pdf", chrono::Local::now().format("%H%M%S-%f")));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
    if let Err(e) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(&path);
        return Err(e.into());
    }
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn get_settings(state: State<AppState>, session_token: Option<String>) -> AppResult<AppSettings> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::get_settings(&conn)
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, session_token: Option<String>, app: AppHandle, mut payload: AppSettings, new_pin: Option<String>, operation_id: Option<String>) -> AppResult<AppSettings> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    if try_catalog(&state, &app, &actor_from(&user), "settings", serde_json::to_value(&payload).unwrap(), operation_id).await?.is_some() {
        let current = db::load_settings(&conn(&state))?;
        payload.business_name=current.business_name; payload.address=current.address; payload.phone=current.phone;
        payload.tax_percent=current.tax_percent; payload.currency_symbol=current.currency_symbol;
        payload.receipt_footer=current.receipt_footer; payload.require_guest_name=current.require_guest_name;
    }
    let conn = conn(&state);
    crate::sync::catalog::local(&conn, &actor_from(&user), "settings", |tx| service::save_settings(tx, &actor_from(&user), payload, new_pin))
}

#[tauri::command]
pub fn verify_pin(state: State<AppState>, session_token: Option<String>, pin: String) -> AppResult<bool> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::verify_pin(&conn, pin)
}

#[tauri::command]
pub fn pin_required(state: State<AppState>, session_token: Option<String>) -> AppResult<bool> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::pin_required(&conn)
}

#[tauri::command]
pub fn print_test(state: State<AppState>, session_token: Option<String>, app: AppHandle) -> AppResult<Option<String>> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    service::authorize(&actor_from(&user), service::Operation::PrintTest)?;
    let conn = conn(&state);
    let settings = db::load_settings(&conn)?;
    let bytes = printer::build_test_receipt(&settings);
    let data_dir = app_data_dir(&app)?;
    printer::print_bytes(&bytes, &settings, &data_dir, "test")
}

#[tauri::command]
pub fn list_printers(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<String>> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    service::authorize(&actor_from(&user), service::Operation::PrintTest)?;
    printer::list_printers()
}

#[tauri::command]
pub fn reprint_receipt(state: State<AppState>, session_token: Option<String>, app: AppHandle, stay_id: i64) -> AppResult<Option<String>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let settings = db::load_settings(&conn)?;
    let bytes = service::receipt_bytes(&conn, stay_id)?;
    let data_dir = app_data_dir(&app)?;
    printer::print_bytes(&bytes, &settings, &data_dir, &format!("reprint-{stay_id}"))
}

#[tauri::command]
pub fn device_mode_get(state: State<AppState>) -> AppResult<Option<String>> {
    Ok(state.device.lock().unwrap().mode.clone())
}

#[tauri::command]
pub fn device_mode_set(state: State<AppState>, app: AppHandle, payload: DeviceModeSetPayload) -> AppResult<()> {
    if !matches!(payload.mode.as_str(), "reception" | "reception_client" | "remote") { return Err(AppError::msg("Modo de equipo inválido")); }
    let mut config=state.device.lock().unwrap();
    if let Some(current)=&config.mode {
        if current==&payload.mode { return Ok(()); }
        return Err(AppError::forbidden("El equipo ya está configurado. No se puede reemplazar su base cambiando de modo"));
    }
    let mut next=config.clone(); next.mode=Some(payload.mode.clone());
    let connection=crate::device::operational_connection(&state.data_dir,Some(&payload.mode))?;
    if payload.mode!="reception_client" {
        service::device_mode_set(&connection,&payload.mode)?;
        next.printer=Some(db::load_settings(&connection)?);
    }
    crate::device::save(&state.data_dir,&next)?;
    *conn(&state)=connection; *config=next; drop(config);
    if payload.mode!="reception_client" {
        let _=crate::credentials::apply_embedded_defaults(&state.data_dir);
        if crate::credentials::device_configured(&state.data_dir).unwrap_or(false) { maybe_start_worker(&state,&app,&state.data_dir)?; }
        maybe_start_backup(&state,&app)?;
    }
    Ok(())
}

#[tauri::command]
pub fn remote_configure(app: AppHandle, payload: RemoteConfigurePayload) -> AppResult<()> {
    let data_dir = app_data_dir(&app)?;
    crate::credentials::save_remote(&data_dir, &payload.project_url, &payload.anon_key)
}

#[tauri::command]
pub fn remote_configured(app: AppHandle) -> AppResult<bool> {
    let data_dir = app_data_dir(&app)?;
    Ok(crate::credentials::remote_configured(&data_dir))
}

#[tauri::command]
pub fn remote_get_config(app: AppHandle) -> AppResult<Option<RemoteConfigurePayload>> {
    let data_dir = app_data_dir(&app)?;
    Ok(crate::credentials::load_remote(&data_dir)?.map(|(project_url, anon_key)| {
        RemoteConfigurePayload {
            project_url,
            anon_key,
        }
    }))
}

#[tauri::command]
pub fn remote_embedded_auth(app: AppHandle) -> AppResult<bool> {
    let data_dir = app_data_dir(&app)?;
    let _ = crate::credentials::apply_embedded_defaults(&data_dir);
    Ok(crate::credentials::remote_auth_configured(&data_dir))
}

#[tauri::command]
pub fn hash_password(payload: HashPasswordPayload, operation_id: Option<String>) -> AppResult<HashPasswordResult> {
    service::hash_password_with_operation(&payload.password, operation_id.as_deref())
}

#[tauri::command]
pub fn sync_status(state: State<AppState>, app: AppHandle) -> AppResult<SyncStatus> {
    let data_dir = app_data_dir(&app)?;
    let _ = crate::credentials::apply_embedded_defaults(&data_dir);
    let configured = crate::credentials::device_configured(&data_dir)?;
    let pending = crate::sync::push::pending_count(&conn(&state)).unwrap_or(0);
    let snapshot = state
        .sync
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|handle| handle.snapshot()))
        .unwrap_or_default();
    Ok(crate::sync::worker::status_from(
        &snapshot,
        pending,
        configured,
        crate::credentials::has_embedded_defaults(),
    ))
}

#[tauri::command]
pub fn sync_pull_now(state: State<AppState>) -> AppResult<()> {
    let handle = state
        .sync
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().cloned());
    match handle {
        Some(handle) => handle.pull_now(),
        None => Err(AppError::storage("Sincronización no configurada")),
    }
}

#[tauri::command]
pub fn sync_configure_device(state: State<AppState>, app: AppHandle, payload: SyncConfigureDevicePayload) -> AppResult<()> {
    if state.device.lock().unwrap().mode.as_deref()!=Some("reception"){return Err(AppError::forbidden("La sincronización se configura en recepción principal"));}
    let data_dir = app_data_dir(&app)?;
    crate::credentials::save_device(
        &data_dir,
        &payload.project_url,
        &payload.anon_key,
        &payload.device_email,
        &payload.device_password,
    )?;
    maybe_start_worker(&state, &app, &data_dir)
}

#[tauri::command]
pub fn backup_status(state: State<AppState>, session_token: Option<String>, app: AppHandle) -> AppResult<BackupStatus> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let data_dir = app_data_dir(&app)?;
    let conn = conn(&state);
    crate::backup::status(&conn, &data_dir)
}

#[tauri::command]
pub fn backup_run_now(state: State<AppState>, session_token: Option<String>, app: AppHandle) -> AppResult<BackupRunResult> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let mode = service::device_mode_get(&conn(&state))?;
    if mode.as_deref() != Some("reception") {
        return Err(AppError::forbidden("Los respaldos solo se generan en el equipo de recepción"));
    }
    maybe_start_backup(&state, &app)?;
    let guard = state.backup.lock().expect("backup lock");
    let handle = guard
        .as_ref()
        .ok_or_else(|| AppError::storage("Motor de respaldos no disponible"))?;
    handle.run_now()
}

#[tauri::command]
pub fn backup_list(state: State<AppState>, session_token: Option<String>, app: AppHandle) -> AppResult<Vec<BackupListItem>> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let data_dir = app_data_dir(&app)?;
    crate::backup::list_backups(&conn(&state), &data_dir)
}

#[tauri::command]
pub fn backup_import_key(state: State<AppState>, session_token: Option<String>, app: AppHandle, payload: BackupImportKeyPayload) -> AppResult<()> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let mode = service::device_mode_get(&conn(&state))?;
    if mode.as_deref() != Some("reception") {
        return Err(AppError::forbidden("La clave de respaldo solo se importa en el equipo de recepción"));
    }
    let data_dir = app_data_dir(&app)?;
    crate::credentials::import_backup_key(&data_dir, &payload.key_hex)
}

#[tauri::command]
pub fn backup_restore(state: State<AppState>, session_token: Option<String>, app: AppHandle, payload: BackupRestorePayload) -> AppResult<()> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let mode = service::device_mode_get(&conn(&state))?;
    if mode.as_deref() != Some("reception") {
        return Err(AppError::forbidden("La restauración solo se hace en el equipo de recepción"));
    }
    let data_dir = app_data_dir(&app)?;
    let db_path = data_dir.join("nightdesk.db");
    let source = payload.source.as_deref().unwrap_or("local");
    if state.device.lock().unwrap().lan_enabled {
        return Err(AppError::conflict("Pausá ambos puestos y detené la conexión en Puestos antes de restaurar"));
    }
    let _maintenance=crate::MAINTENANCE.blocking_write();
    // All business requests and background DB cycles are quiescent. Preserve lock order DB → auth.
    let mut slot=state.db.lock().expect("db lock");
    let mut auth=state.auth.lock().expect("auth lock");
    // Persist the generation BEFORE the restore, even if it later fails. An uncertain old
    // request must never be replayed against a snapshot that might omit its original result.
    {
        let mut config=state.device.lock().unwrap();
        let mut next=config.clone(); next.database_generation=uuid::Uuid::new_v4().to_string();
        crate::device::save(&data_dir,&next)?; *config=next;
    }
    crate::backup::restore_backup(&db_path, &data_dir, &payload.backup_id, source, &mut auth)?;
    *slot=crate::db::open(&db_path)?;
    state.lan.session_stations.lock().unwrap().clear();
    *state.lan.epoch.lock().unwrap()=uuid::Uuid::new_v4().to_string();
    drop(auth); drop(slot);
    wake_push(&state);
    Ok(())
}

fn maybe_start_backup(state: &AppState, app: &AppHandle) -> AppResult<()> {
    let data_dir = app_data_dir(app)?;
    let mode = service::device_mode_get(&conn(state))?;
    if !crate::backup::should_start(mode.as_deref()) {
        return Ok(());
    }
    let mut slot = state.backup.lock().expect("backup lock");
    if slot.is_none() {
        *slot = Some(crate::backup::start(
            app.clone(),
            data_dir.join("nightdesk.db"),
            data_dir,
        )?);
    }
    Ok(())
}

#[tauri::command]
pub async fn app_update_check(app: AppHandle) -> AppResult<Option<AppUpdateInfo>> {
    crate::updater::check(&app).await
}

#[tauri::command]
pub async fn app_update_install(
    app: AppHandle,
    on_progress: tauri::ipc::Channel<AppUpdateProgress>,
) -> AppResult<()> {
    if app.state::<AppState>().device.lock().unwrap().lan_enabled {
        return Err(AppError::conflict("Pausá ambos puestos y detené la conexión en Puestos antes de actualizar"));
    }
    crate::updater::install(&app, on_progress).await
}

#[tauri::command]
pub fn list_product_stock(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<ProductStock>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    crate::stock::list(&conn)
}

#[tauri::command]
pub fn list_stock_movements(state: State<AppState>, session_token: Option<String>, product_id: i64) -> AppResult<Vec<StockMovement>> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let conn = conn(&state);
    crate::stock::movements(&conn, product_id, 30)
}

#[tauri::command]
pub fn list_price_rules(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<PriceRule>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    crate::pricing::load(&conn)
}

#[tauri::command]
pub fn current_prices(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<EffectivePrice>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    crate::pricing::effective_prices(&conn, chrono::Local::now())
}
