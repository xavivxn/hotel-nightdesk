//! Promotions and special prices by weekday (or date) and check-in hour.
//! Rules live in the local `settings` row `price_rules` (JSON), on the reception PC: they are not
//! part of the business settings synced with Supabase yet. The price is snapshotted on the stay at
//! check-in (or at conversion to dormida), so later edits never change an open account.
use crate::billing::PriceOverride;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::{EffectivePrice, PriceRule, RatePlan};
use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;

pub const SETTING_KEY: &str = "price_rules";
const MAX_RULES: usize = 50;
const MAX_NAME: usize = 40;

pub fn load(conn: &Connection) -> AppResult<Vec<PriceRule>> {
    let raw = db::get_setting(conn, SETTING_KEY, "[]")?;
    serde_json::from_str(&raw).map_err(|_| AppError::storage("Las promociones guardadas no se pueden leer"))
}

pub fn save(conn: &Connection, mut rules: Vec<PriceRule>) -> AppResult<Vec<PriceRule>> {
    for rule in &mut rules {
        rule.name = rule.name.trim().to_string();
        rule.days.sort_unstable();
        rule.days.dedup();
        rule.dates = rule.dates.iter().map(|date| date.trim().to_string()).collect();
        rule.dates.sort();
        rule.dates.dedup();
        if rule.id.trim().is_empty() {
            rule.id = uuid::Uuid::new_v4().to_string();
        }
    }
    validate(&rules, &db::list_rate_plans(conn, false)?)?;
    let raw = serde_json::to_string(&rules).map_err(|e| AppError::msg(e.to_string()))?;
    db::upsert_setting(conn, SETTING_KEY, &raw)?;
    Ok(rules)
}

fn day_name(day: u8) -> &'static str {
    match day {
        1 => "lunes",
        2 => "martes",
        3 => "miércoles",
        4 => "jueves",
        5 => "viernes",
        6 => "sábado",
        _ => "domingo",
    }
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

pub fn validate(rules: &[PriceRule], plans: &[RatePlan]) -> AppResult<()> {
    if rules.len() > MAX_RULES {
        return Err(AppError::msg("Se pueden guardar hasta 50 promociones"));
    }
    let mut ids = HashSet::new();
    for rule in rules {
        let name = &rule.name;
        if !ids.insert(rule.id.as_str()) {
            return Err(AppError::msg("Hay una promoción repetida"));
        }
        if name.is_empty() || name.chars().count() > MAX_NAME {
            return Err(AppError::msg("Poné un nombre de hasta 40 caracteres a cada promoción"));
        }
        if !plans.iter().any(|plan| plan.id == rule.rate_plan_id) {
            return Err(AppError::msg(format!("«{name}»: la tarifa ya no existe")));
        }
        if rule.from_hour > 23 || rule.to_hour > 24 || rule.from_hour >= rule.to_hour {
            return Err(AppError::msg(format!(
                "«{name}»: el horario va de 0 a 24 y «desde» tiene que ser menor que «hasta»"
            )));
        }
        if rule.base_amount_cents <= 0 {
            return Err(AppError::msg(format!("«{name}»: el precio tiene que ser mayor que 0")));
        }
        if rule.extra_hour_cents.is_some_and(|extra| extra < 0) {
            return Err(AppError::msg(format!("«{name}»: el adicional no puede ser negativo")));
        }
        match (rule.days.is_empty(), rule.dates.is_empty()) {
            (true, true) => {
                return Err(AppError::msg(format!("«{name}»: elegí al menos un día o una fecha")))
            }
            (false, false) => {
                return Err(AppError::msg(format!("«{name}»: usá días de la semana o fechas, no las dos cosas")))
            }
            _ => {}
        }
        if rule.days.iter().any(|day| !(1..=7).contains(day)) {
            return Err(AppError::msg(format!("«{name}»: día de la semana inválido")));
        }
        if let Some(bad) = rule.dates.iter().find(|date| parse_date(date).is_none()) {
            return Err(AppError::msg(format!("«{name}»: la fecha «{bad}» no es válida")));
        }
    }
    let active: Vec<&PriceRule> = rules.iter().filter(|rule| rule.active).collect();
    for (index, a) in active.iter().enumerate() {
        for b in &active[index + 1..] {
            if a.rate_plan_id != b.rate_plan_id || a.from_hour >= b.to_hour || b.from_hour >= a.to_hour {
                continue;
            }
            let when = if !a.days.is_empty() && !b.days.is_empty() {
                a.days.iter().copied().find(|day| b.days.contains(day)).map(|day| format!("el {}", day_name(day)))
            } else if !a.dates.is_empty() && !b.dates.is_empty() {
                a.dates
                    .iter()
                    .find(|date| b.dates.contains(*date))
                    .and_then(|date| parse_date(date))
                    .map(|date| format!("el {}", date.format("%d/%m/%Y")))
            } else {
                None
            };
            if let Some(when) = when {
                return Err(AppError::msg(format!(
                    "«{}» y «{}» se superponen {when} entre las {}:00 y las {}:00",
                    a.name,
                    b.name,
                    a.from_hour.max(b.from_hour),
                    a.to_hour.min(b.to_hour)
                )));
            }
        }
    }
    Ok(())
}

