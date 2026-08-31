import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { formatDateTime, formatMoney, paymentLabel } from "@/lib/format";
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

  async function load(nextDate = date) {
    const rows = await api.listHistory(nextDate);
    setItems(rows);
  }

  useEffect(() => {
    load().catch((e) => setError(String(e)));
  }, [date]);

  const total = items.reduce((sum, item) => sum + item.total_cents, 0);

  return (
    <div className="px-6 py-6 lg:px-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="text-xs uppercase tracking-[0.22em] text-[var(--muted)]">Cierre</p>
          <h1 className="font-display text-4xl">Historial del día</h1>
        </div>
        <div className="flex items-center gap-3">
          <div className="rounded-2xl border border-[var(--line)] bg-[var(--surface)] px-4 py-3 text-sm">
            <span className="text-[var(--muted)]">Total cobrado</span>
            <p className="text-lg font-semibold">{formatMoney(total, settings.currency_symbol)}</p>
          </div>
          <Input type="date" className="w-44" value={date} onChange={(e) => setDate(e.target.value)} />
        </div>
      </header>
      {error ? <p className="mt-4 text-[var(--danger)]">{error}</p> : null}
      {notice ? <p className="mt-4 text-sm text-[var(--muted)]">{notice}</p> : null}
      <div className="mt-6 overflow-hidden rounded-2xl border border-[var(--line)] bg-[var(--surface)]">
        <table className="w-full text-left text-sm">
          <thead className="bg-[var(--surface-2)] text-xs uppercase tracking-[0.12em] text-[var(--muted)]">
            <tr>
              <th className="px-4 py-3">Habitación</th>
              <th className="px-4 py-3">Huésped</th>
              <th className="px-4 py-3">Salida</th>
              <th className="px-4 py-3">Pago</th>
              <th className="px-4 py-3 text-right">Total</th>
              <th className="px-4 py-3"></th>
            </tr>
          </thead>
          <tbody>
            {items.map((item) => (
              <tr key={item.stay.id} className="border-t border-[var(--line)]">
                <td className="px-4 py-3 font-semibold">{item.stay.room_number}</td>
                <td className="px-4 py-3">{item.stay.guest_name}</td>
                <td className="px-4 py-3">{item.stay.check_out_at ? formatDateTime(item.stay.check_out_at) : "—"}</td>
                <td className="px-4 py-3">{paymentLabel(item.payment_method)}</td>
                <td className="px-4 py-3 text-right font-medium">
                  {formatMoney(item.total_cents, settings.currency_symbol)}
                </td>
                <td className="px-4 py-3 text-right">
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={async () => {
                      setNotice(null);
                      const printError = await api.reprintReceipt(item.stay.id);
                      setNotice(printError ? `Checkout cobrado. Impresora: ${printError}` : "Ticket reimpreso (o archivado en local).");
                    }}
                  >
                    Reimprimir
                  </Button>
                </td>
              </tr>
            ))}
            {items.length === 0 ? (
              <tr>
                <td className="px-4 py-10 text-center text-[var(--muted)]" colSpan={6}>
                  No hay estadías cerradas en esta fecha.
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </div>
  );
}
