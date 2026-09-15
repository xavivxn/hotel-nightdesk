use crate::billing::{self, BillingContext};
use crate::db::{self, now_rfc3339};
use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::printer;
use crate::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

fn conn(state: &AppState) -> std::sync::MutexGuard<'_, Connection> {
    state.db.lock().expect("db lock")
}

fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::msg(e.to_string()))
}

fn build_preview(conn: &Connection, stay: &Stay) -> AppResult<BillPreview> {
    let settings = db::load_settings(conn)?;
    let rate = db::get_rate_plan(conn, stay.rate_plan_id)?;
    let overnight = if let Some(id) = stay.overnight_rate_plan_id {
        db::get_rate_plan(conn, id).ok()
    } else {
        db::find_plan_by_kind(conn, RateKind::Overnight)?
    };
    let night = db::find_plan_by_kind(conn, RateKind::Night)?;
    let check_in = db::parse_dt(&stay.check_in_at)?;
    let now = stay
        .check_out_at
        .as_deref()
        .map(db::parse_dt)
        .transpose()?
        .unwrap_or_else(chrono::Local::now);
    let manual_lines = db::list_charges(conn, stay.id)?
        .into_iter()
        .filter(|c| c.kind == "surcharge" || c.kind == "discount")
        .map(|c| LineItem {
            kind: c.kind,
            description: c.description,
            amount_cents: c.amount_cents,
        })
        .collect();

    Ok(billing::preview(BillingContext {
        stay_id: stay.id,
        check_in_at: check_in,
        now,
        rate: &rate,
        converted_to_overnight: stay.converted_to_overnight,
        overnight_plan: overnight.as_ref(),
        night_plan: night.as_ref(),
        manual_lines,
        tax_percent: settings.tax_percent,
    }))
}

/// Once an account is closed, its persisted charge lines are the historical
/// source of truth. Future rate or tax edits must not rewrite that history.
fn bill_for_stay(conn: &Connection, stay: &Stay) -> AppResult<BillPreview> {
    if stay.status != "closed" {
        return build_preview(conn, stay);
    }
    let charges = db::list_charges(conn, stay.id)?;
    let has_persisted_lines = charges
        .iter()
        .any(|charge| matches!(charge.kind.as_str(), "stay" | "extra_hour" | "tax"));
    if !has_persisted_lines {
        return build_preview(conn, stay);
    }
    let tax_cents = charges
        .iter()
        .filter(|charge| charge.kind == "tax")
        .map(|charge| charge.amount_cents)
        .sum();
    let snapshot = db::closed_snapshot(conn, stay.id)?;
    let tax_percent = snapshot.tax_percent.unwrap_or_else(|| {
        charges
            .iter()
            .find(|charge| charge.kind == "tax")
            .and_then(|charge| charge.description.strip_prefix("IVA "))
            .and_then(|value| value.trim_end_matches('%').parse::<f64>().ok())
            .unwrap_or(0.0)
    });
    let lines: Vec<LineItem> = charges
        .into_iter()
        .filter(|charge| charge.kind != "tax")
        .map(|charge| LineItem {
            kind: charge.kind,
            description: charge.description,
            amount_cents: charge.amount_cents,
        })
        .collect();
    let subtotal_cents = lines.iter().map(|line| line.amount_cents).sum();
    Ok(BillPreview {
        stay_id: stay.id,
        lines,
        subtotal_cents,
        tax_percent,
        tax_cents,
        total_cents: subtotal_cents + tax_cents,
        applied_kind: snapshot
            .applied_kind
            .as_deref()
            .map(RateKind::parse)
            .unwrap_or(stay.rate_kind),
        duration_label: snapshot
            .duration_label
            .unwrap_or_else(|| "Cuenta cerrada".into()),
        overnight_applied: stay.converted_to_overnight,
    })
}

fn persist_computed_charges(conn: &Connection, bill: &BillPreview) -> AppResult<()> {
    conn.execute(
        "DELETE FROM charges WHERE stay_id = ?1 AND kind IN ('stay', 'extra_hour', 'tax')",
        [bill.stay_id],
    )?;
    let now = now_rfc3339();
    for line in &bill.lines {
        if line.kind == "surcharge" || line.kind == "discount" {
            continue;
        }
        conn.execute(
            "INSERT INTO charges (stay_id, kind, description, amount_cents, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![bill.stay_id, line.kind, line.description, line.amount_cents, now],
        )?;
    }
    if bill.tax_cents != 0 {
        conn.execute(
            "INSERT INTO charges (stay_id, kind, description, amount_cents, created_at) VALUES (?1, 'tax', ?2, ?3, ?4)",
            params![
                bill.stay_id,
                format!("IVA {}%", bill.tax_percent),
                bill.tax_cents,
                now
            ],
        )?;
    }
    Ok(())
}