/// Active rule for `plan_id` at `at`. A rule for that exact date wins over weekday rules.
pub fn matching(rules: &[PriceRule], plan_id: i64, at: DateTime<Local>) -> Option<&PriceRule> {
    let hour = at.hour() as u8;
    let date = at.format("%Y-%m-%d").to_string();
    let weekday = at.weekday().number_from_monday() as u8;
    let fits = move |rule: &&PriceRule| {
        rule.active && rule.rate_plan_id == plan_id && rule.from_hour <= hour && hour < rule.to_hour
    };
    rules
        .iter()
        .filter(fits)
        .find(|rule| rule.dates.contains(&date))
        .or_else(|| {
            rules
                .iter()
                .filter(fits)
                .find(|rule| rule.dates.is_empty() && rule.days.contains(&weekday))
        })
}

pub fn price_for(plan: &RatePlan, rule: &PriceRule) -> PriceOverride {
    PriceOverride {
        plan_id: plan.id,
        base_amount_cents: rule.base_amount_cents,
        extra_hour_cents: rule.extra_hour_cents.unwrap_or(plan.extra_hour_cents),
        rule_name: rule.name.clone(),
    }
}

/// Snapshots the promotion of `plan` at `at` on the stay, or clears it when none applies.
pub fn snapshot_on_stay(conn: &Connection, stay_id: i64, plan: &RatePlan, at: DateTime<Local>) -> AppResult<()> {
    // A broken rules row must never block a check-in: without rules the plan price applies.
    let rules = load(conn).unwrap_or_default();
    let price = matching(&rules, plan.id, at).map(|rule| price_for(plan, rule));
    conn.execute(
        "UPDATE stays SET price_plan_id = ?1, price_base_cents = ?2, price_extra_cents = ?3, price_rule = ?4
         WHERE id = ?5",
        params![
            price.as_ref().map(|p| p.plan_id),
            price.as_ref().map(|p| p.base_amount_cents),
            price.as_ref().map(|p| p.extra_hour_cents),
            price.as_ref().map(|p| p.rule_name.clone()),
            stay_id
        ],
    )?;
    Ok(())
}

