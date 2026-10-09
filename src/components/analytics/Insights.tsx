import { useState, type CSSProperties } from "react";
import { Button } from "@/components/ui/Button";
import { formatMoney } from "@/lib/format";
import { analyticsTime } from "@/lib/analytics-metrics";
import type { AnalyticsSummary } from "@/lib/types";

const money = (value: number) => formatMoney(value, "Gs.");
const decimal = (value: number) => value.toLocaleString("es-PY", { maximumFractionDigits: 1 });
export const modeLabel = { hourly: "Por hora", overnight: "Noche / dormida", unclassified: "Sin clasificar" };
export const weekdays = ["Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo"];
const hourLabel = (hour: number) => `${String(hour).padStart(2, "0")}:00`;

function ProductCoverage({ report }: { report: AnalyticsSummary }) {
  const since = report.product_tracking_since;
  return <p className="analytics-mini-note">
    {since ? `Registro de productos desde ${analyticsTime(since).date}. ` : "La cobertura de productos todavía no está disponible. "}
    {report.product_purchase.incomplete_accounts > 0
      ? `${report.product_purchase.incomplete_accounts} cuentas tienen cobertura incompleta; los consumos históricos sin identificar no se estiman.`
      : "Los recargos manuales y los productos anulados quedan excluidos."}
  </p>;
}

export function ProductSales({ report }: { report: AnalyticsSummary }) {
  const [sort, setSort] = useState<"units" | "revenue_cents">("units");
  const [showAll, setShowAll] = useState(false);
  const sorted = [...report.product_sales].sort((a, b) => b[sort] - a[sort] || a.product_uid.localeCompare(b.product_uid));
  const leader = sorted[0];
  const rows = showAll ? sorted : sorted.slice(0, 5);
  const max = Math.max(1, ...sorted.map(row => row[sort]));
  return <section className="analytics-panel">
    <div className="analytics-panel-head"><h2>Productos más vendidos</h2><p>Productos identificados en cuentas cerradas del período.</p></div>
    <div className="analytics-switch" role="group" aria-label="Ordenar productos por">
      <button type="button" aria-pressed={sort === "units"} onClick={() => setSort("units")}>Unidades</button>
      <button type="button" aria-pressed={sort === "revenue_cents"} onClick={() => setSort("revenue_cents")}>Importe</button>
    </div>
    {leader ? <>
      <p className="analytics-insight">{sort === "units" ? "Producto más vendido" : "Producto con mayor importe"}: <strong>{leader.description}</strong> · {sort === "units" ? `${leader.units} unidades` : money(leader.revenue_cents)}</p>
      <ol className="analytics-ranking">{rows.map(row => <li key={row.product_uid}>
        <div className="analytics-rank-top"><span><b>{row.description}</b><small>{row.units} {row.units === 1 ? "unidad" : "unidades"}</small></span><strong>{money(row.revenue_cents)}</strong></div>
        <div className="analytics-rank-track" aria-hidden="true"><span style={{ width: `${row[sort] / max * 100}%` }} /></div>
      </li>)}</ol>
      {sorted.length > 5 ? <Button variant="ghost" className="analytics-more" onClick={() => setShowAll(value => !value)} aria-expanded={showAll}>{showAll ? "Ver los primeros 5" : `Ver todos (${sorted.length})`}</Button> : null}
      <p className="analytics-mini-note">Importes registrados en la venta, antes de descuentos e impuestos de la cuenta.</p>
    </> : <p className="analytics-empty">No hay ventas de productos identificados para este filtro.</p>}
    <ProductCoverage report={report} />
  </section>;
}

export function ProductPurchase({ report }: { report: AnalyticsSummary }) {
  const purchase = report.product_purchase;
  const rate = purchase.rate_percent;
  return <section className="analytics-panel analytics-purchase">
    <div className="analytics-panel-head"><h2>Estadías que compran productos</h2><p>Cuentas cerradas con registro completo de productos.</p></div>
    <strong className="analytics-big-number">{rate === null ? "—" : `${decimal(rate)} %`}</strong>
    <p className="analytics-insight">{purchase.eligible_accounts ? `${purchase.purchasing_accounts} de ${purchase.eligible_accounts} cuentas evaluables compraron productos.` : "Todavía no hay cuentas cerradas evaluables para este filtro."}</p>
    {rate !== null ? <>
      <div className="analytics-stacked" role="img" aria-label={`${decimal(rate)} % con productos; ${decimal(100 - rate)} % sin productos`}>
        <span className="analytics-segment--hourly" style={{ width: `${rate}%` }} />
        <span className="analytics-segment--unclassified" style={{ width: `${100 - rate}%` }} />
      </div>
      <div className="analytics-split-labels"><span>Con productos <b>{purchase.purchasing_accounts}</b></span><span>Sin productos <b>{purchase.eligible_accounts - purchase.purchasing_accounts}</b></span></div>
    </> : null}
    <div className="analytics-secondary-stat"><span>Productos por cuenta compradora</span><strong>{purchase.average_purchase_cents === null ? "—" : money(purchase.average_purchase_cents)}</strong><small>Importe promedio · Una cuenta se cuenta una sola vez.</small></div>
    <ProductCoverage report={report} />
  </section>;
}

