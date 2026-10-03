//! Explicit typed command allowlist shared by desktop IPC and LAN.
use crate::{
    auth, commands,
    error::{AppError, AppResult},
    operations::{self, encode, field},
    service, AppState,
};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

pub async fn dispatch(
    app: &AppHandle,
    station: &str,
    command: &str,
    args: Value,
    token: Option<String>,
) -> AppResult<Value> {
    let _maintenance = crate::MAINTENANCE.read().await;
    let state = app.state::<AppState>();
    if state.device.lock().unwrap().mode.as_deref() != Some("reception") {
        return Err(AppError::forbidden(
            "Este equipo no es la recepción principal",
        ));
    }
    if operations::MUTATIONS.contains(&command) {
        {
            let config = state.device.lock().unwrap();
            if station != config.station_id
                && args["_database_generation"].as_str()
                    != Some(config.database_generation.as_str())
            {
                return Err(AppError::new(crate::error::ErrorCode::RecoveryRequired,"La base principal fue restaurada. Esta solicitud requiere revisión manual; no se repetirá sobre otra base"));
            }
        }
        let handle = app.clone();
        let station = station.to_string();
        let command = command.to_string();
        return tauri::async_runtime::spawn_blocking(move || {
            let state = handle.state::<AppState>();
            let result = operations::execute(&state, token.as_deref(), &station, &command, args)?;
            crate::lan::changed(&handle);
            if let Some(worker) = state.sync.lock().unwrap().as_ref() {
                worker.wake_push();
            }
            Ok(result)
        })
        .await
        .map_err(|_| AppError::storage("No se pudo ejecutar la operación"))?;
    }
    // Catalog writes preserve the existing write-through policy and shared adapters.
    match command {
        "auth_login" => {
            let handle = app.clone();
            let payload = field(&args, "payload")?;
            let mut value = tauri::async_runtime::spawn_blocking(move || {
                encode(auth::auth_login(handle.state(), handle.clone(), payload)?)
            })
            .await
            .map_err(|_| AppError::storage("No se pudo iniciar sesión"))??;
            value["actor_uid"] = json!(crate::db::uid_of(
                &state.db.lock().unwrap(),
                "users",
                field(&value["user"], "id")?
            )?);
            value["database_generation"] = json!(state.device.lock().unwrap().database_generation);
            return Ok(value);
        }
        "auth_session" => return encode(auth::auth_session(app.state(), token)?),
        "auth_logout" => return encode(auth::auth_logout(app.state(), token)?),
        "auth_setup_required" => return encode(auth::auth_setup_required(app.state())?),
        "auth_create_user" => {
            return encode(
                auth::auth_create_user(
                    app.state(),
                    token,
                    app.clone(),
                    field(&args, "payload")?,
                    field(&args, "operation_id")?,
                )
                .await?,
            )
        }
        "save_room" => {
            return encode(
                commands::save_room(app.state(), token, app.clone(), field(&args, "payload")?)
                    .await?,
            )
        }
        "save_rate_plan" => {
            return encode(
                commands::save_rate_plan(app.state(), token, app.clone(), field(&args, "payload")?)
                    .await?,
            )
        }
        "save_product" => {
            return encode(
                commands::save_product(app.state(), token, app.clone(), field(&args, "payload")?)
                    .await?,
            )
        }
        "set_product_active" => {
            return encode(
                commands::set_product_active(
                    app.state(),
                    token,
                    app.clone(),
                    field(&args, "product_id")?,
                    field(&args, "active")?,
                    field(&args, "operation_id")?,
                    field(&args, "expected_version")?,
                )
                .await?,
            )
        }
        "set_user_active" => {
            return encode(
                commands::set_user_active(
                    app.state(),
                    token,
                    app.clone(),
                    field(&args, "user_id")?,
                    field(&args, "active")?,
                    field(&args, "operation_id")?,
                    field(&args, "expected_version")?,
                )
                .await?,
            )
        }
        "delete_user" => {
            return encode(
                commands::delete_user(
                    app.state(),
                    token,
                    app.clone(),
                    field(&args, "user_id")?,
                    field(&args, "operation_id")?,
                    field(&args, "expected_version")?,
                )
                .await?,
            )
        }
        "save_settings" => {
            // Never let another PC replace the main PC's printer or PIN.
            let mut p: crate::models::AppSettings = field(&args, "payload")?;
            let local = state.device.lock().unwrap().station_id == station;
            if !local {
                let current = service::get_settings(&state.db.lock().unwrap())?;
                p.printer_name = current.printer_name;
                p.printer_path = current.printer_path;
                p.printer_enabled = current.printer_enabled;
                p.paper_width = current.paper_width;
                p.auto_print_on_checkout = current.auto_print_on_checkout;
                p.theme = current.theme;
            }
            return encode(
                commands::save_settings(
                    app.state(),
                    token,
                    app.clone(),
                    p,
                    if local {
                        field(&args, "new_pin")?
                    } else {
                        None
                    },
                    field(&args, "operation_id")?,
                )
                .await?,
            );
        }
        "receipt_print_host" => {
            let handle = app.clone();
            return tauri::async_runtime::spawn_blocking(move || {
                crate::printing::host_print(&handle, token.as_deref(), args)
            })
            .await
            .map_err(|_| AppError::storage("No se pudo imprimir"))?;
        }
        "sync_status" => {
            auth::require(&state, token.as_deref(), false)?;
            return encode(commands::sync_status(app.state(), app.clone())?);
        }
        _ => {}
    }
    let user = auth::require(&state, token.as_deref(), false)?;
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::storage("Base no disponible"))?;
    let actor = service::Actor::from(&user);
    match command {
        "contract_info" => encode(service::contract_info(&conn)?),
        "list_board" => {
            let mut rows = encode(service::list_board(&conn)?)?;
            for row in rows.as_array_mut().unwrap() {
                let id = field(&row["room"], "id")?;
                if let Some(id) = row["reservation"]["id"].as_i64() {
                    row["reservation"]["operational_version"] =
                        json!(operations::entity_version(&conn, "reservations", id)?);
                }
                row["room"]["operational_version"] =
                    json!(operations::entity_version(&conn, "rooms", id)?);
            }
            Ok(rows)
        }
        "list_rooms" => {
            let mut rows = encode(service::list_rooms(&conn)?)?;
            for row in rows.as_array_mut().unwrap() {
                row["operational_version"] = json!(operations::entity_version(
                    &conn,
                    "rooms",
                    field(row, "id")?
                )?);
            }
            Ok(rows)
        }
        "list_rate_plans" => encode(crate::db::list_rate_plans(
            &conn,
            args["active_only"].as_bool().unwrap_or(false),
        )?),
        "preview_bill" => encode(service::preview_bill(&conn, field(&args, "stay_id")?)?),
        "account_quote" => encode(operations::quote(&conn, field(&args, "stay_id")?)?),
        "get_stay_detail" => encode(service::get_stay_detail(&conn, field(&args, "stay_id")?)?),
        "list_products" => encode(crate::db::list_products(
            &conn,
            args["active_only"].as_bool().unwrap_or(true),
        )?),
        "list_users" => encode(service::list_users(&conn, &actor)?),
        "list_reservations" => {
            let mut rows = encode(service::list_reservations(&conn)?)?;
            for row in rows.as_array_mut().unwrap() {
                row["operational_version"] = json!(operations::entity_version(
                    &conn,
                    "reservations",
                    field(row, "id")?
                )?);
            }
            Ok(rows)
        }
        "list_history" => encode(service::list_history(&conn, field(&args, "date")?)?),
        "daily_report" => encode(crate::reports::daily_report(
            &conn,
            &field::<String>(&args, "date")?,
        )?),
        "analytics_summary" => {
            if !user.role.is_admin() {
                return Err(AppError::forbidden(
                    "Esta operación requiere administración",
                ));
            }
            encode(crate::analytics::summary(
                &conn,
                &field::<String>(&args, "from")?,
                &field::<String>(&args, "to")?,
                args["room_type"].as_str(),
            )?)
        }
        "get_settings" => encode(service::get_settings(&conn)?),
        "verify_pin" => encode(service::verify_pin(&conn, field(&args, "pin")?)?),
        "pin_required" => encode(service::pin_required(&conn)?),
        "list_product_stock" => encode(crate::stock::list(&conn)?),
        "list_stock_movements" => {
            if !user.role.is_admin() {
                return Err(AppError::forbidden(
                    "Esta operación requiere administración",
                ));
            }
            encode(crate::stock::movements(
                &conn,
                field(&args, "product_id")?,
                30,
            )?)
        }
        "list_price_rules" => encode(crate::pricing::load(&conn)?),
        "current_prices" => encode(crate::pricing::effective_prices(
            &conn,
            chrono::Local::now(),
        )?),
        "operation_result" => {
            if let Some(generation) = args["_database_generation"].as_str() {
                if generation != state.device.lock().unwrap().database_generation {
                    return Err(AppError::new(crate::error::ErrorCode::RecoveryRequired,"La base principal fue restaurada; revisá manualmente esta operación antes de continuar"));
                }
            }
            encode(operations::result(
                &conn,
                &user,
                station,
                &field::<String>(&args, "operation_id")?,
            )?)
        }
        "operator_activity" => operations::activity(
            &conn,
            &user,
            &field::<String>(&args, "from")?,
            &field::<String>(&args, "to")?,
            args["user_uid"].as_str(),
        ),
        "receipt_document" => crate::printing::document(&conn, field(&args, "stay_id")?),
        "backup_status" => encode(crate::backup::status(&conn, &state.data_dir)?),
        "lan_revision" => Ok(
            json!({"epoch":state.lan.epoch.lock().unwrap().clone(),"revision":operations::revision(&conn)?}),
        ),
        _ => Err(AppError::forbidden(
            "Este comando no está disponible por la conexión local",
        )),
    }
}
