import { useEffect, useMemo, useState, type CSSProperties } from "react";
import {
  CalendarDays, ChartNoAxesCombined, Clock3, Download, RefreshCw, ReceiptText,
  TrendingUp, Wallet,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import { formatMoney } from "@/lib/format";
import type { AnalyticsSummary, DeviceMode } from "@/lib/types";
import "./analytics.css";

type Range = { from: string; to: string; roomType?: string };
type Preset = "today" | "yesterday" | "week" | "month30" | "thisMonth" | "custom";

const pad = (n: number) => String(n).padStart(2, "0");
function businessToday() {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: "America/Asuncion", year: "numeric", month: "2-digit", day: "2-digit",
  }).formatToParts(new Date());
  const get = (type: string) => parts.find(part => part.type === type)?.value ?? "";
  return `${get("year")}-${get("month")}-${get("day")}`;
}
function shiftDay(day: string, offset: number) {
  const date = new Date(`${day}T12:00:00Z`);
  date.setUTCDate(date.getUTCDate() + offset);
  return `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}`;
}
function presetRange(preset: Exclude<Preset, "custom">): Pick<Range, "from" | "to"> {
  const today = businessToday();
  if (preset === "today") return { from: today, to: today };
  if (preset === "yesterday") {
    const yesterday = shiftDay(today, -1);
    return { from: yesterday, to: yesterday };
  }
  if (preset === "week") return { from: shiftDay(today, -6), to: today };
  if (preset === "month30") return { from: shiftDay(today, -29), to: today };
  return { from: `${today.slice(0, 8)}01`, to: today };
}

const money = (amount: number) => formatMoney(amount, "Gs.");
const shortMoney = (amount: number) => {
  const sign = amount < 0 ? "−" : "";
  const absolute = Math.abs(amount);
  return sign + (absolute >= 1_000_000
    ? `${(absolute / 1_000_000).toLocaleString("es-PY", { maximumFractionDigits: 1 })} M`
    : absolute >= 1_000
      ? `${Math.round(absolute / 1_000).toLocaleString("es-PY")} mil`
      : String(absolute));
};
const statusMeta: Record<string, { label: string; color: string }> = {
  available: { label: "Libres", color: "#247d66" },
  occupied: { label: "Ocupadas", color: "#b84b31" },
  dirty: { label: "Por limpiar", color: "#7850a5" },
  reserved: { label: "Reservadas", color: "#315fbd" },
  blocked: { label: "Bloqueadas", color: "#a32235" },
};

