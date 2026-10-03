import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { AppSettings } from "@/lib/types";
import { Button } from "@/components/ui/Button";
import { Field, Select } from "@/components/ui/Field";

export function PrinterPage() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [target, setTarget] = useState<"local" | "principal">("local");
  const [client, setClient] = useState(false);
  const [printers, setPrinters] = useState<string[]>([]);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  async function detect() { setPrinters(await api.listPrinters()); }
  useEffect(() => {
    void Promise.all([api.printerConfig(), api.deviceModeGet(), detect()]).then(([config, mode]) => {
      setSettings(config.settings); setTarget(config.target); setClient(mode === "reception_client");
    }).catch(e => setError(String(e)));
  }, []);
  async function save(test: boolean) {
    if (!settings) return;
    setBusy(true); setNotice(""); setError("");
    try {
      await api.savePrinterConfig({ ...settings, printer_path: "" }, target);
      if (test) { const warning = await api.printTest(); setNotice(warning || "Prueba enviada al spooler. Verificá el papel en la impresora."); }
      else setNotice("Preferencias guardadas en este equipo.");
      window.dispatchEvent(new Event("printer:changed"));
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  return <div className="max-w-3xl space-y-5 px-6 py-6 lg:px-8">
    <header><p className="page-kicker">Este puesto</p><h1 className="page-title">Impresora</h1><p className="mt-2 text-[var(--muted)]">Instalá el driver de Windows antes de trabajar sin internet. La selección se conserva al reconectar la misma cola.</p></header>
    {settings && <section className="space-y-4 rounded-lg border border-[var(--line)] bg-[var(--surface)] p-5">
      {client && <Field label="Destino de los comprobantes"><Select value={target} onChange={e => setTarget(e.target.value as "local" | "principal")}><option value="local">Impresora de este puesto</option><option value="principal">Impresora de recepción principal</option></Select></Field>}
      <label className="flex items-center gap-3"><input type="checkbox" checked={settings.auto_print_on_checkout} onChange={e => setSettings({ ...settings, auto_print_on_checkout: e.target.checked })} />Imprimir al cerrar la cuenta</label>
      <label className="flex items-center gap-3"><input type="checkbox" checked={settings.printer_enabled} onChange={e => setSettings({ ...settings, printer_enabled: e.target.checked })} />Habilitar impresora local</label>
      <Field label="Cola instalada en Windows"><Select value={settings.printer_name} onChange={e => setSettings({ ...settings, printer_name: e.target.value })}><option value="">Elegí una impresora</option>{Array.from(new Set([...printers, settings.printer_name].filter(Boolean))).map(p => <option key={p}>{p}</option>)}</Select></Field>
      <Field label="Ancho de papel"><Select value={settings.paper_width} onChange={e => setSettings({ ...settings, paper_width: Number(e.target.value) })}><option value={80}>80 mm</option><option value={58}>58 mm</option></Select></Field>
      <p className="text-sm text-[var(--muted)]">Perfil: cola de Windows con driver compatible. Se imprimen dos copias al cerrar y una al reimprimir. Los tickets antiguos conservan su formato original.</p>
      <div className="flex flex-wrap gap-3"><Button disabled={busy} onClick={() => void save(false)}>Guardar</Button><Button variant="secondary" disabled={busy} onClick={() => void save(true)}>Guardar y probar impresora local</Button><Button variant="secondary" disabled={busy} onClick={() => void detect().catch(e => setError(String(e)))}>Detectar de nuevo</Button></div>
      <p className="text-sm text-[var(--muted)]">Si un envío queda incierto, verificá la impresora antes de pedir otra copia. Un fallo de impresión no revierte el cierre.</p>
    </section>}
    {notice && <p role="status">{notice}</p>}{error && <p role="alert" className="text-[var(--danger)]">{error}</p>}
  </div>;
}
