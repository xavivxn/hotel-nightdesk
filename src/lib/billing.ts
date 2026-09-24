/** Mock-only preview for `npm run dev`. Authority is `src-tauri/src/billing.rs`. */
import type { BillPreview, LineItem, RateKind, RatePlan } from "./types";

function extraHoursAfter(includedMinutes: number, elapsedMinutes: number, graceMinutes: number) {
  if (elapsedMinutes <= includedMinutes + graceMinutes) return 0;
  const remainder = elapsedMinutes - includedMinutes;
  let extra = Math.floor(remainder / 60);
  if (remainder % 60 > graceMinutes) extra += 1;
  return extra;
}

function atLocal(date: Date, hour: number) {
  const next = new Date(date);
  next.setHours(hour, 0, 0, 0);
  return next;
}

export function theoreticalNightEnd(checkIn: Date, cutoffHour: number) {
  const sameDay = atLocal(checkIn, cutoffHour);
  if (checkIn < sameDay) return sameDay;
  const next = atLocal(checkIn, cutoffHour);
  next.setDate(next.getDate() + 1);
  return next;
}

export function dormidaEnd(checkIn: Date, cutoffHour = 10) {
  return theoreticalNightEnd(checkIn, cutoffHour);
}

function ymd(date: Date) {
  return { y: date.getFullYear(), m: date.getMonth() + 1, d: date.getDate() };
}

function gregorianEaster(year: number) {
  const a = year % 19;
  const b = Math.floor(year / 100);
  const c = year % 100;
  const d = Math.floor(b / 4);
  const e = b % 4;
  const f = Math.floor((b + 8) / 25);
  const g = Math.floor((b - f + 1) / 3);
  const h = (19 * a + b - d - g + 15) % 30;
  const i = Math.floor(c / 4);
  const k = c % 4;
  const l = (32 + 2 * e + 2 * i - h - k) % 7;
  const m = Math.floor((a + 11 * h + 22 * l) / 451);
  const month = Math.floor((h + l - 7 * m + 114) / 31);
  const day = ((h + l - 7 * m + 114) % 31) + 1;
  return new Date(year, month - 1, day);
}

function isParaguayHoliday(date: Date) {
  const { y, m, d } = ymd(date);
  const fixed = (
    (m === 1 && d === 1) ||
    (m === 3 && d === 1) ||
    (m === 5 && (d === 1 || d === 14 || d === 15)) ||
    (m === 6 && (d === 12 || d === 20)) ||
    (m === 8 && d === 15) ||
    (m === 9 && d === 29) ||
    (m === 12 && (d === 8 || d === 25))
  );
  if (fixed) return true;
  const easter = gregorianEaster(y);
  const thursday = new Date(easter);
  thursday.setDate(easter.getDate() - 3);
  const friday = new Date(easter);
  friday.setDate(easter.getDate() - 2);
  return (
    (thursday.getFullYear() === y && thursday.getMonth() + 1 === m && thursday.getDate() === d) ||
    (friday.getFullYear() === y && friday.getMonth() + 1 === m && friday.getDate() === d)
  );
}

export function dormidaStartsAtMidnight(checkIn: Date, cutoffHour = 10) {
  const end = dormidaEnd(checkIn, cutoffHour);
  const evening = new Date(end);
  evening.setDate(evening.getDate() - 1);
  const dow = evening.getDay();
  return dow === 5 || dow === 6 || isParaguayHoliday(evening) || isParaguayHoliday(end);
}

export function dormidaStart(checkIn: Date, cutoffHour = 10) {
  const end = dormidaEnd(checkIn, cutoffHour);
  if (dormidaStartsAtMidnight(checkIn, cutoffHour)) {
    return atLocal(end, 0);
  }
  const evening = new Date(end);
  evening.setDate(evening.getDate() - 1);
  return atLocal(evening, 22);
}

export function dormidaWindowOpen(now: Date, cutoffHour = 10) {
  return now >= dormidaStart(now, cutoffHour) && now < dormidaEnd(now, cutoffHour);
}

