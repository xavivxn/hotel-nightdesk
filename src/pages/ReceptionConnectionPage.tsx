import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { LanHost, LanStatus } from "@/lib/types";
import { Button } from "@/components/ui/Button";
import { Field, Input, Select, Textarea } from "@/components/ui/Field";

type AdminInfo = {
  identity: LanHost | null; pairing_open: boolean;
  pending: { station_id: string; name: string; verification_code: string }[];
  stations: { id: string; name: string; active: boolean }[];
};

export function ReceptionPairingPage({ onPaired }: { onPaired: () => void }) {
  const [hosts, setHosts] = useState<LanHost[]>([]);
  const [chosen, setChosen] = useState<LanHost | null>(null);
  const [manual, setManual] = useState("");
  const [name, setName] = useState("Recepción adicional");
  const [verified, setVerified] = useState(false);
  const [pending, setPending] = useState(false);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function search() {
    setBusy(true); setError("");
    try { setHosts(await api.discoverReception()); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  useEffect(() => { void search(); }, []);
  useEffect(() => {
    if (!pending) return;
    const timer = window.setInterval(() => {
      void api.lanControl("pair_status").then(onPaired).catch(() => undefined);
    }, 2000);
    return () => window.clearInterval(timer);
  }, [pending, onPaired]);

  return <div className="h-full overflow-auto px-6 py-8"><div className="mx-auto max-w-2xl space-y-5">
    <header><p className="page-kicker">Recepción adicional</p><h1 className="page-title">Conectar con la principal</h1>
      <p className="mt-2 text-[var(--muted)]">Conectá ambas PCs a la misma red. En la principal, un administrador debe abrir Puestos y habilitar la vinculación.</p></header>
    <Field label="Nombre de este puesto"><Input value={name} maxLength={64} onChange={e => setName(e.target.value)} /></Field>
    <Button variant="secondary" disabled={busy || pending} onClick={() => void search()}>{busy ? "Buscando…" : "Buscar recepción"}</Button>
    {hosts.map(host => <button key={host.station_id} className="card block w-full rounded-lg p-4 text-left" onClick={() => { setChosen(host); setVerified(false); }}>
      <strong>{host.name}</strong><span className="ml-3 font-mono text-sm text-[var(--muted)]">{host.address}</span>
    </button>)}
    {!busy && hosts.length === 0 && <p className="text-sm text-[var(--muted)]">No se encontró una recepción. Revisá el firewall o usá los datos de conexión manual.</p>}
    <details className="card rounded-lg p-4"><summary>Conexión manual</summary>
      <p className="my-3 text-sm text-[var(--muted)]">Copiá los datos que muestra Puestos en la principal. Podés ajustar la dirección si cambió.</p>
      <Textarea aria-label="Datos de conexión" value={manual} onChange={e => setManual(e.target.value)} />
      <Button variant="secondary" className="mt-3" onClick={() => { try { const value = JSON.parse(manual) as LanHost; if (!value || typeof value.certificate !== "string" || typeof value.fingerprint !== "string" || typeof value.station_id !== "string" || !Number.isInteger(value.port)) throw new Error(); setChosen(value); setVerified(false); } catch { setError("Los datos de conexión no son válidos."); } }}>Usar estos datos</Button>
    </details>
    {chosen && <section className="card space-y-3 rounded-lg p-5">
      <h2 className="text-lg font-semibold">Verificar recepción</h2>
      <Field label="Dirección local"><Input value={chosen.address ?? ""} onChange={e => setChosen({ ...chosen, address: e.target.value })} /></Field>
      <p className="text-sm text-[var(--muted)]">Compará esta huella completa con la que aparece en la PC principal:</p>
      <p className="break-all font-mono text-sm">{chosen.fingerprint}</p>
      <label className="flex items-center gap-3 text-sm"><input type="checkbox" checked={verified} onChange={e => setVerified(e.target.checked)} />Las huellas coinciden en ambas PCs</label>
      <Button disabled={!verified || busy || pending} onClick={async () => {
        setBusy(true); setError("");
        try { const result = await api.lanControl<{ verification_code: string }>("pair", { host: chosen, name }); setCode(result.verification_code); setPending(true); }
        catch (e) { setError(String(e)); } finally { setBusy(false); }
      }}>Solicitar vinculación</Button>
    </section>}
    {pending && <div className="rounded-lg border border-[var(--info)] bg-[var(--info-soft)] p-4">
      <p>En la principal, aprobá únicamente la solicitud con este código:</p><p className="my-2 font-mono text-2xl">{code}</p>
      <p className="text-sm">La solicitud vence a los dos minutos.</p>
      <Button variant="secondary" className="mt-3" onClick={() => { setPending(false); setError(""); }}>Volver a intentar</Button>
    </div>}
    {error && <p role="alert" className="text-[var(--danger)]">{error}</p>}
  </div></div>;
}

export function ReceptionConnectionPage() {
  const [status, setStatus] = useState<LanStatus | null>(null);
  const [info, setInfo] = useState<AdminInfo | null>(null);
  const [interfaces, setInterfaces] = useState<{ name: string; address: string }[]>([]);
  const [address, setAddress] = useState("");
  const [port, setPort] = useState("17443");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function refresh() { const [s, i] = await Promise.all([api.lanStatus(), api.lanControl<AdminInfo>("admin_info")]); setStatus(s); setInfo(i); }
  useEffect(() => {
    void Promise.all([api.lanControl<{ name: string; address: string }[]>("interfaces"), api.lanStatus(), api.lanControl<AdminInfo>("admin_info")])
      .then(([rows, s, i]) => {
        setInterfaces(rows); setStatus(s); setInfo(i);
        setAddress(s.bind_address || rows[0]?.address || "");
        setPort(String(s.port));
      }).catch(e => setError(String(e)));
    const timer = window.setInterval(() => void refresh().catch(e => setError(String(e))), 3000);
    return () => window.clearInterval(timer);
  }, []);
  async function run(action: string, args: Record<string, unknown> = {}) {
    setBusy(true); setError("");
    try { await api.lanControl(action, args); await refresh(); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  return <div className="px-6 py-6 lg:px-8"><div className="max-w-3xl space-y-5">
    <header><p className="page-kicker">Red del establecimiento</p><h1 className="page-title">Puestos de recepción</h1>
      <p className="mt-2 text-[var(--muted)]">Esta PC conserva los datos. Mantenela encendida y con Nightdesk activo para que trabaje la recepción adicional.</p></header>
    <section className="card space-y-4 rounded-lg p-5">
      <p className="font-semibold">{status?.running ? "Conexión local activa" : "Conexión local detenida"}</p>
      <Field label="Interfaz de red"><Select value={address} onChange={e => setAddress(e.target.value)}>{interfaces.map(i => <option key={`${i.name}-${i.address}`} value={i.address}>{i.name} · {i.address}</option>)}</Select></Field>
      <Field label="Puerto"><Input inputMode="numeric" value={port} onChange={e => setPort(e.target.value)} /></Field>
      <div className="flex flex-wrap gap-3">
        <Button disabled={busy || !address} onClick={() => void run("configure", { enabled: true, address, port: Number(port) })}>Habilitar conexión local</Button>
        <Button variant="secondary" disabled={busy || !status?.enabled} onClick={() => void run("configure", { enabled: false, address, port: Number(port) })}>Detener conexión</Button>
        <Button variant="secondary" disabled={busy || !status?.enabled} onClick={() => void run("firewall")}>Configurar firewall de Windows</Button>
      </div>
      <p className="text-sm text-[var(--muted)]">El asistente de firewall solicita permiso de Windows y permite conexiones desde la subred local en el perfil privado.</p>
    </section>
    {info?.identity && <section className="card space-y-3 rounded-lg p-5">
      <h2 className="text-lg font-semibold">Vincular un puesto</h2>
      <p className="break-all font-mono text-sm">{info.identity.fingerprint}</p>
      <Button disabled={busy || !status?.running} onClick={() => void run("pairing_open")}>{info.pairing_open ? "Renovar los dos minutos de vinculación" : "Permitir vinculación durante dos minutos"}</Button>
      <details><summary className="cursor-pointer text-sm">Datos para conexión manual</summary><Textarea className="mt-3 font-mono text-xs" readOnly value={JSON.stringify(info.identity)} /></details>
      {info.pending.map(p => <div key={p.station_id} className="rounded-lg border border-[var(--info)] p-4">
        <p>{p.name} · <span className="font-mono">{p.verification_code}</span></p>
        <p className="my-2 text-sm text-[var(--muted)]">Aprobá solo si el código coincide con el de la PC adicional.</p>
        <Button disabled={busy} onClick={() => void run("approve", { station_id: p.station_id })}>Aprobar este puesto</Button>
      </div>)}
    </section>}
    <section className="card rounded-lg p-5"><h2 className="mb-3 text-lg font-semibold">Puestos vinculados</h2>
      {info?.stations.filter(s => s.active).map(s => <div className="flex items-center justify-between gap-4 border-t border-[var(--line)] py-3" key={s.id}><span>{s.name}</span><Button variant="danger" disabled={busy} onClick={() => void run("revoke", { station_id: s.id })}>Revocar acceso</Button></div>)}
      {!info?.stations.some(s => s.active) && <p className="text-sm text-[var(--muted)]">Todavía no hay puestos vinculados.</p>}
    </section>
    {(error || status?.connection.last_error) && <p role="alert" className="text-[var(--danger)]">{error || status?.connection.last_error}</p>}
  </div></div>;
}