fn close_account(
    conn: &mut Connection,
    stay: &Stay,
    bill: &BillPreview,
    checkout_at: &str,
) -> AppResult<()> {
    let tx = conn.transaction()?;
    persist_computed_charges(&tx, bill)?;
    tx.execute(
        "UPDATE stays SET status = 'closed', check_out_at = ?1,
                closed_applied_kind = ?2, closed_tax_percent = ?3, closed_duration_label = ?4
         WHERE id = ?5 AND status = 'open'",
        params![
            checkout_at,
            bill.applied_kind.as_str(),
            bill.tax_percent,
            bill.duration_label,
            stay.id
        ],
    )?;
    if tx.changes() != 1 {
        return Err(AppError::msg("La estadía ya está cerrada"));
    }
    tx.execute(
        "UPDATE rooms SET status = 'dirty' WHERE id = ?1",
        [stay.room_id],
    )?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub fn list_board(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<BoardRoom>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let mut stmt = conn.prepare(
        "SELECT id, number, room_type, floor, status, notes, active FROM rooms WHERE active = 1 ORDER BY floor, number",
    )?;
    let rooms: Vec<Room> = stmt
        .query_map([], |row| {
            Ok(Room {
                id: row.get(0)?,
                number: row.get(1)?,
                room_type: row.get(2)?,
                floor: row.get(3)?,
                status: row.get(4)?,
                notes: row.get(5)?,
                active: row.get::<_, i64>(6)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut board = Vec::with_capacity(rooms.len());
    for room in rooms {
        let stay = db::open_stay_for_room(&conn, room.id)?;
        let reservation = if stay.is_none() {
            db::today_hold_for_room(&conn, room.id)?
        } else {
            None
        };
        let display_status = if stay.is_some() {
            "occupied".to_string()
        } else if room.status == "blocked" {
            "blocked".to_string()
        } else if room.status == "dirty" {
            "dirty".to_string()
        } else if reservation.is_some() {
            "reserved".to_string()
        } else {
            "available".to_string()
        };

        let mut estimated_total_cents = None;
        let mut elapsed_minutes = None;
        if let Some(stay_ref) = &stay {
            if let Ok(bill) = bill_for_stay(&conn, stay_ref) {
                estimated_total_cents = Some(bill.total_cents);
            }
            if let Ok(check_in) = db::parse_dt(&stay_ref.check_in_at) {
                elapsed_minutes = Some(billing::elapsed_minutes_now(check_in, chrono::Local::now()));
            }
        }

        board.push(BoardRoom {
            room,
            display_status,
            stay,
            reservation,
            estimated_total_cents,
            elapsed_minutes,
        });
    }
    Ok(board)
}

#[tauri::command]
pub fn list_rooms(state: State<AppState>, session_token: Option<String>) -> AppResult<Vec<Room>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let mut stmt = conn.prepare(
        "SELECT id, number, room_type, floor, status, notes, active FROM rooms WHERE active = 1 ORDER BY floor, number",
    )?;
    let rooms = stmt
        .query_map([], |row| {
            Ok(Room {
                id: row.get(0)?,
                number: row.get(1)?,
                room_type: row.get(2)?,
                floor: row.get(3)?,
                status: row.get(4)?,
                notes: row.get(5)?,
                active: row.get::<_, i64>(6)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rooms)
}

#[tauri::command]
pub fn save_room(state: State<AppState>, session_token: Option<String>, payload: SaveRoomPayload) -> AppResult<Room> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let conn = conn(&state);
    if payload.number.trim().is_empty() {
        return Err(AppError::msg("El número de habitación es obligatorio"));
    }
    if let Some(id) = payload.id {
        conn.execute(
            "UPDATE rooms SET number = ?1, room_type = ?2, floor = ?3, notes = ?4 WHERE id = ?5",
            params![
                payload.number.trim(),
                payload.room_type.trim(),
                payload.floor,
                payload.notes,
                id
            ],
        )?;
        db::get_room(&conn, id)
    } else {
        conn.execute(
            "INSERT INTO rooms (number, room_type, floor, status, notes, created_at) VALUES (?1, ?2, ?3, 'available', ?4, ?5)",
            params![
                payload.number.trim(),
                payload.room_type.trim(),
                payload.floor,
                payload.notes,
                now_rfc3339()
            ],
        )?;
        db::get_room(&conn, conn.last_insert_rowid())
    }
}

#[tauri::command]
pub fn set_room_status(state: State<AppState>, session_token: Option<String>, room_id: i64, status: String) -> AppResult<Room> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let allowed = ["available", "dirty", "blocked"];
    if !allowed.contains(&status.as_str()) {
        return Err(AppError::msg("Estado de habitación inválido"));
    }
    let conn = conn(&state);
    let room = db::get_room(&conn, room_id)?;
    if !room.active {
        return Err(AppError::msg("La habitación ya no está habilitada"));
    }
    if db::open_stay_for_room(&conn, room_id)?.is_some() {
        return Err(AppError::msg("No se puede cambiar el estado de una habitación ocupada"));
    }
    db::set_room_status(&conn, room_id, &status)?;
    db::get_room(&conn, room_id)
}

#[tauri::command]
pub fn list_rate_plans(state: State<AppState>, session_token: Option<String>, active_only: Option<bool>) -> AppResult<Vec<RatePlan>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    db::list_rate_plans(&conn, active_only.unwrap_or(false))
}

#[tauri::command]
pub fn save_rate_plan(state: State<AppState>, session_token: Option<String>, payload: SaveRatePlanPayload) -> AppResult<RatePlan> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let conn = conn(&state);
    if payload.name.trim().is_empty() {
        return Err(AppError::msg("El nombre de la tarifa es obligatorio"));
    }
    let active = if payload.active { 1 } else { 0 };
    if let Some(id) = payload.id {
        conn.execute(
            "UPDATE rate_plans SET name=?1, kind=?2, base_amount_cents=?3, extra_hour_cents=?4,
             included_hours=?5, grace_minutes=?6, night_cutoff_hour=?7, active=?8 WHERE id=?9",
            params![
                payload.name.trim(),
                payload.kind.as_str(),
                payload.base_amount_cents,
                payload.extra_hour_cents,
                payload.included_hours.max(1),
                payload.grace_minutes.max(0),
                payload.night_cutoff_hour.clamp(0, 23),
                active,
                id
            ],
        )?;
        db::get_rate_plan(&conn, id)
    } else {
        conn.execute(
            "INSERT INTO rate_plans (name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                payload.name.trim(),
                payload.kind.as_str(),
                payload.base_amount_cents,
                payload.extra_hour_cents,
                payload.included_hours.max(1),
                payload.grace_minutes.max(0),
                payload.night_cutoff_hour.clamp(0, 23),
                active
            ],
        )?;
        db::get_rate_plan(&conn, conn.last_insert_rowid())
    }
}

#[tauri::command]
pub fn check_in(state: State<AppState>, session_token: Option<String>, payload: CheckInPayload) -> AppResult<Stay> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    check_in_on(&mut conn, payload)
}

fn check_in_on(conn: &mut Connection, payload: CheckInPayload) -> AppResult<Stay> {
    let tx = conn.transaction()?;
    let stay_id = check_in_in_tx(&tx, payload)?;
    tx.commit()?;
    db::get_stay(conn, stay_id)
}

#[cfg(test)]
fn check_in_on_failing(conn: &mut Connection, payload: CheckInPayload) -> AppResult<Stay> {
    let tx = conn.transaction()?;
    let _stay_id = check_in_in_tx(&tx, payload)?;
    Err(AppError::msg("fallo inyectado"))
}

fn check_in_in_tx(conn: &Connection, payload: CheckInPayload) -> AppResult<i64> {
    let room = db::get_room(conn, payload.room_id)?;
    if !room.active {
        return Err(AppError::msg("La habitación ya no está habilitada"));
    }
    if db::open_stay_for_room(conn, room.id)?.is_some() {
        return Err(AppError::msg("La habitación ya está ocupada"));
    }
    if room.status == "blocked" {
        return Err(AppError::msg("La habitación está bloqueada"));
    }
    if room.status == "dirty" {
        return Err(AppError::msg("La habitación necesita limpieza antes del check-in"));
    }
    if let Some(hold) = db::today_hold_for_room(conn, room.id)? {
        if payload.reservation_id != Some(hold.id) {
            return Err(AppError::msg(format!(
                "La habitación {} tiene una reserva para hoy",
                room.number
            )));
        }
    }
    let rate = db::get_rate_plan(conn, payload.rate_plan_id)?;
    if !rate.active {
        return Err(AppError::msg("La tarifa no está activa"));
    }

    let (guest_id, reservation_id) = if let Some(res_id) = payload.reservation_id {
        let res = db::get_reservation(conn, res_id)?;
        if res.status != "hold" {
            return Err(AppError::msg("La reserva ya no está vigente"));
        }
        if res.room_id != room.id || res.rate_plan_id != rate.id {
            return Err(AppError::msg("La reserva no coincide con la habitación o tarifa seleccionada"));
        }
        conn.execute(
            "UPDATE reservations SET status = 'checked_in' WHERE id = ?1",
            [res_id],
        )?;
        (res.guest_id, Some(res_id))
    } else {
        let guest = db::insert_guest(
            conn,
            &payload.guest_name,
            payload.document.as_deref(),
            payload.phone.as_deref(),
        )?;
        (guest.id, None)
    };

    let check_in_utc = chrono::Utc::now();
    let check_in_local = check_in_utc.with_timezone(&chrono::Local);
    let expected = match rate.kind {
        RateKind::Hourly => {
            let hours = payload.expected_hours.unwrap_or(rate.included_hours);
            Some(
                billing::expected_checkout_hourly(check_in_local, hours)
                    .with_timezone(&chrono::Utc)
                    .to_rfc3339(),
            )
        }
        RateKind::Night | RateKind::Overnight => Some(
            billing::expected_checkout_night(check_in_local, 1, rate.night_cutoff_hour)
                .with_timezone(&chrono::Utc)
                .to_rfc3339(),
        ),
    };

    conn.execute(
        "INSERT INTO stays (room_id, guest_id, rate_plan_id, reservation_id, check_in_at, expected_checkout_at, status, converted_to_overnight)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', 0)",
        params![
            room.id,
            guest_id,
            rate.id,
            reservation_id,
            check_in_utc.to_rfc3339(),
            expected
        ],
    )
    .map_err(|error| {
        if db::is_unique_violation(&error) {
            AppError::msg("La habitación ya está ocupada")
        } else {
            error.into()
        }
    })?;
    db::set_room_status(conn, room.id, "occupied")?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn preview_bill(state: State<AppState>, session_token: Option<String>, stay_id: i64) -> AppResult<BillPreview> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let stay = db::get_stay(&conn, stay_id)?;
    bill_for_stay(&conn, &stay)
}

#[tauri::command]
pub fn get_stay_detail(
    state: State<AppState>, session_token: Option<String>,
    stay_id: i64,
) -> AppResult<(Stay, BillPreview, Vec<Charge>, Vec<Payment>)> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let stay = db::get_stay(&conn, stay_id)?;
    let bill = bill_for_stay(&conn, &stay)?;
    let charges = db::list_charges(&conn, stay_id)?;
    let payments = db::list_payments(&conn, stay_id)?;
    Ok((stay, bill, charges, payments))
}

