use crate::{db, error::{AppError, AppResult}, models::{DailyAccount, DailyReport}, service};
use chrono::{Local, NaiveDate, TimeZone};
use rusqlite::Connection;
use std::collections::BTreeSet;

pub fn validate_date(date: &str) -> AppResult<NaiveDate> {
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| AppError::msg("Elegí una fecha válida"))?;
    if parsed.format("%Y-%m-%d").to_string() != date { return Err(AppError::msg("Elegí una fecha válida")); }
    if parsed > Local::now().date_naive() { return Err(AppError::msg("El informe no admite fechas futuras")); }
    Ok(parsed)
}

pub fn daily_report(conn: &Connection, date: &str) -> AppResult<DailyReport> {
    let day = validate_date(date)?;
    let next = day.succ_opt().ok_or_else(|| AppError::msg("Fecha fuera de rango"))?;
    let start = Local.from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap()).earliest()
        .ok_or_else(|| AppError::msg("Inicio del día no válido en la zona local"))?;
    let end = Local.from_local_datetime(&next.and_hms_opt(0, 0, 0).unwrap()).earliest()
        .ok_or_else(|| AppError::msg("Fin del día no válido en la zona local"))?;
    let now = Local::now();
    let cutoff = end.min(now);
    let tx = conn.unchecked_transaction()?;
    let ids: Vec<i64> = tx.prepare("SELECT id FROM stays ORDER BY check_in_at, id")?
        .query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    let mut report = DailyReport {
        date: date.into(), generated_at: now.to_rfc3339(), cutoff_at: cutoff.to_rfc3339(),
        timezone: format!("Hora local de recepción (UTC{})", start.format("%:z")),
        occupied_rooms: 0, closed_total_cents: 0, adjustments_total_cents: 0,
        accounts: vec![], adjustments: vec![],
    };
    let mut rooms = BTreeSet::new();
    for id in ids {
        let stay = db::get_stay(&tx, id)?;
        let check_in = db::parse_dt(&stay.check_in_at)?;
        let check_out = stay.check_out_at.as_deref().map(db::parse_dt).transpose()?;
        let closed_on_day = check_out.map(|dt| dt >= start && dt < end && dt <= now).unwrap_or(false);
        let overlaps = check_in < cutoff && check_out.map(|dt| dt > start).unwrap_or(true);
        if !overlaps && !closed_on_day { continue; }
        if overlaps { rooms.insert(stay.room_id); }
        let total = if closed_on_day { Some(service::bill_for_stay(&tx, &stay)?.total_cents) } else { None };
        report.closed_total_cents = report.closed_total_cents.checked_add(total.unwrap_or(0))
            .ok_or_else(|| AppError::msg("Total fuera de rango"))?;
        for charge in db::list_charges(&tx, id)? {
            let at = db::parse_dt(&charge.created_at)?;
            if matches!(charge.kind.as_str(), "surcharge" | "discount") && at >= start && at < end && at <= now {
                report.adjustments_total_cents = report.adjustments_total_cents.checked_add(charge.amount_cents)
                    .ok_or_else(|| AppError::msg("Total de consumos fuera de rango"))?;
                report.adjustments.push(charge);
            }
        }
        report.accounts.push(DailyAccount {
            stay_id: stay.id, room_number: stay.room_number, check_in_at: stay.check_in_at,
            check_out_at: stay.check_out_at, closed_on_day,
            open_at_cutoff: check_in < cutoff && check_out.map(|dt| dt >= cutoff).unwrap_or(true),
            total_cents: total,
        });
    }
    report.occupied_rooms = rooms.len();
    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    #[test]
    fn daily_report_observes_local_midnight_and_keeps_closed_totals() -> AppResult<()> {
        let conn = db::open(std::path::Path::new(":memory:"))?;
        let midnight = Local.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).single().unwrap();
        let before = (midnight - chrono::Duration::hours(2)).to_rfc3339();
        let at = midnight.with_timezone(&chrono::Utc).to_rfc3339();
        let next = (midnight + chrono::Duration::days(1)).with_timezone(&chrono::Utc).to_rfc3339();
        conn.execute("INSERT INTO guests(id,name,created_at) VALUES (1,'Prueba',?1)", [&before])?;
        for (id, out) in [(1, Some(at.as_str())), (2, Some(next.as_str())), (3, None)] {
            conn.execute("INSERT INTO stays(id,room_id,guest_id,rate_plan_id,check_in_at,check_out_at,status,closed_tax_percent) VALUES (?1,?1,1,1,?2,?3,?4,0)", params![id,before,out,if out.is_some() { "closed" } else { "open" }])?;
            conn.execute("INSERT INTO charges(stay_id,kind,description,amount_cents,created_at) VALUES (?1,'stay','Tarifa original',80000,?2)", params![id,at])?;
        }
        conn.execute("INSERT INTO charges(stay_id,kind,description,amount_cents,created_at) VALUES (2,'surcharge','Agua',5000,?1)", [&at])?;
        conn.execute("INSERT INTO charges(stay_id,kind,description,amount_cents,created_at) VALUES (2,'discount','Descuento',-1000,?1)", [&at])?;
        conn.execute("INSERT INTO charges(stay_id,kind,description,amount_cents,created_at) VALUES (2,'surcharge','Día siguiente',9000,?1)", [&next])?;
        let report = daily_report(&conn, "2026-01-15")?;
        assert_eq!(report.closed_total_cents, 80000);
        assert_eq!(report.adjustments_total_cents, 4000);
        assert_eq!(report.occupied_rooms, 2);
        assert_eq!(report.accounts.iter().filter(|a| a.open_at_cutoff).count(), 2);
        assert_eq!(report.accounts.iter().filter(|a| a.closed_on_day).count(), 1);
        conn.execute("UPDATE rate_plans SET base_amount_cents=999999", [])?;
        assert_eq!(daily_report(&conn, "2026-01-15")?.closed_total_cents, 80000);
        Ok(())
    }
    #[test]
    fn empty_day_and_invalid_dates() -> AppResult<()> {
        let conn = db::open(std::path::Path::new(":memory:"))?;
        let report = daily_report(&conn, "2026-01-15")?;
        assert!(report.accounts.is_empty()); assert_eq!(report.closed_total_cents, 0);
        for date in ["2026-02-30", "", "../../x", "2099-01-01"] { assert!(daily_report(&conn, date).is_err()); }
        Ok(())
    }
}
