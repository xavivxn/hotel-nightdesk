import { Button } from "@/components/ui/Button";
import { Drawer } from "@/components/ui/Drawer";
import { Field, Input, Select } from "@/components/ui/Field";
import { api, peekDeviceMode } from "@/lib/api";
import { formatMoney, guaraniesToInput, parseMoneyInteger } from "@/lib/format";
import { WEEKDAYS, hourLabel, ruleSchedule, shortDate } from "@/lib/price-rules";
import type { PriceRule, RatePlan } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Plus, Tag, X } from "lucide-react";
import { useEffect, useState } from "react";

type Draft = {
  id: string;
  name: string;
  rate_plan_id: number;
  when: "days" | "dates";
  days: number[];
  dates: string[];
  newDate: string;
  from_hour: number;
  to_hour: number;
  price: string;
  extra: string;
  active: boolean;
};

const HOURS = Array.from({ length: 25 }, (_, hour) => hour);

function draftFrom(rule: PriceRule | null, rates: RatePlan[]): Draft {
  const plan = rates.find((rate) => rate.id === rule?.rate_plan_id) ?? rates.find((rate) => rate.active) ?? rates[0];
  return {
    id: rule?.id ?? "",
    name: rule?.name ?? "",
    rate_plan_id: rule?.rate_plan_id ?? plan?.id ?? 0,
    when: rule?.dates.length ? "dates" : "days",
    days: rule?.days ?? [1, 2, 3, 4],
    dates: rule?.dates ?? [],
    newDate: "",
    from_hour: rule?.from_hour ?? 14,
    to_hour: rule?.to_hour ?? 18,
    price: rule ? guaraniesToInput(rule.base_amount_cents) : "",
    extra: rule?.extra_hour_cents != null ? guaraniesToInput(rule.extra_hour_cents) : "",
    active: rule?.active ?? true,
  };
}

