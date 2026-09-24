mod billing;
mod auth;
mod backup;
mod commands;
mod credentials;
mod db;
mod error;
mod models;
mod printer;
mod service;
mod reports;
mod sync;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::{LogicalSize, Manager};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub auth: Mutex<auth::AuthState>,
    pub sync: Mutex<Option<sync::worker::SyncHandle>>,
    pub backup: Mutex<Option<backup::BackupHandle>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("nightdesk.db");
            let conn = db::open(&db_path).map_err(|e| e.to_string())?;
            let mode = service::device_mode_get(&conn).ok().flatten();
            // A vault failure must not block offline reception. sync_status and
            // catalog operations will surface the underlying error to the UI.
            let configured = credentials::device_configured(&dir).unwrap_or(false);
            let sync = if sync::worker::should_start(mode.as_deref(), configured) {
                sync::worker::start(app.handle().clone(), db_path.clone(), dir.clone()).ok()
            } else {
                None
            };
            let backup = if backup::should_start(mode.as_deref()) {
                Some(backup::start(app.handle().clone(), db_path.clone(), dir.clone()).map_err(|e| e.to_string())?)
            } else {
                None
            };
            app.manage(AppState {
                db: Mutex::new(conn),
                auth: Mutex::new(auth::AuthState::default()),
                sync: Mutex::new(sync),
                backup: Mutex::new(backup),
            });
            // Force window/taskbar icon (bundle icons alone often stay cached in `tauri dev` on Windows).
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                    .map_err(|e| e.to_string())?;
                let _ = window.set_icon(icon);
                if let Some(name) = app.config().product_name.as_deref() {
                    let _ = window.set_title(name);
                }
                let _ = window.set_fullscreen(false);
                let _ = window.unmaximize();
                let _ = window.set_size(LogicalSize::new(1680.0, 1050.0));
                let _ = window.center();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::auth_setup_required,
            auth::auth_setup,
            auth::auth_create_user,
            auth::auth_login,
            auth::auth_session,
            auth::auth_logout,
            commands::contract_info,
            commands::list_board,
            commands::list_rooms,
            commands::save_room,
            commands::set_room_status,
            commands::list_rate_plans,
            commands::save_rate_plan,
            commands::check_in,
            commands::preview_bill,
            commands::get_stay_detail,
            commands::convert_to_overnight,
            commands::list_products,
            commands::save_product,
            commands::set_product_active,
            commands::add_charge,
            commands::add_product_charge,
            commands::delete_charge,
            commands::check_out,
            commands::list_reservations,
            commands::create_reservation,
            commands::set_reservation_status,
            commands::check_in_reservation,
            commands::list_history,
            commands::daily_report,
            commands::save_daily_pdf,
            commands::get_settings,
            commands::save_settings,
            commands::verify_pin,
            commands::pin_required,
            commands::print_test,
            commands::list_printers,
            commands::reprint_receipt,
            commands::device_mode_get,
            commands::device_mode_set,
            commands::remote_configure,
            commands::remote_configured,
            commands::remote_get_config,
            commands::hash_password,
            commands::sync_status,
            commands::sync_pull_now,
            commands::sync_configure_device,
            commands::backup_status,
            commands::backup_run_now,
            commands::backup_list,
            commands::backup_import_key,
            commands::backup_restore,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nightdesk");
}
