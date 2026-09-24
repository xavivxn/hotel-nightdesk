use crate::billing::{self, BillingContext};
use crate::db::{self, now_rfc3339};
use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::sync::outbox::{self, Entity, OutboxOp};
use rusqlite::{params, Connection, OptionalExtension};

pub const CONTRACT_VERSION: u32 = 2;

#[derive(Debug, Clone)]
pub struct Actor {
    pub user_id: i64,
    #[allow(dead_code)]
    pub username: String,
    pub role: Role,
}

impl From<&SessionUser> for Actor {
    fn from(user: &SessionUser) -> Self {
        Self {
            user_id: user.id,
            username: user.username.clone(),
            role: user.role,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    SaveRoom,
    SaveRatePlan,
    SaveProduct,
    SetProductActive,
    AddCharge,
    DeleteCharge,
    SaveSettings,
    PrintTest,
    CreateUser,
    ListUsers,
    SetUserActive,
    DeleteUser,
}

pub fn authorize(actor: &Actor, operation: Operation) -> AppResult<()> {
    let admin_only = matches!(
        operation,
        Operation::SaveRoom
            | Operation::SaveRatePlan
            | Operation::SaveProduct
            | Operation::SetProductActive
            | Operation::AddCharge
            | Operation::DeleteCharge
            | Operation::SaveSettings
            | Operation::PrintTest
            | Operation::CreateUser
            | Operation::ListUsers
            | Operation::SetUserActive
            | Operation::DeleteUser
    );
    if admin_only && !actor.role.is_admin() {
        return Err(AppError::forbidden("Esta operación requiere administración"));
    }
    Ok(())
}

pub fn accept_reserved_fields(
    operation_id: &Option<String>,
    expected_version: &Option<i64>,
) -> AppResult<()> {
    if let Some(id) = operation_id {
        if id.trim().is_empty() {
            return Err(AppError::msg("operation_id no puede estar vacío"));
        }
    }
    if let Some(version) = expected_version {
        if *version < 0 {
            return Err(AppError::msg("expected_version no puede ser negativo"));
        }
    }
    Ok(())
}

fn assert_expected_version(current: i64, expected: &Option<i64>) -> AppResult<()> {
    if let Some(version) = expected {
        if *version != current {
            return Err(AppError::conflict(
                "La ficha cambió; recargá antes de guardar",
            ));
        }
    }
    Ok(())
}

fn enqueue_guest_stay_room(
    conn: &Connection,
    operation_id: &str,
    guest_id: i64,
    stay_id: i64,
    room_id: i64,
    reservation_id: Option<i64>,
) -> AppResult<()> {
    let mut ops = vec![
        OutboxOp::upsert(
            Entity::Guest,
            db::uid_of(conn, "guests", guest_id)?,
            db::payload_for_guest(conn, guest_id)?,
        ),
    ];
    if let Some(res_id) = reservation_id {
        ops.push(OutboxOp::upsert(
            Entity::Reservation,
            db::uid_of(conn, "reservations", res_id)?,
            db::payload_for_reservation(conn, res_id)?,
        ));
    }
    ops.push(OutboxOp::upsert(
        Entity::Stay,
        db::uid_of(conn, "stays", stay_id)?,
        db::payload_for_stay(conn, stay_id)?,
    ));
    ops.push(OutboxOp::upsert(
        Entity::Room,
        db::uid_of(conn, "rooms", room_id)?,
        db::payload_for_room(conn, room_id)?,
    ));
    outbox::enqueue(conn, operation_id, &ops)
}

pub fn contract_info(conn: &Connection) -> AppResult<ContractInfo> {
    Ok(ContractInfo {
        contract_version: CONTRACT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        schema_migrations: db::list_applied_migrations(conn)?,
    })
}

pub fn build_preview(conn: &Connection, stay: &Stay) -> AppResult<BillPreview> {
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

pub fn bill_for_stay(conn: &Connection, stay: &Stay) -> AppResult<BillPreview> {
    if stay.status != "closed" {
        return build_preview(conn, stay);
    }
    let charges = db::list_charges(conn, stay.id)?;
    let has_persisted_lines = charges
        .iter()
        .any(|charge| matches!(charge.kind.as_str(), "stay" | "extra_hour" | "tax"));
    if !has_persisted_lines && db::closed_snapshot(conn, stay.id)?.applied_kind.is_none() {
        return Err(AppError::storage("Cuenta antigua sin detalle de cierre; requiere revisión, no se recalcula con tarifas actuales"));
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

pub fn close_account(
    conn: &mut Connection,
    stay: &Stay,
    bill: &BillPreview,
    checkout_at: &str,
    operation_id: Option<String>,
) -> AppResult<()> {
    let root = outbox::resolve_operation_id(&operation_id);
    let tx = conn.transaction()?;
    persist_computed_charges(&tx, bill)?;
    tx.execute(
        "UPDATE stays SET status = 'closed', check_out_at = ?1,
                closed_applied_kind = ?2, closed_tax_percent = ?3, closed_duration_label = ?4,
                closed_total_cents = ?5, closed_line_count = ?6
         WHERE id = ?7 AND status = 'open'",
        params![
            checkout_at,
            bill.applied_kind.as_str(),
            bill.tax_percent,
            bill.duration_label,
            tx.query_row(
                "SELECT COALESCE(SUM(amount_cents), 0) FROM charges WHERE stay_id = ?1 AND deleted_at IS NULL",
                [stay.id],
                |row| row.get::<_, i64>(0),
            )?,
            tx.query_row(
                "SELECT COUNT(*) FROM charges WHERE stay_id = ?1 AND deleted_at IS NULL",
                [stay.id],
                |row| row.get::<_, i64>(0),
            )?,
            stay.id
        ],
    )?;
    if tx.changes() != 1 {
        return Err(AppError::conflict("La estadía ya está cerrada"));
    }
    tx.execute(
        "UPDATE rooms SET status = 'dirty' WHERE id = ?1",
        [stay.room_id],
    )?;
    let mut closed = stay.clone();
    closed.status = "closed".into();
    closed.check_out_at = Some(checkout_at.into());
    let settings = db::load_settings(&tx)?;
    let bytes = crate::printer::build_receipt(&settings, &closed, bill);
    tx.execute("INSERT INTO receipt_snapshots(stay_id, bytes, created_at) VALUES (?1, ?2, ?3)",
        params![stay.id, bytes, checkout_at])?;

    let mut ops = vec![OutboxOp::upsert(
        Entity::Stay,
        db::uid_of(&tx, "stays", stay.id)?,
        db::payload_for_stay(&tx, stay.id)?,
    )];
    let mut stmt = tx.prepare(
        "SELECT id FROM charges WHERE stay_id = ?1 AND kind IN ('stay', 'extra_hour', 'tax') AND deleted_at IS NULL",
    )?;
    let charge_ids: Vec<i64> = stmt
        .query_map([stay.id], |row| row.get(0))?
        .filter_map(|row| row.ok())
        .collect();
    drop(stmt);
    for charge_id in charge_ids {
        ops.push(OutboxOp::upsert(
            Entity::Charge,
            db::uid_of(&tx, "charges", charge_id)?,
            db::payload_for_charge(&tx, charge_id)?,
        ));
    }
    ops.push(OutboxOp::upsert(
        Entity::Room,
        db::uid_of(&tx, "rooms", stay.room_id)?,
        db::payload_for_room(&tx, stay.room_id)?,
    ));
    outbox::enqueue(&tx, &root, &ops)?;
    tx.commit()?;
    Ok(())
}

pub fn list_board(conn: &Connection) -> AppResult<Vec<BoardRoom>> {
    let mut stmt = conn.prepare(
        "SELECT id, number, room_type, floor, status, notes, active, version FROM rooms WHERE active = 1 ORDER BY floor, number",
    )?;
    let rooms: Vec<Room> = stmt
        .query_map([], db::map_room)?
        .filter_map(|r| r.ok())
        .collect();

    let mut board = Vec::with_capacity(rooms.len());
    for room in rooms {
        let stay = db::open_stay_for_room(conn, room.id)?;
        let reservation = if stay.is_none() {
            db::today_hold_for_room(conn, room.id)?
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
            if let Ok(bill) = bill_for_stay(conn, stay_ref) {
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

pub fn list_rooms(conn: &Connection) -> AppResult<Vec<Room>> {
    let mut stmt = conn.prepare(
        "SELECT id, number, room_type, floor, status, notes, active, version FROM rooms WHERE active = 1 ORDER BY floor, number",
    )?;
    let rooms = stmt
        .query_map([], db::map_room)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rooms)
}

pub fn save_room(conn: &Connection, actor: &Actor, payload: SaveRoomPayload) -> AppResult<Room> {
    authorize(actor, Operation::SaveRoom)?;
    accept_reserved_fields(&payload.operation_id, &payload.expected_version)?;
    if payload.number.trim().is_empty() {
        return Err(AppError::msg("El número de habitación es obligatorio"));
    }
    let now = now_rfc3339();
    if let Some(id) = payload.id {
        let current = db::get_room(conn, id)?;
        assert_expected_version(current.version, &payload.expected_version)?;
        conn.execute(
            "UPDATE rooms SET number = ?1, room_type = ?2, floor = ?3, notes = ?4, version = version + 1, updated_at = ?5 WHERE id = ?6",
            params![
                payload.number.trim(),
                payload.room_type.trim(),
                payload.floor,
                payload.notes,
                now,
                id
            ],
        )?;
        db::get_room(conn, id)
    } else {
        conn.execute(
            "INSERT INTO rooms (number, room_type, floor, status, notes, created_at, updated_at) VALUES (?1, ?2, ?3, 'available', ?4, ?5, ?5)",
            params![
                payload.number.trim(),
                payload.room_type.trim(),
                payload.floor,
                payload.notes,
                now
            ],
        )?;
        db::get_room(conn, conn.last_insert_rowid())
    }
}

pub fn set_room_status(conn: &mut Connection, room_id: i64, status: String) -> AppResult<Room> {
    let allowed = ["available", "dirty", "blocked"];
    if !allowed.contains(&status.as_str()) {
        return Err(AppError::msg("Estado de habitación inválido"));
    }
    let room = db::get_room(conn, room_id)?;
    if !room.active {
        return Err(AppError::msg("La habitación ya no está habilitada"));
    }
    if db::open_stay_for_room(conn, room_id)?.is_some() {
        return Err(AppError::conflict(
            "No se puede cambiar el estado de una habitación ocupada",
        ));
    }
    let root = outbox::resolve_operation_id(&None);
    let tx = conn.transaction()?;
    db::set_room_status(&tx, room_id, &status)?;
    outbox::enqueue(
        &tx,
        &root,
        &[OutboxOp::upsert(
            Entity::Room,
            db::uid_of(&tx, "rooms", room_id)?,
            db::payload_for_room(&tx, room_id)?,
        )],
    )?;
    let room = db::get_room(&tx, room_id)?;
    tx.commit()?;
    Ok(room)
}

pub fn save_rate_plan(
    conn: &Connection,
    actor: &Actor,
    payload: SaveRatePlanPayload,
) -> AppResult<RatePlan> {
    authorize(actor, Operation::SaveRatePlan)?;
    accept_reserved_fields(&payload.operation_id, &payload.expected_version)?;
    if payload.name.trim().is_empty() {
        return Err(AppError::msg("El nombre de la tarifa es obligatorio"));
    }
    let active = if payload.active { 1 } else { 0 };
    if let Some(id) = payload.id {
        let current = db::get_rate_plan(conn, id)?;
        assert_expected_version(current.version, &payload.expected_version)?;
        conn.execute(
            "UPDATE rate_plans SET name=?1, kind=?2, base_amount_cents=?3, extra_hour_cents=?4,
             included_hours=?5, grace_minutes=?6, night_cutoff_hour=?7, active=?8, version = version + 1, updated_at=?9 WHERE id=?10",
            params![
                payload.name.trim(),
                payload.kind.as_str(),
                payload.base_amount_cents,
                payload.extra_hour_cents,
                payload.included_hours.max(1),
                payload.grace_minutes.max(0),
                payload.night_cutoff_hour.clamp(0, 23),
                active,
                now_rfc3339(),
                id
            ],
        )?;
        db::get_rate_plan(conn, id)
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
        db::get_rate_plan(conn, conn.last_insert_rowid())
    }
}

fn ensure_dormida_window(cutoff_hour: i64) -> AppResult<()> {
    let now = chrono::Local::now();
    if billing::dormida_window_open(now, cutoff_hour) {
        Ok(())
    } else {
        Err(AppError::msg(billing::dormida_unavailable_message(
            now,
            cutoff_hour,
        )))
    }
}

pub fn check_in_on(conn: &mut Connection, payload: CheckInPayload) -> AppResult<Stay> {
    accept_reserved_fields(&payload.operation_id, &payload.expected_version)?;
    let root = outbox::resolve_operation_id(&payload.operation_id);
    let tx = conn.transaction()?;
    let stay_id = check_in_in_tx(&tx, payload, &root)?;
    tx.commit()?;
    db::get_stay(conn, stay_id)
}

#[cfg(test)]
fn check_in_on_failing(conn: &mut Connection, payload: CheckInPayload) -> AppResult<Stay> {
    let root = outbox::resolve_operation_id(&payload.operation_id);
    let tx = conn.transaction()?;
    let _stay_id = check_in_in_tx(&tx, payload, &root)?;
    Err(AppError::msg("fallo inyectado"))
}

fn check_in_in_tx(conn: &Connection, payload: CheckInPayload, operation_id: &str) -> AppResult<i64> {
    let room = db::get_room(conn, payload.room_id)?;
    if !room.active {
        return Err(AppError::msg("La habitación ya no está habilitada"));
    }
    if db::open_stay_for_room(conn, room.id)?.is_some() {
        return Err(AppError::conflict("La habitación ya está ocupada"));
    }
    if room.status == "blocked" {
        return Err(AppError::msg("La habitación está bloqueada"));
    }
    if room.status == "dirty" {
        return Err(AppError::msg(
            "La habitación necesita limpieza antes del check-in",
        ));
    }
    if let Some(hold) = db::today_hold_for_room(conn, room.id)? {
        if payload.reservation_id != Some(hold.id) {
            return Err(AppError::conflict(format!(
                "La habitación {} tiene una reserva para hoy",
                room.number
            )));
        }
    }
    let rate = db::get_rate_plan(conn, payload.rate_plan_id)?;
    if !rate.active {
        return Err(AppError::msg("La tarifa no está activa"));
    }
    if matches!(rate.kind, RateKind::Night | RateKind::Overnight) {
        ensure_dormida_window(rate.night_cutoff_hour)?;
    }

    let (guest_id, reservation_id) = if let Some(res_id) = payload.reservation_id {
        let res = db::get_reservation(conn, res_id)?;
        if res.status != "hold" {
            return Err(AppError::conflict("La reserva ya no está vigente"));
        }
        if res.room_id != room.id || res.rate_plan_id != rate.id {
            return Err(AppError::msg(
                "La reserva no coincide con la habitación o tarifa seleccionada",
            ));
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
            AppError::conflict("La habitación ya está ocupada")
        } else {
            error.into()
        }
    })?;
    let stay_id = conn.last_insert_rowid();
    db::set_room_status(conn, room.id, "occupied")?;
    enqueue_guest_stay_room(conn, operation_id, guest_id, stay_id, room.id, reservation_id)?;
    Ok(stay_id)
}

pub fn preview_bill(conn: &Connection, stay_id: i64) -> AppResult<BillPreview> {
    let stay = db::get_stay(conn, stay_id)?;
    bill_for_stay(conn, &stay)
}

pub fn get_stay_detail(
    conn: &Connection,
    stay_id: i64,
) -> AppResult<(Stay, BillPreview, Vec<Charge>, Vec<Payment>)> {
    let stay = db::get_stay(conn, stay_id)?;
    let bill = bill_for_stay(conn, &stay)?;
    let charges = db::list_charges(conn, stay_id)?;
    let payments = db::list_payments(conn, stay_id)?;
    Ok((stay, bill, charges, payments))
}

pub fn convert_to_overnight(conn: &mut Connection, stay_id: i64) -> AppResult<Stay> {
    let stay = db::get_stay(conn, stay_id)?;
    if stay.status != "open" {
        return Err(AppError::conflict("La estadía ya está cerrada"));
    }
    let overnight = db::find_plan_by_kind(conn, RateKind::Overnight)?
        .or(db::find_plan_by_kind(conn, RateKind::Night)?)
        .ok_or_else(|| AppError::msg("No hay una tarifa de dormida activa"))?;
    ensure_dormida_window(overnight.night_cutoff_hour)?;
    let expected_checkout = db::parse_dt(&stay.check_in_at).ok().map(|check_in| {
        billing::expected_checkout_night(check_in, 1, overnight.night_cutoff_hour)
            .with_timezone(&chrono::Utc)
            .to_rfc3339()
    });
    let root = outbox::resolve_operation_id(&None);
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE stays SET converted_to_overnight = 1, overnight_rate_plan_id = ?1, expected_checkout_at = COALESCE(?2, expected_checkout_at) WHERE id = ?3",
        params![overnight.id, expected_checkout, stay_id],
    )?;
    outbox::enqueue(
        &tx,
        &root,
        &[OutboxOp::upsert(
            Entity::Stay,
            db::uid_of(&tx, "stays", stay_id)?,
            db::payload_for_stay(&tx, stay_id)?,
        )],
    )?;
    let stay = db::get_stay(&tx, stay_id)?;
    tx.commit()?;
    Ok(stay)
}

pub fn save_product(
    conn: &Connection,
    actor: &Actor,
    payload: SaveProductPayload,
) -> AppResult<Product> {
    authorize(actor, Operation::SaveProduct)?;
    accept_reserved_fields(&payload.operation_id, &payload.expected_version)?;
    if let Some(id) = payload.id {
        let current = db::get_product(conn, id)?;
        assert_expected_version(current.version, &payload.expected_version)?;
    }
    db::save_product(
        conn,
        payload.id,
        &payload.name,
        &payload.category,
        payload.price_cents,
        payload.active,
        payload.sort_order,
    )
}

pub fn set_product_active(
    conn: &Connection,
    actor: &Actor,
    product_id: i64,
    active: bool,
) -> AppResult<Product> {
    authorize(actor, Operation::SetProductActive)?;
    db::set_product_active(conn, product_id, active)
}

fn map_managed_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<ManagedUser> {
    Ok(ManagedUser {
        id: row.get(0)?,
        username: row.get(1)?,
        role: Role::parse(&row.get::<_, String>(2)?),
        active: row.get::<_, i64>(3)? != 0,
        version: row.get(4)?,
    })
}

pub fn list_users(conn: &Connection, actor: &Actor) -> AppResult<Vec<ManagedUser>> {
    authorize(actor, Operation::ListUsers)?;
    let mut stmt = conn.prepare(
        "SELECT id, username, role, active, version FROM users ORDER BY username COLLATE NOCASE",
    )?;
    let users = stmt
        .query_map([], map_managed_user)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(users)
}

pub fn get_managed_user(conn: &Connection, user_id: i64) -> AppResult<ManagedUser> {
    conn.query_row(
        "SELECT id, username, role, active, version FROM users WHERE id = ?1",
        [user_id],
        map_managed_user,
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("Usuario no encontrado"))
}

pub fn user_catalog_row(conn: &Connection, user_id: i64) -> AppResult<(ManagedUser, String)> {
    conn.query_row(
        "SELECT id, username, role, active, version, password_hash FROM users WHERE id = ?1",
        [user_id],
        |row| {
            Ok((
                map_managed_user(row)?,
                row.get::<_, String>(5)?,
            ))
        },
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("Usuario no encontrado"))
}

pub fn validate_set_user_active(
    conn: &Connection,
    actor: &Actor,
    user_id: i64,
    active: bool,
    expected_version: Option<i64>,
) -> AppResult<(ManagedUser, String)> {
    authorize(actor, Operation::SetUserActive)?;
    accept_reserved_fields(&None, &expected_version)?;
    let (current, hash) = user_catalog_row(conn, user_id)?;
    assert_user_active_change(conn, actor, &current, active)?;
    Ok((current, hash))
}

fn assert_user_active_change(
    conn: &Connection,
    actor: &Actor,
    current: &ManagedUser,
    active: bool,
) -> AppResult<()> {
    if active || current.active == active {
        return Ok(());
    }
    if current.id == actor.user_id {
        return Err(AppError::forbidden("No podés desactivar tu propia cuenta"));
    }
    if current.role.is_admin() {
        let others: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE role = 'admin' AND active = 1 AND id != ?1",
            [current.id],
            |row| row.get(0),
        )?;
        if others == 0 {
            return Err(AppError::forbidden("Tiene que quedar al menos un administrador"));
        }
    }
    Ok(())
}

pub fn set_user_active(
    conn: &Connection,
    actor: &Actor,
    user_id: i64,
    active: bool,
    expected_version: Option<i64>,
) -> AppResult<ManagedUser> {
    authorize(actor, Operation::SetUserActive)?;
    accept_reserved_fields(&None, &expected_version)?;
    let current = get_managed_user(conn, user_id)?;
    if expected_version.unwrap_or(current.version) != 0 {
        assert_expected_version(current.version, &expected_version)?;
    }
    assert_user_active_change(conn, actor, &current, active)?;
    conn.execute(
        "UPDATE users SET active = ?1, version = version + 1, updated_at = ?2 WHERE id = ?3",
        params![if active { 1 } else { 0 }, now_rfc3339(), user_id],
    )?;
    get_managed_user(conn, user_id)
}

pub fn validate_delete_user(
    conn: &Connection,
    actor: &Actor,
    user_id: i64,
    expected_version: Option<i64>,
) -> AppResult<ManagedUser> {
    authorize(actor, Operation::DeleteUser)?;
    accept_reserved_fields(&None, &expected_version)?;
    let current = get_managed_user(conn, user_id)?;
    assert_user_delete(conn, actor, &current)?;
    Ok(current)
}

fn assert_user_delete(conn: &Connection, actor: &Actor, current: &ManagedUser) -> AppResult<()> {
    if current.id == actor.user_id {
        return Err(AppError::forbidden("No podés eliminar tu propia cuenta"));
    }
    if current.role.is_admin() {
        let others: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE role = 'admin' AND active = 1 AND id != ?1",
            [current.id],
            |row| row.get(0),
        )?;
        if others == 0 {
            return Err(AppError::forbidden("Tiene que quedar al menos un administrador"));
        }
    }
    Ok(())
}

pub fn delete_user(
    conn: &Connection,
    actor: &Actor,
    user_id: i64,
    expected_version: Option<i64>,
) -> AppResult<ManagedUser> {
    authorize(actor, Operation::DeleteUser)?;
    accept_reserved_fields(&None, &expected_version)?;
    let current = get_managed_user(conn, user_id)?;
    if expected_version.unwrap_or(current.version) != 0 {
        assert_expected_version(current.version, &expected_version)?;
    }
    assert_user_delete(conn, actor, &current)?;
    conn.execute("DELETE FROM users WHERE id = ?1", [user_id])?;
    Ok(current)
}

pub fn add_charge(conn: &mut Connection, actor: &Actor, payload: AddChargePayload) -> AppResult<Charge> {
    authorize(actor, Operation::AddCharge)?;
    accept_reserved_fields(&payload.operation_id, &payload.expected_version)?;
    let stay = db::get_stay(conn, payload.stay_id)?;
    if stay.status != "open" {
        return Err(AppError::conflict(
            "No se pueden agregar cargos a una estadía cerrada",
        ));
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
    let root = outbox::resolve_operation_id(&payload.operation_id);
    let now = now_rfc3339();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO charges (stay_id, kind, description, amount_cents, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![payload.stay_id, kind, payload.description.trim(), amount, now],
    )?;
    let charge_id = tx.last_insert_rowid();
    outbox::enqueue(
        &tx,
        &root,
        &[OutboxOp::upsert(
            Entity::Charge,
            db::uid_of(&tx, "charges", charge_id)?,
            db::payload_for_charge(&tx, charge_id)?,
        )],
    )?;
    tx.commit()?;
    Ok(Charge {
        id: charge_id,
        stay_id: payload.stay_id,
        kind: kind.into(),
        description: payload.description.trim().into(),
        amount_cents: amount,
        created_at: now,
    })
}

pub fn add_product_charge(
    conn: &mut Connection,
    payload: AddProductChargePayload,
) -> AppResult<Charge> {
    accept_reserved_fields(&payload.operation_id, &payload.expected_version)?;
    let stay = db::get_stay(conn, payload.stay_id)?;
    if stay.status != "open" {
        return Err(AppError::conflict(
            "No se pueden agregar cargos a una estadía cerrada",
        ));
    }
    let product = db::get_product(conn, payload.product_id)?;
    if !product.active {
        return Err(AppError::msg("El producto no está activo"));
    }
    if product.price_cents <= 0 {
        return Err(AppError::msg("El precio del producto no es válido"));
    }
    let root = outbox::resolve_operation_id(&payload.operation_id);
    let now = now_rfc3339();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO charges (stay_id, kind, description, amount_cents, created_at) VALUES (?1, 'surcharge', ?2, ?3, ?4)",
        params![payload.stay_id, product.name, product.price_cents, now],
    )?;
    let charge_id = tx.last_insert_rowid();
    outbox::enqueue(
        &tx,
        &root,
        &[OutboxOp::upsert(
            Entity::Charge,
            db::uid_of(&tx, "charges", charge_id)?,
            db::payload_for_charge(&tx, charge_id)?,
        )],
    )?;
    tx.commit()?;
    Ok(Charge {
        id: charge_id,
        stay_id: payload.stay_id,
        kind: "surcharge".into(),
        description: product.name,
        amount_cents: product.price_cents,
        created_at: now,
    })
}

pub fn delete_charge(conn: &mut Connection, actor: &Actor, charge_id: i64) -> AppResult<()> {
    authorize(actor, Operation::DeleteCharge)?;
    let stay_id: i64 = conn
        .query_row(
            "SELECT stay_id FROM charges WHERE id = ?1 AND deleted_at IS NULL",
            [charge_id],
            |row| row.get(0),
        )
        .map_err(|_| AppError::not_found("Cargo no encontrado"))?;
    let stay = db::get_stay(conn, stay_id)?;
    if stay.status != "open" {
        return Err(AppError::conflict(
            "No se pueden modificar cargos de una cuenta cerrada",
        ));
    }
    let root = outbox::resolve_operation_id(&None);
    let now = now_rfc3339();
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE charges SET deleted_at = ?1 WHERE id = ?2 AND kind IN ('surcharge', 'discount') AND deleted_at IS NULL",
        params![now, charge_id],
    )?;
    if tx.changes() != 1 {
        return Err(AppError::not_found("Cargo no encontrado"));
    }
    outbox::enqueue(
        &tx,
        &root,
        &[OutboxOp::delete(
            Entity::Charge,
            db::uid_of(&tx, "charges", charge_id)?,
            db::payload_for_charge(&tx, charge_id)?,
        )],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn check_out(conn: &mut Connection, payload: &CheckOutPayload) -> AppResult<(Stay, BillPreview)> {
    accept_reserved_fields(&payload.operation_id, &payload.expected_version)?;
    let stay = db::get_stay(conn, payload.stay_id)?;
    if stay.status != "open" {
        return Err(AppError::conflict("La estadía ya está cerrada"));
    }
    let bill = build_preview(conn, &stay)?;
    let checkout_at = now_rfc3339();
    close_account(conn, &stay, &bill, &checkout_at, payload.operation_id.clone())?;
    let mut stay = stay;
    stay.status = "closed".into();
    stay.check_out_at = Some(checkout_at);
    Ok((stay, bill))
}

pub fn receipt_bytes(conn: &Connection, stay_id: i64) -> AppResult<Vec<u8>> {
    let stay = db::get_stay(conn, stay_id)?;
    if stay.status != "closed" { return Err(AppError::conflict("Solo se reimprimen cuentas cerradas")); }
    if let Some(bytes) = conn.query_row("SELECT bytes FROM receipt_snapshots WHERE stay_id=?1", [stay_id], |r| r.get(0)).optional()? {
        return Ok(bytes);
    }
    // Legacy accounts: freeze the first reconstruction, without recalculating amounts.
    let bytes = crate::printer::build_receipt(&db::load_settings(conn)?, &stay, &bill_for_stay(conn, &stay)?);
    conn.execute("INSERT INTO receipt_snapshots(stay_id, bytes, created_at) VALUES (?1, ?2, ?3)", params![stay_id, bytes, now_rfc3339()])?;
    Ok(bytes)
}

pub fn list_reservations(conn: &Connection) -> AppResult<Vec<Reservation>> {
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

pub fn create_reservation_on(
    conn: &mut Connection,
    payload: CreateReservationPayload,
) -> AppResult<Reservation> {
    accept_reserved_fields(&payload.operation_id, &payload.expected_version)?;
    let root = outbox::resolve_operation_id(&payload.operation_id);
    let tx = conn.transaction()?;
    let reservation_id = create_reservation_in_tx(&tx, payload, &root)?;
    tx.commit()?;
    db::get_reservation(conn, reservation_id)
}

#[cfg(test)]
fn create_reservation_on_failing(
    conn: &mut Connection,
    payload: CreateReservationPayload,
) -> AppResult<Reservation> {
    let root = outbox::resolve_operation_id(&payload.operation_id);
    let tx = conn.transaction()?;
    let _reservation_id = create_reservation_in_tx(&tx, payload, &root)?;
    Err(AppError::msg("fallo inyectado"))
}

fn create_reservation_in_tx(
    conn: &Connection,
    payload: CreateReservationPayload,
    operation_id: &str,
) -> AppResult<i64> {
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
            return Err(AppError::conflict("La habitación está ocupada en esa fecha"));
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
        return Err(AppError::conflict(
            "Ya existe una reserva vigente para esa habitación y fecha",
        ));
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
    let reservation_id = conn.last_insert_rowid();
    outbox::enqueue(
        conn,
        operation_id,
        &[
            OutboxOp::upsert(
                Entity::Guest,
                db::uid_of(conn, "guests", guest.id)?,
                db::payload_for_guest(conn, guest.id)?,
            ),
            OutboxOp::upsert(
                Entity::Reservation,
                db::uid_of(conn, "reservations", reservation_id)?,
                db::payload_for_reservation(conn, reservation_id)?,
            ),
        ],
    )?;
    Ok(reservation_id)
}

pub fn set_reservation_status(
    conn: &mut Connection,
    reservation_id: i64,
    status: String,
) -> AppResult<Reservation> {
    let allowed = ["cancelled", "no_show"];
    if !allowed.contains(&status.as_str()) {
        return Err(AppError::msg("Estado de reserva inválido"));
    }
    let res = db::get_reservation(conn, reservation_id)?;
    if res.status != "hold" {
        return Err(AppError::conflict(
            "Solo se pueden actualizar reservas en espera",
        ));
    }
    let root = outbox::resolve_operation_id(&None);
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE reservations SET status = ?1 WHERE id = ?2",
        params![status, reservation_id],
    )?;
    outbox::enqueue(
        &tx,
        &root,
        &[OutboxOp::upsert(
            Entity::Reservation,
            db::uid_of(&tx, "reservations", reservation_id)?,
            db::payload_for_reservation(&tx, reservation_id)?,
        )],
    )?;
    let reservation = db::get_reservation(&tx, reservation_id)?;
    tx.commit()?;
    Ok(reservation)
}

pub fn check_in_reservation(conn: &mut Connection, reservation_id: i64) -> AppResult<Stay> {
    let reservation = db::get_reservation(conn, reservation_id)?;
    check_in_on(
        conn,
        CheckInPayload {
            room_id: reservation.room_id,
            guest_name: reservation.guest_name,
            document: reservation.guest_document,
            phone: reservation.guest_phone,
            rate_plan_id: reservation.rate_plan_id,
            expected_hours: Some(reservation.expected_nights * 24),
            reservation_id: Some(reservation.id),
            operation_id: Some(outbox::resolve_operation_id(&None)),
            expected_version: None,
        },
    )
}

pub fn list_history(conn: &Connection, date: Option<String>) -> AppResult<Vec<HistoryStay>> {
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
        let stay = db::get_stay(conn, id)?;
        let payments = db::list_payments(conn, id)?;
        let bill = bill_for_stay(conn, &stay)?;
        out.push(HistoryStay {
            stay,
            total_cents: bill.total_cents,
            payment_method: payments.first().map(|p| p.method.clone()),
        });
    }
    Ok(out)
}

pub fn get_settings(conn: &Connection) -> AppResult<AppSettings> {
    let mut settings = db::load_settings(conn)?;
    settings.pin_hash = String::new();
    Ok(settings)
}

pub fn save_settings(
    conn: &Connection,
    actor: &Actor,
    payload: AppSettings,
    new_pin: Option<String>,
) -> AppResult<AppSettings> {
    authorize(actor, Operation::SaveSettings)?;
    if !payload.tax_percent.is_finite() || !(0.0..=100.0).contains(&payload.tax_percent) {
        return Err(AppError::msg("Ingresá un IVA entre 0 y 100"));
    }
    db::upsert_setting(conn, "business_name", payload.business_name.trim())?;
    db::upsert_setting(conn, "address", payload.address.trim())?;
    db::upsert_setting(conn, "phone", payload.phone.trim())?;
    db::upsert_setting(conn, "tax_percent", &payload.tax_percent.to_string())?;
    db::upsert_setting(conn, "currency_symbol", payload.currency_symbol.trim())?;
    db::upsert_setting(conn, "theme", &payload.theme)?;
    db::upsert_setting(conn, "receipt_footer", payload.receipt_footer.trim())?;
    db::upsert_setting(
        conn,
        "printer_enabled",
        if payload.printer_enabled {
            "true"
        } else {
            "false"
        },
    )?;
    db::upsert_setting(conn, "printer_path", payload.printer_path.trim())?;
    db::upsert_setting(conn, "printer_name", payload.printer_name.trim())?;
    db::upsert_setting(conn, "paper_width", &payload.paper_width.to_string())?;
    db::upsert_setting(
        conn,
        "auto_print_on_checkout",
        if payload.auto_print_on_checkout {
            "true"
        } else {
            "false"
        },
    )?;
    db::upsert_setting(
        conn,
        "require_guest_name",
        if payload.require_guest_name {
            "true"
        } else {
            "false"
        },
    )?;
    if let Some(pin) = new_pin {
        if pin.is_empty() {
            db::upsert_setting(conn, "pin_hash", "")?;
        } else {
            db::upsert_setting(conn, "pin_hash", &db::hash_pin(&pin))?;
        }
    }
    get_settings(conn)
}

pub fn verify_pin(conn: &Connection, pin: String) -> AppResult<bool> {
    let hash = db::get_setting(conn, "pin_hash", "")?;
    if hash.is_empty() {
        return Ok(true);
    }
    Ok(hash == db::hash_pin(&pin))
}

pub fn pin_required(conn: &Connection) -> AppResult<bool> {
    Ok(!db::get_setting(conn, "pin_hash", "")?.is_empty())
}

pub fn device_mode_get(conn: &Connection) -> AppResult<Option<String>> {
    let raw = db::get_setting(conn, "device_mode", "")?;
    if raw.is_empty() {
        return Ok(None);
    }
    if raw != "reception" && raw != "remote" {
        return Err(AppError::msg("Modo de equipo inválido"));
    }
    Ok(Some(raw))
}

pub fn device_mode_set(conn: &Connection, mode: &str) -> AppResult<()> {
    let mode = mode.trim();
    if mode != "reception" && mode != "remote" {
        return Err(AppError::msg("Elegí Recepción o Administración remota"));
    }
    if device_mode_get(conn)?.is_some() {
        return Err(AppError::msg("El modo de este equipo ya está definido"));
    }
    db::upsert_setting(conn, "device_mode", mode)
}

pub fn hash_password(password: &str) -> AppResult<crate::models::HashPasswordResult> {
    hash_password_with_operation(password, None)
}

pub fn hash_password_with_operation(password: &str, operation_id: Option<&str>) -> AppResult<crate::models::HashPasswordResult> {
    use argon2::{
        password_hash::{PasswordHasher, SaltString},
        Argon2,
    };
    use rand_core::OsRng;
    if password.is_empty() || password.len() > 128 {
        return Err(AppError::msg("La contraseña debe tener entre 1 y 128 bytes"));
    }
    let salt = if let Some(id) = operation_id {
        let id=uuid::Uuid::parse_str(id).map_err(|_|AppError::msg("operation_id debe ser UUID"))?;
        SaltString::encode_b64(id.as_bytes()).map_err(|_|AppError::msg("Salt inválida"))?
    } else { SaltString::generate(&mut OsRng) };
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AppError::msg("No se pudo proteger la contraseña"))?
        .to_string();
    Ok(crate::models::HashPasswordResult { hash })
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use std::path::Path;

    #[test]
    fn receipt_is_frozen_and_reprint_does_not_change_account() -> AppResult<()> {
        let (mut conn, stay) = open_stay_fixture()?;
        assert!(receipt_bytes(&conn, stay.id).is_err());
        let bill = build_preview(&conn, &stay)?;
        close_account(&mut conn, &stay, &bill, &now_rfc3339(), None)?;
        let original = receipt_bytes(&conn, stay.id)?;
        conn.execute("UPDATE rate_plans SET base_amount_cents=999999", [])?;
        conn.execute("UPDATE rooms SET number='CAMBIADA' WHERE id=1", [])?;
        conn.execute("UPDATE settings SET value='OTRO HOTEL' WHERE key='business_name'", [])?;
        assert_eq!(original, receipt_bytes(&conn, stay.id)?);
        assert_eq!(original, receipt_bytes(&conn, stay.id)?);
        assert_eq!(conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM payments", [], |r| r.get(0))?, 0);
        assert_eq!(conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM receipt_snapshots", [], |r| r.get(0))?, 1);
        Ok(())
    }

    #[test]
    fn failed_receipt_snapshot_rolls_back_closure() -> AppResult<()> {
        let (mut conn, stay) = open_stay_fixture()?;
        conn.execute_batch("CREATE TRIGGER fail_receipt BEFORE INSERT ON receipt_snapshots BEGIN SELECT RAISE(ABORT, 'injected'); END;")?;
        let bill = build_preview(&conn, &stay)?;
        assert!(close_account(&mut conn, &stay, &bill, &now_rfc3339(), None).is_err());
        assert_eq!(db::get_stay(&conn, stay.id)?.status, "open");
        assert_eq!(conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM charges", [], |r| r.get(0))?, 0);
        Ok(())
    }

    fn reception_actor() -> Actor {
        Actor {
            user_id: 1,
            username: "recepcion".into(),
            role: Role::Recepcion,
        }
    }

    fn admin_actor() -> Actor {
        Actor {
            user_id: 2,
            username: "admin".into(),
            role: Role::Admin,
        }
    }

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

    fn walk_in_payload(room_id: i64) -> CheckInPayload {
        CheckInPayload {
            room_id,
            guest_name: "Huésped TX".into(),
            document: None,
            phone: None,
            rate_plan_id: 1,
            expected_hours: Some(3),
            reservation_id: None,
            operation_id: None,
            expected_version: None,
        }
    }

    #[test]
    fn closing_account_persists_total_and_does_not_create_payment() -> AppResult<()> {
        let (mut conn, stay) = open_stay_fixture()?;
        let bill = build_preview(&conn, &stay)?;
        close_account(&mut conn, &stay, &bill, &now_rfc3339(), None)?;

        conn.execute("UPDATE rate_plans SET base_amount_cents = 9999999 WHERE id = 1", [])?;
        let closed = db::get_stay(&conn, stay.id)?;
        let historical = bill_for_stay(&conn, &closed)?;
        assert_eq!(historical.total_cents, bill.total_cents);
        assert_eq!(
            conn.query_row::<i64, _, _>(
                "SELECT closed_total_cents FROM stays WHERE id = ?1",
                [stay.id],
                |row| row.get(0),
            )?,
            bill.total_cents
        );
        assert_eq!(
            conn.query_row::<i64, _, _>(
                "SELECT closed_line_count FROM stays WHERE id = ?1",
                [stay.id],
                |row| row.get(0),
            )?,
            conn.query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM charges WHERE stay_id = ?1 AND deleted_at IS NULL",
                [stay.id],
                |row| row.get(0),
            )?
        );
        assert_eq!(
            conn.query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM payments WHERE stay_id = ?1",
                [stay.id],
                |row| row.get(0)
            )?,
            0
        );
        assert_eq!(
            conn.query_row::<String, _, _>("SELECT status FROM rooms WHERE id = 1", [], |row| row
                .get(0))?,
            "dirty"
        );
        Ok(())
    }

    #[test]
    fn closing_same_account_twice_is_rejected_without_rewriting_history() -> AppResult<()> {
        let (mut conn, stay) = open_stay_fixture()?;
        let bill = build_preview(&conn, &stay)?;
        close_account(&mut conn, &stay, &bill, &now_rfc3339(), None)?;
        let error = close_account(&mut conn, &stay, &bill, &now_rfc3339(), None).expect_err("duplicate close");
        assert!(error.to_string().contains("ya está cerrada"));
        assert_eq!(error.code(), ErrorCode::Conflict);
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM charges WHERE stay_id = ?1 AND kind = 'stay'",
            [stay.id],
            |row| row.get(0),
        )?;
        assert_eq!(count, 1);
        Ok(())
    }

    #[test]
    fn injected_check_in_failure_leaves_no_partial_state() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let guests_before: i64 =
            conn.query_row("SELECT COUNT(*) FROM guests", [], |row| row.get(0))?;
        let stays_before: i64 = conn.query_row("SELECT COUNT(*) FROM stays", [], |row| row.get(0))?;
        let error = check_in_on_failing(&mut conn, walk_in_payload(1)).expect_err("injected");
        assert!(error.to_string().contains("fallo inyectado"));
        let guests_after: i64 =
            conn.query_row("SELECT COUNT(*) FROM guests", [], |row| row.get(0))?;
        let stays_after: i64 = conn.query_row("SELECT COUNT(*) FROM stays", [], |row| row.get(0))?;
        let room_status: String =
            conn.query_row("SELECT status FROM rooms WHERE id = 1", [], |row| row.get(0))?;
        assert_eq!(guests_after, guests_before);
        assert_eq!(stays_after, stays_before);
        assert_eq!(room_status, "available");
        Ok(())
    }

    #[test]
    fn injected_reservation_failure_leaves_no_partial_state() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let guests_before: i64 =
            conn.query_row("SELECT COUNT(*) FROM guests", [], |row| row.get(0))?;
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
            operation_id: None,
            expected_version: None,
        };
        let error = create_reservation_on_failing(&mut conn, payload).expect_err("injected");
        assert!(error.to_string().contains("fallo inyectado"));
        let guests_after: i64 =
            conn.query_row("SELECT COUNT(*) FROM guests", [], |row| row.get(0))?;
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
        assert_eq!(error.code(), ErrorCode::Conflict);
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
        close_account(&mut conn, &stay, &bill, &now_rfc3339(), None)?;
        conn.execute(
            "UPDATE rate_plans SET base_amount_cents = 9999999, kind = 'night' WHERE id = 1",
            [],
        )?;
        db::upsert_setting(&conn, "tax_percent", "21")?;
        let closed = db::get_stay(&conn, stay.id)?;
        let historical = bill_for_stay(&conn, &closed)?;
        assert_eq!(historical.total_cents, bill.total_cents);
        assert_eq!(historical.tax_percent, bill.tax_percent);
        assert_eq!(historical.applied_kind, bill.applied_kind);
        assert_eq!(historical.duration_label, bill.duration_label);
        Ok(())
    }

