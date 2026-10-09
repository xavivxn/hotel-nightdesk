import type { AnalyticsSummary } from "./types";
import { fail } from "./errors";

export const ANALYTICS_TIME_ZONE = "America/Asuncion";
const formatter = new Intl.DateTimeFormat("en-CA", {
  timeZone: ANALYTICS_TIME_ZONE, year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", hourCycle: "h23",
});
export function analyticsTime(value: string) {
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) fail("storage", "Fecha de análisis inválida");
  const parts = formatter.formatToParts(timestamp);
  const part = (type: string) => parts.find(item => item.type === type)!.value;
  return { date: `${part("year")}-${part("month")}-${part("day")}`, hour: Number(part("hour")) };
}
const weekday = (day: string) => (new Date(`${day}T12:00:00Z`).getUTCDay() + 6) % 7;
function add(a: number, b: number) {
  if (!Number.isSafeInteger(a) || !Number.isSafeInteger(b) || !Number.isSafeInteger(a + b)) {
    fail("storage", "El total del análisis está fuera de rango");
  }
  return a + b;
}

/** Both transports use complete local two-hour blocks, excluding the current unfinished block. */
export function buildCheckInHeatmap(days: string[], checkIns: string[], generatedAt: string): AnalyticsSummary["check_in_heatmap"] {
  const now = analyticsTime(generatedAt);
  const observed = (date: string, hour: number) => date < now.date || (date === now.date && hour + 2 <= now.hour);
  const included = new Set(days);
  const cells = Array.from({ length: 84 }, (_, index) => ({
    weekday: Math.floor(index / 12), hour: index % 12 * 2, count: 0, observed_blocks: 0, average: null as number | null,
  }));
  for (const date of days) for (let hour = 0; hour < 24; hour += 2) {
    if (observed(date, hour)) cells[weekday(date) * 12 + hour / 2].observed_blocks += 1;
  }
  for (const stamp of checkIns) {
    const { date, hour } = analyticsTime(stamp);
    const blockHour = Math.floor(hour / 2) * 2;
    if (included.has(date) && observed(date, blockHour)) cells[weekday(date) * 12 + blockHour / 2].count += 1;
  }
  for (const cell of cells) cell.average = cell.observed_blocks ? cell.count / cell.observed_blocks : null;
  return cells;
}

type ProductLine = { product_uid?: string | null; product_quantity?: number | null; description: string; amount_cents: number; created_at: string };
export function createSalesMetrics() {
  const products = new Map<string, AnalyticsSummary["product_sales"][number] & { latest: number }>();
  const purchase: AnalyticsSummary["product_purchase"] = {
    eligible_accounts: 0, purchasing_accounts: 0, incomplete_accounts: 0,
    rate_percent: null, revenue_cents: 0, average_purchase_cents: null,
  };
  const modes: AnalyticsSummary["by_stay_mode"] = ["hourly", "overnight", "unclassified"].map(mode => ({
    mode: mode as AnalyticsSummary["by_stay_mode"][number]["mode"], closed_accounts: 0, lodging_cents: 0, average_lodging_cents: null,
  }));
  return {
    account(checkIn: string, since: string | null | undefined, appliedKind: string | null | undefined, lodging: number, lines: ProductLine[]) {
      const mode = modes[appliedKind === "hourly" ? 0 : appliedKind === "night" || appliedKind === "overnight" ? 1 : 2];
      mode.closed_accounts += 1;
      mode.lodging_cents = add(mode.lodging_cents, lodging);
      const eligible = !!since && Date.parse(checkIn) >= Date.parse(since);
      let hasProducts = false, productTotal = 0;
      for (const line of lines) {
        if (line.product_uid == null && line.product_quantity == null) continue;
        if (!line.product_uid || !Number.isSafeInteger(line.product_quantity) || line.product_quantity! <= 0 || line.amount_cents < 0) {
          fail("storage", "Venta de producto sin identificación completa");
        }
        const at = Date.parse(line.created_at);
        if (!Number.isFinite(at)) fail("storage", "Fecha de venta inválida");
        const row = products.get(line.product_uid) ?? {
          product_uid: line.product_uid, description: line.description, units: 0, revenue_cents: 0, latest: at,
        };
        if (at > row.latest || (at === row.latest && line.description < row.description)) {
          row.description = line.description; row.latest = at;
        }
        row.units = add(row.units, line.product_quantity!);
        row.revenue_cents = add(row.revenue_cents, line.amount_cents);
        productTotal = add(productTotal, line.amount_cents);
        products.set(line.product_uid, row);
        hasProducts = true;
      }
      if (eligible) {
        purchase.eligible_accounts += 1;
        if (hasProducts) {
          purchase.purchasing_accounts += 1;
          purchase.revenue_cents = add(purchase.revenue_cents, productTotal);
        }
      } else purchase.incomplete_accounts += 1;
    },
    finish() {
      purchase.rate_percent = purchase.eligible_accounts ? purchase.purchasing_accounts * 100 / purchase.eligible_accounts : null;
      purchase.average_purchase_cents = purchase.purchasing_accounts ? Math.trunc(purchase.revenue_cents / purchase.purchasing_accounts) : null;
      for (const mode of modes) mode.average_lodging_cents = mode.closed_accounts ? Math.trunc(mode.lodging_cents / mode.closed_accounts) : null;
      return {
        product_sales: [...products.values()].map(({ latest: _, ...row }) => row)
          .sort((a, b) => b.units - a.units || (a.product_uid < b.product_uid ? -1 : a.product_uid > b.product_uid ? 1 : 0)),
        product_purchase: purchase,
        by_stay_mode: modes,
      };
    },
  };
}
