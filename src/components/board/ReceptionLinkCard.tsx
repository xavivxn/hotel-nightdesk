import { useContext, useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { ArrowRight, RefreshCw, ShieldCheck, Wifi, X } from "lucide-react";
import { api } from "@/lib/api";
import { RoleContext } from "@/lib/permissions";
import type { DeviceMode } from "@/lib/types";
import type { ReceptionLan } from "@/components/layout/ReceptionConnectionStatus";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Field";

function preferredInterface(rows: { name: string; address: string }[]) {
  const score = ({ name, address }: { name: string; address: string }) => {
    const label = name.toLowerCase();
    return (/(wi-?fi|wlan|ethernet|^en[0-9]+$)/.test(label) ? 10 : 0)
      + (address.startsWith("192.168.") ? 4 : address.startsWith("10.") ? 2 : 0)
      - (/(virtual|vEthernet|vpn|utun|docker|bridge|tailscale)/i.test(label) ? 20 : 0)
      - (address.startsWith("169.254.") ? 20 : 0);
  };
  return [...rows].sort((a, b) => score(b) - score(a))[0];
}

export function ReceptionLanControl({ mode, lan }: { mode: DeviceMode; lan: ReceptionLan }) {
  const admin = useContext(RoleContext) === "admin";
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState("");
  const [reviewNote, setReviewNote] = useState("");
  const [interfaces, setInterfaces] = useState<{ name: string; address: string }[]>([]);
  const rootRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (lan.openRequest) setOpen(true);
  }, [lan.openRequest]);

  useEffect(() => {
    if (!open) return;
    panelRef.current?.querySelector<HTMLButtonElement>("button")?.focus();
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") { setOpen(false); buttonRef.current?.focus(); }
    }
    function onPointer(event: PointerEvent) {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    }
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer);
    return () => { document.removeEventListener("keydown", onKey); document.removeEventListener("pointerdown", onPointer); };
  }, [open]);

  async function run(action: string, args: Record<string, unknown> = {}) {
    setBusy(true); setError("");
    try { await api.lanControl(action, args); await lan.refresh(); }
    catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  }

  async function enableLan() {
    setBusy(true); setError("");
    try {
      const rows = await api.lanControl<{ name: string; address: string }[]>("interfaces");
      setInterfaces(rows);
      if (!rows.length) { setError("Conectá esta PC al Wi-Fi o por cable y volvé a intentar."); return; }
      await api.lanControl("configure", { enabled: true, address: preferredInterface(rows).address, port: lan.status?.port ?? 17443 });
      await lan.refresh();
    } catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  }

  async function scan() {
    setScanning(true); setError("");
    try { await lan.scan(); } catch (cause) { setError(String(cause)); }
    finally { setScanning(false); }
  }

  if (mode === "remote") return null;
  const principal = mode === "reception";
  const status = lan.status;
  const connected = principal ? !!status?.running : !!status?.connection.connected;
  const lastSeen = status?.connection.last_seen_at ? Date.parse(status.connection.last_seen_at) : 0;
  const tone = !status || (principal && !status.enabled) ? "off" : connected ? "on"
    : !principal && lastSeen > 0 && Date.now() - lastSeen < 30000 ? "retry" : "lost";
  const label = tone === "off" ? "LAN sin configurar" : tone === "on"
    ? principal ? "Servicio LAN disponible" : "Principal conectada"
    : tone === "retry" ? "Reconectando con principal"
      : principal ? "Servicio LAN detenido" : "Sin conexión con principal";
  const discovered = lan.nearby.filter(station => !lan.info?.stations.some(saved => saved.id === station.station_id && saved.active));

  return <div className="lan-control-wrap" ref={rootRef}>
    <button type="button" ref={buttonRef} className={`lan-control is-${tone}`} aria-label={`Red local: ${label}`}
      aria-expanded={open} aria-controls="lan-control-panel" title={`Red local · ${label}`}
      onClick={() => setOpen(value => !value)}>
      <span className="lan-control-dot" aria-hidden="true" /><span>LAN</span>
      {lan.pending.length > 0 && <span className="lan-control-count" aria-label={`${lan.pending.length} operaciones sin confirmar`}>{lan.pending.length}</span>}
    </button>
    {open && <div id="lan-control-panel" ref={panelRef} className="lan-control-panel" role="dialog" aria-label="Estado y vinculación de red local">
      <div className="lan-control-panel-head"><div><p className="lan-control-eyebrow">Red local · {principal ? "Principal" : "Adicional"}</p><h2>Conexión entre recepciones</h2></div>
        <button type="button" className="lan-panel-close" aria-label="Cerrar panel LAN" onClick={() => { setOpen(false); buttonRef.current?.focus(); }}><X size={18} /></button>
      </div>
      <div className={`lan-panel-status is-${tone}`} role="status"><span className="lan-control-dot" aria-hidden="true" /><strong>{label}</strong></div>
      <div className="lan-panel-body">
        {principal ? <>
          {!admin ? <p className="text-sm text-[var(--muted)]">Un administrador de la principal puede habilitar y aprobar nuevos puestos.</p> : <>
            {!status?.running ? <div className="space-y-3"><p className="text-sm text-[var(--muted)]">Activá la red local para que la otra recepción encuentre esta PC automáticamente.</p>
              <Button disabled={busy} onClick={() => void enableLan()}>{busy ? "Activando…" : "Activar red local"}</Button>
              {interfaces.length > 1 && <p className="text-xs text-[var(--muted)]">Se eligió {status?.bind_address || preferredInterface(interfaces)?.address}. Podés cambiar la red en Puestos.</p>}
            </div> : <>
              <div className="reception-link-discovery"><div className="flex items-center justify-between gap-3"><strong>Recepciones encontradas</strong>
                <button type="button" className="reception-link-refresh" disabled={scanning} onClick={() => void scan()}><RefreshCw size={15} className={scanning ? "reception-link-spin" : ""} /> Buscar ahora</button></div>
                {discovered.length ? discovered.map(station => <div key={station.station_id} className="reception-link-peer"><span className="reception-link-peer-icon"><Wifi size={16} /></span><div><strong>{station.name}</strong><p>{station.address}</p></div></div>)
                  : <p className="text-sm text-[var(--muted)]">{scanning ? "Buscando recepciones…" : lan.info?.stations.some(station => station.active) ? "No hay puestos nuevos por vincular." : "Buscando otra recepción en esta red…"}</p>}
              </div>
              {!!lan.info?.stations.some(station => station.active) && <div className="lan-linked-list"><strong>Puestos vinculados</strong>
                {lan.info.stations.filter(station => station.active).map(station => <p key={station.id}>{station.name}</p>)}
                <small>La presencia en línea de estos puestos no se mide desde aquí.</small>
              </div>}
              <p className="text-sm text-[var(--muted)]">Compará la huella de seguridad en ambos equipos antes de aprobar una solicitud.</p>
              {lan.info?.identity && <details className="reception-link-details"><summary>Huella de seguridad de esta principal</summary><p className="break-all font-mono text-xs">{lan.info.identity.fingerprint}</p></details>}
              <Button disabled={busy} onClick={() => void run("pairing_open")}>{lan.info?.pairing_open ? "Renovar vinculación" : "Permitir vinculación por 2 minutos"}</Button>
              {lan.info?.pairing_open && <p className="text-xs text-[var(--ok)]">La principal está lista para recibir la solicitud.</p>}
              {lan.info?.pending.map(request => <div key={request.station_id} className="reception-link-request"><ShieldCheck size={20} /><div><strong>{request.name}</strong><p>Confirmá el código mostrado en la adicional:</p><code>{request.verification_code}</code></div><Button disabled={busy} onClick={() => void run("approve", { station_id: request.station_id })}>Aprobar</Button></div>)}
            </>}
            <Link className="reception-link-settings" to="/puestos" onClick={() => setOpen(false)}>Configuración avanzada y conexión manual <ArrowRight size={15} /></Link>
          </>}
        </> : <>
          <p className="text-sm text-[var(--muted)]">{connected ? `Vinculada con ${status?.host_name ?? "la principal"}.` : "La conexión se restablece automáticamente cuando vuelve la red local. Las operaciones nuevas están detenidas."}</p>
          {status?.connection.last_seen_at && !connected && <p className="text-xs text-[var(--muted)]">Última conexión: {new Date(status.connection.last_seen_at).toLocaleString("es-PY", { timeZone: "America/Asuncion" })}</p>}
          <Button variant="secondary" disabled={busy} onClick={() => void lan.refresh().catch(cause => setError(String(cause)))}>Comprobar conexión</Button>
          {!connected && <p className="text-xs text-[var(--muted)]">Si cambiaste de principal, cerrá sesión y elegí “Volver a vincular recepción”.</p>}
          {lan.pending.map(p => <div key={p.operation_id} className="lan-pending-operation"><strong>Operación sin confirmar</strong>
            <p>{p.command} · {p.username} · {new Date(p.created_at).toLocaleTimeString("es-PY")}</p>
            <Button size="sm" variant="secondary" disabled={!connected || busy || !p.can_retry} onClick={async () => { setBusy(true); setError(""); try { await lan.retry(p.operation_id); } catch (cause) { setError(String(cause)); } finally { setBusy(false); } }}>Consultar y reenviar la misma solicitud</Button>
            {!p.can_retry && <p>Revisá esta solicitud con su usuario original o con administración.</p>}
            {admin && <details><summary>Revisión manual de administración</summary><p>Verificá el historial, los consumos y la cuenta en la principal. Archivar no ejecuta la operación.</p>
              <Input aria-label="Resultado de la revisión" value={reviewNote} onChange={e => setReviewNote(e.target.value)} placeholder="Qué se verificó y cómo se resolvió" />
              <Button size="sm" variant="secondary" disabled={!connected || busy || reviewNote.trim().length < 12} onClick={async () => { setBusy(true); setError(""); try { await lan.review(p.operation_id, reviewNote); setReviewNote(""); } catch (cause) { setError(String(cause)); } finally { setBusy(false); } }}>Registrar revisión y archivar</Button>
            </details>}
          </div>)}
        </>}
        {error && <p role="alert" className="text-sm text-[var(--danger)]">{error}</p>}
      </div>
    </div>}
  </div>;
}
