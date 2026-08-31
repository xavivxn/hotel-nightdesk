use crate::models::{BillPreview, LineItem, RateKind, RatePlan};
use chrono::{DateTime, Duration, Local, TimeZone};

pub struct BillingContext<'a> {
    pub stay_id: i64,
    pub check_in_at: DateTime<Local>,
    pub now: DateTime<Local>,
    pub rate: &'a RatePlan,
    pub converted_to_overnight: bool,
    pub overnight_plan: Option<&'a RatePlan>,
    pub night_plan: Option<&'a RatePlan>,
    pub manual_lines: Vec<LineItem>,
    pub tax_percent: f64,
}

pub fn preview(ctx: BillingContext<'_>) -> BillPreview {
    let crossed = crossed_cutoff(ctx.check_in_at, ctx.now, ctx.rate.night_cutoff_hour);
    let overnight_applied = ctx.converted_to_overnight || (ctx.rate.kind == RateKind::Hourly && crossed);

    let (applied, applied_kind) = if overnight_applied {
        if let Some(plan) = ctx.overnight_plan {
            (plan, plan.kind)
        } else if let Some(plan) = ctx.night_plan {
            (plan, plan.kind)
        } else {
            (ctx.rate, ctx.rate.kind)
        }
    } else {
        (ctx.rate, ctx.rate.kind)
    };

    let mut lines = match applied_kind {
        RateKind::Hourly => bill_hourly(applied, ctx.check_in_at, ctx.now),
        RateKind::Night | RateKind::Overnight => bill_night(applied, ctx.check_in_at, ctx.now),
    };
    lines.extend(ctx.manual_lines);

    let subtotal_cents: i64 = lines.iter().map(|l| l.amount_cents).sum();
    let tax_cents = tax_amount(subtotal_cents, ctx.tax_percent);
    let duration_label = format_duration(ctx.check_in_at, ctx.now);

    BillPreview {
        stay_id: ctx.stay_id,
        lines,
        subtotal_cents,
        tax_percent: ctx.tax_percent,
        tax_cents,
        total_cents: subtotal_cents + tax_cents,
        applied_kind,
        duration_label,
        overnight_applied,
    }
}

pub fn extra_hours_after(included_minutes: i64, elapsed_minutes: i64, grace_minutes: i64) -> i64 {
    if elapsed_minutes <= included_minutes + grace_minutes {
        return 0;
    }
    let remainder = elapsed_minutes - included_minutes;
    let mut extra = remainder / 60;
    if remainder % 60 > grace_minutes {
        extra += 1;
    }
    extra
}

fn at_local(date: chrono::NaiveDate, hour: u32) -> DateTime<Local> {
    let naive = date.and_hms_opt(hour, 0, 0).expect("valid time");
    Local
        .from_local_datetime(&naive)
        .earliest()
        .unwrap_or_else(|| Local.from_utc_datetime(&naive))
}

pub fn theoretical_night_end(check_in: DateTime<Local>, cutoff_hour: i64) -> DateTime<Local> {
    let hour = cutoff_hour.clamp(0, 23) as u32;
    let same_day = at_local(check_in.date_naive(), hour);
    if check_in < same_day {
        same_day
    } else {
        at_local(
            check_in.date_naive() + chrono::Days::new(1),
            hour,
        )
    }
}

pub fn crossed_cutoff(check_in: DateTime<Local>, now: DateTime<Local>, cutoff_hour: i64) -> bool {
    now > theoretical_night_end(check_in, cutoff_hour)
}

fn bill_hourly(rate: &RatePlan, check_in: DateTime<Local>, now: DateTime<Local>) -> Vec<LineItem> {
    let elapsed = elapsed_minutes(check_in, now);
    let included_minutes = rate.included_hours.max(1) * 60;
    let extra = extra_hours_after(included_minutes, elapsed, rate.grace_minutes);
    let mut lines = vec![LineItem {
        kind: "stay".into(),
        description: format!("{} ({} h)", rate.name, rate.included_hours),
        amount_cents: rate.base_amount_cents,
    }];
    if extra > 0 {
        lines.push(LineItem {
            kind: "extra_hour".into(),
            description: format!("Horas extra ({extra})"),
            amount_cents: extra * rate.extra_hour_cents,
        });
    }
    lines
}

fn bill_night(rate: &RatePlan, check_in: DateTime<Local>, now: DateTime<Local>) -> Vec<LineItem> {
    let mut period_end = theoretical_night_end(check_in, rate.night_cutoff_hour);
    let mut nights: i64 = 1;
    let extra;

    loop {
        if now <= period_end {
            extra = 0;
            break;
        }
        if now.date_naive() == period_end.date_naive() {
            extra = extra_hours_after(0, (now - period_end).num_minutes().max(0), rate.grace_minutes);
            break;
        }
        nights += 1;
        period_end += Duration::days(1);
    }

    let unit_label = match rate.kind {
        RateKind::Overnight => "pernocte",
        _ => "noche",
    };
    let mut lines = vec![LineItem {
        kind: "stay".into(),
        description: if nights == 1 {
            format!("{} (1 {unit_label})", rate.name)
        } else {
            format!("{} ({nights} {unit_label}s)", rate.name)
        },
        amount_cents: nights * rate.base_amount_cents,
    }];
    if extra > 0 {
        lines.push(LineItem {
            kind: "extra_hour".into(),
            description: format!("Horas extra ({extra})"),
            amount_cents: extra * rate.extra_hour_cents,
        });
    }
    lines
}

fn elapsed_minutes(check_in: DateTime<Local>, now: DateTime<Local>) -> i64 {
    (now - check_in).num_minutes().max(0)
}

