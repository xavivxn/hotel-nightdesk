import { useState } from "react";
import { api } from "@/lib/api";
import { useOperationalRefresh } from "@/lib/useOperationalRefresh";
import { asuncionInput, asuncionInstant } from "@/lib/asuncion";
import { formatMoney } from "@/lib/format";
import type { OperatorActivity, SessionUser } from "@/lib/types";
import { Button } from "@/components/ui/Button";
import { Field, Input, Select } from "@/components/ui/Field";
const labels: Record<string, string> = { check_in: "Ingreso", check_out: "Cierre", add_product_charge: "Consumo", add_charge: "Ajuste de cuenta", delete_charge: "Anulación de consumo", set_room_status: "Estado de habitación", convert_to_overnight: "Conversión a dormida", create_reservation: "Reserva", set_reservation_status: "Estado de reserva", check_in_reservation: "Ingreso de reserva", update_product_stock: "Stock", save_price_rules: "Promociones" };
export function ActivityPage({ user }: { user: SessionUser }) {
  const [from, setFrom] = useState(() => `${asuncionInput().slice(0, 10)}T00:00`);
  const [to, setTo] = useState(() => asuncionInput(new Date(Date.now() + 60000)));
  const [report, setReport] = useState<OperatorActivity | null>(null);
  const [selected, setSelected] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [range, setRange] = useState<{ from: string; to: string } | null>(null);
  async function load(start: string, end: string) { const rows = await api.operatorActivity(start, end); setReport(rows); }
  useOperationalRefresh(async () => { if (range) await load(range.from, range.to); });
  const rows = report?.operations.filter(r => !selected || r.actor_uid === selected) ?? [];
  const accounts = rows.filter(r => r.command === "check_out");
  const users = new Map((report?.operations ?? []).map(r => [r.actor_uid, r.username]));
  return <div className="space-y-5 px-6 py-6 lg:px-8">
    <header><p className="page-kicker">Recepción</p><h1 className="page-title">{user.role === "admin" ? "Actividad por recepcionista" : "Mi actividad"}</h1><p className="mt-2 text-[var(--muted)]">Horario de Paraguay (America/Asuncion). El total corresponde al usuario que cerró la cuenta; no es un arqueo de efectivo.</p></header>
    <form className="flex flex-wrap items-end gap-3" onSubmit={async e => { e.preventDefault(); setBusy(true); setError(""); try { const start = asuncionInstant(from), end = asuncionInstant(to); await load(start, end); setRange({ from: start, to: end }); setSelected(""); } catch (cause) { setError(String(cause)); } finally { setBusy(false); } }}>
      <Field label="Desde (incluido)"><Input type="datetime-local" required value={from} onChange={e => setFrom(e.target.value)} /></Field><Field label="Hasta (sin incluir)"><Input type="datetime-local" required value={to} onChange={e => setTo(e.target.value)} /></Field><Button disabled={busy} type="submit">Consultar</Button>
      {user.role === "admin" && <Field label="Usuario"><Select value={selected} onChange={e => setSelected(e.target.value)}><option value="">Todos</option>{Array.from(users).map(([id, name]) => <option key={id} value={id}>{name} · {id.slice(0, 8)}</option>)}</Select></Field>}
    </form>
    {error && <p role="alert" className="text-[var(--danger)]">{error}</p>}
    {report && <><p className="rounded-lg border border-[var(--line)] p-4">{rows.length} operaciones · {accounts.length} cuentas cerradas · <strong>{formatMoney(accounts.reduce((sum, r) => sum + (r.closed_total_cents ?? 0), 0), "Gs.")}</strong></p>
      <div className="overflow-auto"><table className="w-full text-left text-sm"><thead><tr>{["Hora", "Usuario histórico", "Puesto", "Acción", "Total cerrado"].map(t => <th key={t} className="p-3">{t}</th>)}</tr></thead><tbody>{rows.map(row => <tr key={row.uid} className="border-t border-[var(--line)]"><td className="p-3">{new Date(row.created_at).toLocaleString("es-PY", { timeZone: "America/Asuncion" })}</td><td className="p-3">{row.username}</td><td className="p-3 font-mono" title={row.station_id}>{row.station_id.slice(0, 8) || "Sin identificar"}</td><td className="p-3">{labels[row.command] ?? row.command}</td><td className="p-3">{row.closed_total_cents == null ? "—" : formatMoney(row.closed_total_cents, "Gs.")}</td></tr>)}</tbody></table></div></>}
    <p className="text-sm text-[var(--muted)]">La atribución verificable comienza con esta versión. Administración ve los cierres anteriores como “Sin identidad verificable”; no se asignan a usuarios nuevos a partir de nombres antiguos.</p>
  </div>;
}
