import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { formatDateTime, formatMoney } from "@/lib/format";
import type { AppSettings, HistoryStay } from "@/lib/types";
import { useEffect, useState } from "react";

function todayInput() {
  const date = new Date();
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function HistoryPage({ settings }: { settings: AppSettings }) {
  const [date, setDate] = useState(todayInput);
  const [items, setItems] = useState<HistoryStay[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [printing, setPrinting] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let current = true;
    setLoading(true); setItems([]); setError(null); setNotice(null);
    api.listHistory(date).then(rows => { if (current) setItems(rows); })
      .catch(e => { if (current) setError(String(e)); })
      .finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [date]);

  const total = items.reduce((sum, item) => sum + item.total_cents, 0);

  return (
    <div className="px-6 py-6 lg:px-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="page-kicker">Cierre</p>
          <h1 className="page-title">Historial del día</h1>
        </div>
        <div className="flex items-center gap-3">
          <div className="rounded-lg border border-[var(--line)] bg-[var(--surface)] px-4 py-3 text-sm">
            <span className="text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">Total del día</span>
            <p className="font-mono text-lg font-semibold tabular-nums">{formatMoney(total, settings.currency_symbol)}</p>
          </div>
          <Input type="date" className="w-44" value={date} onChange={(e) => setDate(e.target.value)} />
          <Button disabled={exporting || !date || loading} onClick={async () => {
            setExporting(true); setNotice(null); setError(null);
            try { const path = await api.exportDailyPdf(date); setNotice(`PDF guardado: ${path}`); }
            catch (e) { setError(String(e)); }
            finally { setExporting(false); }
          }}>{exporting ? "Generando PDF…" : "Exportar PDF diario"}</Button>
        </div>
      </header>
      {error ? <p className="mt-4 text-[var(--danger)]">{error}</p> : null}
      {notice ? <p className="mt-4 text-sm text-[var(--muted)]">{notice}</p> : null}
      <div className="mt-6 overflow-hidden rounded-lg border border-[var(--line)] bg-[var(--surface)]">
        <table className="w-full text-left text-sm">
          <thead className="bg-[var(--surface-2)] text-xs uppercase tracking-[0.12em] text-[var(--muted)]">
            <tr>
              <th className="px-4 py-3">Habitación</th>
              <th className="px-4 py-3">Salida</th>
              <th className="px-4 py-3">Cuenta</th>
              <th className="px-4 py-3 text-right">Total</th>
              <th className="px-4 py-3"></th>
            </tr>
          </thead>
          <tbody>
            {items.map((item) => (
              <tr key={item.stay.id} className="border-t border-[var(--line)]">
                <td className="px-4 py-3 font-mono font-semibold tabular-nums">{item.stay.room_number}</td>
                <td className="px-4 py-3">{item.stay.check_out_at ? formatDateTime(item.stay.check_out_at) : "—"}</td>
                <td className="px-4 py-3 text-[var(--muted)]">Cerrada</td>
                <td className="px-4 py-3 text-right font-mono font-medium tabular-nums">
                  {formatMoney(item.total_cents, settings.currency_symbol)}
                </td>
                <td className="px-4 py-3 text-right">
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={printing !== null}
                    onClick={async () => {
                      setNotice(null); setError(null); setPrinting(item.stay.id);
                      try {
                        const printError = await api.reprintReceipt(item.stay.id);
                        setNotice(printError ? `Cuenta cerrada. ${printError}` : "Ticket enviado a la cola de recepción. Verificá la salida en papel.");
                      } catch (e) { setError(String(e)); }
                      finally { setPrinting(null); }
                    }}
                  >
                    {printing === item.stay.id ? "Enviando…" : "Reimprimir"}
                  </Button>
                </td>
              </tr>
            ))}
            {items.length === 0 ? (
              <tr>
                <td className="px-4 py-10 text-center text-[var(--muted)]" colSpan={5}>
                  {loading ? "Cargando cuentas…" : "No hay estadías cerradas en esta fecha."}
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </div>
  );
}
