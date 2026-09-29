import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle, BedDouble, CheckCircle2, ChevronLeft, ChevronRight, ClipboardCopy, Info, ReceiptText,
  RefreshCw, TrendingDown, TrendingUp, Wallet,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import {
  buildAlerts, businessToday, dayLabel, revenueBeforeHour, shareText, shiftDay, shiftTotals, type AlertLevel,
} from "@/lib/daily-summary";
import { formatMoney } from "@/lib/format";
import type {
  AnalyticsSummary, AppSettings, BackupStatus, BoardRoom, DeviceMode, Product, ProductStock, RatePlan,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import "./analytics.css";

type Loaded = {
  report: AnalyticsSummary;
  lastWeek: AnalyticsSummary | null;
  board: BoardRoom[] | null;
  rates: RatePlan[];
  backup: BackupStatus | null;
  stock: ProductStock[];
  products: Product[];
  at: Date;
};

const REFRESH_MS = 5 * 60 * 1000;

/** Settles to `null` so one missing source (offline sync, no stock) never hides the summary. */
function optional<T>(promise: Promise<T>): Promise<T | null> {
  return promise.catch(() => null);
}

const levelStyle: Record<AlertLevel, { border: string; icon: string; Icon: typeof AlertTriangle }> = {
  danger: { border: "border-l-[var(--danger)]", icon: "text-[var(--danger)]", Icon: AlertTriangle },
  warn: { border: "border-l-[var(--warn)]", icon: "text-[var(--warn)]", Icon: AlertTriangle },
  info: { border: "border-l-[var(--line)]", icon: "text-[var(--muted)]", Icon: Info },
};

function Delta({ current, previous, label }: { current: number; previous: number | null; label: string }) {
  if (previous == null) return <small>{label}: sin datos</small>;
  if (previous === 0) return <small>{current ? `${label}: 0 Gs.` : `Igual que ${label.toLowerCase()}`}</small>;
  const pct = Math.round(((current - previous) / Math.abs(previous)) * 100);
  const Icon = pct >= 0 ? TrendingUp : TrendingDown;
  return (
    <small className={cn("inline-flex items-center gap-1", pct >= 0 ? "text-[var(--ok)]" : "text-[var(--danger)]")}>
      <Icon size={13} aria-hidden="true" />
      {pct >= 0 ? "+" : ""}{pct} % {label.toLowerCase()}
    </small>
  );
}

/** Admin landing for the owner: what happened today, and what needs a look now. */
export function SummaryPage({ settings, deviceMode }: { settings: AppSettings; deviceMode: DeviceMode }) {
  const today = businessToday();
  const [day, setDay] = useState(today);
  const [data, setData] = useState<Loaded | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const currency = settings.currency_symbol;
  const money = useCallback((amount: number) => formatMoney(amount, currency), [currency]);
  const isToday = day === today;

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);
    Promise.all([
      api.analyticsSummary(day, day),
      optional(api.analyticsSummary(shiftDay(day, -7), shiftDay(day, -7))),
      optional(api.listBoard()),
      optional(api.listRatePlans(true)),
      optional(api.backupStatus()),
      optional(api.listProductStock()),
      optional(api.listProducts(false)),
    ])
      .then(([report, lastWeek, board, rates, backup, stock, products]) => {
        if (!active) return;
        setData({
          report, lastWeek, board, rates: rates ?? [], backup, stock: stock ?? [], products: products ?? [], at: new Date(),
        });
      })
      .catch((reason) => { if (active) setError(String(reason)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [day, refresh]);

  useEffect(() => {
    if (!isToday) return;
    const id = window.setInterval(() => setRefresh((value) => value + 1), REFRESH_MS);
    return () => window.clearInterval(id);
  }, [isToday]);

  const alerts = useMemo(() => data ? buildAlerts({
    now: data.at, board: data.board, rates: data.rates, report: isToday ? data.report : null,
    backup: data.backup, stock: data.stock, products: data.products, currency,
  }) : [], [data, isToday, currency]);
  const dayAlerts = useMemo(() => data && !isToday ? buildAlerts({
    now: data.at, board: null, rates: [], report: data.report, backup: null, stock: [], products: [], currency,
  }) : [], [data, isToday, currency]);
  const shifts = useMemo(() => data ? shiftTotals(data.report) : [], [data]);
  const maxShift = Math.max(1, ...shifts.map((shift) => shift.revenue));
  const rooms = data?.board?.filter((item) => item.room.active).length ?? null;
  const occupied = data?.board?.filter((item) => item.stay).length ?? null;
  // Today is compared with the same hours a week ago; a closed day with the whole day.
  const compareHour = isToday && data ? Number(new Intl.DateTimeFormat("en-US", { hour: "numeric", hourCycle: "h23", timeZone: "America/Asuncion" }).format(data.at)) + 1 : 24;
  const previousRevenue = data?.lastWeek ? revenueBeforeHour(data.lastWeek, compareHour) : null;
  const previousLabel = isToday ? "vs. hace 7 días a esta hora" : `vs. ${dayLabel(shiftDay(day, -7))}`;

  async function copySummary() {
    if (!data) return;
    const text = shareText({
      business: settings.business_name && settings.business_name !== "MotelApp" ? settings.business_name : "Love Nestt",
      day, report: data.report, occupied: isToday ? occupied : null, rooms: isToday ? rooms : null,
      alerts: isToday ? alerts : dayAlerts, currency,
    });
    try {
      await navigator.clipboard.writeText(text);
      setCopied("Resumen copiado: pegalo en WhatsApp o donde quieras.");
    } catch {
      setCopied("No se pudo copiar automáticamente.");
    }
    window.setTimeout(() => setCopied(null), 4000);
  }

  const shownAlerts = isToday ? alerts : dayAlerts;

  return (
    <div className="analytics-page analytics-page--enter">
      <header className="analytics-header">
        <div>
          <p className="page-kicker">Administración</p>
          <h1 className="page-title">Resumen del día</h1>
          <p className="analytics-intro">Lo que pasó en el día y lo que hay que mirar ahora, sin estar en recepción.</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="flex items-center rounded-lg border border-[var(--line)] bg-[var(--surface)]">
            <button type="button" aria-label="Día anterior" className="grid h-11 w-11 place-items-center text-[var(--muted)] hover:text-[var(--ink)]" onClick={() => setDay(shiftDay(day, -1))}>
              <ChevronLeft size={18} />
            </button>
            <span className="min-w-[132px] text-center text-sm font-semibold capitalize">{isToday ? `Hoy · ${dayLabel(day).split(" ")[1]}` : dayLabel(day)}</span>
            <button type="button" aria-label="Día siguiente" disabled={isToday} className="grid h-11 w-11 place-items-center text-[var(--muted)] hover:text-[var(--ink)] disabled:opacity-30" onClick={() => setDay(shiftDay(day, 1))}>
              <ChevronRight size={18} />
            </button>
          </div>
          {isToday ? null : <Button variant="ghost" onClick={() => setDay(today)}>Hoy</Button>}
          <Button variant="secondary" disabled={loading} onClick={() => setRefresh((value) => value + 1)}>
            <RefreshCw size={16} /> {loading ? "Cargando…" : "Actualizar"}
          </Button>
          <Button disabled={!data} onClick={copySummary}><ClipboardCopy size={16} /> Copiar resumen</Button>
        </div>
      </header>

      {copied ? <div className="analytics-notice" role="status">{copied}</div> : null}
      {error ? <div className="analytics-alert" role="alert">{error}</div> : null}
      {loading && !data ? <div className="analytics-loading" role="status">Armando el resumen…</div> : null}

      {data ? (
        <div className="analytics-results">
          <div className="analytics-context">
            <span className="capitalize">{dayLabel(day)}</span>
            <span>{deviceMode === "remote" ? "Réplica sincronizada de recepción" : "Datos locales de recepción"} · Consultado {data.at.toLocaleTimeString("es-PY", { hour: "2-digit", minute: "2-digit" })}</span>
          </div>

          <div className="analytics-kpis">
            <div className="analytics-kpi analytics-kpi-primary">
              <span className="analytics-kpi-icon"><Wallet size={21} /></span>
              <p>Ingresos de cuentas cerradas</p>
              <strong>{money(data.report.total_revenue_cents)}</strong>
              <Delta current={data.report.total_revenue_cents} previous={previousRevenue} label={previousLabel} />
            </div>
            <div className="analytics-kpi">
              <span className="analytics-kpi-icon"><ReceiptText size={21} /></span>
              <p>Cuentas cerradas</p>
              <strong>{data.report.closed_accounts}</strong>
              <small>{data.report.closed_accounts ? `Ticket promedio ${money(data.report.average_ticket_cents)}` : "Todavía sin cierres"}</small>
            </div>
            <div className="analytics-kpi">
              <span className="analytics-kpi-icon"><TrendingUp size={21} /></span>
              <p>Entradas</p>
              <strong>{data.report.daily.reduce((sum, item) => sum + item.check_ins, 0)}</strong>
              <small>Consumos por {money(data.report.extras_cents)}</small>
            </div>
            <div className="analytics-kpi">
              <span className="analytics-kpi-icon"><BedDouble size={21} /></span>
              <p>Ocupadas ahora</p>
              <strong>{occupied == null || rooms == null ? "—" : `${occupied} de ${rooms}`}</strong>
              <small>{data.board ? `${data.board.filter((item) => item.display_status === "dirty").length} por limpiar` : "Sin datos del tablero"}</small>
            </div>
          </div>

          <section className="analytics-panel">
            <div className="analytics-panel-head">
              <h2>{isToday ? "Para revisar ahora" : "Para revisar de ese día"}</h2>
              <p>{isToday ? "Cuentas largas, dinero que no se cobró, respaldos y stock." : "Descuentos y consumos anulados del día elegido."}</p>
            </div>
            {shownAlerts.length ? (
              <ul className="space-y-2">
                {shownAlerts.map((alert) => {
                  const style = levelStyle[alert.level];
                  return (
                    <li key={alert.key} className={cn("flex items-start gap-3 rounded-lg border border-l-4 border-[var(--line)] bg-[var(--surface)] px-4 py-3", style.border)}>
                      <style.Icon size={18} className={cn("mt-0.5 shrink-0", style.icon)} aria-hidden="true" />
                      <div className="min-w-0">
                        <p className="text-sm font-semibold">{alert.title}</p>
                        <p className="text-xs text-[var(--muted)]">{alert.detail}</p>
                      </div>
                    </li>
                  );
                })}
              </ul>
            ) : (
              <p className="flex items-center gap-2 text-sm text-[var(--ok)]"><CheckCircle2 size={18} /> Todo en orden: nada pendiente para revisar.</p>
            )}
          </section>

          <div className="analytics-lists">
            <section className="analytics-panel">
              <div className="analytics-panel-head">
                <h2>Por franja horaria</h2>
                <p>Entradas por hora de ingreso; cierres e ingresos por hora de salida.</p>
              </div>
              <ol className="analytics-ranking">
                {shifts.map((shift) => (
                  <li key={shift.key}>
                    <div className="analytics-rank-top">
                      <span><b>{shift.label}</b><small className="font-mono">{shift.range} · {shift.checkIns} entradas · {shift.closed} cierres</small></span>
                      <strong>{money(shift.revenue)}</strong>
                    </div>
                    <div className="analytics-rank-track"><span style={{ width: `${shift.revenue ? Math.max(3, (shift.revenue / maxShift) * 100) : 0}%` }} /></div>
                  </li>
                ))}
              </ol>
            </section>
            <section className="analytics-panel">
              <div className="analytics-panel-head">
                <h2>Consumos del día</h2>
                <p>Cargos adicionales en cuentas cerradas, por descripción.</p>
              </div>
              {data.report.top_extras.length ? (
                <ol className="analytics-ranking">
                  {data.report.top_extras.slice(0, 6).map((item) => (
                    <li key={item.description}>
                      <div className="analytics-rank-top">
                        <span><b>{item.description}</b><small>{item.count} {item.count === 1 ? "cargo" : "cargos"}</small></span>
                        <strong>{money(item.revenue_cents)}</strong>
                      </div>
                    </li>
                  ))}
                </ol>
              ) : <p className="analytics-empty">Sin consumos en cuentas cerradas.</p>}
            </section>
          </div>

          <p className="analytics-footnote">
            Los ingresos son importes de cuentas cerradas, no cobros verificados. {deviceMode === "remote" ? "En administración remota, los datos llegan con la sincronización de recepción; el total en curso de cada habitación se ve en recepción." : "Se actualiza solo cada 5 minutos."}
          </p>
        </div>
      ) : null}
    </div>
  );
}
