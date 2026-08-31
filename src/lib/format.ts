export function formatMoney(cents: number, symbol = "$") {
  const negative = cents < 0;
  const abs = Math.abs(Math.round(cents));
  const whole = Math.floor(abs / 100);
  const frac = abs % 100;
  const wholeStr = whole.toLocaleString("es-AR");
  return `${negative ? "-" : ""}${symbol}\u00a0${wholeStr},${frac.toString().padStart(2, "0")}`;
}

export function pesosToCents(value: string | number) {
  if (typeof value === "number") return Math.round(value * 100);
  const normalized = value.replace(/\./g, "").replace(",", ".").trim();
  const parsed = Number.parseFloat(normalized);
  if (Number.isNaN(parsed)) return 0;
  return Math.round(parsed * 100);
}

export function centsToInput(cents: number) {
  return (cents / 100).toFixed(2);
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
