use crate::{
    db,
    error::{AppError, AppResult},
    models::{
        AnalyticsDay, AnalyticsExtra, AnalyticsHour, AnalyticsHourRevenue, AnalyticsRoomTotal,
        AnalyticsStatusCount, AnalyticsSummary, AnalyticsTypeTotal,
        AnalyticsProduct, AnalyticsPurchase, AnalyticsHeatCell, AnalyticsStayMode,
    },
    service,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::{America::Asuncion, Tz};
use rusqlite::Connection;
use std::collections::BTreeMap;

const MAX_DAYS: i64 = 366;

fn business_time(value: &str) -> AppResult<DateTime<Tz>> {
    DateTime::parse_from_rfc3339(value).map(|date| date.with_timezone(&Asuncion))
        .map_err(|_| AppError::storage("Fecha de análisis inválida"))
}

fn validate_date(value: &str) -> AppResult<NaiveDate> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::msg("Elegí una fecha válida"))?;
    if date.to_string() != value || date > Utc::now().with_timezone(&Asuncion).date_naive() {
        return Err(AppError::msg("Elegí una fecha válida, sin fechas futuras"));
    }
    Ok(date)
}

fn observed_block(day: NaiveDate, hour: u32, now: DateTime<Tz>) -> bool {
    let end = day.and_hms_opt(0, 0, 0).unwrap() + Duration::hours(i64::from(hour + 2));
    Asuncion.from_local_datetime(&end).latest().is_some_and(|end| end <= now)
}

fn heatmap(start: NaiveDate, end: NaiveDate, now: DateTime<Tz>) -> Vec<AnalyticsHeatCell> {
    let mut cells: Vec<_> = (0..7).flat_map(|weekday| (0..24).step_by(2).map(move |hour|
        AnalyticsHeatCell { weekday, hour, count: 0, observed_blocks: 0, average: None }
    )).collect();
    for day in start.iter_days().take_while(|day| *day <= end) {
        for hour in (0..24).step_by(2) {
            if observed_block(day, hour, now) {
                cells[(day.weekday().num_days_from_monday() * 12 + hour / 2) as usize].observed_blocks += 1;
            }
        }
    }
    cells
}

pub fn validate_range(from: &str, to: &str) -> AppResult<(NaiveDate, NaiveDate)> {
    let start = validate_date(from)?;
    let end = validate_date(to)?;
    if start > end {
        return Err(AppError::msg(
            "La fecha inicial debe ser anterior o igual a la final",
        ));
    }
    if (end - start).num_days() >= MAX_DAYS {
        return Err(AppError::msg("El período no puede superar 366 días"));
    }
    Ok((start, end))
}

fn room_type_filter(room_type: Option<&str>) -> AppResult<Option<&'static str>> {
    match room_type.map(str::trim) {
        None | Some("") => Ok(None),
        Some(value) if value.eq_ignore_ascii_case("all") => Ok(None),
        Some(value) if value.eq_ignore_ascii_case("normal") => Ok(Some("normal")),
        Some(value) if value.eq_ignore_ascii_case("jacuzzi") => Ok(Some("jacuzzi")),
        _ => Err(AppError::msg(
            "El tipo de habitación debe ser Normal o Jacuzzi",
        )),
    }
}

fn matches_type(room_type: &str, filter: Option<&str>) -> bool {
    filter.is_none_or(|expected| room_type.eq_ignore_ascii_case(expected))
}

fn checked_add(target: &mut i64, amount: i64) -> AppResult<()> {
    *target = target
        .checked_add(amount)
        .ok_or_else(|| AppError::storage("Los importes del análisis exceden el rango admitido"))?;
    Ok(())
}

struct StayRow {
    id: i64,
    check_in_at: String,
    check_out_at: Option<String>,
    status: String,
    room_number: String,
    room_type: String,
    product_tracking_since: Option<String>,
}

struct ReservationRow {
    expected_arrival_at: String,
    status: String,
    room_type: String,
}

