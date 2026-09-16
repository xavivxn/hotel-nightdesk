use crate::models::{BillPreview, LineItem, RateKind, RatePlan};
use chrono::{Datelike, DateTime, Duration, Local, NaiveDate, TimeZone, Weekday};

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
    let overnight_applied = ctx.converted_to_overnight;

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
        at_local(check_in.date_naive() + chrono::Days::new(1), hour)
    }
}

/// Dormida always ends at the next 10:00: a 05:00 check-in checks out the same morning.
pub fn dormida_end(check_in: DateTime<Local>, cutoff_hour: i64) -> DateTime<Local> {
    theoretical_night_end(check_in, cutoff_hour)
}

/// Sun–Thu nights start at 22:00. Fri/Sat/holiday nights start at 00:00.
pub fn dormida_starts_at_midnight(check_in: DateTime<Local>, cutoff_hour: i64) -> bool {
    let end = dormida_end(check_in, cutoff_hour);
    let evening = end.date_naive() - chrono::Days::new(1);
    matches!(evening.weekday(), Weekday::Fri | Weekday::Sat)
        || is_paraguay_holiday(evening)
        || is_paraguay_holiday(end.date_naive())
}

pub fn dormida_start(check_in: DateTime<Local>, cutoff_hour: i64) -> DateTime<Local> {
    let end = dormida_end(check_in, cutoff_hour);
    if dormida_starts_at_midnight(check_in, cutoff_hour) {
        at_local(end.date_naive(), 0)
    } else {
        at_local(end.date_naive() - chrono::Days::new(1), 22)
    }
}

pub fn dormida_window_open(now: DateTime<Local>, cutoff_hour: i64) -> bool {
    now >= dormida_start(now, cutoff_hour) && now < dormida_end(now, cutoff_hour)
}

pub fn dormida_unavailable_message(now: DateTime<Local>, cutoff_hour: i64) -> String {
    if dormida_starts_at_midnight(now, cutoff_hour) {
        "La dormida de viernes, sábado y feriado se habilita a las 00:00 (hasta las 10:00).".into()
    } else {
        "La dormida de domingo a jueves se habilita a las 22:00 (hasta las 10:00).".into()
    }
}

fn is_paraguay_holiday(date: NaiveDate) -> bool {
    let (y, m, d) = (date.year(), date.month(), date.day());
    matches!(
        (m, d),
        (1, 1)
            | (3, 1)
            | (5, 1)
            | (5, 14)
            | (5, 15)
            | (6, 12)
            | (6, 20)
            | (8, 15)
            | (9, 29)
            | (12, 8)
            | (12, 25)
    ) || is_easter_thursday_or_friday(y, m, d)
}

fn is_easter_thursday_or_friday(year: i32, month: u32, day: u32) -> bool {
    let easter = gregorian_easter(year);
    let thursday = easter - chrono::Days::new(3);
    let friday = easter - chrono::Days::new(2);
    date_ymd(year, month, day) == thursday || date_ymd(year, month, day) == friday
}

fn date_ymd(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
}

fn gregorian_easter(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    date_ymd(year, month as u32, day as u32)
}

pub fn crossed_cutoff(check_in: DateTime<Local>, now: DateTime<Local>, cutoff_hour: i64) -> bool {
    now > theoretical_night_end(check_in, cutoff_hour)
}

fn bill_hourly(rate: &RatePlan, check_in: DateTime<Local>, now: DateTime<Local>) -> Vec<LineItem> {
    let elapsed = elapsed_minutes(check_in, now);
    let grace = rate.grace_minutes.max(0);
    let mut paid_until = rate.included_hours.max(1) * 60;
    let mut extra_halves: i64 = 0;
    let mut extra_hours: i64 = 0;
    let mut next_is_half = true;
    while elapsed > paid_until + grace && extra_halves + extra_hours < 500 {
        if next_is_half {
            extra_halves += 1;
            paid_until += 30;
            next_is_half = false;
        } else {
            extra_hours += 1;
            paid_until += 60;
            next_is_half = true;
        }
    }

    let mut lines = vec![LineItem {
        kind: "stay".into(),
        description: format!("{} (1 h)", rate.name),
        amount_cents: rate.base_amount_cents,
    }];
    if extra_halves > 0 {
        lines.push(LineItem {
            kind: "extra_hour".into(),
            description: if extra_halves == 1 {
                "Adicional 30 min".into()
            } else {
                format!("Adicional 30 min ({extra_halves})")
            },
            amount_cents: extra_halves * rate.extra_hour_cents,
        });
    }
    if extra_hours > 0 {
        lines.push(LineItem {
            kind: "extra_hour".into(),
            description: if extra_hours == 1 {
                "Hora adicional".into()
            } else {
                format!("Hora adicional ({extra_hours})")
            },
            amount_cents: extra_hours * rate.base_amount_cents,
        });
    }
    lines
}

