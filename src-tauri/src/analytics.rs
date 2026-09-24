use crate::{
    db,
    error::{AppError, AppResult},
    models::{
        AnalyticsDay, AnalyticsExtra, AnalyticsHour, AnalyticsRoomTotal, AnalyticsStatusCount,
        AnalyticsSummary, AnalyticsTypeTotal,
    },
    reports, service,
};
use chrono::{Duration, Local, NaiveDate, Timelike};
use rusqlite::Connection;
use std::collections::BTreeMap;

const MAX_DAYS: i64 = 366;

pub fn validate_range(from: &str, to: &str) -> AppResult<(NaiveDate, NaiveDate)> {
    let start = reports::validate_date(from)?;
    let end = reports::validate_date(to)?;
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
            "SELECT s.id, s.check_in_at, s.check_out_at, s.status, r.number, r.room_type
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
        generated_at: Local::now().to_rfc3339(),
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
    };
    let mut type_totals = BTreeMap::<String, (i64, i64)>::new();
    let mut room_totals = BTreeMap::<(String, String), (i64, i64)>::new();
    let mut extras = BTreeMap::<String, (i64, i64)>::new();
    let mut duration_total = 0_i64;

    for reservation in reservations {
        if !matches_type(&reservation.room_type, filter) {
            continue;
        }
        let arrival = db::parse_dt(&reservation.expected_arrival_at)?;
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
        let check_in = db::parse_dt(&row.check_in_at)?;
        if let Some(day) = daily.get_mut(&check_in.date_naive()) {
            checked_add(&mut day.check_ins, 1)?;
            checked_add(
                &mut result.check_in_hours[check_in.hour() as usize].count,
                1,
            )?;
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
            .and_then(db::parse_dt)?;
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
                    checked_add(&mut result.lodging_cents, charge.amount_cents)?
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
    tx.commit()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rusqlite::params;

    #[test]
    fn analytics_uses_closed_lines_and_filters_room_type() -> AppResult<()> {
        let conn = db::open(std::path::Path::new(":memory:"))?;
        let day = Local::now().date_naive() - Duration::days(2);
        let date = day.format("%Y-%m-%d").to_string();
        let check_in = Local
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
        let jacuzzi = summary(&conn, &date, &date, Some("JACUZZI"))?;
        assert_eq!(jacuzzi.total_revenue_cents, 125_000);
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
        let today = Local::now().date_naive();
        let date = today.format("%Y-%m-%d").to_string();
        let yesterday = (today - Duration::days(1)).format("%Y-%m-%d").to_string();
        assert!(summary(&conn, &date, &yesterday, None).is_err());
        assert!(summary(&conn, "2099-01-01", "2099-01-02", None).is_err());
        assert!(summary(&conn, "2020-01-01", &date, None).is_err());
        assert!(summary(&conn, &date, &date, Some("suite")).is_err());

        let stamp = Local::now().to_rfc3339();
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
        let day = Local::now().date_naive() - Duration::days(2);
        let date = day.format("%Y-%m-%d").to_string();
        let arrival = Local
            .from_local_datetime(&day.and_hms_opt(14, 0, 0).unwrap())
            .single()
            .unwrap()
            .to_rfc3339();
        let check_in = Local
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
