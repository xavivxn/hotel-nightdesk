import { RoleContext } from "@/lib/permissions";
import { Input } from "@/components/ui/Field";
import { useContext, useEffect, useRef, useState } from "react";
import { api } from "@/lib/api";
import type { DeviceMode, LanStatus, PendingOperation } from "@/lib/types";
import { Button } from "@/components/ui/Button";

export function ReceptionConnectionStatus({ mode }: { mode: DeviceMode }) {
  const admin = useContext(RoleContext) === "admin";
  const [reviewNote, setReviewNote] = useState("");
  const [status, setStatus] = useState<LanStatus | null>(null);
  const [pending, setPending] = useState<PendingOperation[]>([]);
  const [error, setError] = useState("");
  const [retrying, setRetrying] = useState(false);
  const revision = useRef("");
  useEffect(() => {
    if (mode === "remote") return;
    let active = true;
    async function load() {
      try {
        const r = await api.receptionRevision();
        const key = `${r.epoch}:${r.revision}`;
        if (key !== revision.current) { revision.current = key; window.dispatchEvent(new Event("reception:changed")); }
        if (mode === "reception_client") { const rows = await api.pendingOperations(); if (active) setPending(rows); }
      } catch { /* Keep the last visible data and show the separate LAN indicator. */ }
      try { const next = await api.lanStatus(); if (active) setStatus(next); } catch { /* local status can be retried */ }
    }
    void load(); const timer = window.setInterval(() => void load(), 5000);
    return () => { active = false; window.clearInterval(timer); };
  }, [mode]);
  if (mode === "remote" || (mode === "reception" && !status?.enabled)) return null;
  const connected = mode === "reception" ? status?.running : status?.connection.connected;
  return <div className="space-y-2 border-b border-[var(--line)] bg-[var(--surface-2)] px-6 py-2 text-sm" role="status">
    <p className={connected ? "text-[var(--ok)]" : "text-[var(--danger)]"}>
      {mode === "reception" ? "Recepción principal" : "Conexión con recepción principal"}: {connected ? "activa" : "sin conexión"}
      {!connected && mode === "reception_client" && " · Nuevas operaciones detenidas. Los datos visibles pueden estar desactualizados."}
    </p>
    {status?.connection.last_seen_at && !connected && <p className="text-[var(--muted)]">Última conexión: {new Date(status.connection.last_seen_at).toLocaleString("es-PY", { timeZone: "America/Asuncion" })}</p>}
    {pending.map(p => <div key={p.operation_id} className="flex flex-wrap items-center gap-3">
      <p>Operación de {p.username} enviada a las {new Date(p.created_at).toLocaleTimeString()}: {p.command}. Su resultado no está confirmado.</p>
      <Button size="sm" variant="secondary" disabled={!connected || retrying || !p.can_retry} onClick={async () => {
        setRetrying(true); setError(""); try { setPending(await api.retryOperation(p.operation_id)); } catch (e) { setError(String(e)); } finally { setRetrying(false); }
      }}>Consultar y reenviar la misma solicitud</Button>
      {!p.can_retry && <p>Debe revisarse con su usuario original o con administración si cambió la base principal.</p>}
      {admin && <details className="w-full"><summary>Revisión manual de administración</summary><p className="my-2">Verificá el historial, los consumos y la cuenta en la principal. Archivar esta solicitud permite continuar y no ejecuta ninguna operación.</p><Input aria-label="Resultado de la revisión" value={reviewNote} onChange={e => setReviewNote(e.target.value)} placeholder="Qué se verificó y cómo se resolvió" /><Button size="sm" variant="secondary" disabled={!connected || retrying || reviewNote.trim().length < 12} onClick={async () => { setRetrying(true); try { await api.lanControl("review_pending", { operation_id: p.operation_id, note: reviewNote }); setPending(await api.pendingOperations()); setReviewNote(""); } catch (e) { setError(String(e)); } finally { setRetrying(false); } }}>Registrar revisión y archivar solicitud</Button></details>}
    </div>)}
    {error && <p className="text-[var(--danger)]">{error}</p>}
  </div>;
}