export function StayModes({ report }: { report: AnalyticsSummary }) {
  const rows = report.by_stay_mode.filter(row => row.mode !== "unclassified" || row.closed_accounts > 0);
  return <section className="analytics-panel">
    <div className="analytics-panel-head"><h2>Por hora y dormidas</h2><p>Modalidad aplicada al cerrar cada cuenta.</p></div>
    {report.closed_accounts ? <>
      {(["closed_accounts", "lodging_cents"] as const).map(metric => {
        const total = rows.reduce((sum, row) => sum + row[metric], 0);
        return <div className="analytics-mode-row" key={metric}>
          <h3>{metric === "closed_accounts" ? "Cuentas cerradas" : "Importe de alojamiento"}</h3>
          <div className="analytics-stacked" role="img" aria-label={rows.map(row => `${modeLabel[row.mode]}: ${decimal(total ? row[metric] * 100 / total : 0)} %`).join("; ")}>
            {rows.map(row => <span key={row.mode} className={`analytics-segment--${row.mode}`} style={{ width: `${total ? row[metric] * 100 / total : 0}%` }} />)}
          </div>
          <div className="analytics-mode-legend">{rows.map(row => <span key={row.mode}><i className={`analytics-segment--${row.mode}`} />{modeLabel[row.mode]} <b>{total ? `${decimal(row[metric] * 100 / total)} %` : "—"}</b></span>)}</div>
        </div>;
      })}
      <div className="analytics-table-scroll analytics-mode-table"><table><caption className="sr-only">Alojamiento por modalidad</caption><thead><tr><th>Modalidad</th><th>Cuentas</th><th>Alojamiento</th><th>Promedio</th></tr></thead><tbody>{rows.map(row => <tr key={row.mode}>
        <th scope="row">{modeLabel[row.mode]}</th><td>{row.closed_accounts}</td><td>{money(row.lodging_cents)}</td><td>{row.average_lodging_cents === null ? "—" : money(row.average_lodging_cents)}</td>
      </tr>)}</tbody></table></div>
    </> : <p className="analytics-empty">No hay cuentas cerradas para comparar modalidades.</p>}
    <p className="analytics-mini-note">Alojamiento y adicionales de tiempo, antes de descuentos e impuestos. Una cuenta convertida se incluye en dormidas.</p>
  </section>;
}

export function DemandHeatmap({ cells }: { cells: AnalyticsSummary["check_in_heatmap"] }) {
  const [selected, setSelected] = useState<number | null>(null);
  const peak = [...cells].filter(cell => cell.average !== null).sort((a, b) => b.average! - a.average! || a.weekday - b.weekday || a.hour - b.hour)[0];
  const max = Math.max(1, ...cells.map(cell => cell.average ?? 0));
  const detail = selected === null ? null : cells[selected];
  return <section className="analytics-panel analytics-demand">
    <div className="analytics-panel-head"><h2>Días y horarios de mayor movimiento</h2><p>Promedio de entradas por bloque completo de dos horas · Hora de Asunción.</p></div>
    <div className="analytics-heatmap-scroll"><table className="analytics-heatmap"><caption className="sr-only">Promedio de entradas por día de la semana y bloque de dos horas</caption>
      <thead><tr><th scope="col">Día / hora</th>{Array.from({ length: 12 }, (_, index) => <th scope="col" key={index}>{hourLabel(index * 2)}</th>)}</tr></thead>
      <tbody>{weekdays.map((day, weekday) => <tr key={day}><th scope="row">{day}</th>{cells.filter(cell => cell.weekday === weekday).map(cell => {
        const index = weekday * 12 + cell.hour / 2;
        const label = `${day}, ${hourLabel(cell.hour)} a ${hourLabel(cell.hour + 2)}: ${cell.average === null ? "sin bloques observados" : `${cell.count} entradas en ${cell.observed_blocks} bloques; promedio ${decimal(cell.average)}`}`;
        return <td key={cell.hour}><button type="button" className="analytics-heat-cell" disabled={cell.average === null} aria-label={label} title={label} aria-pressed={selected === index}
          onClick={() => setSelected(index)} style={{ "--heat": `${cell.average ? 12 + cell.average / max * 42 : 0}%` } as CSSProperties}>{cell.average === null ? "—" : decimal(cell.average)}</button></td>;
      })}</tr>)}</tbody>
    </table></div>
    <div className="analytics-heat-legend"><span>Menos entradas</span><i /><span>Más entradas</span><span>— Sin bloques observados</span></div>
    <p className="analytics-insight" aria-live="polite">{detail ? `${weekdays[detail.weekday]}, ${hourLabel(detail.hour)} a ${hourLabel(detail.hour + 2)}: ${detail.count} entradas en ${detail.observed_blocks} bloques · Promedio ${decimal(detail.average ?? 0)}.`
      : peak?.average ? `Mayor movimiento promedio: ${weekdays[peak.weekday].toLowerCase()}, entre ${hourLabel(peak.hour)} y ${hourLabel(peak.hour + 2)} · ${decimal(peak.average)} entradas por bloque.` : "Todavía no hay entradas en bloques completos para este período."}</p>
    <p className="analytics-mini-note">Seleccioná una celda para ver el detalle. Se normaliza por la cantidad de bloques observados de cada día de la semana; el bloque actual y los futuros quedan excluidos. Mide entradas, no ocupación.</p>
  </section>;
}
