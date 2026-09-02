export function formatMoney(amount: number, symbol = "Gs.") {
  const negative = amount < 0;
  const abs = Math.abs(Math.round(amount));
  const wholeStr = abs.toLocaleString("es-PY");
  return `${negative ? "-" : ""}${wholeStr}\u00a0${symbol}`;
}

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
  return date.toISOString();
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
      return "Pernocte";
    default:
      return "Por hora";
  }
}