/** Promotions and special prices by day/date and check-in hour, in "Habitaciones y tarifas". */
export function PriceRulesSection({ rates, currency }: { rates: RatePlan[]; currency: string }) {
  const [rules, setRules] = useState<PriceRule[]>([]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const remote = peekDeviceMode() === "remote";

  useEffect(() => {
    api.listPriceRules().then(setRules).catch((e) => setListError(String(e)));
  }, []);

  function open(rule: PriceRule | null) {
    setError(null);
    setDraft(draftFrom(rule, rates));
  }

  async function persist(next: PriceRule[]) {
    setBusy(true);
    setError(null);
    try {
      setRules(await api.savePriceRules(next));
      setDraft(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  function save() {
    if (!draft) return;
    const name = draft.name.trim();
    if (!name) return setError("Poné un nombre, por ejemplo «Promo tarde».");
    const price = parseMoneyInteger(draft.price);
    if (price == null || price <= 0) return setError("Ingresá el precio en guaraníes.");
    const extra = draft.extra.trim() ? parseMoneyInteger(draft.extra) : null;
    if (draft.extra.trim() && (extra == null || extra < 0)) return setError("El adicional tiene que ser un monto en guaraníes.");
    if (draft.when === "days" && !draft.days.length) return setError("Elegí al menos un día.");
    if (draft.when === "dates" && !draft.dates.length) return setError("Agregá al menos una fecha.");
    if (draft.from_hour >= draft.to_hour) return setError("«Desde» tiene que ser antes que «hasta».");
    const rule: PriceRule = {
      id: draft.id,
      name,
      rate_plan_id: draft.rate_plan_id,
      days: draft.when === "days" ? draft.days : [],
      dates: draft.when === "dates" ? draft.dates : [],
      from_hour: draft.from_hour,
      to_hour: draft.to_hour,
      base_amount_cents: price,
      extra_hour_cents: extra,
      active: draft.active,
    };
    const editing = draft.id !== "" && rules.some((item) => item.id === draft.id);
    void persist(editing ? rules.map((item) => (item.id === draft.id ? rule : item)) : [...rules, rule]);
  }

  function toggleDay(day: number) {
    if (!draft) return;
    const days = draft.days.includes(day) ? draft.days.filter((d) => d !== day) : [...draft.days, day].sort((a, b) => a - b);
    setDraft({ ...draft, days });
  }

  function addDate() {
    if (!draft || !/^\d{4}-\d{2}-\d{2}$/.test(draft.newDate) || draft.dates.includes(draft.newDate)) return;
    setDraft({ ...draft, dates: [...draft.dates, draft.newDate].sort(), newDate: "" });
  }

  const planName = (id: number) => rates.find((rate) => rate.id === id)?.name ?? "Tarifa borrada";
  const planPrice = (id: number) => rates.find((rate) => rate.id === id)?.base_amount_cents;

  return (
    <section className="card mt-6 rounded-lg p-4">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="flex items-center gap-2 text-lg font-semibold tracking-tight">
            <Tag size={17} className="text-[var(--accent)]" /> Promociones y precios especiales
          </h2>
          <p className="text-xs text-[var(--muted)]">
            Se aplican según el día y la hora del ingreso. Una cuenta abierta conserva el precio con el que entró.
          </p>
        </div>
        {remote ? null : (
          <Button size="sm" variant="secondary" onClick={() => open(null)} disabled={!rates.length}>
            <Plus size={15} /> Nueva promoción
          </Button>
        )}
      </div>
      {listError ? <p className="text-sm text-[var(--danger)]">{listError}</p> : null}
      {remote ? (
        <p className="text-sm text-[var(--muted)]">Las promociones se configuran en la PC de recepción.</p>
      ) : rules.length === 0 ? (
        <p className="rounded-lg bg-[var(--surface-2)] px-3 py-4 text-sm text-[var(--muted)]">
          Sin promociones: cada tarifa cobra siempre su precio. Ejemplos: tarde de lunes a jueves, fin de semana, San Valentín.
        </p>
      ) : (
        <div className="grid gap-2 lg:grid-cols-2">
          {rules.map((rule) => {
            const normal = planPrice(rule.rate_plan_id);
            return (
              <button
                key={rule.id}
                type="button"
                onClick={() => open(rule)}
                className={cn(
                  "flex min-h-11 w-full items-center justify-between gap-3 rounded-lg bg-[var(--surface-2)] px-3 py-3 text-left hover:bg-[var(--bg-2)]",
                  !rule.active && "opacity-60",
                )}
              >
                <div className="min-w-0">
                  <p className="truncate font-semibold">
                    {rule.name}
                    {rule.active ? null : <span className="ml-2 text-xs font-normal text-[var(--muted)]">Pausada</span>}
                  </p>
                  <p className="truncate text-xs text-[var(--muted)]">
                    {planName(rule.rate_plan_id)} · {ruleSchedule(rule)}
                  </p>
                </div>
                <div className="shrink-0 text-right font-mono tabular-nums">
                  <p className="text-sm font-semibold">{formatMoney(rule.base_amount_cents, currency)}</p>
                  {normal != null && normal !== rule.base_amount_cents ? (
                    <p className="text-xs text-[var(--muted)] line-through">{formatMoney(normal, currency)}</p>
                  ) : null}
                </div>
              </button>
            );
          })}
        </div>
      )}

      <Drawer
        open={Boolean(draft)}
        title={draft?.id ? "Editar promoción" : "Nueva promoción"}
        subtitle="El precio se elige por la hora de ingreso."
        onClose={() => !busy && setDraft(null)}
      >
        {draft ? (
          <form
            className="space-y-4"
            onSubmit={(e) => {
              e.preventDefault();
              save();
            }}
          >
            <Field label="Nombre">
              <Input autoFocus maxLength={40} value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} placeholder="Promo tarde" />
            </Field>
            <Field label="Tarifa">
              <Select value={draft.rate_plan_id} onChange={(e) => setDraft({ ...draft, rate_plan_id: Number(e.target.value) })}>
                {rates.map((rate) => (
                  <option key={rate.id} value={rate.id}>
                    {rate.name} · {formatMoney(rate.base_amount_cents, currency)}{rate.active ? "" : " (inactiva)"}
                  </option>
                ))}
              </Select>
            </Field>
            <div className="grid grid-cols-2 gap-1 rounded-lg bg-[var(--surface-2)] p-1" role="radiogroup" aria-label="Cuándo se aplica">
              {([
                ["days", "Días de la semana"],
                ["dates", "Fechas especiales"],
              ] as const).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  role="radio"
                  aria-checked={draft.when === value}
                  onClick={() => setDraft({ ...draft, when: value })}
                  className={cn(
                    "min-h-11 rounded-md px-3 text-sm font-semibold",
                    draft.when === value ? "bg-[var(--surface)] text-[var(--ink)]" : "text-[var(--muted)] hover:text-[var(--ink)]",
                  )}
                >
                  {label}
                </button>
              ))}
            </div>
            {draft.when === "days" ? (
              <div className="grid grid-cols-7 gap-1" role="group" aria-label="Días">
                {WEEKDAYS.map((day) => {
                  const on = draft.days.includes(day.id);
                  return (
                    <button
                      key={day.id}
                      type="button"
                      aria-pressed={on}
                      title={day.label}
                      onClick={() => toggleDay(day.id)}
                      className={cn(
                        "min-h-11 rounded-lg border text-sm font-semibold",
                        on ? "border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--accent)]" : "border-[var(--line)] text-[var(--muted)] hover:bg-[var(--surface-2)]",
                      )}
                    >
                      {day.short}
                    </button>
                  );
                })}
              </div>
            ) : (
              <div className="space-y-2">
                <div className="flex gap-2">
                  <Input type="date" value={draft.newDate} onChange={(e) => setDraft({ ...draft, newDate: e.target.value })} aria-label="Fecha especial" />
                  <Button type="button" variant="secondary" onClick={addDate} disabled={!draft.newDate}>Agregar</Button>
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {draft.dates.map((date) => (
                    <span key={date} className="inline-flex items-center gap-1 rounded-lg bg-[var(--surface-2)] py-1 pl-2.5 pr-1 font-mono text-xs tabular-nums">
                      {shortDate(date)}
                      <button
                        type="button"
                        aria-label={`Quitar ${shortDate(date)}`}
                        className="grid h-6 w-6 place-items-center rounded-md text-[var(--muted)] hover:text-[var(--danger)]"
                        onClick={() => setDraft({ ...draft, dates: draft.dates.filter((d) => d !== date) })}
                      >
                        <X size={13} />
                      </button>
                    </span>
                  ))}
                  {draft.dates.length ? null : <span className="text-xs text-[var(--muted)]">Ej. 14/02 San Valentín. En esa fecha gana sobre las promos por día.</span>}
                </div>
              </div>
            )}
            <div className="grid grid-cols-2 gap-3">
              <Field label="Ingreso desde">
                <Select value={draft.from_hour} onChange={(e) => setDraft({ ...draft, from_hour: Number(e.target.value) })}>
                  {HOURS.slice(0, 24).map((hour) => <option key={hour} value={hour}>{hourLabel(hour)}</option>)}
                </Select>
              </Field>
              <Field label="Hasta">
                <Select value={draft.to_hour} onChange={(e) => setDraft({ ...draft, to_hour: Number(e.target.value) })}>
                  {HOURS.slice(1).map((hour) => <option key={hour} value={hour}>{hour === 24 ? "24:00 (fin del día)" : hourLabel(hour)}</option>)}
                </Select>
              </Field>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <Field label="Precio (Gs.)">
                <Input inputMode="numeric" value={draft.price} onChange={(e) => setDraft({ ...draft, price: e.target.value })} placeholder="35.000" />
              </Field>
              <Field label="Adicional 30 min (Gs.)">
                <Input inputMode="numeric" value={draft.extra} onChange={(e) => setDraft({ ...draft, extra: e.target.value })} placeholder="Igual que la tarifa" />
              </Field>
            </div>
            <label className="flex min-h-11 items-center gap-2 text-sm">
              <input type="checkbox" checked={draft.active} onChange={(e) => setDraft({ ...draft, active: e.target.checked })} />
              Activa
            </label>
            {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
            <div className="flex items-center justify-between gap-2">
              {draft.id ? (
                <Button type="button" variant="danger" size="sm" disabled={busy} onClick={() => void persist(rules.filter((item) => item.id !== draft.id))}>
                  Eliminar
                </Button>
              ) : <span />}
              <Button type="submit" disabled={busy}>{busy ? "Guardando…" : "Guardar promoción"}</Button>
            </div>
          </form>
        ) : null}
      </Drawer>
    </section>
  );
}
