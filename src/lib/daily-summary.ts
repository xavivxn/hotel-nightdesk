/** Pure rules for the admin "Resumen del día": alerts, shift buckets and the text to share. */
import { formatMoney } from "./format";
import type { AnalyticsSummary, BackupStatus, BoardRoom, Product, ProductStock, RatePlan } from "./types";

export type AlertLevel = "danger" | "warn" | "info";
export type SummaryAlert = { key: string; level: AlertLevel; title: string; detail: string };

/** An hourly stay this long past its expected checkout may have been left open by mistake. */
export const FORGOTTEN_MINUTES = 6 * 60;
/** Past expected checkout by this much, the stay is worth a look. */
export const OVERDUE_MINUTES = 60;
/** A confirmed backup older than this is late (daily schedule + margin). */
export const BACKUP_LATE_HOURS = 26;

const pad = (n: number) => String(n).padStart(2, "0");

export function businessToday(now = new Date()) {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: "America/Asuncion", year: "numeric", month: "2-digit", day: "2-digit",
  }).formatToParts(now);
  const get = (type: string) => parts.find((part) => part.type === type)?.value ?? "";
  return `${get("year")}-${get("month")}-${get("day")}`;
}

export function shiftDay(day: string, offset: number) {
  const date = new Date(`${day}T12:00:00Z`);
  date.setUTCDate(date.getUTCDate() + offset);
  return `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}`;
}

/** "martes 29/09" */
export function dayLabel(day: string) {
  const date = new Date(`${day}T12:00:00Z`);
  const weekday = new Intl.DateTimeFormat("es-PY", { weekday: "long", timeZone: "UTC" }).format(date);
  return `${weekday} ${day.slice(8, 10)}/${day.slice(5, 7)}`;
}

export function durationLabel(minutes: number) {
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours >= 48) return `${Math.floor(hours / 24)} días ${hours % 24} h`;
  return hours ? `${hours} h ${pad(rest)} min` : `${rest} min`;
}

export const SHIFTS = [
  { key: "madrugada", label: "Madrugada", from: 0, to: 6 },
  { key: "manana", label: "Mañana", from: 6, to: 12 },
  { key: "tarde", label: "Tarde", from: 12, to: 18 },
  { key: "noche", label: "Noche", from: 18, to: 24 },
] as const;

export type ShiftTotals = { key: string; label: string; range: string; checkIns: number; closed: number; revenue: number };

export function shiftTotals(report: AnalyticsSummary): ShiftTotals[] {
  return SHIFTS.map((shift) => {
    const inShift = (hour: number) => hour >= shift.from && hour < shift.to;
    const closed = report.closed_hours.filter((item) => inShift(item.hour));
    return {
      key: shift.key,
      label: shift.label,
      range: `${pad(shift.from)}–${pad(shift.to)}`,
      checkIns: report.check_in_hours.filter((item) => inShift(item.hour)).reduce((sum, item) => sum + item.count, 0),
      closed: closed.reduce((sum, item) => sum + item.count, 0),
      revenue: closed.reduce((sum, item) => sum + item.revenue_cents, 0),
    };
  });
}

/** Revenue closed before `hour` (exclusive): compares today so far with the same hours a week ago. */
export function revenueBeforeHour(report: AnalyticsSummary, hour: number) {
  return report.closed_hours.filter((item) => item.hour < hour).reduce((sum, item) => sum + item.revenue_cents, 0);
}

export function dormidaPrice(rates: RatePlan[]) {
  const plan = rates.find((rate) => rate.active && rate.kind === "overnight") ?? rates.find((rate) => rate.active && rate.kind === "night");
  return plan?.base_amount_cents ?? null;
}

