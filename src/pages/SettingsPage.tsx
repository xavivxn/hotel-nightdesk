import { ArrowDownToLine, ArrowUpFromLine, Building2, Cloud, KeyRound, Printer, RefreshCw, Wifi, WifiOff } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { BackupPanel } from "@/components/backup/BackupPanel";
import { Field, Input, Select, Textarea, reportInputIssue } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { FIELD_EMPTY, parseTaxPercent, requireTrimmed } from "@/lib/format";
import { useTheme } from "@/lib/theme";
import type { AppSettings, DeviceMode, SyncStatus } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useEffect, useRef, useState } from "react";

type SettingsSection = "business" | "printer" | "sync" | "backup";

function syncMoment(value: string | null | undefined) {
  if (!value) return { when: "Todavía no", ago: "Sin registro en este equipo" };
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return { when: value, ago: "Fecha no reconocida" };
  const when = date.toLocaleString("es-AR", {
    weekday: "short",
    day: "2-digit",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
  const minutes = Math.round((Date.now() - date.getTime()) / 60_000);
  if (minutes < 0) return { when, ago: "Por delante del reloj de este equipo" };
  if (minutes < 1) return { when, ago: "Hace un momento" };
  if (minutes < 60) return { when, ago: `Hace ${minutes} min` };
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return { when, ago: hours === 1 ? "Hace 1 hora" : `Hace ${hours} horas` };
  const days = Math.floor(hours / 24);
  return { when, ago: days === 1 ? "Hace 1 día" : `Hace ${days} días` };
}

function syncTone(sync: SyncStatus | null) {
  if (!sync) return { label: "Consultando", className: "bg-[var(--surface-2)] text-[var(--muted)]" };
  if (!sync.configured) return { label: "Sin configurar", className: "bg-[var(--surface-2)] text-[var(--muted)]" };
  if (!sync.connected) return { label: "Sin conexión", className: "bg-[var(--warn-soft)] text-[var(--warn)]" };
  return { label: "En línea", className: "bg-[var(--ok-soft)] text-[var(--ok)]" };
}

export function SettingsPage({
  settings,
  deviceMode = "reception",
  onSaved,
}: {
  settings: AppSettings;
  deviceMode?: DeviceMode;
  onSaved: (settings: AppSettings) => void;
}) {
  const { setTheme } = useTheme();
  const [section, setSection] = useState<SettingsSection>("business");
  const [form, setForm] = useState(settings);
  const [taxInput, setTaxInput] = useState(String(settings.tax_percent));
  const [pin, setPin] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [printers, setPrinters] = useState<string[]>([]);
  const [sync, setSync] = useState<SyncStatus | null>(null);
  const [replacingDevice, setReplacingDevice] = useState(false);
  const [deviceForm, setDeviceForm] = useState({
    project_url: "",
    anon_key: "",
    device_email: "",
    device_password: "",
  });
  const nameRef = useRef<HTMLInputElement>(null);
  const taxRef = useRef<HTMLInputElement>(null);
  const deviceUrlRef = useRef<HTMLInputElement>(null);
  const deviceKeyRef = useRef<HTMLInputElement>(null);
  const deviceEmailRef = useRef<HTMLInputElement>(null);

  const reception = deviceMode === "reception";
  const tabs = [
    { id: "business" as const, label: "Negocio", icon: Building2 },
    { id: "printer" as const, label: "Impresora", icon: Printer },
    ...(reception ? [{ id: "sync" as const, label: "Sincronización", icon: RefreshCw }] : []),
    { id: "backup" as const, label: "Respaldos", icon: Cloud },
  ];
  const active = section === "sync" && !reception ? "business" : section;
  const canSave = active === "business" || active === "printer";

  useEffect(() => {
    if (!reception) return;
    void api.syncStatus().then(setSync).catch(() => setSync(null));
  }, [reception]);

  async function save(clearPin = false) {
    const business_name = requireTrimmed(form.business_name);
    if (!business_name) {
      setSection("business");
      queueMicrotask(() => reportInputIssue(nameRef.current, FIELD_EMPTY));
      return false;
    }
    const tax = parseTaxPercent(taxInput);
    if (tax == null) {
      setSection("business");
      queueMicrotask(() => reportInputIssue(taxRef.current, "Ingresá un IVA entre 0 y 100."));
      return false;
    }
    setBusy(true);
    setError(null);
    try {
      const next = await api.saveSettings({
        ...form,
        business_name,
        address: form.address.trim(),
        phone: form.phone.trim(),
        currency_symbol: form.currency_symbol.trim() || "Gs.",
        tax_percent: tax,
        receipt_footer: form.receipt_footer.trim(),
        printer_path: form.printer_path.trim(),
        printer_name: form.printer_name.trim(),
        require_guest_name: false,
      }, clearPin ? "" : pin.trim() || undefined);
      setForm(next);
      setTaxInput(String(next.tax_percent));
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

  const push = syncMoment(sync?.last_push_at);
  const pull = syncMoment(sync?.last_pull_at);
  const tone = syncTone(sync);
  const LiveIcon = sync?.realtime_connected ? Wifi : WifiOff;

  return (
    <div className="px-6 py-6 lg:px-8">
      <div className="mx-auto w-full max-w-3xl">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="page-kicker">Establecimiento</p>
          <h1 className="page-title">Ajustes</h1>
          <p className="page-description">Datos del local, tickets y, en recepción, la réplica con administración.</p>
        </div>
        {canSave ? (
          <Button disabled={busy} onClick={() => void save()}>
            Guardar ajustes
          </Button>
        ) : null}
      </header>

      <div
        role="tablist"
        aria-label="Secciones de ajustes"
        className="mt-6 flex flex-wrap gap-1 rounded-lg border border-[var(--line)] bg-[var(--surface)] p-1"
      >
        {tabs.map((tab) => {
          const selected = active === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              role="tab"
              id={`settings-tab-${tab.id}`}
              aria-selected={selected}
              aria-controls={`settings-panel-${tab.id}`}
              className={cn(
                "flex min-h-11 min-w-[7.5rem] flex-1 items-center justify-center gap-2 rounded-lg px-3 py-2 text-sm font-medium text-[var(--muted)] hover:bg-[var(--surface-2)] hover:text-[var(--ink)]",
                selected && "bg-[var(--accent-soft)] text-[var(--accent)]",
              )}
              onClick={() => setSection(tab.id)}
            >
              <tab.icon size={16} />
              {tab.label}
            </button>
          );
        })}
      </div>

      {error ? <p className="login-error mt-4 rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-4 py-3 text-sm text-[var(--ink)]">{error}</p> : null}
      {notice ? <p className="mt-4 rounded-lg border border-[var(--line)] bg-[var(--surface-2)] px-4 py-3 text-sm text-[var(--muted)]">{notice}</p> : null}

      {active === "business" ? (
        <section
          role="tabpanel"
          id="settings-panel-business"
          aria-labelledby="settings-tab-business"
          className="card mt-6 space-y-4 rounded-lg p-5"
        >
          <div>
            <p className="page-kicker">Negocio</p>
            <h2 className="flex items-center gap-2 text-lg font-semibold tracking-tight">
              <Building2 size={18} className="text-[var(--accent)]" /> Datos del hotel
            </h2>
            <p className="mt-1 text-sm text-[var(--muted)]">Aparecen en el ticket y en las cuentas de este equipo.</p>
          </div>
          <Field label="Nombre">
            <Input ref={nameRef} value={form.business_name} onChange={(e) => setForm({ ...form, business_name: e.target.value })} />
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
              ref={taxRef}
              inputMode="decimal"
              value={taxInput}
              onChange={(e) => setTaxInput(e.target.value)}
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
              <option value="light">Claro</option>
              <option value="dark">Oscuro</option>
            </Select>
          </Field>
        </section>
      ) : null}

      {active === "printer" ? (
        <div role="tabpanel" id="settings-panel-printer" aria-labelledby="settings-tab-printer" className="mt-6 space-y-4">
          <section className="card space-y-4 rounded-lg p-5">
            <div>
              <p className="page-kicker">Tickets</p>
              <h2 className="flex items-center gap-2 text-lg font-semibold tracking-tight">
                <Printer size={18} className="text-[var(--accent)]" /> Impresora ESC/POS
              </h2>
              <p className="mt-1 text-sm text-[var(--muted)]">Solo esta PC imprime. Un fallo de papel no deshace el cobro.</p>
            </div>
            <label className="flex items-start gap-3 rounded-lg border border-[var(--line)] bg-[var(--surface-2)] px-3 py-3 text-sm">
              <input
                className="mt-0.5"
                type="checkbox"
                checked={form.printer_enabled}
                onChange={(e) => setForm({ ...form, printer_enabled: e.target.checked })}
              />
              Enviar tickets a la impresora
            </label>
            <label className="flex items-start gap-3 rounded-lg border border-[var(--line)] bg-[var(--surface-2)] px-3 py-3 text-sm">
              <input
                className="mt-0.5"
                type="checkbox"
                checked={form.auto_print_on_checkout}
                onChange={(e) => setForm({ ...form, auto_print_on_checkout: e.target.checked })}
              />
              <span>Imprimir al cerrar la cuenta. Si la impresora falla, el cierre igual queda hecho.</span>
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
          </section>
          <section className="card space-y-4 rounded-lg p-5">
            <h3 className="flex items-center gap-2 font-semibold">
              <KeyRound size={16} className="text-[var(--accent)]" /> PIN de desbloqueo
            </h3>
            <p className="text-sm text-[var(--muted)]">
              {form.has_pin ? "Hay un PIN activo en este equipo." : "Opcional. Si lo definís, la app pide PIN al abrir."}
            </p>
            <Field label="Nuevo PIN">
              <Input type="password" inputMode="numeric" value={pin} onChange={(e) => setPin(e.target.value)} />
            </Field>
            {form.has_pin ? (
              <Button variant="ghost" onClick={() => void save(true)}>
                Quitar PIN
              </Button>
            ) : null}
          </section>
        </div>
      ) : null}

      {active === "sync" && reception ? (
        <section
          role="tabpanel"
          id="settings-panel-sync"
          aria-labelledby="settings-tab-sync"
          className="card mt-6 space-y-4 rounded-lg p-5"
        >
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <p className="page-kicker">Réplica</p>
              <h2 className="text-lg font-semibold tracking-tight">Sincronización</h2>
              <p className="mt-1 max-w-2xl text-sm text-[var(--muted)]">
                {sync?.configured
                  ? "El envío y la bajada ya usan las credenciales de este equipo. No se vuelven a mostrar."
                  : "Estas credenciales habilitan el worker y la subida de respaldos. Se guardan acá y no se vuelven a mostrar."}
              </p>
            </div>
            <span className={cn("rounded-lg px-2.5 py-1 text-xs font-semibold", tone.className)}>{tone.label}</span>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4">
              <p className="page-kicker">Cola</p>
              <p className="mt-1 font-mono text-2xl tabular-nums">{sync?.pending_outbox ?? 0}</p>
              <p className="mt-1 text-xs text-[var(--muted)]">Operaciones de recepción esperando envío</p>
            </div>
            <div className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4">
              <p className="page-kicker flex items-center gap-1.5"><ArrowUpFromLine size={12} /> Envío</p>
              <p className="mt-1 font-mono text-lg tabular-nums">{push.when}</p>
              <p className="mt-1 text-xs text-[var(--muted)]">{push.ago}. Ocupación, cuentas y reservas hacia administración.</p>
            </div>
            <div className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4">
              <p className="page-kicker flex items-center gap-1.5"><ArrowDownToLine size={12} /> Bajada</p>
              <p className="mt-1 font-mono text-lg tabular-nums">{pull.when}</p>
              <p className="mt-1 text-xs text-[var(--muted)]">{pull.ago}. Catálogo, usuarios y ajustes hacia este equipo.</p>
            </div>
            <div className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4">
              <p className="page-kicker flex items-center gap-1.5"><LiveIcon size={12} /> Tiempo real</p>
              <p className="mt-1 text-lg font-semibold">{sync?.realtime_connected ? "Activo" : "En pausa"}</p>
              <p className="mt-1 text-xs text-[var(--muted)]">
                {sync?.realtime_connected
                  ? "Los cambios remotos avisan a esta PC."
                  : "Sin aviso en vivo. La bajada igual corre al sincronizar."}
              </p>
            </div>
          </div>
          {sync?.last_error ? (
            <p className="rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-4 py-3 text-sm text-[var(--ink)]" role="alert">
              Último problema: {sync.last_error}
            </p>
          ) : null}
          <div className="flex flex-wrap gap-2">
            <Button
              variant="secondary"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                setError(null);
                try {
                  await api.syncPullNow();
                  window.dispatchEvent(new Event("sync:catalog-updated"));
                  setSync(await api.syncStatus());
                  setNotice("Sincronización ejecutada");
                } catch (e) {
                  setError(String(e));
                } finally {
                  setBusy(false);
                }
              }}
            >
              <RefreshCw size={15} /> Sincronizar ahora
            </Button>
          </div>
          {sync === null || sync.embedded ? null : sync.configured && !replacingDevice ? (
            <Button variant="secondary" disabled={busy} onClick={() => setReplacingDevice(true)}>
              Reemplazar credenciales
            </Button>
          ) : (
            <>
              <div className="grid gap-3 border-t border-[var(--line)] pt-4 md:grid-cols-2">
                <Field label="URL del proyecto">
                  <Input
                    ref={deviceUrlRef}
                    value={deviceForm.project_url}
                    onChange={(e) => setDeviceForm({ ...deviceForm, project_url: e.target.value })}
                    placeholder="https://xxxx.supabase.co"
                  />
                </Field>
                <Field label="Clave anónima">
                  <Input
                    ref={deviceKeyRef}
                    type="password"
                    value={deviceForm.anon_key}
                    onChange={(e) => setDeviceForm({ ...deviceForm, anon_key: e.target.value })}
                  />
                </Field>
                <Field label="Email del dispositivo">
                  <Input
                    ref={deviceEmailRef}
                    value={deviceForm.device_email}
                    onChange={(e) => setDeviceForm({ ...deviceForm, device_email: e.target.value })}
                  />
                </Field>
                <Field label="Contraseña del dispositivo">
                  <Input
                    type="password"
                    value={deviceForm.device_password}
                    onChange={(e) => setDeviceForm({ ...deviceForm, device_password: e.target.value })}
                  />
                </Field>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  disabled={busy}
                  onClick={async () => {
                    const project_url = requireTrimmed(deviceForm.project_url);
                    const anon_key = requireTrimmed(deviceForm.anon_key);
                    const device_email = requireTrimmed(deviceForm.device_email);
                    if (!project_url) {
                      reportInputIssue(deviceUrlRef.current, FIELD_EMPTY);
                      return;
                    }
                    if (!anon_key) {
                      reportInputIssue(deviceKeyRef.current, FIELD_EMPTY);
                      return;
                    }
                    if (!device_email) {
                      reportInputIssue(deviceEmailRef.current, FIELD_EMPTY);
                      return;
                    }
                    setBusy(true);
                    setError(null);
                    try {
                      await api.syncConfigureDevice({
                        project_url,
                        anon_key,
                        device_email,
                        device_password: deviceForm.device_password,
                      });
                      setDeviceForm({ project_url: "", anon_key: "", device_email: "", device_password: "" });
                      setReplacingDevice(false);
                      setSync(await api.syncStatus());
                      setNotice("Dispositivo configurado en este equipo. El worker y la subida de respaldos lo usan al abrir la app.");
                    } catch (e) {
                      setError(String(e));
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  {sync?.configured ? "Guardar credenciales" : "Configurar dispositivo"}
                </Button>
                {sync?.configured ? (
                  <Button
                    variant="ghost"
                    disabled={busy}
                    onClick={() => {
                      setReplacingDevice(false);
                      setDeviceForm({ project_url: "", anon_key: "", device_email: "", device_password: "" });
                    }}
                  >
                    Cancelar
                  </Button>
                ) : null}
              </div>
            </>
          )}
        </section>
      ) : null}

      {active === "backup" ? (
        <div role="tabpanel" id="settings-panel-backup" aria-labelledby="settings-tab-backup" className="mt-6">
          <BackupPanel deviceMode={deviceMode} />
        </div>
      ) : null}
      </div>
    </div>
  );
}
