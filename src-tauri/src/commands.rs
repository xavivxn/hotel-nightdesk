use crate::db;
use crate::error::{AppError, AppResult};
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

pub(crate) async fn try_catalog(state: &AppState, app: &AppHandle, actor: &Actor, entity: &str, payload: serde_json::Value, operation_id: Option<String>) -> AppResult<Option<i64>> {
    let dir = app_data_dir(app)?;
    if !crate::sync::catalog::configured(&dir) { return Ok(None); }
    let request = crate::sync::catalog::prepare(&conn(state), actor, entity, payload, operation_id)?;
    if entity == "settings" && request["p_payload"]["values"].as_object().is_some_and(|v| v.is_empty()) { return Ok(Some(0)); }
    let remote = crate::sync::remote::SupabaseRemote::load(&dir)?;
    // Do not retain the database mutex during authentication/network IO.
    let remote_request=request.clone();
    let result = tauri::async_runtime::spawn_blocking(move || crate::sync::catalog::execute(&remote, &remote_request)).await.map_err(|_| AppError::storage("No se pudo completar la operación remota"))??;
    let id = crate::sync::catalog::apply_result(&mut conn(state), &request, &result)?;
    Ok(Some(id))
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
pub fn set_room_status(state: State<AppState>, session_token: Option<String>, room_id: i64, status: String) -> AppResult<Room> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    service::set_room_status(&mut conn, room_id, status)
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
pub fn check_in(state: State<AppState>, session_token: Option<String>, payload: CheckInPayload) -> AppResult<Stay> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    service::check_in_on(&mut conn, payload)
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
pub fn convert_to_overnight(state: State<AppState>, session_token: Option<String>, stay_id: i64) -> AppResult<Stay> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    service::convert_to_overnight(&mut conn, stay_id)
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
pub fn add_charge(state: State<AppState>, session_token: Option<String>, payload: AddChargePayload) -> AppResult<Charge> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    let mut conn = conn(&state);
    service::add_charge(&mut conn, &actor_from(&user), payload)
}

#[tauri::command]
pub fn add_product_charge(state: State<AppState>, session_token: Option<String>, payload: AddProductChargePayload) -> AppResult<Charge> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    service::add_product_charge(&mut conn, payload)
}

#[tauri::command]
pub fn delete_charge(state: State<AppState>, session_token: Option<String>, charge_id: i64) -> AppResult<()> {
    let user = crate::auth::require(&state, session_token.as_deref(), true)?;
    let mut conn = conn(&state);
    service::delete_charge(&mut conn, &actor_from(&user), charge_id)
}

#[tauri::command]
pub fn check_out(
    state: State<AppState>,
    session_token: Option<String>,
    app: AppHandle,
    payload: CheckOutPayload,
) -> AppResult<CheckOutResult> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    let (stay, bill) = service::check_out(&mut conn, &payload)?;
    let mut print_error = None;
    if payload.print {
        let attempt = (|| -> AppResult<Option<String>> {
            let settings = db::load_settings(&conn)?;
            let bytes = service::receipt_bytes(&conn, stay.id)?;
            let data_dir = app_data_dir(&app)?;
            printer::print_bytes(&bytes, &settings, &data_dir, &format!("stay-{}", stay.id))
        })();
        print_error = attempt.unwrap_or_else(|e| Some(e.to_string()));
    }
    Ok(CheckOutResult {
        stay,
        bill,
        print_error,
    })
}

#[tauri::command]
pub fn list_reservations(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<Reservation>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    service::list_reservations(&conn)
}

#[tauri::command]
pub fn create_reservation(state: State<AppState>, session_token: Option<String>, payload: CreateReservationPayload) -> AppResult<Reservation> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    service::create_reservation_on(&mut conn, payload)
}

#[tauri::command]
pub fn set_reservation_status(state: State<AppState>, session_token: Option<String>, reservation_id: i64, status: String) -> AppResult<Reservation> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    service::set_reservation_status(&mut conn, reservation_id, status)
}

#[tauri::command]
pub fn check_in_reservation(state: State<AppState>, session_token: Option<String>, reservation_id: i64) -> AppResult<Stay> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    service::check_in_reservation(&mut conn, reservation_id)
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
    let conn = conn(&state);
    service::device_mode_get(&conn)
}

#[tauri::command]
pub fn device_mode_set(state: State<AppState>, payload: DeviceModeSetPayload) -> AppResult<()> {
    let conn = conn(&state);
    service::device_mode_set(&conn, &payload.mode)
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
pub fn hash_password(payload: HashPasswordPayload, operation_id: Option<String>) -> AppResult<HashPasswordResult> {
    service::hash_password_with_operation(&payload.password, operation_id.as_deref())
}

#[tauri::command]
pub fn sync_status(app: AppHandle) -> AppResult<SyncStatus> {
    let data_dir = app_data_dir(&app)?;
    Ok(service::sync_status_stub(&data_dir))
}

#[tauri::command]
pub fn sync_pull_now() -> AppResult<()> {
    // Stub until I07 worker exists.
    Ok(())
}

#[tauri::command]
pub fn sync_configure_device(app: AppHandle, payload: SyncConfigureDevicePayload) -> AppResult<()> {
    let data_dir = app_data_dir(&app)?;
    crate::credentials::save_device(
        &data_dir,
        &payload.project_url,
        &payload.anon_key,
        &payload.device_email,
        &payload.device_password,
    )
}

#[tauri::command]
pub fn backup_status() -> AppResult<BackupStatus> {
    Ok(BackupStatus {
        last_local_at: None,
        last_remote_at: None,
        pending: 0,
        last_error: Some("Respaldos pendientes de I08".into()),
    })
}