#[tauri::command]
pub fn convert_to_overnight(state: State<AppState>, session_token: Option<String>, stay_id: i64) -> AppResult<Stay> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let stay = db::get_stay(&conn, stay_id)?;
    if stay.status != "open" {
        return Err(AppError::msg("La estadía ya está cerrada"));
    }
    let overnight = db::find_plan_by_kind(&conn, RateKind::Overnight)?
        .or(db::find_plan_by_kind(&conn, RateKind::Night)?)
        .ok_or_else(|| AppError::msg("No hay una tarifa de pernocte o noche activa"))?;
    conn.execute(
        "UPDATE stays SET converted_to_overnight = 1, overnight_rate_plan_id = ?1 WHERE id = ?2",
        params![overnight.id, stay_id],
    )?;
    db::get_stay(&conn, stay_id)
}

#[tauri::command]
pub fn list_products(state: State<AppState>, session_token: Option<String>, active_only: Option<bool>) -> AppResult<Vec<Product>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    db::list_products(&conn, active_only.unwrap_or(true))
}

#[tauri::command]
pub fn save_product(state: State<AppState>, session_token: Option<String>, payload: SaveProductPayload) -> AppResult<Product> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let conn = conn(&state);
    db::save_product(
        &conn,
        payload.id,
        &payload.name,
        &payload.category,
        payload.price_cents,
        payload.active,
        payload.sort_order,
    )
}

