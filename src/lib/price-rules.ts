/** Labels and a browser-mock mirror of `src-tauri/src/pricing.rs` (Rust is the authority). */
import type { PriceRule, RatePlan } from "./types";

export const WEEKDAYS = [
  { id: 1, short: "Lu", label: "lunes" },
  { id: 2, short: "Ma", label: "martes" },
  { id: 3, short: "Mi", label: "miércoles" },
  { id: 4, short: "Ju", label: "jueves" },
  { id: 5, short: "Vi", label: "viernes" },
  { id: 6, short: "Sá", label: "sábado" },
  { id: 7, short: "Do", label: "domingo" },
] as const;

const pad = (n: number) => String(n).padStart(2, "0");

export function isoDate(date: Date) {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function shortDate(iso: string) {
  const [y, m, d] = iso.split("-");
  return `${d}/${m}/${y}`;
}

export function hourLabel(hour: number) {
  return `${pad(hour)}:00`;
}

function daysLabel(days: number[]) {
  const key = [...days].sort((a, b) => a - b).join(",");
  if (key === "1,2,3,4,5,6,7") return "Todos los días";
  if (key === "1,2,3,4,5") return "Lunes a viernes";
  if (key === "1,2,3,4") return "Lunes a jueves";
  if (key === "5,6") return "Viernes y sábado";
  if (key === "6,7") return "Fin de semana";
  return [...days].sort((a, b) => a - b).map((day) => WEEKDAYS[day - 1]?.short ?? "?").join(" ");
}

export function ruleSchedule(rule: PriceRule) {
  const when = rule.dates.length ? rule.dates.map(shortDate).join(", ") : daysLabel(rule.days);
  const hours = rule.from_hour === 0 && rule.to_hour === 24
    ? "todo el día"
    : `de ${hourLabel(rule.from_hour)} a ${hourLabel(rule.to_hour % 24)}`;
  return `${when} · ${hours}`;
}

/** Mirror of `pricing::matching`: a date rule wins over weekday rules; hours are [from, to). */
export function matchingRule(rules: PriceRule[], planId: number, at: Date) {
  const hour = at.getHours();
  const date = isoDate(at);
  const weekday = ((at.getDay() + 6) % 7) + 1;
  const fits = (rule: PriceRule) =>
    rule.active && rule.rate_plan_id === planId && rule.from_hour <= hour && hour < rule.to_hour;
  return (
    rules.find((rule) => fits(rule) && rule.dates.includes(date)) ??
    rules.find((rule) => fits(rule) && rule.dates.length === 0 && rule.days.includes(weekday))
  );
}

/** Mirror of `pricing::validate` for the mock. Returns the first problem or null. */
export function validateRules(rules: PriceRule[], plans: RatePlan[]): string | null {
  if (rules.length > 50) return "Se pueden guardar hasta 50 promociones";
  const ids = new Set<string>();
  for (const rule of rules) {
    const name = rule.name;
    if (ids.has(rule.id)) return "Hay una promoción repetida";
    ids.add(rule.id);
    if (!name || name.length > 40) return "Poné un nombre de hasta 40 caracteres a cada promoción";
    if (!plans.some((plan) => plan.id === rule.rate_plan_id)) return `«${name}»: la tarifa ya no existe`;
    if (rule.from_hour > 23 || rule.to_hour > 24 || rule.from_hour >= rule.to_hour) {
      return `«${name}»: el horario va de 0 a 24 y «desde» tiene que ser menor que «hasta»`;
    }
    if (!Number.isInteger(rule.base_amount_cents) || rule.base_amount_cents <= 0) return `«${name}»: el precio tiene que ser mayor que 0`;
    if (rule.extra_hour_cents != null && rule.extra_hour_cents < 0) return `«${name}»: el adicional no puede ser negativo`;
    if (!rule.days.length && !rule.dates.length) return `«${name}»: elegí al menos un día o una fecha`;
    if (rule.days.length && rule.dates.length) return `«${name}»: usá días de la semana o fechas, no las dos cosas`;
    if (rule.days.some((day) => day < 1 || day > 7)) return `«${name}»: día de la semana inválido`;
    const bad = rule.dates.find((date) => !/^\d{4}-\d{2}-\d{2}$/.test(date) || isoDate(new Date(`${date}T12:00:00`)) !== date);
    if (bad) return `«${name}»: la fecha «${bad}» no es válida`;
  }
  const active = rules.filter((rule) => rule.active);
  for (let i = 0; i < active.length; i++) {
    for (const b of active.slice(i + 1)) {
      const a = active[i];
      if (a.rate_plan_id !== b.rate_plan_id || a.from_hour >= b.to_hour || b.from_hour >= a.to_hour) continue;
      let when: string | null = null;
      if (a.days.length && b.days.length) {
        const day = a.days.find((d) => b.days.includes(d));
        if (day) when = `el ${WEEKDAYS[day - 1].label}`;
      } else if (a.dates.length && b.dates.length) {
        const date = a.dates.find((d) => b.dates.includes(d));
        if (date) when = `el ${shortDate(date)}`;
      }
      if (when) {
        return `«${a.name}» y «${b.name}» se superponen ${when} entre las ${Math.max(a.from_hour, b.from_hour)}:00 y las ${Math.min(a.to_hour, b.to_hour)}:00`;
      }
    }
  }
  return null;
}