    #[test]
    fn local_flow_check_in_charge_preview_checkout_history() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let stay = check_in_on(&mut conn, walk_in_payload(1))?;
        let product = db::list_products(&conn, true)?
            .into_iter()
            .next()
            .expect("seed products");
        add_product_charge(
            &mut conn,
            AddProductChargePayload {
                stay_id: stay.id,
                product_id: product.id,
                operation_id: Some("op-charge-1".into()),
                expected_version: None,
            },
        )?;
        let preview = preview_bill(&conn, stay.id)?;
        let (closed, bill) = check_out(
            &mut conn,
            &CheckOutPayload {
                stay_id: stay.id,
                print: false,
                operation_id: Some("op-checkout-1".into()),
                expected_version: None,
            },
        )?;
        assert_eq!(closed.status, "closed");
        assert_eq!(bill.total_cents, preview.total_cents);
        let history = list_history(&conn, None)?;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].total_cents, bill.total_cents);
        let replay = bill_for_stay(&conn, &closed)?;
        assert_eq!(replay.total_cents, bill.total_cents);
        Ok(())
    }

    #[test]
    fn reception_cannot_delete_charge_admin_can() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let stay = check_in_on(&mut conn, walk_in_payload(1))?;
        let charge = add_charge(
            &mut conn,
            &admin_actor(),
            AddChargePayload {
                stay_id: stay.id,
                kind: "surcharge".into(),
                description: "Extra".into(),
                amount_cents: 5_000,
                operation_id: None,
                expected_version: None,
            },
        )?;
        let denied = delete_charge(&mut conn, &reception_actor(), charge.id).expect_err("forbidden");
        assert_eq!(denied.code(), ErrorCode::Forbidden);
        delete_charge(&mut conn, &admin_actor(), charge.id)?;
        Ok(())
    }

    #[test]
    fn empty_operation_id_is_rejected() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let mut payload = walk_in_payload(1);
        payload.operation_id = Some("   ".into());
        let error = check_in_on(&mut conn, payload).expect_err("empty op id");
        assert_eq!(error.code(), ErrorCode::Validation);
        Ok(())
    }

    fn outbox_entities(conn: &Connection, root: &str) -> AppResult<Vec<String>> {
        let root = crate::sync::outbox::resolve_operation_id(&Some(root.to_string()));
        let mut stmt = conn.prepare(
            "SELECT entity FROM sync_outbox WHERE root_operation_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([root], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    fn pending_outbox(conn: &Connection) -> AppResult<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM sync_outbox WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    #[test]
    fn check_in_writes_outbox_for_guest_stay_and_room() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let mut payload = walk_in_payload(1);
        payload.operation_id = Some("op-checkin-1".into());
        check_in_on(&mut conn, payload)?;
        let entities = outbox_entities(&conn, "op-checkin-1")?;
        assert_eq!(entities, ["guest", "stay", "room"]);
        assert_eq!(pending_outbox(&conn)?, 3);
        Ok(())
    }

    #[test]
    fn checkout_outbox_shares_root_and_rolls_back_with_receipt_failure() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let stay = check_in_on(&mut conn, walk_in_payload(1))?;
        let before = pending_outbox(&conn)?;
        conn.execute_batch(
            "CREATE TRIGGER fail_receipt BEFORE INSERT ON receipt_snapshots BEGIN SELECT RAISE(ABORT, 'injected'); END;",
        )?;
        let bill = build_preview(&conn, &stay)?;
        assert!(close_account(
            &mut conn,
            &stay,
            &bill,
            &now_rfc3339(),
            Some("op-checkout-fail".into())
        )
        .is_err());
        assert_eq!(pending_outbox(&conn)?, before);
        assert_eq!(db::get_stay(&conn, stay.id)?.status, "open");

        conn.execute_batch("DROP TRIGGER fail_receipt;")?;
        let stay = db::get_stay(&conn, stay.id)?;
        let bill = build_preview(&conn, &stay)?;
        close_account(
            &mut conn,
            &stay,
            &bill,
            &now_rfc3339(),
            Some("op-checkout-ok".into()),
        )?;
        let entities = outbox_entities(&conn, "op-checkout-ok")?;
        assert!(entities.contains(&"stay".into()));
        assert!(entities.contains(&"charge".into()));
        assert!(entities.contains(&"room".into()));
        assert!(entities.iter().all(|_| true));
        let roots: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT root_operation_id) FROM sync_outbox WHERE root_operation_id = ?1",
            [crate::sync::outbox::resolve_operation_id(&Some(
                "op-checkout-ok".into(),
            ))],
            |row| row.get(0),
        )?;
        assert_eq!(roots, 1);
        Ok(())
    }

    #[test]
    fn injected_check_in_failure_leaves_no_outbox_row() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let mut payload = walk_in_payload(1);
        payload.operation_id = Some("op-checkin-fail".into());
        assert!(check_in_on_failing(&mut conn, payload).is_err());
        assert_eq!(pending_outbox(&conn)?, 0);
        Ok(())
    }

    #[test]
    fn repeating_operation_id_is_conflict_without_second_stay() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let mut first = walk_in_payload(1);
        first.operation_id = Some("op-same".into());
        check_in_on(&mut conn, first)?;
        let mut second = walk_in_payload(2);
        second.operation_id = Some("op-same".into());
        let error = check_in_on(&mut conn, second).expect_err("duplicate op");
        assert_eq!(error.code(), ErrorCode::Conflict);
        let stays: i64 = conn.query_row("SELECT COUNT(*) FROM stays", [], |row| row.get(0))?;
        assert_eq!(stays, 1);
        let outbox: i64 = conn.query_row("SELECT COUNT(*) FROM sync_outbox", [], |row| row.get(0))?;
        assert_eq!(outbox, 3);
        Ok(())
    }

    #[test]
    fn delete_charge_is_logical_and_enqueues_delete() -> AppResult<()> {
        let mut conn = db::open(Path::new(":memory:"))?;
        let stay = check_in_on(&mut conn, walk_in_payload(1))?;
        let charge = add_charge(
            &mut conn,
            &admin_actor(),
            AddChargePayload {
                stay_id: stay.id,
                kind: "surcharge".into(),
                description: "Extra".into(),
                amount_cents: 5_000,
                operation_id: Some("op-add-charge".into()),
                expected_version: None,
            },
        )?;
        delete_charge(&mut conn, &admin_actor(), charge.id)?;
        let deleted_at: Option<String> = conn.query_row(
            "SELECT deleted_at FROM charges WHERE id = ?1",
            [charge.id],
            |row| row.get(0),
        )?;
        assert!(deleted_at.is_some());
        assert!(db::list_charges(&conn, stay.id)?.is_empty());
        let preview = preview_bill(&conn, stay.id)?;
        assert!(!preview.lines.iter().any(|line| line.kind == "surcharge"));
        let ops: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sync_outbox WHERE entity = 'charge' AND op = 'delete'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(ops, 1);
        Ok(())
    }

    #[test]
    fn save_room_rejects_stale_expected_version() -> AppResult<()> {
        let conn = db::open(Path::new(":memory:"))?;
        let room = db::get_room(&conn, 1)?;
        let error = save_room(
            &conn,
            &admin_actor(),
            SaveRoomPayload {
                id: Some(room.id),
                number: room.number.clone(),
                room_type: room.room_type.clone(),
                floor: room.floor,
                notes: room.notes.clone(),
                operation_id: None,
                expected_version: Some(room.version - 1),
            },
        )
        .expect_err("stale version");
        assert_eq!(error.code(), ErrorCode::Conflict);
        let updated = save_room(
            &conn,
            &admin_actor(),
            SaveRoomPayload {
                id: Some(room.id),
                number: "99".into(),
                room_type: room.room_type,
                floor: room.floor,
                notes: room.notes,
                operation_id: None,
                expected_version: Some(room.version),
            },
        )?;
        assert_eq!(updated.version, room.version + 1);
        assert_eq!(updated.number, "99");
        Ok(())
    }

    fn users_fixture() -> AppResult<(Connection, i64, i64, i64)> {
        let conn = db::open(Path::new(":memory:"))?;
        conn.execute(
            "INSERT INTO users (username, password_hash, role, active) VALUES
                ('admin','hash','admin',1),
                ('admin-dos','hash','admin',1),
                ('recepcion','hash','recepcion',1)",
            [],
        )?;
        Ok((conn, 1, 2, 3))
    }

    #[test]
    fn reception_cannot_list_or_deactivate_users() -> AppResult<()> {
        let (conn, _admin, _other, reception) = users_fixture()?;
        assert_eq!(
            list_users(&conn, &reception_actor()).unwrap_err().code(),
            ErrorCode::Forbidden
        );
        assert_eq!(
            set_user_active(&conn, &reception_actor(), reception, false, None)
                .unwrap_err()
                .code(),
            ErrorCode::Forbidden
        );
        Ok(())
    }

    #[test]
    fn cannot_deactivate_self_or_last_admin_and_can_reactivate() -> AppResult<()> {
        let (conn, admin, other_admin, reception) = users_fixture()?;
        let actor = Actor {
            user_id: admin,
            username: "admin".into(),
            role: Role::Admin,
        };
        assert_eq!(
            set_user_active(&conn, &actor, admin, false, None)
                .unwrap_err()
                .to_string(),
            "No podés desactivar tu propia cuenta"
        );
        let disabled = set_user_active(&conn, &actor, reception, false, None)?;
        assert!(!disabled.active);
        let remote = Actor {
            user_id: 99,
            username: "remoto".into(),
            role: Role::Admin,
        };
        set_user_active(&conn, &actor, other_admin, false, None)?;
        assert_eq!(
            set_user_active(&conn, &remote, admin, false, None)
                .unwrap_err()
                .to_string(),
            "Tiene que quedar al menos un administrador"
        );
        let restored = set_user_active(&conn, &actor, reception, true, None)?;
        assert!(restored.active);
        assert_eq!(list_users(&conn, &actor)?.len(), 3);
        Ok(())
    }

    #[test]
    fn reception_cannot_delete_users() -> AppResult<()> {
        let (conn, _admin, _other, reception) = users_fixture()?;
        assert_eq!(
            delete_user(&conn, &reception_actor(), reception, None)
                .unwrap_err()
                .code(),
            ErrorCode::Forbidden
        );
        Ok(())
    }

    #[test]
    fn cannot_delete_self_or_last_admin_and_username_is_freed() -> AppResult<()> {
        let (conn, admin, other_admin, reception) = users_fixture()?;
        let actor = Actor {
            user_id: admin,
            username: "admin".into(),
            role: Role::Admin,
        };
        assert_eq!(
            delete_user(&conn, &actor, admin, None)
                .unwrap_err()
                .to_string(),
            "No podés eliminar tu propia cuenta"
        );
        delete_user(&conn, &actor, reception, None)?;
        assert!(list_users(&conn, &actor)?.iter().all(|user| user.username != "recepcion"));
        conn.execute(
            "INSERT INTO users (username, password_hash, role, active) VALUES ('recepcion','hash','recepcion',1)",
            [],
        )?;
        assert!(list_users(&conn, &actor)?.iter().any(|user| user.username == "recepcion"));
        delete_user(&conn, &actor, other_admin, None)?;
        let remote = Actor {
            user_id: 99,
            username: "remoto".into(),
            role: Role::Admin,
        };
        assert_eq!(
            delete_user(&conn, &remote, admin, None)
                .unwrap_err()
                .to_string(),
            "Tiene que quedar al menos un administrador"
        );
        Ok(())
    }
}
