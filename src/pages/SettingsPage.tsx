import { Button } from "@/components/ui/Button";
import { Field, Input, Select, Textarea } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { useTheme } from "@/lib/theme";
import type { AppSettings } from "@/lib/types";
import { useState } from "react";

export function SettingsPage({
  settings,
  onSaved,
}: {
  settings: AppSettings;
  onSaved: (settings: AppSettings) => void;
}) {
  const { setTheme } = useTheme();
  const [form, setForm] = useState(settings);
  const [pin, setPin] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [printers, setPrinters] = useState<string[]>([]);

  async function save(clearPin = false) {
    setBusy(true);
    setError(null);
    try {
      const next = await api.saveSettings({ ...form, require_guest_name: false }, clearPin ? "" : pin || undefined);
      setForm(next);
      onSaved(next);
      if (next.theme === "light" || next.theme === "dark") setTheme(next.theme);
      setPin("");
      setNotice("Ajustes guardados en este equipo.");
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="px-6 py-6 lg:px-8">
      <header className="mb-6">
        <p className="page-kicker">Establecimiento</p>
        <h1 className="page-title">Ajustes</h1>
      </header>
      <div className="grid max-w-4xl gap-6 lg:grid-cols-2">
        <section className="card space-y-4 rounded-lg p-5">
          <h2 className="text-lg font-semibold tracking-tight">Datos del hotel</h2>
          <Field label="Nombre">
            <Input value={form.business_name} onChange={(e) => setForm({ ...form, business_name: e.target.value })} />
          </Field>
          <Field label="Dirección">
            <Input value={form.address} onChange={(e) => setForm({ ...form, address: e.target.value })} />
          </Field>
          <Field label="Teléfono">
            <Input value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} />
          </Field>
          <Field label="Símbolo de moneda">
            <Input
              value={form.currency_symbol}
              onChange={(e) => setForm({ ...form, currency_symbol: e.target.value })}
              placeholder="Gs."
            />
          </Field>
          <Field label="IVA %">
            <Input
              type="number"
              step="0.01"
              value={form.tax_percent}
              onChange={(e) => setForm({ ...form, tax_percent: Number(e.target.value) })}
            />
          </Field>
          <Field label="Pie del ticket">
            <Textarea value={form.receipt_footer} onChange={(e) => setForm({ ...form, receipt_footer: e.target.value })} />
          </Field>
          <Field label="Tema">
            <Select
              value={form.theme}
              onChange={(e) => setForm({ ...form, theme: e.target.value })}
            >
              <option value="dark">Oscuro</option>
              <option value="light">Claro</option>
            </Select>
          </Field>
        </section>
        <section className="card space-y-4 rounded-lg p-5">
          <h2 className="text-lg font-semibold tracking-tight">Impresora ESC/POS</h2>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={form.printer_enabled}
              onChange={(e) => setForm({ ...form, printer_enabled: e.target.checked })}
            />
            Enviar tickets a la impresora
          </label>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={form.auto_print_on_checkout}
              onChange={(e) => setForm({ ...form, auto_print_on_checkout: e.target.checked })}
            />
            Imprimir al cerrar la cuenta (un fallo no revierte el cierre)
          </label>
          <Field label="Ancho">
            <Select
              value={form.paper_width}
              onChange={(e) => setForm({ ...form, paper_width: Number(e.target.value) })}
            >
              <option value={58}>58 mm</option>
              <option value={80}>80 mm</option>
            </Select>
          </Field>
          <Field label="Nombre de impresora (lp / Windows)">
            <Input
              value={form.printer_name}
              onChange={(e) => setForm({ ...form, printer_name: e.target.value })}
              placeholder="POS-80"
            />
          </Field>
          <Button variant="secondary" disabled={busy} onClick={async () => {
            setBusy(true); setError(null);
            try { const names = await api.listPrinters(); setPrinters(names); if (!names.length) setNotice("No se encontraron colas. La detección requiere la app Windows y el controlador instalado."); }
            catch (e) { setError(String(e)); }
            finally { setBusy(false); }
          }}>Buscar impresoras instaladas</Button>
          {printers.length > 0 && <Field label="Colas de Windows"><Select value={form.printer_name} onChange={e => setForm({...form, printer_name: e.target.value, printer_path: ""})}>
            <option value="">Seleccionar impresora</option>
            {printers.map(name => <option key={name} value={name}>{name}</option>)}
          </Select></Field>}
          <Field label="Ruta del puerto (USB/serial)">
            <Input
              value={form.printer_path}
              onChange={(e) => setForm({ ...form, printer_path: e.target.value })}
              placeholder="/dev/usb/lp0"
            />
          </Field>
          <Button
            variant="secondary"
            className="w-full"
            disabled={busy}
            onClick={async () => {
              setError(null);
              try {
                if (!await save()) return;
                setBusy(true);
                const printError = await api.printTest();
                setNotice(printError ? printError : "Prueba enviada a la cola de recepción. Verificá márgenes, legibilidad y corte en papel.");
              } catch (e) {
                setError(String(e));
              } finally {
                setBusy(false);
              }
            }}
          >
            Imprimir prueba
          </Button>
          <div className="border-t border-[var(--line)] pt-4">
            <h3 className="mb-2 font-semibold">PIN de desbloqueo</h3>
            <p className="mb-3 text-sm text-[var(--muted)]">
              {form.has_pin ? "Hay un PIN activo en este equipo." : "Opcional. Si lo definís, la app pide PIN al abrir."}
            </p>
            <Field label="Nuevo PIN">
              <Input type="password" inputMode="numeric" value={pin} onChange={(e) => setPin(e.target.value)} />
            </Field>
            {form.has_pin ? (
              <Button variant="ghost" className="mt-2" onClick={() => save(true)}>
                Quitar PIN
              </Button>
            ) : null}
          </div>
        </section>
      </div>
      {error ? <p className="mt-4 text-[var(--danger)]">{error}</p> : null}
      {notice ? <p className="mt-4 text-sm text-[var(--muted)]">{notice}</p> : null}
      <Button className="mt-6" disabled={busy} onClick={() => save()}>
        Guardar ajustes
      </Button>
    </div>
  );
}
