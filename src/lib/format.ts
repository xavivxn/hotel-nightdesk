export function formatMoney(amount: number, symbol = "Gs.") {
  const value = Math.round(Number(amount));
  const safe = Number.isFinite(value) ? value : 0;
  const negative = safe < 0;
  const abs = Math.abs(safe);
  const wholeStr = abs.toLocaleString("es-PY");
  return `${negative ? "-" : ""}${wholeStr}\u00a0${symbol}`;
}

export const FIELD_EMPTY = "Completá este campo.";

export function parseGuaranies(value: string | number) {
  if (typeof value === "number") return Math.round(value);
  const normalized = value
    .replace(/Gs\.?/gi, "")
    .replace(/[\s.]/g, "")
    .replace(/,/g, "")
    .trim();
  const parsed = Number.parseFloat(normalized);
  if (Number.isNaN(parsed)) return 0;
  return Math.round(parsed);
}

/** Trim a visible text field. Empty after trim is invalid. */
export function requireTrimmed(value: string): string | null {
  const next = value.trim();
  return next.length > 0 ? next : null;
}

/** Integer for floor, nights, hours, grace, cutoff. Rejects empty, NaN, and out of range. */
export function parseIntegerField(
  value: string | number,
  opts?: { min?: number; max?: number },
): number | null {
  let parsed: number;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) return null;
    parsed = Math.trunc(value);
  } else {
    const trimmed = value.trim().replace(/\s/g, "").replace(",", ".");
    if (!trimmed) return null;
    const next = Number(trimmed);
    if (!Number.isFinite(next)) return null;
    parsed = Math.trunc(next);
  }
  if (opts?.min != null && parsed < opts.min) return null;
  if (opts?.max != null && parsed > opts.max) return null;
  return parsed;
}

/** IVA: 10, 10.5 and 10,5. Finite, 0–100. */
export function parseTaxPercent(value: string | number): number | null {
  if (typeof value === "number") {
    return Number.isFinite(value) && value >= 0 && value <= 100 ? value : null;
  }
  const trimmed = value.trim().replace(/%/g, "").trim();
  if (!trimmed) return null;
  const normalized = trimmed.replace(",", ".");
  const parsed = Number(normalized);
  if (!Number.isFinite(parsed) || parsed < 0 || parsed > 100) return null;
  return parsed;
}

/** Guaraníes as a safe integer. Empty or non-numeric → null (never NaN). */
export function parseMoneyInteger(value: string | number): number | null {
  if (typeof value === "number") {
    if (!Number.isFinite(value)) return null;
    const rounded = Math.round(value);
    return Number.isSafeInteger(rounded) ? rounded : null;
  }
  const normalized = value
    .replace(/Gs\.?/gi, "")
    .replace(/[\s.]/g, "")
    .replace(/,/g, "")
    .trim();
  if (!/^-?\d+$/.test(normalized)) return null;
  const parsed = Number(normalized);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

export function guaraniesToInput(amount: number) {
  return Math.round(amount).toLocaleString("es-PY");
}

export function formatDateTime(rfc: string) {
  const date = new Date(rfc);
  if (Number.isNaN(date.getTime())) return rfc;
  return date.toLocaleString("es-AR", {
    day: "2-digit",
    month: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatDuration(minutes: number) {
  const h = Math.floor(Math.max(0, minutes) / 60);
  const m = Math.max(0, minutes) % 60;
  return `${h}h ${m.toString().padStart(2, "0")}m`;
}

export function toDateTimeLocal(date = new Date()) {
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function localInputToRfc3339(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const pad = (n: number) => n.toString().padStart(2, "0");
  const offset = -date.getTimezoneOffset();
  const sign = offset >= 0 ? "+" : "-";
  const hours = pad(Math.floor(Math.abs(offset) / 60));
  const minutes = pad(Math.abs(offset) % 60);
  return `${value}:00${sign}${hours}:${minutes}`;
}

export function paymentLabel(method?: string | null) {
  switch (method) {
    case "card":
      return "Tarjeta";
    case "transfer":
      return "Transferencia";
    default:
      return "Efectivo";
  }
}

export function statusLabel(status: string) {
  switch (status) {
    case "occupied":
      return "Ocupada";
    case "dirty":
      return "Sucia";
    case "blocked":
      return "Bloqueada";
    case "reserved":
      return "Reservada";
    case "hold":
      return "En espera";
    case "checked_in":
      return "Check-in";
    case "cancelled":
      return "Cancelada";
    case "no_show":
      return "No show";
    case "open":
      return "Abierta";
    case "closed":
      return "Cerrada";
    default:
      return "Libre";
  }
}

export function rateKindLabel(kind: string) {
  switch (kind) {
    case "night":
      return "Por noche";
    case "overnight":
      return "Dormida";
    default:
      return "Por hora";
  }
}