function RevenueChart({ days }: { days: AnalyticsSummary["daily"] }) {
  const max = Math.max(0, ...days.map(day => day.revenue_cents));
  const min = Math.min(0, ...days.map(day => day.revenue_cents));
  const span = Math.max(1, max - min);
  const yFor = (amount: number) => 224 - (amount - min) * 174 / span;
  const zeroY = yFor(0);
  const points = days.map((day, index) => ({
    x: days.length === 1 ? 390 : 56 + index * 680 / (days.length - 1),
    y: yFor(day.revenue_cents),
    day,
  }));
  const path = points.map((point, index) => `${index ? "L" : "M"}${point.x.toFixed(1)} ${point.y.toFixed(1)}`).join(" ");
  const area = points.length ? `${path} L${points.at(-1)!.x.toFixed(1)} ${zeroY.toFixed(1)} L${points[0].x.toFixed(1)} ${zeroY.toFixed(1)} Z` : "";
  const labelEvery = Math.max(1, Math.ceil(days.length / 6));
  return (
    <div className="analytics-chart-wrap">
      <svg viewBox="0 0 790 270" role="img" aria-label="Evolución diaria de importes de cuentas cerradas" className="analytics-chart">
        <defs>
          <linearGradient id="analytics-revenue-fill" x1="0" x2="0" y1="0" y2="1">
            <stop offset="0%" stopColor="#a72b48" stopOpacity=".27" />
            <stop offset="100%" stopColor="#a72b48" stopOpacity="0" />
          </linearGradient>
        </defs>
        {[max, (max + min) / 2, min].map((value, index) => {
          const y = yFor(value);
          return <g key={index}>
            <line x1="56" x2="738" y1={y} y2={y} stroke="var(--line)" strokeDasharray={value !== 0 ? "4 6" : undefined} />
            <text x="45" y={y + 4} textAnchor="end" className="analytics-chart-label">{shortMoney(Math.round(value))}</text>
          </g>;
        })}
        {min < 0 && max > 0 ? <line x1="56" x2="738" y1={zeroY} y2={zeroY} stroke="var(--accent)" strokeOpacity=".55" strokeDasharray="4 5" /> : null}
        {points.length ? <path className="analytics-chart-area" d={area} fill="url(#analytics-revenue-fill)" /> : null}
        {points.length ? <path className="analytics-chart-line" d={path} fill="none" stroke="var(--accent)" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" /> : null}
        {points.filter((_, index) => index % labelEvery === 0 || index === points.length - 1).map((point, index) => (
          <g key={point.day.date} className="analytics-chart-dot" style={{ "--enter-delay": `${520 + index * 40}ms` } as CSSProperties}>
            <circle cx={point.x} cy={point.y} r="4.5" fill="var(--accent)" stroke="var(--surface)" strokeWidth="2" />
            <text x={point.x} y="249" textAnchor="middle" className="analytics-chart-label">{point.day.date.slice(5)}</text>
          </g>
        ))}
      </svg>
    </div>
  );
}

function RoomStatus({ items }: { items: AnalyticsSummary["current_rooms"] }) {
  const total = items.reduce((sum, item) => sum + item.count, 0);
  let offset = 0;
  const slices = items.filter(item => item.count > 0).map(item => {
    const begin = offset;
    offset += total ? item.count / total * 100 : 0;
    return `${statusMeta[item.status]?.color ?? "#8d7775"} ${begin}% ${offset}%`;
  });
  return (
    <div className="analytics-status-layout">
      <div className="analytics-donut" role="img" aria-label={`Estado actual de ${total} habitaciones`} style={{ background: slices.length ? `conic-gradient(${slices.join(", ")})` : "var(--surface-2)" }}>
        <div className="analytics-donut-hole"><strong>{total}</strong><span>habitaciones</span></div>
      </div>
      <ul className="analytics-legend">
        {items.map(item => <li key={item.status}>
          <span className="analytics-legend-dot" style={{ background: statusMeta[item.status]?.color ?? "#8d7775" }} />
          <span>{statusMeta[item.status]?.label ?? item.status}</span>
          <strong>{item.count}</strong>
        </li>)}
      </ul>
    </div>
  );
}

function Ranking({ title, subtitle, rows, empty }: {
  title: string; subtitle: string;
  rows: { label: string; detail: string; amount: number }[]; empty: string;
}) {
  const max = Math.max(1, ...rows.map(row => row.amount));
  return <section className="analytics-panel">
    <div className="analytics-panel-head"><h2>{title}</h2><p>{subtitle}</p></div>
    {rows.length ? <ol className="analytics-ranking">{rows.map((row, index) =>
      <li key={`${row.label}-${index}`}>
        <div className="analytics-rank-top"><span><b>{row.label}</b><small>{row.detail}</small></span><strong>{money(row.amount)}</strong></div>
        <div className="analytics-rank-track"><span style={{ width: `${Math.max(3, row.amount / max * 100)}%` }} /></div>
      </li>,
    )}</ol> : <p className="analytics-empty">{empty}</p>}
  </section>;
}

function ActivityHours({ hours }: { hours: AnalyticsSummary["check_in_hours"] }) {
  const max = Math.max(1, ...hours.map(item => item.count));
  const total = hours.reduce((sum, item) => sum + item.count, 0);
  const peak = [...hours].sort((a, b) => b.count - a.count || a.hour - b.hour)[0];
  return <section className="analytics-panel">
    <div className="analytics-panel-head"><h2>Horas de más actividad</h2><p>Entradas registradas por hora en el período.</p></div>
    <div className="analytics-hours" role="img" aria-label={`Distribución de ${total} entradas por hora`}>
      {hours.map(item => <div key={item.hour} className="analytics-hour" title={`${pad(item.hour)}:00 · ${item.count} entradas`}>
        <span className="analytics-hour-value">{item.count || ""}</span>
        <span className="analytics-hour-track"><span style={{ height: `${item.count ? Math.max(6, item.count / max * 100) : 0}%` }} /></span>
        <span className="analytics-hour-label">{item.hour % 4 === 0 ? pad(item.hour) : ""}</span>
      </div>)}
    </div>
    <p className="analytics-mini-note">{total ? `Mayor movimiento: ${pad(peak.hour)}:00 a ${pad(peak.hour)}:59 · ${peak.count} entradas.` : "Todavía no hay entradas para el período."}</p>
  </section>;
}

function ReservationFlow({ report }: { report: AnalyticsSummary }) {
  const rows = [
    { label: "Llegadas previstas", value: report.reservation_arrivals, color: "var(--accent)" },
    { label: "Canceladas", value: report.reservation_cancellations, color: "var(--danger)" },
    { label: "No se presentaron", value: report.no_shows, color: "var(--gold)" },
  ];
  const max = Math.max(1, report.reservation_arrivals);
  return <section className="analytics-panel">
    <div className="analytics-panel-head"><h2>Movimiento de reservas</h2><p>Según la fecha de llegada prevista, no la fecha en que se canceló.</p></div>
    <div className="analytics-reservation-rows">{rows.map(row => <div key={row.label}>
      <div className="analytics-rank-top"><span><b>{row.label}</b></span><strong>{row.value}</strong></div>
      <div className="analytics-rank-track"><span style={{ width: `${row.value ? Math.max(3, row.value / max * 100) : 0}%`, background: row.color }} /></div>
    </div>)}</div>
    <p className="analytics-mini-note">Las reservas confirmadas o ya utilizadas siguen incluidas en «llegadas previstas».</p>
  </section>;
}

export function AnalyticsPage({ deviceMode }: { deviceMode: DeviceMode }) {
  const [initial] = useState(() => presetRange("week"));
  const [draft, setDraft] = useState<Range>(initial);
  const [query, setQuery] = useState<Range>(initial);
  const [preset, setPreset] = useState<Preset>("week");
  const [refresh, setRefresh] = useState(0);
  const [report, setReport] = useState<AnalyticsSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const today = businessToday();

  useEffect(() => {
    let active = true;
    setLoading(true); setReport(null); setError(null); setNotice(null);
    api.analyticsSummary(query.from, query.to, query.roomType)
      .then(data => { if (active) setReport(data); })
      .catch(reason => { if (active) setError(String(reason)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [query, refresh]);

  function apply(next: Range = draft) {
    if (!next.from || !next.to || next.from > next.to || next.to > today ||
        (Date.parse(next.to) - Date.parse(next.from)) / 86400000 >= 366) {
      setError("Elegí un rango válido de hasta 366 días, sin fechas futuras.");
      return;
    }
    setError(null); setDraft(next);
    if (query.from === next.from && query.to === next.to && query.roomType === next.roomType) setRefresh(value => value + 1);
    else setQuery(next);
  }

  const roomTypeLabel = query.roomType ?? "Todas";
  const statusTotal = report?.current_rooms.reduce((sum, item) => sum + item.count, 0) ?? 0;
  const dailyTotal = report?.daily.reduce((sum, day) => sum + day.revenue_cents, 0) ?? 0;
  const rooms = useMemo(() => report?.by_room.slice(0, 8).map(row => ({
    label: `Hab. ${row.room_number}`, detail: `${row.room_type} · ${row.closed_accounts} cuentas`, amount: row.revenue_cents,
  })) ?? [], [report]);
  const extras = useMemo(() => report?.top_extras.slice(0, 8).map(row => ({
    label: row.description, detail: `${row.count} ${row.count === 1 ? "cargo" : "cargos"}`, amount: row.revenue_cents,
  })) ?? [], [report]);

  return <div className="analytics-page analytics-page--enter">
    <header className="analytics-header">
      <div><p className="page-kicker">Administración / Información del negocio</p><h1 className="page-title">Análisis</h1>
        <p className="analytics-intro">Un vistazo claro a las cuentas cerradas y al movimiento de las habitaciones.</p>
      </div>
      <div className="analytics-header-mark" aria-hidden="true"><ChartNoAxesCombined size={28} /></div>
    </header>

    <section className="analytics-filters" aria-label="Filtros del análisis">
      <div className="analytics-presets" role="group" aria-label="Período rápido">
        {([["today", "Hoy"], ["yesterday", "Ayer"], ["week", "7 días"], ["month30", "30 días"], ["thisMonth", "Este mes"]] as const).map(([key, label]) =>
          <button key={key} type="button" className={preset === key ? "analytics-preset is-active" : "analytics-preset"} aria-pressed={preset === key}
            onClick={() => { const next = { ...presetRange(key), roomType: draft.roomType }; setPreset(key); apply(next); }}>{label}</button>,
        )}
      </div>
      <div className="analytics-filter-row">
        <label>Desde<input type="date" value={draft.from} max={today} onChange={event => { setDraft({ ...draft, from: event.target.value }); setPreset("custom"); }} /></label>
        <label>Hasta<input type="date" value={draft.to} max={today} onChange={event => { setDraft({ ...draft, to: event.target.value }); setPreset("custom"); }} /></label>
        <label>Habitaciones<select value={draft.roomType ?? "all"} onChange={event => setDraft({ ...draft, roomType: event.target.value === "all" ? undefined : event.target.value })}>
          <option value="all">Todas</option><option value="Normal">Normales</option><option value="Jacuzzi">Jacuzzi</option>
        </select></label>
        <Button onClick={() => apply()} disabled={loading}><RefreshCw size={16} /> {loading ? "Cargando…" : "Actualizar"}</Button>
        <Button variant="secondary" disabled={!report || loading || exporting} onClick={async () => {
          setExporting(true); setError(null); setNotice(null);
          try { const path = await api.exportAnalyticsPdf(query.from, query.to, query.roomType); setNotice(`PDF generado: ${path}`); }
          catch (reason) { setError(String(reason)); }
          finally { setExporting(false); }
        }}><Download size={16} /> {exporting ? "Generando…" : "Exportar PDF"}</Button>
      </div>
    </section>

    {error ? <div className="analytics-alert" role="alert">{error}</div> : null}
    {notice ? <div className="analytics-notice" role="status">{notice}</div> : null}
    {loading ? <div className="analytics-loading" role="status">Preparando los indicadores…</div> : null}

    {report ? <div key={`${report.from}-${report.to}-${query.roomType ?? "all"}-${refresh}`} className="analytics-results analytics-results--enter">
      <div className="analytics-context"><span><CalendarDays size={15} /> {report.from} al {report.to} · {roomTypeLabel}</span>
        <span>{deviceMode === "remote" ? "Réplica sincronizada de recepción" : "Datos locales de recepción"} · Consultado {new Date(report.generated_at).toLocaleString("es-PY")}</span></div>
      <div className="analytics-kpis">
        <div className="analytics-kpi analytics-kpi-primary"><span className="analytics-kpi-icon"><Wallet size={21} /></span><p>Ingresos de cuentas cerradas</p><strong>{money(report.total_revenue_cents)}</strong><small>Importes históricos del período</small></div>
        <div className="analytics-kpi"><span className="analytics-kpi-icon"><ReceiptText size={21} /></span><p>Cuentas cerradas</p><strong>{report.closed_accounts}</strong><small>Salidas registradas</small></div>
        <div className="analytics-kpi"><span className="analytics-kpi-icon"><TrendingUp size={21} /></span><p>Ticket promedio</p><strong>{money(report.average_ticket_cents)}</strong><small>Por cuenta cerrada</small></div>
        <div className="analytics-kpi"><span className="analytics-kpi-icon"><Clock3 size={21} /></span><p>Estadía promedio</p><strong>{report.average_stay_minutes === null ? "—" : `${Math.floor(report.average_stay_minutes / 60)} h ${pad(report.average_stay_minutes % 60)} min`}</strong><small>De cuentas cerradas</small></div>
      </div>

      <div className="analytics-main-grid">
        <section className="analytics-panel analytics-trend"><div className="analytics-panel-head"><h2>Ingresos día a día</h2><p>Total al cierre de cada cuenta · {money(dailyTotal)} en el período</p></div><RevenueChart days={report.daily} /></section>
        <section className="analytics-panel"><div className="analytics-panel-head"><h2>Habitaciones ahora</h2><p>Estado actual · {statusTotal} habitaciones en el filtro</p></div><RoomStatus items={report.current_rooms} /></section>
      </div>

      <section className="analytics-panel analytics-breakdown"><div className="analytics-panel-head"><h2>Composición de las cuentas</h2><p>El total incluye cargos adicionales, descuentos e impuestos aplicados al cierre.</p></div>
        <div className="analytics-breakdown-grid">
          <div><span>Alojamiento y horas extra</span><strong>{money(report.lodging_cents)}</strong></div>
          <div><span>Cargos adicionales</span><strong>{money(report.extras_cents)}</strong></div>
          <div><span>Descuentos</span><strong>{money(report.discount_cents)}</strong></div>
          <div><span>Impuestos</span><strong>{money(report.tax_cents)}</strong></div>
        </div>
      </section>

      <div className="analytics-lists analytics-activity-grid">
        <ActivityHours hours={report.check_in_hours} />
        <ReservationFlow report={report} />
      </div>

      <div className="analytics-lists">
        <Ranking title="Habitaciones con más ingresos" subtitle="Cuentas cerradas por habitación en el período" rows={rooms} empty="Aún no hay cuentas cerradas para este filtro." />
        <Ranking title="Cargos adicionales destacados" subtitle="Agrupados por descripción; incluye consumos y recargos manuales" rows={extras} empty="No se registraron cargos adicionales en cuentas cerradas." />
      </div>
      {report.by_room_type.length > 1 ? <section className="analytics-panel analytics-types"><div className="analytics-panel-head"><h2>Por tipo de habitación</h2><p>El tipo corresponde a la ficha actual de cada habitación.</p></div>
        <div className="analytics-type-grid">{report.by_room_type.map(type => <div key={type.room_type}><span>{type.room_type}</span><strong>{money(type.revenue_cents)}</strong><small>{type.closed_accounts} cuentas</small></div>)}</div>
      </section> : null}
      <section className="analytics-panel analytics-daily"><div className="analytics-panel-head"><h2>Detalle diario</h2><p>Entradas, cierres e ingresos registrados por día.</p></div>
        <div className="analytics-table-scroll"><table><thead><tr><th>Fecha</th><th>Entradas</th><th>Cuentas cerradas</th><th>Llegadas previstas</th><th>Canceladas</th><th>No se presentaron</th><th>Ingresos</th></tr></thead><tbody>
          {report.daily.map(day => <tr key={day.date}><td>{day.date}</td><td>{day.check_ins}</td><td>{day.closed_accounts}</td><td>{day.reservation_arrivals}</td><td>{day.reservation_cancellations}</td><td>{day.no_shows}</td><td>{money(day.revenue_cents)}</td></tr>)}
        </tbody></table></div>
      </section>
      <p className="analytics-footnote">Los ingresos muestran importes de cuentas cerradas, no cobros verificados. Los cargos adicionales pueden ser consumos o ajustes manuales. En administración remota, los datos dependen de la última sincronización.</p>
    </div> : !loading && !error ? <div className="analytics-loading">No hay datos disponibles para este período.</div> : null}
  </div>;
}
