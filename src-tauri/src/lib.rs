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
mod analytics;
mod pricing;
mod stock;
mod sync;
mod updater;
mod window_fit;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub auth: Mutex<auth::AuthState>,
    pub sync: Mutex<Option<sync::worker::SyncHandle>>,
    pub backup: Mutex<Option<backup::BackupHandle>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin: a second launch (desktop icon while the app already started
        // with Windows) focuses this window instead of opening another copy over the same SQLite.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                window_fit::ensure_visible(&window);
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&dir)?;
            let _ = credentials::apply_embedded_defaults(&dir);
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
                // The window starts hidden (tauri.conf.json) and is shown once it fits the
                // monitor, so it never flashes partly off-screen.
                window_fit::place_initial(&window);
                let _ = window.show();
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
            commands::list_users,
            commands::set_user_active,
            commands::delete_user,
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
            commands::analytics_summary,
            commands::save_analytics_pdf,
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
            commands::remote_embedded_auth,
            commands::hash_password,
            commands::sync_status,
            commands::sync_pull_now,
            commands::sync_configure_device,
            commands::backup_status,
            commands::backup_run_now,
            commands::backup_list,
            commands::backup_import_key,
            commands::backup_restore,
            commands::app_update_check,
            commands::app_update_install,
            commands::list_product_stock,
            commands::update_product_stock,
            commands::list_stock_movements,
            commands::list_price_rules,
            commands::save_price_rules,
            commands::current_prices,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nightdesk");
}