pub fn format_duration(check_in: DateTime<Local>, now: DateTime<Local>) -> String {
    let total = elapsed_minutes(check_in, now);
    let hours = total / 60;
    let minutes = total % 60;
    format!("{hours}h {minutes:02}m")
}

pub fn tax_amount(subtotal_cents: i64, tax_percent: f64) -> i64 {
    if tax_percent <= 0.0 {
        return 0;
    }
    ((subtotal_cents as f64) * tax_percent / 100.0).round() as i64
}

pub fn elapsed_minutes_now(check_in: DateTime<Local>, now: DateTime<Local>) -> i64 {
    elapsed_minutes(check_in, now)
}

pub fn expected_checkout_hourly(check_in: DateTime<Local>, hours: i64) -> DateTime<Local> {
    check_in + Duration::hours(hours.max(1))
}

pub fn expected_checkout_night(check_in: DateTime<Local>, nights: i64, cutoff_hour: i64) -> DateTime<Local> {
    let mut end = theoretical_night_end(check_in, cutoff_hour);
    for _ in 1..nights.max(1) {
        end += Duration::days(1);
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn plan(kind: RateKind, base: i64, extra: i64, included: i64, grace: i64, cutoff: i64) -> RatePlan {
        RatePlan {
            id: 1,
            name: "Test".into(),
            kind,
            base_amount_cents: base,
            extra_hour_cents: extra,
            included_hours: included,
            grace_minutes: grace,
            night_cutoff_hour: cutoff,
            active: true,
        }
    }

    fn dt(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn hourly_within_included_is_base_only() {
        let rate = plan(RateKind::Hourly, 1800, 500, 3, 10, 12);
        let check_in = dt(2026, 8, 31, 22, 0);
        let now = dt(2026, 8, 31, 23, 50);
        let lines = bill_hourly(&rate, check_in, now);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount_cents, 1800);
    }

    #[test]
    fn hourly_grace_prevents_extra_hour() {
        let rate = plan(RateKind::Hourly, 1800, 500, 3, 10, 12);
        let check_in = dt(2026, 8, 31, 20, 0);
        let now = dt(2026, 8, 31, 23, 8);
        assert_eq!(extra_hours_after(180, 188, 10), 0);
        let lines = bill_hourly(&rate, check_in, now);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn hourly_extra_hours_round_up_after_grace() {
        let rate = plan(RateKind::Hourly, 1800, 500, 3, 10, 12);
        let check_in = dt(2026, 8, 31, 20, 0);
        let now = dt(2026, 9, 1, 0, 20);
        let lines = bill_hourly(&rate, check_in, now);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].amount_cents, 1000);
    }

    #[test]
    fn night_before_cutoff_is_one_night() {
        let rate = plan(RateKind::Night, 4500, 600, 24, 15, 12);
        let check_in = dt(2026, 8, 31, 15, 0);
        let now = dt(2026, 9, 1, 11, 0);
        let lines = bill_night(&rate, check_in, now);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount_cents, 4500);
    }

    #[test]
    fn night_late_same_afternoon_adds_extra_hours() {
        let rate = plan(RateKind::Night, 4500, 600, 24, 15, 12);
        let check_in = dt(2026, 8, 31, 15, 0);
        let now = dt(2026, 9, 1, 14, 20);
        let lines = bill_night(&rate, check_in, now);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].amount_cents, 4500);
        assert_eq!(lines[1].amount_cents, 600 * 3);
    }

    #[test]
    fn night_next_morning_is_second_night() {
        let rate = plan(RateKind::Night, 4500, 600, 24, 15, 12);
        let check_in = dt(2026, 8, 31, 15, 0);
        let now = dt(2026, 9, 2, 10, 0);
        let lines = bill_night(&rate, check_in, now);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount_cents, 9000);
    }

    #[test]
    fn hourly_past_cutoff_switches_to_overnight() {
        let hourly = plan(RateKind::Hourly, 1800, 500, 3, 10, 12);
        let overnight = plan(RateKind::Overnight, 3200, 500, 12, 15, 12);
        let check_in = dt(2026, 8, 31, 22, 0);
        let now = dt(2026, 9, 1, 12, 30);
        let bill = preview(BillingContext {
            stay_id: 1,
            check_in_at: check_in,
            now,
            rate: &hourly,
            converted_to_overnight: false,
            overnight_plan: Some(&overnight),
            night_plan: None,
            manual_lines: vec![],
            tax_percent: 0.0,
        });
        assert!(bill.overnight_applied);
        assert_eq!(bill.applied_kind, RateKind::Overnight);
        assert!(bill.total_cents >= 3200);
    }

    #[test]
    fn manual_conversion_replaces_hourly_total() {
        let hourly = plan(RateKind::Hourly, 1800, 500, 3, 10, 12);
        let overnight = plan(RateKind::Overnight, 3200, 500, 12, 15, 12);
        let check_in = dt(2026, 8, 31, 22, 0);
        let now = dt(2026, 9, 1, 2, 0);
        let bill = preview(BillingContext {
            stay_id: 1,
            check_in_at: check_in,
            now,
            rate: &hourly,
            converted_to_overnight: true,
            overnight_plan: Some(&overnight),
            night_plan: None,
            manual_lines: vec![],
            tax_percent: 0.0,
        });
        assert!(bill.overnight_applied);
        assert_eq!(bill.total_cents, 3200);
    }

    #[test]
    fn tax_rounds_half_up() {
        assert_eq!(tax_amount(1000, 21.0), 210);
        assert_eq!(tax_amount(333, 10.0), 33);
    }
}