#[tauri::command]
pub fn set_product_active(state: State<AppState>, session_token: Option<String>, product_id: i64, active: bool) -> AppResult<Product> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let conn = conn(&state);
    db::set_product_active(&conn, product_id, active)
}

#[tauri::command]
pub fn add_charge(state: State<AppState>, session_token: Option<String>, payload: AddChargePayload) -> AppResult<Charge> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let conn = conn(&state);
    let stay = db::get_stay(&conn, payload.stay_id)?;
    if stay.status != "open" {
        return Err(AppError::msg("No se pueden agregar cargos a una estadía cerrada"));
    }
    let kind = if payload.kind == "discount" || payload.amount_cents < 0 {
        "discount"
    } else {
        "surcharge"
    };
    let amount = if kind == "discount" {
        -payload.amount_cents.abs()
    } else {
        payload.amount_cents.abs()
    };
    if payload.description.trim().is_empty() {
        return Err(AppError::msg("La descripción del cargo es obligatoria"));
    }
    if payload.amount_cents == 0 {
        return Err(AppError::msg("El importe del cargo debe ser distinto de cero"));
    }
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO charges (stay_id, kind, description, amount_cents, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![payload.stay_id, kind, payload.description.trim(), amount, now],
    )?;
    Ok(Charge {
        id: conn.last_insert_rowid(),
        stay_id: payload.stay_id,
        kind: kind.into(),
        description: payload.description.trim().into(),
        amount_cents: amount,
        created_at: now,
    })
}