fn bill_night(rate: &RatePlan, check_in: DateTime<Local>, now: DateTime<Local>) -> Vec<LineItem> {
    let mut period_end = dormida_end(check_in, rate.night_cutoff_hour);
    let mut nights: i64 = 1;
    let extra;

    loop {
        if now <= period_end {
            extra = 0;
            break;
        }
        if now.date_naive() == period_end.date_naive() {
            extra = extra_hours_after(
                0,
                (now - period_end).num_minutes().max(0),
                rate.grace_minutes,
            );
            break;
        }
        nights += 1;
        period_end += Duration::days(1);
    }

    let unit_label = match rate.kind {
        RateKind::Overnight => "dormida",
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

pub fn expected_checkout_night(
    check_in: DateTime<Local>,
    nights: i64,
    cutoff_hour: i64,
) -> DateTime<Local> {
    let mut end = dormida_end(check_in, cutoff_hour);
    for _ in 1..nights.max(1) {
        end += Duration::days(1);
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn plan(
        kind: RateKind,
        base: i64,
        extra: i64,
        included: i64,
        grace: i64,
        cutoff: i64,
    ) -> RatePlan {
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

    fn hourly() -> RatePlan {
        plan(RateKind::Hourly, 45_000, 15_000, 1, 5, 10)
    }

    fn overnight() -> RatePlan {
        plan(RateKind::Overnight, 120_000, 15_000, 12, 5, 10)
    }

    fn night() -> RatePlan {
        plan(RateKind::Night, 4500, 600, 24, 15, 12)
    }

    fn preview_turno(
        check_in: DateTime<Local>,
        now: DateTime<Local>,
        converted: bool,
        overnight_plan: Option<&RatePlan>,
        night_plan: Option<&RatePlan>,
        manual_lines: Vec<LineItem>,
        tax_percent: f64,
    ) -> BillPreview {
        let rate = hourly();
        preview(BillingContext {
            stay_id: 1,
            check_in_at: check_in,
            now,
            rate: &rate,
            converted_to_overnight: converted,
            overnight_plan,
            night_plan,
            manual_lines,
            tax_percent,
        })
    }

    fn stay_and_extra(lines: &[LineItem]) -> (i64, i64) {
        let stay = lines
            .iter()
            .find(|l| l.kind == "stay")
            .map(|l| l.amount_cents)
            .unwrap_or(0);
        let extra = lines
            .iter()
            .filter(|l| l.kind == "extra_hour")
            .map(|l| l.amount_cents)
            .sum();
        (stay, extra)
    }

    #[test]
    fn hourly_within_included_is_base_only() {
        let rate = hourly();
        let check_in = dt(2026, 8, 31, 22, 0);
        let now = dt(2026, 8, 31, 22, 50);
        let lines = bill_hourly(&rate, check_in, now);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount_cents, 45_000);
    }

    #[test]
    fn hourly_grace_prevents_extra_hour() {
        let rate = hourly();
        let check_in = dt(2026, 8, 31, 20, 0);
        let now = dt(2026, 8, 31, 21, 5);
        let lines = bill_hourly(&rate, check_in, now);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount_cents, 45_000);
    }

    #[test]
    fn hourly_five_minutes_past_hour_adds_half_hour() {
        let rate = hourly();
        let check_in = dt(2026, 8, 31, 14, 0);
        let lines = bill_hourly(&rate, check_in, dt(2026, 8, 31, 15, 6));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].amount_cents, 45_000);
        assert_eq!(lines[1].amount_cents, 15_000);
        assert_eq!(lines[1].description, "Adicional 30 min");
    }

    #[test]
    fn hourly_five_minutes_past_half_adds_another_hour() {
        let rate = hourly();
        let check_in = dt(2026, 8, 31, 14, 0);
        let on_grace = bill_hourly(&rate, check_in, dt(2026, 8, 31, 15, 35));
        assert_eq!(stay_and_extra(&on_grace), (45_000, 15_000));

        let past = bill_hourly(&rate, check_in, dt(2026, 8, 31, 15, 36));
        assert_eq!(stay_and_extra(&past), (45_000, 15_000 + 45_000));
        assert!(past.iter().any(|line| line.description == "Hora adicional"));
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
    fn hourly_does_not_switch_to_dormida_by_itself() {
        let rate = hourly();
        let overnight = overnight();
        let check_in = dt(2026, 8, 31, 22, 0);
        let now = dt(2026, 9, 1, 12, 30);
        let bill = preview(BillingContext {
            stay_id: 1,
            check_in_at: check_in,
            now,
            rate: &rate,
            converted_to_overnight: false,
            overnight_plan: Some(&overnight),
            night_plan: None,
            manual_lines: vec![],
            tax_percent: 0.0,
        });
        assert!(!bill.overnight_applied);
        assert_eq!(bill.applied_kind, RateKind::Hourly);
        assert_eq!(bill.lines[0].amount_cents, 45_000);
    }

    #[test]
    fn manual_conversion_replaces_hourly_total() {
        let rate = hourly();
        let overnight = overnight();
        let check_in = dt(2026, 8, 31, 22, 0);
        let now = dt(2026, 9, 1, 2, 0);
        let bill = preview(BillingContext {
            stay_id: 1,
            check_in_at: check_in,
            now,
            rate: &rate,
            converted_to_overnight: true,
            overnight_plan: Some(&overnight),
            night_plan: None,
            manual_lines: vec![],
            tax_percent: 0.0,
        });
        assert!(bill.overnight_applied);
        assert_eq!(bill.total_cents, 120_000);
    }

    #[test]
    fn tax_rounds_half_up() {
        assert_eq!(tax_amount(1000, 21.0), 210);
        assert_eq!(tax_amount(333, 10.0), 33);
        assert_eq!(tax_amount(5, 10.0), 1);
        assert_eq!(tax_amount(15, 10.0), 2);
        assert_eq!(tax_amount(100, 0.0), 0);
        assert_eq!(tax_amount(100, -21.0), 0);
        assert_eq!(tax_amount(0, 21.0), 0);
    }

    #[test]
    fn extra_hours_after_covers_grace_and_round_up() {
        let cases = [
            // included, elapsed, grace, extra
            (180, 0, 10, 0),
            (180, 180, 10, 0),
            (180, 190, 10, 0),
            (180, 191, 10, 1),
            (180, 240, 10, 1),
            (180, 250, 10, 1),
            (180, 251, 10, 2),
            (180, 300, 10, 2),
            (180, 310, 10, 2),
            (180, 311, 10, 3),
            (180, 180, 0, 0),
            (180, 181, 0, 1),
            (180, 240, 0, 1),
            (180, 241, 0, 2),
            (0, 0, 15, 0),
            (0, 15, 15, 0),
            (0, 16, 15, 1),
            (0, 60, 15, 1),
            (0, 75, 15, 1),
            (0, 76, 15, 2),
        ];
        for (included, elapsed, grace, expected) in cases {
            assert_eq!(
                extra_hours_after(included, elapsed, grace),
                expected,
                "included={included} elapsed={elapsed} grace={grace}"
            );
        }
    }

    #[test]
    fn hourly_turno_edges_are_base_then_half_then_hour() {
        let rate = hourly();
        let check_in = dt(2026, 8, 31, 14, 0);
        let cases = [
            (dt(2026, 8, 31, 14, 0), 45_000, 0),
            (dt(2026, 8, 31, 15, 0), 45_000, 0),
            (dt(2026, 8, 31, 15, 5), 45_000, 0),
            (dt(2026, 8, 31, 15, 6), 45_000, 15_000),
            (dt(2026, 8, 31, 15, 35), 45_000, 15_000),
            (dt(2026, 8, 31, 15, 36), 45_000, 60_000),
            (dt(2026, 8, 31, 16, 35), 45_000, 60_000),
            (dt(2026, 8, 31, 16, 36), 45_000, 75_000),
        ];
        for (now, stay, extra) in cases {
            let (got_stay, got_extra) = stay_and_extra(&bill_hourly(&rate, check_in, now));
            assert_eq!(
                (got_stay, got_extra),
                (stay, extra),
                "now={now} duration={}",
                format_duration(check_in, now)
            );
        }
    }

    #[test]
    fn hourly_zero_included_hours_counts_as_one() {
        let rate = plan(RateKind::Hourly, 45_000, 15_000, 0, 5, 10);
        let check_in = dt(2026, 8, 31, 14, 0);
        let now = dt(2026, 8, 31, 15, 6);
        let (stay, extra) = stay_and_extra(&bill_hourly(&rate, check_in, now));
        assert_eq!(stay, 45_000);
        assert_eq!(extra, 15_000);
    }

    #[test]
    fn hourly_now_before_check_in_is_base_only() {
        let rate = hourly();
        let check_in = dt(2026, 8, 31, 14, 0);
        let now = dt(2026, 8, 31, 13, 0);
        let lines = bill_hourly(&rate, check_in, now);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount_cents, 45_000);
        assert_eq!(format_duration(check_in, now), "0h 00m");
    }

    #[test]
    fn hourly_past_midnight_stays_hourly() {
        let overnight = overnight();
        let bill = preview_turno(
            dt(2026, 8, 31, 22, 0),
            dt(2026, 9, 1, 2, 0),
            false,
            Some(&overnight),
            None,
            vec![],
            0.0,
        );
        assert!(!bill.overnight_applied);
        assert_eq!(bill.applied_kind, RateKind::Hourly);
        assert_eq!(bill.duration_label, "4h 00m");
        let (stay, extra) = stay_and_extra(&bill.lines);
        assert_eq!(stay, 45_000);
        assert_eq!(extra, 120_000);
        assert_eq!(bill.total_cents, 165_000);
    }

    #[test]
    fn hourly_past_cutoff_stays_hourly_until_converted() {
        let overnight = overnight();
        let bill = preview_turno(
            dt(2026, 8, 31, 22, 0),
            dt(2026, 9, 1, 12, 1),
            false,
            Some(&overnight),
            None,
            vec![],
            0.0,
        );
        assert!(!bill.overnight_applied);
        assert_eq!(bill.applied_kind, RateKind::Hourly);
        assert_eq!(bill.lines[0].amount_cents, 45_000);
    }

    #[test]
    fn hourly_morning_turno_stays_hourly() {
        let overnight = overnight();
        let still_hourly = preview_turno(
            dt(2026, 9, 1, 10, 0),
            dt(2026, 9, 1, 11, 50),
            false,
            Some(&overnight),
            None,
            vec![],
            0.0,
        );
        assert!(!still_hourly.overnight_applied);
        assert_eq!(still_hourly.total_cents, 105_000);

        let later = preview_turno(
            dt(2026, 9, 1, 10, 0),
            dt(2026, 9, 1, 12, 30),
            false,
            Some(&overnight),
            None,
            vec![],
            0.0,
        );
        assert!(!later.overnight_applied);
        assert_eq!(later.applied_kind, RateKind::Hourly);
    }

    #[test]
    fn hourly_check_in_at_cutoff_stays_hourly() {
        let overnight = overnight();
        let same_day = preview_turno(
            dt(2026, 9, 1, 12, 0),
            dt(2026, 9, 1, 15, 0),
            false,
            Some(&overnight),
            None,
            vec![],
            0.0,
        );
        assert!(!same_day.overnight_applied);
        assert_eq!(same_day.applied_kind, RateKind::Hourly);
    }

    #[test]
    fn overnight_fallback_uses_night_plan_when_converted() {
        let night = night();
        let bill = preview_turno(
            dt(2026, 8, 31, 22, 0),
            dt(2026, 9, 1, 12, 30),
            true,
            None,
            Some(&night),
            vec![],
            0.0,
        );
        assert!(bill.overnight_applied);
        assert_eq!(bill.applied_kind, RateKind::Night);
        let (stay, extra) = stay_and_extra(&bill.lines);
        assert_eq!(stay, 4500);
        assert_eq!(extra, 600);
        assert_eq!(bill.total_cents, 5100);
    }

    #[test]
    fn overnight_without_plan_keeps_hourly_if_not_converted() {
        let bill = preview_turno(
            dt(2026, 8, 31, 22, 0),
            dt(2026, 9, 1, 12, 30),
            false,
            None,
            None,
            vec![],
            0.0,
        );
        assert!(!bill.overnight_applied);
        assert_eq!(bill.applied_kind, RateKind::Hourly);
        assert_eq!(bill.lines[0].amount_cents, 45_000);
    }

    #[test]
    fn manual_conversion_after_cutoff_uses_overnight() {
        let overnight = overnight();
        let bill = preview_turno(
            dt(2026, 8, 31, 22, 0),
            dt(2026, 9, 1, 9, 0),
            true,
            Some(&overnight),
            None,
            vec![],
            0.0,
        );
        assert!(bill.overnight_applied);
        let (stay, extra) = stay_and_extra(&bill.lines);
        assert_eq!(stay, 120_000);
        assert_eq!(extra, 0);
        assert_eq!(bill.total_cents, 120_000);
    }

    #[test]
    fn turno_tax_applies_to_base_and_extras() {
        let overnight = overnight();
        let bill = preview_turno(
            dt(2026, 8, 31, 14, 0),
            dt(2026, 8, 31, 15, 6),
            false,
            Some(&overnight),
            None,
            vec![],
            10.0,
        );
        assert!(!bill.overnight_applied);
        assert_eq!(bill.subtotal_cents, 60_000);
        assert_eq!(bill.tax_cents, 6_000);
        assert_eq!(bill.total_cents, 66_000);
    }

    #[test]
    fn turno_manual_lines_enter_subtotal_and_tax() {
        let overnight = overnight();
        let manuals = vec![
            LineItem {
                kind: "surcharge".into(),
                description: "Frigobar".into(),
                amount_cents: 10_000,
            },
            LineItem {
                kind: "discount".into(),
                description: "Cortesía".into(),
                amount_cents: -5_000,
            },
        ];
        let bill = preview_turno(
            dt(2026, 8, 31, 14, 0),
            dt(2026, 8, 31, 14, 40),
            false,
            Some(&overnight),
            None,
            manuals,
            10.0,
        );
        assert_eq!(bill.subtotal_cents, 50_000);
        assert_eq!(bill.tax_cents, 5_000);
        assert_eq!(bill.total_cents, 55_000);
        assert_eq!(bill.lines.len(), 3);
    }

    #[test]
    fn night_grace_at_cutoff_then_extra_hour() {
        let rate = night();
        let check_in = dt(2026, 8, 31, 15, 0);
        let on_grace = bill_night(&rate, check_in, dt(2026, 9, 1, 12, 15));
        assert_eq!(on_grace.len(), 1);
        assert_eq!(on_grace[0].amount_cents, 4500);

        let past_grace = bill_night(&rate, check_in, dt(2026, 9, 1, 12, 16));
        assert_eq!(past_grace.len(), 2);
        assert_eq!(past_grace[1].amount_cents, 600);
    }

    #[test]
    fn night_just_after_midnight_is_second_night() {
        let rate = night();
        let lines = bill_night(&rate, dt(2026, 8, 31, 15, 0), dt(2026, 9, 2, 0, 1));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].amount_cents, 9000);
    }

    #[test]
    fn format_duration_pads_minutes() {
        assert_eq!(
            format_duration(dt(2026, 8, 31, 14, 0), dt(2026, 8, 31, 16, 5)),
            "2h 05m"
        );
        assert_eq!(
            format_duration(dt(2026, 8, 31, 14, 0), dt(2026, 8, 31, 14, 0)),
            "0h 00m"
        );
    }

    #[test]
    fn expected_checkout_helpers() {
        let check_in = dt(2026, 8, 31, 22, 0);
        assert_eq!(expected_checkout_hourly(check_in, 3), dt(2026, 9, 1, 1, 0));
        assert_eq!(
            expected_checkout_night(check_in, 1, 12),
            dt(2026, 9, 1, 12, 0)
        );
        assert_eq!(
            expected_checkout_night(check_in, 2, 12),
            dt(2026, 9, 2, 12, 0)
        );
    }

    #[test]
    fn dormida_5am_checks_out_same_morning() {
        let check_in = dt(2026, 9, 16, 5, 0);
        assert_eq!(dormida_end(check_in, 10), dt(2026, 9, 16, 10, 0));
        let rate = overnight();
        let before = bill_night(&rate, check_in, dt(2026, 9, 16, 9, 50));
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].amount_cents, 120_000);
        let after = bill_night(&rate, check_in, dt(2026, 9, 16, 10, 6));
        assert_eq!(after.len(), 2);
    }

    #[test]
    fn dormida_weekday_starts_at_22() {
        let evening = dt(2026, 9, 16, 23, 0);
        assert_eq!(dormida_start(evening, 10), dt(2026, 9, 16, 22, 0));
        assert_eq!(dormida_end(evening, 10), dt(2026, 9, 17, 10, 0));
        assert!(!dormida_starts_at_midnight(evening, 10));
    }

    #[test]
    fn dormida_friday_night_starts_at_midnight() {
        let friday_night = dt(2026, 9, 18, 23, 0);
        assert!(dormida_starts_at_midnight(friday_night, 10));
        assert_eq!(dormida_start(friday_night, 10), dt(2026, 9, 19, 0, 0));
        assert_eq!(dormida_end(friday_night, 10), dt(2026, 9, 19, 10, 0));
    }

    #[test]
    fn dormida_saturday_dawn_checks_out_at_10() {
        let dawn = dt(2026, 9, 19, 5, 0);
        assert_eq!(dormida_end(dawn, 10), dt(2026, 9, 19, 10, 0));
        assert!(dormida_starts_at_midnight(dawn, 10));
        assert_eq!(dormida_start(dawn, 10), dt(2026, 9, 19, 0, 0));
    }

    #[test]
    fn dormida_christmas_uses_midnight_window() {
        let eve = dt(2026, 12, 24, 23, 0);
        assert!(dormida_starts_at_midnight(eve, 10));
        assert_eq!(dormida_start(eve, 10), dt(2026, 12, 25, 0, 0));
        assert_eq!(dormida_end(eve, 10), dt(2026, 12, 25, 10, 0));
    }

    #[test]
    fn dormida_window_weekday_opens_at_22() {
        assert!(!dormida_window_open(dt(2026, 9, 16, 21, 59), 10));
        assert!(dormida_window_open(dt(2026, 9, 16, 22, 0), 10));
        assert!(dormida_window_open(dt(2026, 9, 17, 9, 59), 10));
        assert!(!dormida_window_open(dt(2026, 9, 17, 10, 0), 10));
    }

    #[test]
    fn dormida_window_friday_opens_at_midnight() {
        assert!(dormida_window_open(dt(2026, 9, 18, 9, 0), 10));
        assert!(!dormida_window_open(dt(2026, 9, 18, 10, 0), 10));
        assert!(!dormida_window_open(dt(2026, 9, 18, 23, 0), 10));
        assert!(dormida_window_open(dt(2026, 9, 19, 0, 0), 10));
        assert!(dormida_window_open(dt(2026, 9, 19, 9, 59), 10));
        assert!(!dormida_window_open(dt(2026, 9, 19, 10, 0), 10));
    }

    #[test]
    fn pyg_seed_hourly_and_dormida() {
        let paso = hourly();
        let overnight = overnight();

        let turno = preview(BillingContext {
            stay_id: 1,
            check_in_at: dt(2026, 8, 31, 14, 0),
            now: dt(2026, 8, 31, 15, 6),
            rate: &paso,
            converted_to_overnight: false,
            overnight_plan: Some(&overnight),
            night_plan: None,
            manual_lines: vec![],
            tax_percent: 10.0,
        });
        assert!(!turno.overnight_applied);
        assert_eq!(turno.subtotal_cents, 60_000);
        assert_eq!(turno.tax_cents, 6_000);
        assert_eq!(turno.total_cents, 66_000);

        let dormida = preview(BillingContext {
            stay_id: 1,
            check_in_at: dt(2026, 8, 31, 22, 0),
            now: dt(2026, 9, 1, 9, 0),
            rate: &overnight,
            converted_to_overnight: false,
            overnight_plan: Some(&overnight),
            night_plan: None,
            manual_lines: vec![],
            tax_percent: 10.0,
        });
        assert!(!dormida.overnight_applied);
        assert_eq!(dormida.applied_kind, RateKind::Overnight);
        assert_eq!(dormida.lines[0].amount_cents, 120_000);
        assert_eq!(dormida.subtotal_cents, 120_000);
        assert_eq!(dormida.tax_cents, 12_000);
        assert_eq!(dormida.total_cents, 132_000);
    }
}