export function dormidaUnavailableMessage(now: Date, cutoffHour = 10) {
  return dormidaStartsAtMidnight(now, cutoffHour)
    ? "La dormida de viernes, sábado y feriado se habilita a las 00:00 (hasta las 10:00)."
    : "La dormida de domingo a jueves se habilita a las 22:00 (hasta las 10:00).";
}

function elapsedMinutes(checkIn: Date, now: Date) {
  return Math.max(0, Math.floor((now.getTime() - checkIn.getTime()) / 60000));
}

export function formatDurationLabel(checkIn: Date, now: Date) {
  const total = elapsedMinutes(checkIn, now);
  const hours = Math.floor(total / 60);
  const minutes = total % 60;
  return `${hours}h ${minutes.toString().padStart(2, "0")}m`;
}

function billHourly(rate: RatePlan, checkIn: Date, now: Date): LineItem[] {
  const elapsed = elapsedMinutes(checkIn, now);
  const grace = Math.max(0, rate.grace_minutes);
  const includedMinutes = Math.max(1, rate.included_hours) * 60;
  const extraMinutes = Math.max(0, elapsed - includedMinutes - grace);
  const extraBlocks = Math.min(500, Math.ceil(extraMinutes / 30));
  const lines: LineItem[] = [
    {
      kind: "stay",
      description: `${rate.name} (1 h)`,
      amount_cents: rate.base_amount_cents,
    },
  ];
  if (extraBlocks > 0) {
    lines.push({
      kind: "extra_hour",
      description: extraBlocks === 1 ? "Adicional 30 min" : `Extra 30 min x${extraBlocks}`,
      amount_cents: extraBlocks * rate.extra_hour_cents,
    });
  }
  return lines;
}

function billNight(rate: RatePlan, checkIn: Date, now: Date): LineItem[] {
  let periodEnd = dormidaEnd(checkIn, rate.night_cutoff_hour);
  let nights = 1;
  let extra = 0;
  while (true) {
    if (now <= periodEnd) {
      extra = 0;
      break;
    }
    if (now.toDateString() === periodEnd.toDateString()) {
      extra = extraHoursAfter(0, elapsedMinutes(periodEnd, now), rate.grace_minutes);
      break;
    }
    nights += 1;
    periodEnd = new Date(periodEnd.getTime() + 24 * 60 * 60 * 1000);
  }
  const unit = rate.kind === "overnight" ? "dormida" : "noche";
  const lines: LineItem[] = [
    {
      kind: "stay",
      description: nights === 1 ? `${rate.name} (1 ${unit})` : `${rate.name} (${nights} ${unit}s)`,
      amount_cents: nights * rate.base_amount_cents,
    },
  ];
  if (extra > 0) {
    lines.push({
      kind: "extra_hour",
      description: `Horas extra (${extra})`,
      amount_cents: extra * rate.extra_hour_cents,
    });
  }
  return lines;
}

export function previewBill(args: {
  stayId: number;
  checkIn: Date;
  now: Date;
  rate: RatePlan;
  converted: boolean;
  overnightPlan?: RatePlan | null;
  nightPlan?: RatePlan | null;
  manualLines: LineItem[];
  taxPercent: number;
}): BillPreview {
  const overnightApplied = args.converted;
  let applied = args.rate;
  let appliedKind: RateKind = args.rate.kind;
  if (overnightApplied) {
    if (args.overnightPlan) {
      applied = args.overnightPlan;
      appliedKind = args.overnightPlan.kind;
    } else if (args.nightPlan) {
      applied = args.nightPlan;
      appliedKind = args.nightPlan.kind;
    }
  }
  const lines =
    appliedKind === "hourly"
      ? billHourly(applied, args.checkIn, args.now)
      : billNight(applied, args.checkIn, args.now);
  lines.push(...args.manualLines);
  const subtotal = lines.reduce((sum, line) => sum + line.amount_cents, 0);
  const tax = args.taxPercent <= 0 ? 0 : Math.round((subtotal * args.taxPercent) / 100);
  return {
    stay_id: args.stayId,
    lines,
    subtotal_cents: subtotal,
    tax_percent: args.taxPercent,
    tax_cents: tax,
    total_cents: subtotal + tax,
    applied_kind: appliedKind,
    duration_label: formatDurationLabel(args.checkIn, args.now),
    overnight_applied: overnightApplied,
  };
}