pub fn summary(
    conn: &Connection,
    from: &str,
    to: &str,
    room_type: Option<&str>,
) -> AppResult<AnalyticsSummary> {
    let (start, end) = validate_range(from, to)?;
    let filter = room_type_filter(room_type)?;
    let now = Utc::now().with_timezone(&Asuncion);
    // A read transaction keeps the status snapshot and all historical totals consistent.
    let tx = conn.unchecked_transaction()?;
    let mut daily = BTreeMap::new();
    let mut day = start;
    loop {
        daily.insert(
            day,
            AnalyticsDay {
                date: day.format("%Y-%m-%d").to_string(),
                revenue_cents: 0,
                closed_accounts: 0,
                check_ins: 0,
                reservation_arrivals: 0,
                reservation_cancellations: 0,
                no_shows: 0,
            },
        );
        if day == end {
            break;
        }
        day = day
            .checked_add_signed(Duration::days(1))
            .ok_or_else(|| AppError::msg("Fecha fuera de rango"))?;
    }

    let mut status_counts = BTreeMap::<String, i64>::new();
    for status in ["available", "occupied", "dirty", "reserved", "blocked"] {
        status_counts.insert(status.to_string(), 0);
    }
    for board_room in service::list_board(&tx)? {
        if matches_type(&board_room.room.room_type, filter) {
            *status_counts.entry(board_room.display_status).or_default() += 1;
        }
    }

    let stays: Vec<StayRow> = tx
        .prepare(
            "SELECT s.id, s.check_in_at, s.check_out_at, s.status, r.number, r.room_type, s.product_tracking_since
             FROM stays s JOIN rooms r ON r.id = s.room_id ORDER BY s.id",
        )?
        .query_map([], |row| {
            Ok(StayRow {
                id: row.get(0)?,
                check_in_at: row.get(1)?,
                check_out_at: row.get(2)?,
                status: row.get(3)?,
                room_number: row.get(4)?,
                room_type: row.get(5)?,
                product_tracking_since: row.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let reservations: Vec<ReservationRow> = tx
        .prepare(
            "SELECT res.expected_arrival_at, res.status, r.room_type
             FROM reservations res JOIN rooms r ON r.id = res.room_id ORDER BY res.id",
        )?
        .query_map([], |row| {
            Ok(ReservationRow {
                expected_arrival_at: row.get(0)?,
                status: row.get(1)?,
                room_type: row.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut result = AnalyticsSummary {
        from: from.into(),
        to: to.into(),
        generated_at: now.to_rfc3339(),
        total_revenue_cents: 0,
        closed_accounts: 0,
        average_ticket_cents: 0,
        lodging_cents: 0,
        extras_cents: 0,
        discount_cents: 0,
        tax_cents: 0,
        average_stay_minutes: None,
        reservation_arrivals: 0,
        reservation_cancellations: 0,
        no_shows: 0,
        current_rooms: Vec::new(),
        daily: Vec::new(),
        check_in_hours: (0..24)
            .map(|hour| AnalyticsHour { hour, count: 0 })
            .collect(),
        by_room_type: Vec::new(),
        by_room: Vec::new(),
        top_extras: Vec::new(),
        product_tracking_since: Some(tx.query_row("SELECT value FROM settings WHERE key='product_tracking_since'", [], |r| r.get(0))?),
        product_sales: Vec::new(),
        product_purchase: AnalyticsPurchase::default(),
        check_in_heatmap: heatmap(start, end, now),
        by_stay_mode: ["hourly", "overnight", "unclassified"].into_iter().map(|mode|
            AnalyticsStayMode { mode: mode.into(), closed_accounts: 0, lodging_cents: 0, average_lodging_cents: None }
        ).collect(),
        closed_hours: (0..24)
            .map(|hour| AnalyticsHourRevenue { hour, count: 0, revenue_cents: 0 })
            .collect(),
        voided_count: 0,
        voided_cents: 0,
    };
    let mut type_totals = BTreeMap::<String, (i64, i64)>::new();
    let mut room_totals = BTreeMap::<(String, String), (i64, i64)>::new();
    let mut extras = BTreeMap::<String, (i64, i64)>::new();
    let mut duration_total = 0_i64;
    let mut products = BTreeMap::<String, (AnalyticsProduct, DateTime<Tz>)>::new();

    for reservation in reservations {
        if !matches_type(&reservation.room_type, filter) {
            continue;
        }
        let arrival = business_time(&reservation.expected_arrival_at)?;
        let Some(day) = daily.get_mut(&arrival.date_naive()) else {
            continue;
        };
        checked_add(&mut result.reservation_arrivals, 1)?;
        checked_add(&mut day.reservation_arrivals, 1)?;
        match reservation.status.as_str() {
            "cancelled" => {
                checked_add(&mut result.reservation_cancellations, 1)?;
                checked_add(&mut day.reservation_cancellations, 1)?;
            }
            "no_show" => {
                checked_add(&mut result.no_shows, 1)?;
                checked_add(&mut day.no_shows, 1)?;
            }
            _ => {}
        }
    }

    for row in stays {
        if !matches_type(&row.room_type, filter) {
            continue;
        }
        let check_in = business_time(&row.check_in_at)?;
        if let Some(day) = daily.get_mut(&check_in.date_naive()) {
            checked_add(&mut day.check_ins, 1)?;
            checked_add(
                &mut result.check_in_hours[check_in.hour() as usize].count,
                1,
            )?;
            let hour = check_in.hour() / 2 * 2;
            if observed_block(check_in.date_naive(), hour, now) {
                let cell = &mut result.check_in_heatmap[(check_in.weekday().num_days_from_monday() * 12 + hour / 2) as usize];
                checked_add(&mut cell.count, 1)?;
            }
        }
        if row.status != "closed" {
            continue;
        }
        let check_out = row
            .check_out_at
            .as_deref()
            .ok_or_else(|| {
                AppError::storage(format!(
                    "La cuenta cerrada {} no tiene fecha de cierre",
                    row.id
                ))
            })
            .and_then(business_time)?;
        let Some(day) = daily.get_mut(&check_out.date_naive()) else {
            continue;
        };
        let minutes = (check_out - check_in).num_minutes();
        if minutes < 0 {
            return Err(AppError::storage(format!(
                "La cuenta {} tiene fechas de estadía inconsistentes",
                row.id
            )));
        }
        checked_add(&mut duration_total, minutes)?;

        let charges = db::list_charges(&tx, row.id)?;
        if charges.is_empty() {
            return Err(AppError::storage(format!(
                "La cuenta {} está sin detalle de cierre",
                row.id
            )));
        }
        let snapshot = db::closed_snapshot(&tx, row.id)?;
        let mode_index = match snapshot.applied_kind.as_deref() {
            Some("hourly") => 0, Some("night" | "overnight") => 1, _ => 2,
        };
        let eligible = row.product_tracking_since.as_deref().map(business_time).transpose()?
            .is_some_and(|since| check_in >= since);
        let mut product_total = 0_i64;
        let mut has_products = false;
        // Current closures have a frozen snapshot, so compare the persisted
        // lines with it. Legacy rows without that snapshot still use only the
        // amounts already stored in their charges; they are never repriced.
        let frozen_total = if snapshot.applied_kind.is_some() {
            let stay = db::get_stay(&tx, row.id)?;
            Some(service::bill_for_stay(&tx, &stay)?.total_cents)
        } else {
            None
        };
        let mut line_total = 0_i64;
        for charge in charges {
            checked_add(&mut line_total, charge.amount_cents)?;
            match charge.kind.as_str() {
                "stay" | "extra_hour" => {
                    checked_add(&mut result.lodging_cents, charge.amount_cents)?;
                    checked_add(&mut result.by_stay_mode[mode_index].lodging_cents, charge.amount_cents)?;
                }
                "surcharge" | "product" => {
                    checked_add(&mut result.extras_cents, charge.amount_cents)?
                }
                "discount" => checked_add(&mut result.discount_cents, charge.amount_cents)?,
                "tax" => checked_add(&mut result.tax_cents, charge.amount_cents)?,
                _ => {
                    return Err(AppError::storage(format!(
                        "La cuenta {} tiene un tipo de cargo de cierre inválido",
                        row.id
                    )))
                }
            }
            if matches!(charge.kind.as_str(), "surcharge" | "product") {
                if let (Some(uid), Some(quantity)) = (&charge.product_uid, charge.product_quantity) {
                    if quantity <= 0 || charge.amount_cents < 0 {
                        return Err(AppError::storage("Venta de producto inválida"));
                    }
                    let at = business_time(&charge.created_at)?;
                    let (product, latest) = products.entry(uid.clone()).or_insert_with(|| (AnalyticsProduct {
                        product_uid: uid.clone(), description: charge.description.clone(), units: 0, revenue_cents: 0,
                    }, at));
                    if at > *latest || (at == *latest && charge.description < product.description) {
                        product.description = charge.description.clone(); *latest = at;
                    }
                    checked_add(&mut product.units, quantity)?;
                    checked_add(&mut product.revenue_cents, charge.amount_cents)?;
                    checked_add(&mut product_total, charge.amount_cents)?;
                    has_products = true;
                } else if charge.product_uid.is_some() || charge.product_quantity.is_some() {
                    return Err(AppError::storage("Venta de producto sin identificación completa"));
                }
                let entry = extras
                    .entry(charge.description.trim().to_string())
                    .or_default();
                checked_add(&mut entry.0, 1)?;
                checked_add(&mut entry.1, charge.amount_cents)?;
            }
        }
        if frozen_total.is_some_and(|total| total != line_total) {
            return Err(AppError::storage(format!(
                "La cuenta {} tiene un cierre inconsistente",
                row.id
            )));
        }
        checked_add(&mut result.total_revenue_cents, line_total)?;
        checked_add(&mut result.closed_accounts, 1)?;
        checked_add(&mut result.by_stay_mode[mode_index].closed_accounts, 1)?;
        if eligible {
            checked_add(&mut result.product_purchase.eligible_accounts, 1)?;
            if has_products {
                checked_add(&mut result.product_purchase.purchasing_accounts, 1)?;
                checked_add(&mut result.product_purchase.revenue_cents, product_total)?;
            }
        } else {
            checked_add(&mut result.product_purchase.incomplete_accounts, 1)?;
        }
        let hour = &mut result.closed_hours[check_out.hour() as usize];
        checked_add(&mut hour.count, 1)?;
        checked_add(&mut hour.revenue_cents, line_total)?;
        checked_add(&mut day.revenue_cents, line_total)?;
        checked_add(&mut day.closed_accounts, 1)?;
        let type_entry = type_totals.entry(row.room_type.clone()).or_default();
        checked_add(&mut type_entry.0, line_total)?;
        checked_add(&mut type_entry.1, 1)?;
        let room_entry = room_totals
            .entry((row.room_number, row.room_type))
            .or_default();
        checked_add(&mut room_entry.0, line_total)?;
        checked_add(&mut room_entry.1, 1)?;
    }

    // Charges removed from open accounts, by the day they were removed: money that did not end
    // up billed. Checkout never deletes charges, so these are always manual voids.
    let voided: Vec<(i64, String, String)> = tx
        .prepare(
            "SELECT c.amount_cents, c.deleted_at, r.room_type
             FROM charges c JOIN stays s ON s.id = c.stay_id JOIN rooms r ON r.id = s.room_id
             WHERE c.deleted_at IS NOT NULL AND c.kind IN ('surcharge', 'product')",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (amount_cents, deleted_at, voided_room_type) in voided {
        if !matches_type(&voided_room_type, filter) {
            continue;
        }
        let deleted = business_time(&deleted_at)?;
        if daily.contains_key(&deleted.date_naive()) {
            checked_add(&mut result.voided_count, 1)?;
            checked_add(&mut result.voided_cents, amount_cents)?;
        }
    }

    if result.closed_accounts > 0 {
        result.average_ticket_cents = result.total_revenue_cents / result.closed_accounts;
        result.average_stay_minutes = Some(duration_total / result.closed_accounts);
    }
    result.current_rooms = status_counts
        .into_iter()
        .map(|(status, count)| AnalyticsStatusCount { status, count })
        .collect();
    result.daily = daily.into_values().collect();
    result.by_room_type = type_totals
        .into_iter()
        .map(
            |(room_type, (revenue_cents, closed_accounts))| AnalyticsTypeTotal {
                room_type,
                revenue_cents,
                closed_accounts,
            },
        )
        .collect();
    result.by_room = room_totals
        .into_iter()
        .map(
            |((room_number, room_type), (revenue_cents, closed_accounts))| AnalyticsRoomTotal {
                room_number,
                room_type,
                revenue_cents,
                closed_accounts,
            },
        )
        .collect();
    result.by_room.sort_by(|a, b| {
        b.revenue_cents
            .cmp(&a.revenue_cents)
            .then_with(|| a.room_number.cmp(&b.room_number))
    });
    result.top_extras = extras
        .into_iter()
        .map(|(description, (count, revenue_cents))| AnalyticsExtra {
            description,
            count,
            revenue_cents,
        })
        .collect();
    result.top_extras.sort_by(|a, b| {
        b.revenue_cents
            .cmp(&a.revenue_cents)
            .then_with(|| a.description.cmp(&b.description))
    });
    result.top_extras.truncate(10);
    result.product_sales = products.into_values().map(|(product, _)| product).collect();
    result.product_sales.sort_by(|a, b| b.units.cmp(&a.units).then_with(|| a.product_uid.cmp(&b.product_uid)));
    let purchase = &mut result.product_purchase;
    if purchase.eligible_accounts > 0 {
        purchase.rate_percent = Some(purchase.purchasing_accounts as f64 * 100.0 / purchase.eligible_accounts as f64);
    }
    if purchase.purchasing_accounts > 0 {
        purchase.average_purchase_cents = Some(purchase.revenue_cents / purchase.purchasing_accounts);
    }
    for cell in &mut result.check_in_heatmap {
        if cell.observed_blocks > 0 { cell.average = Some(cell.count as f64 / cell.observed_blocks as f64); }
    }
    for mode in &mut result.by_stay_mode {
        if mode.closed_accounts > 0 { mode.average_lodging_cents = Some(mode.lodging_cents / mode.closed_accounts); }
    }
    tx.commit()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rusqlite::params;

    #[test]
    fn product_sale_preserves_identity_without_stock_tracking() -> AppResult<()> {
        let mut conn = db::open(std::path::Path::new(":memory:"))?;
        conn.execute("INSERT INTO guests(name,created_at) VALUES ('Prueba','2026-10-01T12:00:00Z')", [])?;
        conn.execute("INSERT INTO stays(room_id,guest_id,rate_plan_id,check_in_at,status) VALUES (1,1,1,'2026-10-01T12:00:00Z','open')", [])?;
        let product_id: i64 = conn.query_row("SELECT id FROM products WHERE active=1 LIMIT 1", [], |r| r.get(0))?;
        let uid = db::uid_of(&conn, "products", product_id)?;
        let charge = service::add_product_charge(&mut conn, crate::models::AddProductChargePayload {
            stay_id: 1, product_id, operation_id: None, expected_version: None,
        })?;
        let payload = db::payload_for_charge(&conn, charge.id)?;
        assert_eq!(payload["product_uid"], serde_json::json!(uid));
        assert_eq!(payload["product_quantity"], 1);
        let queued: String = conn.query_row("SELECT payload FROM sync_outbox WHERE entity='charge' ORDER BY id DESC LIMIT 1", [], |r| r.get(0))?;
        assert_eq!(serde_json::from_str::<serde_json::Value>(&queued).unwrap()["product_uid"], payload["product_uid"]);
        Ok(())
    }

    #[test]
    fn empty_analytics_distinguishes_no_buyers_from_no_coverage() -> AppResult<()> {
        let conn = db::open(std::path::Path::new(":memory:"))?;
        let data = serde_json::to_value(summary(&conn, "2026-09-01", "2026-09-07", None)?).unwrap();
        assert_eq!(data["product_purchase"]["eligible_accounts"], 0);
        assert!(data["product_purchase"]["rate_percent"].is_null());
        assert_eq!(data["check_in_heatmap"].as_array().map(Vec::len), Some(84));
        Ok(())
    }

    #[test]
    fn products_count_units_not_manual_names_and_keep_historical_prices() -> AppResult<()> {
        let conn = db::open(std::path::Path::new(":memory:"))?;
        conn.execute("INSERT INTO guests(name,created_at) VALUES ('Prueba','2026-09-01T12:00:00Z')", [])?;
        for (id, room, kind, since, status) in [
            (1, 1, Some("hourly"), Some("2026-09-01T00:00:00Z"), "closed"),
            (2, 5, Some("overnight"), Some("2026-09-01T00:00:00Z"), "closed"),
            (3, 6, None, None, "closed"),
            (4, 7, None, Some("2026-09-01T00:00:00Z"), "open"),
        ] {
            conn.execute("INSERT INTO stays(id,room_id,guest_id,rate_plan_id,check_in_at,check_out_at,status,closed_applied_kind,closed_tax_percent,product_tracking_since)
                VALUES (?1,?2,1,1,'2026-09-04T01:00:00Z','2026-09-04T06:00:00Z',?5,?3,0,?4)", params![id, room, kind, since, status])?;
            conn.execute("INSERT INTO charges(stay_id,kind,description,amount_cents,created_at) VALUES (?1,'stay','Histórico',40000,'2026-09-04T06:00:00Z')", [id])?;
        }
        for (stay, name, quantity, amount, deleted, at) in [
            (1, "Agua", 2, 10000, None, "2026-09-04T02:00:00Z"),
            (1, "Agua nueva", 1, 5000, None, "2026-09-04T03:00:00Z"),
            (1, "Anulado", 9, 90000, Some("2026-09-04T04:00:00Z"), "2026-09-04T04:00:00Z"),
            (3, "Agua nueva", 1, 7000, None, "2026-09-04T04:00:00Z"),
            (4, "Abierta", 8, 80000, None, "2026-09-04T05:00:00Z"),
        ] {
            conn.execute("INSERT INTO charges(stay_id,kind,description,amount_cents,created_at,product_uid,product_quantity,deleted_at)
                VALUES (?1,'surcharge',?2,?4,?6,'00000000-0000-4000-8000-000000000001',?3,?5)", params![stay,name,quantity,amount,deleted,at])?;
        }
        conn.execute("INSERT INTO charges(stay_id,kind,description,amount_cents,created_at) VALUES (2,'surcharge','Agua nueva',99000,'2026-09-04T03:00:00Z')", [])?;
        conn.execute("UPDATE products SET price_cents=999999, active=0", [])?;
        conn.execute("UPDATE rate_plans SET base_amount_cents=999999", [])?;
        let result = summary(&conn, "2026-09-04", "2026-09-04", None)?;
        assert_eq!(result.product_sales.len(), 1);
        assert_eq!((result.product_sales[0].units, result.product_sales[0].revenue_cents), (4,22000));
        assert_eq!(result.product_sales[0].description, "Agua nueva");
        assert_eq!(result.product_purchase.eligible_accounts, 2);
        assert_eq!(result.product_purchase.purchasing_accounts, 1);
        assert_eq!(result.product_purchase.incomplete_accounts, 1);
        assert_eq!(result.product_purchase.rate_percent, Some(50.0));
        assert_eq!(result.product_purchase.average_purchase_cents, Some(15000));
        assert_eq!(result.by_stay_mode.iter().map(|m| m.lodging_cents).collect::<Vec<_>>(), vec![40000,40000,40000]);
        assert_eq!(result.daily[0].check_ins, 0, "01:00 UTC belongs to the previous business day");
        let jacuzzi = summary(&conn, "2026-09-04", "2026-09-04", Some("jacuzzi"))?;
        assert_eq!(jacuzzi.product_sales[0].units, 3);
        assert_eq!(jacuzzi.product_purchase.rate_percent, Some(100.0));
        let normal = summary(&conn, "2026-09-04", "2026-09-04", Some("normal"))?;
        assert_eq!(normal.product_purchase.rate_percent, Some(0.0));
        assert_eq!(normal.product_purchase.average_purchase_cents, None);
        Ok(())
    }

    #[test]
    fn heatmap_normalizes_weekdays_and_excludes_unfinished_blocks() {
        let now = business_time("2026-09-30T23:30:00-03:00").unwrap();
        let cells = heatmap(NaiveDate::from_ymd_opt(2026,9,1).unwrap(), now.date_naive(), now);
        assert_eq!(cells[12 + 10].observed_blocks, 5, "five Tuesdays at 20:00");
        assert_eq!(cells[4 * 12 + 10].observed_blocks, 4, "four Fridays at 20:00");
        assert_eq!(cells[2 * 12 + 11].observed_blocks, 4, "fifth Wednesday's 22:00 block is unfinished");
        let today = heatmap(now.date_naive(), now.date_naive(), now);
        assert_eq!(today[2 * 12 + 11].observed_blocks, 0);
        assert!(today[2 * 12 + 11].average.is_none());
    }

    #[test]
    fn analytics_uses_closed_lines_and_filters_room_type() -> AppResult<()> {
        let conn = db::open(std::path::Path::new(":memory:"))?;
        let day = Utc::now().with_timezone(&Asuncion).date_naive() - Duration::days(2);
        let date = day.format("%Y-%m-%d").to_string();
        let check_in = Asuncion
            .from_local_datetime(&day.and_hms_opt(10, 0, 0).unwrap())
            .single()
            .unwrap();
        let check_out = (check_in + Duration::hours(2)).to_rfc3339();
        let check_in = check_in.to_rfc3339();
        conn.execute(
            "INSERT INTO guests(name,created_at) VALUES ('Prueba',?1)",
            [&check_in],
        )?;
        for (id, room_id, amount) in [(1_i64, 1_i64, 120_000_i64), (2, 5, 80_000)] {
            conn.execute(
                "INSERT INTO stays(id,room_id,guest_id,rate_plan_id,check_in_at,check_out_at,status,closed_applied_kind,closed_tax_percent)
                 VALUES (?1,?2,1,1,?3,?4,'closed','hourly',0)",
                params![id, room_id, check_in, check_out],
            )?;
            conn.execute(
                "INSERT INTO charges(stay_id,kind,description,amount_cents,created_at) VALUES (?1,'stay','Tarifa al cierre',?2,?3)",
                params![id, amount, check_out],
            )?;
        }
        conn.execute("INSERT INTO charges(stay_id,kind,description,amount_cents,created_at) VALUES (1,'surcharge','Agua',10000,?1)", [&check_out])?;
        conn.execute("INSERT INTO charges(stay_id,kind,description,amount_cents,created_at) VALUES (1,'discount','Descuento',-5000,?1)", [&check_out])?;
        conn.execute("INSERT INTO charges(stay_id,kind,description,amount_cents,created_at,deleted_at) VALUES (1,'surcharge','Anulado',9000,?1,?1)", [&check_out])?;
        conn.execute("UPDATE rate_plans SET base_amount_cents=999999", [])?;

        let all = summary(&conn, &date, &date, None)?;
        assert_eq!(all.total_revenue_cents, 205_000);
        assert_eq!(all.closed_accounts, 2);
        assert_eq!(all.average_ticket_cents, 102_500);
        assert_eq!(all.lodging_cents, 200_000);
        assert_eq!(all.extras_cents, 10_000);
        assert_eq!(all.discount_cents, -5_000);
        assert_eq!(all.top_extras.len(), 1);
        assert_eq!(all.top_extras[0].description, "Agua");
        assert_eq!(all.daily[0].check_ins, 2);
        // Both accounts closed at 12:00 local; the deleted "Anulado" is a void of 9.000.
        assert_eq!(all.closed_hours.len(), 24);
        assert_eq!((all.closed_hours[12].count, all.closed_hours[12].revenue_cents), (2, 205_000));
        assert_eq!((all.voided_count, all.voided_cents), (1, 9_000));
        let jacuzzi = summary(&conn, &date, &date, Some("JACUZZI"))?;
        assert_eq!(jacuzzi.total_revenue_cents, 125_000);
        assert_eq!(jacuzzi.voided_count, 1, "room 1 is a jacuzzi");
        let normal = summary(&conn, &date, &date, Some("normal"))?;
        assert_eq!(normal.voided_count, 0);
        assert_eq!(jacuzzi.closed_accounts, 1);
        assert_eq!(
            jacuzzi
                .current_rooms
                .iter()
                .map(|item| item.count)
                .sum::<i64>(),
            4
        );
        Ok(())
    }

    #[test]
    fn analytics_rejects_invalid_ranges_and_missing_closed_detail() -> AppResult<()> {
        let conn = db::open(std::path::Path::new(":memory:"))?;
        let today = Utc::now().with_timezone(&Asuncion).date_naive();
        let date = today.format("%Y-%m-%d").to_string();
        let yesterday = (today - Duration::days(1)).format("%Y-%m-%d").to_string();
        assert!(summary(&conn, &date, &yesterday, None).is_err());
        assert!(summary(&conn, "2099-01-01", "2099-01-02", None).is_err());
        assert!(summary(&conn, "2020-01-01", &date, None).is_err());
        assert!(summary(&conn, &date, &date, Some("suite")).is_err());

        let stamp = Utc::now().with_timezone(&Asuncion).to_rfc3339();
        conn.execute(
            "INSERT INTO guests(name,created_at) VALUES ('Antigua',?1)",
            [&stamp],
        )?;
        conn.execute("INSERT INTO stays(room_id,guest_id,rate_plan_id,check_in_at,check_out_at,status) VALUES (1,1,1,?1,?1,'closed')", [&stamp])?;
        let err = summary(&conn, &date, &date, None).unwrap_err();
        assert!(err.to_string().contains("sin detalle de cierre"));
        Ok(())
    }

    #[test]
    fn analytics_counts_reservations_by_expected_arrival_and_check_in_hour() -> AppResult<()> {
        let conn = db::open(std::path::Path::new(":memory:"))?;
        let day = Utc::now().with_timezone(&Asuncion).date_naive() - Duration::days(2);
        let date = day.format("%Y-%m-%d").to_string();
        let arrival = Asuncion
            .from_local_datetime(&day.and_hms_opt(14, 0, 0).unwrap())
            .single()
            .unwrap()
            .to_rfc3339();
        let check_in = Asuncion
            .from_local_datetime(&day.and_hms_opt(9, 35, 0).unwrap())
            .single()
            .unwrap()
            .to_rfc3339();
        conn.execute(
            "INSERT INTO guests(name,created_at) VALUES ('Prueba',?1)",
            [&check_in],
        )?;
        conn.execute("INSERT INTO stays(room_id,guest_id,rate_plan_id,check_in_at,status) VALUES (1,1,1,?1,'open')", [&check_in])?;
        for (room_id, status) in [(1_i64, "cancelled"), (5, "no_show"), (2, "hold")] {
            conn.execute(
                "INSERT INTO reservations(guest_id,room_id,rate_plan_id,expected_arrival_at,status,created_at)
                 VALUES (1,?1,1,?2,?3,?2)",
                params![room_id, arrival, status],
            )?;
        }
        let all = summary(&conn, &date, &date, None)?;
        assert_eq!(all.reservation_arrivals, 3);
        assert_eq!(all.reservation_cancellations, 1);
        assert_eq!(all.no_shows, 1);
        assert_eq!(all.daily[0].reservation_arrivals, 3);
        assert_eq!(all.daily[0].reservation_cancellations, 1);
        assert_eq!(all.daily[0].no_shows, 1);
        assert_eq!(all.daily[0].check_ins, 1);
        assert_eq!(all.check_in_hours.len(), 24);
        assert_eq!(all.check_in_hours[9].count, 1);
        assert_eq!(
            all.check_in_hours
                .iter()
                .map(|entry| entry.count)
                .sum::<i64>(),
            1
        );

        let jacuzzi = summary(&conn, &date, &date, Some("jacuzzi"))?;
        assert_eq!(jacuzzi.reservation_arrivals, 2);
        assert_eq!(jacuzzi.reservation_cancellations, 1);
        assert_eq!(jacuzzi.no_shows, 0);
        assert_eq!(jacuzzi.check_in_hours[9].count, 1);
        Ok(())
    }
}