#[tauri::command]
pub fn add_product_charge(state: State<AppState>, session_token: Option<String>, payload: AddProductChargePayload) -> AppResult<Charge> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let stay = db::get_stay(&conn, payload.stay_id)?;
    if stay.status != "open" {
        return Err(AppError::msg("No se pueden agregar cargos a una estadía cerrada"));
    }
    let product = db::get_product(&conn, payload.product_id)?;
    if !product.active {
        return Err(AppError::msg("El producto no está activo"));
    }
    if product.price_cents <= 0 {
        return Err(AppError::msg("El precio del producto no es válido"));
    }
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO charges (stay_id, kind, description, amount_cents, created_at) VALUES (?1, 'surcharge', ?2, ?3, ?4)",
        params![payload.stay_id, product.name, product.price_cents, now],
    )?;
    Ok(Charge {
        id: conn.last_insert_rowid(),
        stay_id: payload.stay_id,
        kind: "surcharge".into(),
        description: product.name,
        amount_cents: product.price_cents,
        created_at: now,
    })
}

#[tauri::command]
pub fn delete_charge(state: State<AppState>, session_token: Option<String>, charge_id: i64) -> AppResult<()> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let conn = conn(&state);
    let stay_id: i64 = conn
        .query_row("SELECT stay_id FROM charges WHERE id = ?1", [charge_id], |row| row.get(0))
        .map_err(|_| AppError::msg("Cargo no encontrado"))?;
    let stay = db::get_stay(&conn, stay_id)?;
    if stay.status != "open" {
        return Err(AppError::msg("No se pueden modificar cargos de una cuenta cerrada"));
    }
    conn.execute(
        "DELETE FROM charges WHERE id = ?1 AND kind IN ('surcharge', 'discount')",
        [charge_id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn check_out(
    state: State<AppState>, session_token: Option<String>,
    app: AppHandle,
    payload: CheckOutPayload,
) -> AppResult<CheckOutResult> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    let stay = db::get_stay(&conn, payload.stay_id)?;
    if stay.status != "open" {
        return Err(AppError::msg("La estadía ya está cerrada"));
    }
    let bill = build_preview(&conn, &stay)?;

    let checkout_at = now_rfc3339();
    close_account(&mut conn, &stay, &bill, &checkout_at)?;

    let stay = db::get_stay(&conn, stay.id)?;
    let mut print_error = None;
    if payload.print {
        let settings = db::load_settings(&conn)?;
        let bytes = printer::build_receipt(&settings, &stay, &bill);
        let data_dir = app_data_dir(&app)?;
        print_error = printer::print_bytes(&bytes, &settings, &data_dir, &format!("stay-{}", stay.id))?;
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
    let mut stmt = conn.prepare(
        "SELECT res.id, res.guest_id, g.name, g.document, g.phone, res.room_id, r.number,
                res.rate_plan_id, rp.name, res.expected_arrival_at, res.expected_nights, res.status, res.notes
         FROM reservations res
         JOIN guests g ON g.id = res.guest_id
         JOIN rooms r ON r.id = res.room_id
         JOIN rate_plans rp ON rp.id = res.rate_plan_id
         ORDER BY res.expected_arrival_at DESC, res.id DESC",
    )?;
    let rows = stmt.query_map([], db::map_reservation)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn create_reservation(state: State<AppState>, session_token: Option<String>, payload: CreateReservationPayload) -> AppResult<Reservation> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let mut conn = conn(&state);
    create_reservation_on(&mut conn, payload)
}

fn create_reservation_on(conn: &mut Connection, payload: CreateReservationPayload) -> AppResult<Reservation> {
    let tx = conn.transaction()?;
    let reservation_id = create_reservation_in_tx(&tx, payload)?;
    tx.commit()?;
    db::get_reservation(conn, reservation_id)
}

#[cfg(test)]
fn create_reservation_on_failing(conn: &mut Connection, payload: CreateReservationPayload) -> AppResult<Reservation> {
    let tx = conn.transaction()?;
    let _reservation_id = create_reservation_in_tx(&tx, payload)?;
    Err(AppError::msg("fallo inyectado"))
}

fn create_reservation_in_tx(conn: &Connection, payload: CreateReservationPayload) -> AppResult<i64> {
    let room = db::get_room(conn, payload.room_id)?;
    if !room.active {
        return Err(AppError::msg("La habitación ya no está habilitada"));
    }
    let _rate = db::get_rate_plan(conn, payload.rate_plan_id)?;
    let _arrival = db::parse_dt(&payload.expected_arrival_at)?;
    if payload.expected_nights < 1 {
        return Err(AppError::msg("La reserva debe tener al menos una noche"));
    }

    if let Some(open) = db::open_stay_for_room(conn, room.id)? {
        let arrival_day = db::local_calendar_date(&payload.expected_arrival_at);
        let stay_day = db::local_calendar_date(&open.check_in_at);
        if arrival_day == stay_day {
            return Err(AppError::msg("La habitación está ocupada en esa fecha"));
        }
    }
    let duplicate: Option<i64> = conn
        .query_row(
            "SELECT id FROM reservations WHERE room_id = ?1 AND status = 'hold' AND substr(expected_arrival_at, 1, 10) = substr(?2, 1, 10) LIMIT 1",
            params![room.id, payload.expected_arrival_at],
            |row| row.get(0),
        )
        .optional()?;
    if duplicate.is_some() {
        return Err(AppError::msg("Ya existe una reserva vigente para esa habitación y fecha"));
    }

    let guest = db::insert_guest(
        conn,
        &payload.guest_name,
        payload.document.as_deref(),
        payload.phone.as_deref(),
    )?;
    conn.execute(
        "INSERT INTO reservations (guest_id, room_id, rate_plan_id, expected_arrival_at, expected_nights, status, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'hold', ?6, ?7)",
        params![
            guest.id,
            payload.room_id,
            payload.rate_plan_id,
            payload.expected_arrival_at,
            payload.expected_nights.max(1),
            payload.notes,
            now_rfc3339()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn set_reservation_status(state: State<AppState>, session_token: Option<String>, reservation_id: i64, status: String) -> AppResult<Reservation> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let allowed = ["cancelled", "no_show"];
    if !allowed.contains(&status.as_str()) {
        return Err(AppError::msg("Estado de reserva inválido"));
    }
    let conn = conn(&state);
    let res = db::get_reservation(&conn, reservation_id)?;
    if res.status != "hold" {
        return Err(AppError::msg("Solo se pueden actualizar reservas en espera"));
    }
    conn.execute(
        "UPDATE reservations SET status = ?1 WHERE id = ?2",
        params![status, reservation_id],
    )?;
    db::get_reservation(&conn, reservation_id)
}

#[tauri::command]
pub fn check_in_reservation(state: State<AppState>, session_token: Option<String>, reservation_id: i64) -> AppResult<Stay> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let reservation = {
        let conn = conn(&state);
        db::get_reservation(&conn, reservation_id)?
    };
    check_in(
        state,
        session_token,
        CheckInPayload {
            room_id: reservation.room_id,
            guest_name: reservation.guest_name,
            document: reservation.guest_document,
            phone: reservation.guest_phone,
            rate_plan_id: reservation.rate_plan_id,
            expected_hours: Some(reservation.expected_nights * 24),
            reservation_id: Some(reservation.id),
        },
    )
}

#[tauri::command]
pub fn list_history(state: State<AppState>, session_token: Option<String>, date: Option<String>) -> AppResult<Vec<HistoryStay>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let day = date.unwrap_or_else(db::local_today);
    let mut stmt = conn.prepare(
        "SELECT s.id, s.check_out_at, s.check_in_at FROM stays s WHERE s.status = 'closed'
         ORDER BY s.check_out_at DESC",
    )?;
    let rows: Vec<(i64, Option<String>, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();
    let ids: Vec<i64> = rows
        .into_iter()
        .filter(|(_, check_out_at, check_in_at)| {
            let stamp = check_out_at.as_deref().unwrap_or(check_in_at.as_str());
            db::local_calendar_date(stamp) == day
        })
        .map(|(id, _, _)| id)
        .collect();
    let mut out = Vec::new();
    for id in ids {
        let stay = db::get_stay(&conn, id)?;
        let payments = db::list_payments(&conn, id)?;
        let bill = bill_for_stay(&conn, &stay)?;
        out.push(HistoryStay {
            stay,
            total_cents: bill.total_cents,
            payment_method: payments.first().map(|p| p.method.clone()),
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn get_settings(state: State<AppState>, session_token: Option<String>) -> AppResult<AppSettings> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let mut settings = db::load_settings(&conn)?;
    settings.pin_hash = String::new();
    Ok(settings)
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, session_token: Option<String>, payload: AppSettings, new_pin: Option<String>) -> AppResult<AppSettings> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let conn = conn(&state);
    db::upsert_setting(&conn, "business_name", payload.business_name.trim())?;
    db::upsert_setting(&conn, "address", payload.address.trim())?;
    db::upsert_setting(&conn, "phone", payload.phone.trim())?;
    db::upsert_setting(&conn, "tax_percent", &payload.tax_percent.to_string())?;
    db::upsert_setting(&conn, "currency_symbol", payload.currency_symbol.trim())?;
    db::upsert_setting(&conn, "theme", &payload.theme)?;
    db::upsert_setting(&conn, "receipt_footer", payload.receipt_footer.trim())?;
    db::upsert_setting(
        &conn,
        "printer_enabled",
        if payload.printer_enabled { "true" } else { "false" },
    )?;
    db::upsert_setting(&conn, "printer_path", payload.printer_path.trim())?;
    db::upsert_setting(&conn, "printer_name", payload.printer_name.trim())?;
    db::upsert_setting(&conn, "paper_width", &payload.paper_width.to_string())?;
    db::upsert_setting(
        &conn,
        "auto_print_on_checkout",
        if payload.auto_print_on_checkout { "true" } else { "false" },
    )?;
    db::upsert_setting(
        &conn,
        "require_guest_name",
        if payload.require_guest_name { "true" } else { "false" },
    )?;
    if let Some(pin) = new_pin {
        if pin.is_empty() {
            db::upsert_setting(&conn, "pin_hash", "")?;
        } else {
            db::upsert_setting(&conn, "pin_hash", &db::hash_pin(&pin))?;
        }
    }
    let mut settings = db::load_settings(&conn)?;
    settings.pin_hash = String::new();
    Ok(settings)
}

#[tauri::command]
pub fn verify_pin(state: State<AppState>, session_token: Option<String>, pin: String) -> AppResult<bool> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let hash = db::get_setting(&conn, "pin_hash", "")?;
    if hash.is_empty() {
        return Ok(true);
    }
    Ok(hash == db::hash_pin(&pin))
}

#[tauri::command]
pub fn pin_required(state: State<AppState>, session_token: Option<String>) -> AppResult<bool> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    Ok(!db::get_setting(&conn, "pin_hash", "")?.is_empty())
}

#[tauri::command]
pub fn print_test(state: State<AppState>, session_token: Option<String>, app: AppHandle) -> AppResult<Option<String>> {
    crate::auth::require(&state, session_token.as_deref(), true)?;
    let conn = conn(&state);
    let settings = db::load_settings(&conn)?;
    let bytes = printer::build_test_receipt(&settings);
    let data_dir = app_data_dir(&app)?;
    printer::print_bytes(&bytes, &settings, &data_dir, "test")
}

#[tauri::command]
pub fn reprint_receipt(state: State<AppState>, session_token: Option<String>, app: AppHandle, stay_id: i64) -> AppResult<Option<String>> {
    crate::auth::require(&state, session_token.as_deref(), false)?;
    let conn = conn(&state);
    let stay = db::get_stay(&conn, stay_id)?;
    let bill = bill_for_stay(&conn, &stay)?;
    let settings = db::load_settings(&conn)?;
    let bytes = printer::build_receipt(&settings, &stay, &bill);
    let data_dir = app_data_dir(&app)?;
    printer::print_bytes(&bytes, &settings, &data_dir, &format!("stay-{}", stay.id))
}

#[cfg(test)]
mod account_tests {
    use super::*;
    use std::path::Path;

    fn open_stay_fixture() -> AppResult<(Connection, Stay)> {
        let conn = db::open(Path::new(":memory:"))?;
        let now = now_rfc3339();
        conn.execute(
            "INSERT INTO guests (name, created_at) VALUES ('Cuenta de prueba', ?1)",
            [&now],
        )?;
        conn.execute(
            "INSERT INTO stays (room_id, guest_id, rate_plan_id, check_in_at, status) VALUES (1, 1, 1, ?1, 'open')",
            [&now],
        )?;
        let stay = db::get_stay(&conn, conn.last_insert_rowid())?;
        Ok((conn, stay))
    }

    #[test]
    fn closing_account_persists_total_and_does_not_create_payment() -> AppResult<()> {
        let (mut conn, stay) = open_stay_fixture()?;
        let bill = build_preview(&conn, &stay)?;
        close_account(&mut conn, &stay, &bill, &now_rfc3339())?;

        conn.execute("UPDATE rate_plans SET base_amount_cents = 9999999 WHERE id = 1", [])?;
        let closed = db::get_stay(&conn, stay.id)?;
        let historical = bill_for_stay(&conn, &closed)?;
        assert_eq!(historical.total_cents, bill.total_cents);
        assert_eq!(conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM payments WHERE stay_id = ?1", [stay.id], |row| row.get(0))?, 0);
        assert_eq!(conn.query_row::<String, _, _>("SELECT status FROM rooms WHERE id = 1", [], |row| row.get(0))?, "dirty");
        Ok(())
    }

    #[test]
    fn closing_same_account_twice_is_rejected_without_rewriting_history() -> AppResult<()> {
        let (mut conn, stay) = open_stay_fixture()?;
        let bill = build_preview(&conn, &stay)?;
        close_account(&mut conn, &stay, &bill, &now_rfc3339())?;
        let error = close_account(&mut conn, &stay, &bill, &now_rfc3339()).expect_err("duplicate close");
        assert!(error.to_string().contains("ya está cerrada"));
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM charges WHERE stay_id = ?1 AND kind = 'stay'", [stay.id], |row| row.get(0))?;
        assert_eq!(count, 1);
        Ok(())
    }

    fn walk_in_payload(room_id: i64) -> CheckInPayload {
        CheckInPayload {
            room_id,
            guest_name: "Huésped TX".into(),
            document: None,
            phone: None,
            rate_plan_id: 1,
            expected_hours: Some(3),
            reservation_id: None,
        }
    }

    #[test]
    fn injected_check_in_failure_leaves_no_partial_state() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let guests_before: i64 = conn.query_row("SELECT COUNT(*) FROM guests", [], |row| row.get(0))?;
        let stays_before: i64 = conn.query_row("SELECT COUNT(*) FROM stays", [], |row| row.get(0))?;
        let error = check_in_on_failing(&mut conn, walk_in_payload(1)).expect_err("injected");
        assert!(error.to_string().contains("fallo inyectado"));
        let guests_after: i64 = conn.query_row("SELECT COUNT(*) FROM guests", [], |row| row.get(0))?;
        let stays_after: i64 = conn.query_row("SELECT COUNT(*) FROM stays", [], |row| row.get(0))?;
        let room_status: String = conn.query_row("SELECT status FROM rooms WHERE id = 1", [], |row| row.get(0))?;
        assert_eq!(guests_after, guests_before);
        assert_eq!(stays_after, stays_before);
        assert_eq!(room_status, "available");
        Ok(())
    }

    #[test]
    fn injected_reservation_failure_leaves_no_partial_state() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let guests_before: i64 = conn.query_row("SELECT COUNT(*) FROM guests", [], |row| row.get(0))?;
        let reservations_before: i64 =
            conn.query_row("SELECT COUNT(*) FROM reservations", [], |row| row.get(0))?;
        let payload = CreateReservationPayload {
            guest_name: "Reserva TX".into(),
            document: None,
            phone: None,
            room_id: 1,
            rate_plan_id: 1,
            expected_arrival_at: now_rfc3339(),
            expected_nights: 1,
            notes: None,
        };
        let error = create_reservation_on_failing(&mut conn, payload).expect_err("injected");
        assert!(error.to_string().contains("fallo inyectado"));
        let guests_after: i64 = conn.query_row("SELECT COUNT(*) FROM guests", [], |row| row.get(0))?;
        let reservations_after: i64 =
            conn.query_row("SELECT COUNT(*) FROM reservations", [], |row| row.get(0))?;
        assert_eq!(guests_after, guests_before);
        assert_eq!(reservations_after, reservations_before);
        Ok(())
    }

    #[test]
    fn check_in_rejects_second_open_stay_for_same_room() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        check_in_on(&mut conn, walk_in_payload(1))?;
        let error = check_in_on(&mut conn, walk_in_payload(1)).expect_err("occupied");
        assert!(error.to_string().contains("ocupada"));
        let open: i64 = conn.query_row(
            "SELECT COUNT(*) FROM stays WHERE room_id = 1 AND status = 'open'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(open, 1);
        Ok(())
    }

    #[test]
    fn preview_bill_keeps_closed_snapshot_after_rate_and_tax_change() -> AppResult<()> {
        let (mut conn, stay) = open_stay_fixture()?;
        let bill = build_preview(&conn, &stay)?;
        close_account(&mut conn, &stay, &bill, &now_rfc3339())?;
        conn.execute("UPDATE rate_plans SET base_amount_cents = 9999999, kind = 'night' WHERE id = 1", [])?;
        db::upsert_setting(&conn, "tax_percent", "21")?;
        let closed = db::get_stay(&conn, stay.id)?;
        let historical = bill_for_stay(&conn, &closed)?;
        assert_eq!(historical.total_cents, bill.total_cents);
        assert_eq!(historical.tax_percent, bill.tax_percent);
        assert_eq!(historical.applied_kind, bill.applied_kind);
        assert_eq!(historical.duration_label, bill.duration_label);
        Ok(())
    }
}