export function buildAlerts(input: {
  now: Date;
  board: BoardRoom[] | null;
  rates: RatePlan[];
  report: AnalyticsSummary | null;
  backup: BackupStatus | null;
  stock: ProductStock[];
  products: Product[];
  currency: string;
}): SummaryAlert[] {
  const { now, board, rates, report, backup, stock, products, currency } = input;
  const money = (amount: number) => formatMoney(amount, currency);
  const alerts: SummaryAlert[] = [];
  const dormida = dormidaPrice(rates);

  for (const item of board ?? []) {
    const stay = item.stay;
    if (!stay) continue;
    const total = item.estimated_total_cents;
    const hourly = stay.rate_kind === "hourly" && !stay.converted_to_overnight;
    const betterAsDormida = hourly && dormida != null && total != null && total > dormida;
    const expected = stay.expected_checkout_at ? Date.parse(stay.expected_checkout_at) : NaN;
    const overdue = Number.isFinite(expected) ? Math.floor((now.getTime() - expected) / 60000) : 0;
    const room = `Hab. ${item.room.number}`;
    const totalText = total != null ? ` · cuenta ${money(total)}` : "";
    const dormidaText = betterAsDormida ? ` · supera la dormida (${money(dormida!)})` : "";
    if (overdue >= OVERDUE_MINUTES) {
      const forgotten = overdue >= FORGOTTEN_MINUTES;
      alerts.push({
        key: `overdue-${item.room.id}`,
        // A few extra hours is normal business (they are billed); many hours looks forgotten.
        level: forgotten ? "danger" : betterAsDormida ? "warn" : "info",
        title: forgotten
          ? `${room} lleva ${durationLabel(overdue)} de más: ¿quedó abierta por error?`
          : `${room} lleva ${durationLabel(overdue)} más de lo previsto`,
        detail: `${stay.rate_plan_name || "Tarifa"}${totalText}${dormidaText}`,
      });
    } else if (betterAsDormida) {
      alerts.push({
        key: `dormida-${item.room.id}`,
        level: "warn",
        title: `${room}: conviene pasar a dormida`,
        detail: `La cuenta por hora (${money(total!)}) ya supera la dormida (${money(dormida!)}).`,
      });
    }
  }

  const lastBackup = backup?.last_remote_at ?? backup?.last_local_at ?? null;
  const backupAge = lastBackup ? (now.getTime() - Date.parse(lastBackup)) / 3_600_000 : Infinity;
  if (backup && backupAge >= BACKUP_LATE_HOURS) {
    alerts.push({
      key: "backup",
      level: "danger",
      title: lastBackup ? `Último respaldo confirmado hace ${durationLabel(Math.floor(backupAge * 60))}` : "No hay respaldos confirmados",
      detail: "Revisá Ajustes › Respaldos en la PC de recepción.",
    });
  }

  if (report && report.voided_count > 0) {
    alerts.push({
      key: "voided",
      level: "warn",
      title: `${report.voided_count} ${report.voided_count === 1 ? "consumo anulado" : "consumos anulados"} por ${money(report.voided_cents)}`,
      detail: "Se quitaron de cuentas abiertas antes del cierre.",
    });
  }
  if (report && report.discount_cents < 0) {
    alerts.push({
      key: "discounts",
      level: "warn",
      title: `Descuentos por ${money(Math.abs(report.discount_cents))}`,
      detail: "Aplicados en cuentas cerradas del día.",
    });
  }

  const low = stock
    .filter((row) => row.quantity <= row.min_quantity || row.quantity <= 0)
    .map((row) => ({ row, product: products.find((item) => item.id === row.product_id && item.active) }))
    .filter((entry) => entry.product)
    .sort((a, b) => a.row.quantity - b.row.quantity);
  if (low.length) {
    alerts.push({
      key: "stock",
      level: low.some((entry) => entry.row.quantity <= 0) ? "warn" : "info",
      title: `${low.length} ${low.length === 1 ? "producto" : "productos"} con stock bajo`,
      detail: low.slice(0, 6).map((entry) => `${entry.product!.name} (${entry.row.quantity})`).join(", ") + (low.length > 6 ? "…" : ""),
    });
  }

  const dirty = (board ?? []).filter((item) => item.display_status === "dirty").map((item) => item.room.number);
  if (dirty.length) {
    alerts.push({
      key: "dirty",
      level: "info",
      title: `${dirty.length} ${dirty.length === 1 ? "habitación" : "habitaciones"} por limpiar`,
      detail: dirty.join(", "),
    });
  }
  const blocked = (board ?? []).filter((item) => item.display_status === "blocked").map((item) => item.room.number);
  if (blocked.length) {
    alerts.push({
      key: "blocked",
      level: "info",
      title: `${blocked.length} ${blocked.length === 1 ? "habitación bloqueada" : "habitaciones bloqueadas"}`,
      detail: blocked.join(", "),
    });
  }

  const order: Record<AlertLevel, number> = { danger: 0, warn: 1, info: 2 };
  return alerts.sort((a, b) => order[a.level] - order[b.level]);
}

export function shareText(input: {
  business: string;
  day: string;
  report: AnalyticsSummary;
  occupied: number | null;
  rooms: number | null;
  alerts: SummaryAlert[];
  currency: string;
}) {
  const { business, day, report, occupied, rooms, alerts, currency } = input;
  const money = (amount: number) => formatMoney(amount, currency).replace(/\u00a0/g, " ");
  const lines = [
    `${business} · Resumen del ${dayLabel(day)}`,
    `Ingresos: ${money(report.total_revenue_cents)} en ${report.closed_accounts} ${report.closed_accounts === 1 ? "cuenta" : "cuentas"}`,
  ];
  if (report.closed_accounts) lines.push(`Ticket promedio: ${money(report.average_ticket_cents)}`);
  if (occupied != null && rooms != null) lines.push(`Ocupadas ahora: ${occupied} de ${rooms}`);
  const review = alerts.filter((alert) => alert.level !== "info");
  lines.push(review.length ? "Para revisar:" : "Sin alertas.");
  for (const alert of review) lines.push(`- ${alert.title}`);
  return lines.join("\n");
}
