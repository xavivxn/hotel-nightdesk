mod billing;
mod auth;
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
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub auth: Mutex<auth::AuthState>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("nightdesk.db");
            let conn = db::open(&db_path).map_err(|e| e.to_string())?;
            app.manage(AppState {
                db: Mutex::new(conn),
                auth: Mutex::new(auth::AuthState::default()),
            });
            // Force window/taskbar icon (bundle icons alone often stay cached in `tauri dev` on Windows).
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))
                    .map_err(|e| e.to_string())?;
                let _ = window.set_icon(icon);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nightdesk");
}
