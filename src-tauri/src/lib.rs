mod billing;
mod commands;
mod db;
mod error;
mod models;
mod printer;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Connection>,
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
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            commands::add_charge,
            commands::add_product_charge,
            commands::delete_charge,
            commands::check_out,
            commands::list_reservations,
            commands::create_reservation,
            commands::set_reservation_status,
            commands::check_in_reservation,
            commands::list_history,
            commands::get_settings,
            commands::save_settings,
            commands::verify_pin,
            commands::pin_required,
            commands::print_test,
            commands::reprint_receipt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nightdesk");
}