pub fn stay_override(conn: &Connection, stay_id: i64) -> AppResult<Option<PriceOverride>> {
    let row = conn
        .query_row(
            "SELECT price_plan_id, price_base_cents, price_extra_cents, price_rule FROM stays WHERE id = ?1",
            [stay_id],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?;
    Ok(match row {
        Some((Some(plan_id), Some(base_amount_cents), Some(extra_hour_cents), Some(rule_name))) => {
            Some(PriceOverride { plan_id, base_amount_cents, extra_hour_cents, rule_name })
        }
        _ => None,
    })
}

pub fn effective_prices(conn: &Connection, at: DateTime<Local>) -> AppResult<Vec<EffectivePrice>> {
    let rules = load(conn).unwrap_or_default();
    Ok(db::list_rate_plans(conn, true)?
        .into_iter()
        .map(|plan| match matching(&rules, plan.id, at) {
            Some(rule) => {
                let price = price_for(&plan, rule);
                EffectivePrice {
                    rate_plan_id: plan.id,
                    base_amount_cents: price.base_amount_cents,
                    extra_hour_cents: price.extra_hour_cents,
                    rule_name: Some(price.rule_name),
                }
            }
            None => EffectivePrice {
                rate_plan_id: plan.id,
                base_amount_cents: plan.base_amount_cents,
                extra_hour_cents: plan.extra_hour_cents,
                rule_name: None,
            },
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RateKind;
    use chrono::TimeZone;

    fn plan(id: i64) -> RatePlan {
        RatePlan {
            id,
            name: "1 hora".into(),
            kind: RateKind::Hourly,
            base_amount_cents: 45_000,
            extra_hour_cents: 15_000,
            included_hours: 1,
            grace_minutes: 5,
            night_cutoff_hour: 10,
            active: true,
            version: 1,
            room_category: "normal".into(),
        }
    }

    fn rule(name: &str, days: &[u8], from_hour: u8, to_hour: u8, price: i64) -> PriceRule {
        PriceRule {
            id: name.into(),
            name: name.into(),
            rate_plan_id: 1,
            days: days.to_vec(),
            dates: vec![],
            from_hour,
            to_hour,
            base_amount_cents: price,
            extra_hour_cents: None,
            active: true,
        }
    }

    fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, 30, 0).unwrap()
    }

    #[test]
    fn weekday_hour_and_date_rules_match_in_order() {
        let tarde = rule("Promo tarde", &[1, 2, 3, 4], 14, 18, 35_000);
        let finde = rule("Fin de semana", &[6, 7], 0, 24, 55_000);
        let valentin = PriceRule { dates: vec!["2026-02-14".into()], days: vec![], ..rule("San Valentín", &[], 0, 24, 70_000) };
        let rules = vec![tarde, finde, valentin];
        // Tuesday 29/09/2026.
        assert_eq!(matching(&rules, 1, at(2026, 9, 29, 15)).map(|r| r.name.as_str()), Some("Promo tarde"));
        assert!(matching(&rules, 1, at(2026, 9, 29, 18)).is_none(), "the hour window is [from, to)");
        assert!(matching(&rules, 2, at(2026, 9, 29, 15)).is_none(), "another plan keeps its price");
        // Saturday 14/02/2026: the date rule wins over the weekend rule.
        assert_eq!(matching(&rules, 1, at(2026, 2, 14, 23)).map(|r| r.name.as_str()), Some("San Valentín"));
        assert_eq!(matching(&rules, 1, at(2026, 2, 21, 23)).map(|r| r.name.as_str()), Some("Fin de semana"));
        let inactive = PriceRule { active: false, ..rules[0].clone() };
        assert!(matching(&[inactive], 1, at(2026, 9, 29, 15)).is_none());
    }

    #[test]
    fn overlaps_and_bad_rules_are_rejected() {
        let plans = vec![plan(1), plan(2)];
        let ok = vec![rule("Tarde", &[1, 2], 14, 18, 35_000), rule("Noche", &[1, 2], 18, 24, 40_000)];
        validate(&ok, &plans).unwrap();
        let overlap = vec![rule("Tarde", &[1, 2], 14, 18, 35_000), rule("Media tarde", &[2, 3], 16, 20, 30_000)];
        let error = validate(&overlap, &plans).unwrap_err().to_string();
        assert!(error.contains("martes") && error.contains("16:00") && error.contains("18:00"), "{error}");
        let other_plan = vec![rule("Tarde", &[1], 14, 18, 35_000), PriceRule { rate_plan_id: 2, ..rule("Otra", &[1], 14, 18, 1) }];
        validate(&other_plan, &plans).unwrap();
        let paused = vec![rule("Tarde", &[1], 14, 18, 35_000), PriceRule { active: false, ..rule("Vieja", &[1], 14, 18, 1) }];
        validate(&paused, &plans).unwrap();
        for bad in [
            rule("Sin días", &[], 14, 18, 1),
            rule("Hora", &[1], 18, 14, 1),
            rule("Cero", &[1], 14, 18, 0),
            rule("Día", &[8], 14, 18, 1),
            rule("", &[1], 14, 18, 1),
            PriceRule { rate_plan_id: 9, ..rule("Tarifa", &[1], 14, 18, 1) },
            PriceRule { dates: vec!["2026-02-30".into()], days: vec![], ..rule("Fecha", &[], 0, 24, 1) },
            PriceRule { dates: vec!["2026-02-14".into()], ..rule("Ambas", &[1], 0, 24, 1) },
        ] {
            assert!(validate(&[bad.clone()], &plans).is_err(), "{:?}", bad.name);
        }
    }

    #[test]
    fn rules_round_trip_and_snapshot_on_a_stay() {
        let conn = db::open(std::path::Path::new(":memory:")).unwrap();
        let plan_one = db::get_rate_plan(&conn, 1).unwrap();
        assert!(load(&conn).unwrap().is_empty());
        let all_day = PriceRule { id: String::new(), rate_plan_id: plan_one.id, ..rule(" Promo ", &[1, 2, 3, 4, 5, 6, 7], 0, 24, 33_000) };
        let saved = save(&conn, vec![all_day]).unwrap();
        assert_eq!(saved[0].name, "Promo");
        assert!(!saved[0].id.is_empty());
        assert_eq!(load(&conn).unwrap(), saved);
        let now = db::now_rfc3339();
        conn.execute("INSERT INTO guests (name, created_at) VALUES ('Prueba', ?1)", [&now]).unwrap();
        conn.execute("INSERT INTO stays (room_id, guest_id, rate_plan_id, check_in_at, status) VALUES (1, 1, ?1, ?2, 'open')", params![plan_one.id, now]).unwrap();
        let stay_id = conn.last_insert_rowid();
        snapshot_on_stay(&conn, stay_id, &plan_one, Local::now()).unwrap();
        let price = stay_override(&conn, stay_id).unwrap().expect("promotion snapshotted");
        assert_eq!((price.base_amount_cents, price.extra_hour_cents), (33_000, plan_one.extra_hour_cents));
        // Editing the rules later does not touch the open stay.
        save(&conn, vec![]).unwrap();
        assert_eq!(stay_override(&conn, stay_id).unwrap(), Some(price));
        let prices = effective_prices(&conn, Local::now()).unwrap();
        assert!(prices.iter().all(|p| p.rule_name.is_none()));
    }
}
