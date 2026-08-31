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
  const extra = extraHoursAfter(Math.max(1, rate.included_hours) * 60, elapsed, rate.grace_minutes);
  const lines: LineItem[] = [
    {
      kind: "stay",
      description: `${rate.name} (${rate.included_hours} h)`,
      amount_cents: rate.base_amount_cents,
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

function billNight(rate: RatePlan, checkIn: Date, now: Date): LineItem[] {
  let periodEnd = theoreticalNightEnd(checkIn, rate.night_cutoff_hour);
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
  const unit = rate.kind === "overnight" ? "pernocte" : "noche";
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
  const crossed = args.now > theoreticalNightEnd(args.checkIn, args.rate.night_cutoff_hour);
  const overnightApplied = args.converted || (args.rate.kind === "hourly" && crossed);
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
