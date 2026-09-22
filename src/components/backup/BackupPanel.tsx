import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { api } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import type { BackupListItem, BackupStatus, DeviceMode } from "@/lib/types";
import { BookOpen, Cloud, HardDrive, RefreshCw, RotateCcw, ShieldAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

const MAX_REMOTE_AGE_MS = 24 * 60 * 60 * 1000;

function lastCopy(value: string | null | undefined) {
  return value ? formatDateTime(value) : "Todavía no hay una copia confirmada";
}

function remoteIsStale(value: string | null | undefined) {
  if (!value) return true;
  const time = Date.parse(value);
  return !Number.isFinite(time) || Date.now() - time > MAX_REMOTE_AGE_MS;
}

function statusLabel(status: string) {
  switch (status) {
    case "pending_upload":
      return "Pendiente de subir";
    case "uploaded":
      return "Subida confirmada";
    case "local_ready":
      return "Lista local";
    case "failed":
      return "Falló la subida";
    default:
      return status;
  }
}

export function BackupPanel({ deviceMode }: { deviceMode: DeviceMode }) {
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [guideOpen, setGuideOpen] = useState(false);
  const [restoreOpen, setRestoreOpen] = useState(false);
  const [items, setItems] = useState<BackupListItem[]>([]);
  const [listLoading, setListLoading] = useState(false);
  const [restoringId, setRestoringId] = useState<string | null>(null);
  const [confirmId, setConfirmId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await api.backupStatus();
      setStatus(next);
      setError(null);
    } catch (cause) {
      setStatus(null);
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 60_000);
    window.addEventListener("focus", refresh);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", refresh);
    };
  }, [refresh]);

  const stale = status ? remoteIsStale(status.last_remote_at) : false;
  const local = deviceMode === "reception";

  async function runBackup() {
    if (!local || !status?.ready || running) return;
    setRunning(true);
    setError(null);
    setNotice(null);
    try {
      const result = await api.backupRunNow();
      setNotice(`Respaldo local listo (${result.backup_id.slice(0, 8)}…). La copia remota se confirma después de la subida.`);
      await refresh();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setRunning(false);
    }
  }

  async function openRestore() {
    if (!local || !status?.ready) return;
    setRestoreOpen(true);
    setConfirmId(null);
    setListLoading(true);
    setError(null);
    try {
      setItems(await api.backupList());
    } catch (cause) {
      setError(String(cause));
      setItems([]);
    } finally {
      setListLoading(false);
    }
  }

  async function restoreBackup(backupId: string) {
    if (!local || restoringId) return;
    setRestoringId(backupId);
    setError(null);
    setNotice(null);
    try {
      await api.backupRestore(backupId);
      setNotice("Restauración aplicada. Volvé a iniciar sesión y verificá habitaciones, cuentas e historial.");
      setRestoreOpen(false);
      setConfirmId(null);
      await refresh();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setRestoringId(null);
    }
  }

  return (
    <section className="card space-y-5 rounded-lg p-5 lg:col-span-2" aria-labelledby="backup-heading">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="page-kicker">Protección de datos</p>
          <h2 id="backup-heading" className="text-lg font-semibold tracking-tight">Respaldos y recuperación</h2>
          <p className="mt-1 text-sm text-[var(--muted)]">
            {local ? "Esta PC conserva la base operativa. La copia remota se envía a Supabase Storage." : "Consulta de las copias remotas confirmadas en Supabase."}
          </p>
        </div>
        <Button variant="secondary" size="sm" onClick={() => void refresh()} disabled={loading || running}>
          <RefreshCw size={15} aria-hidden="true" /> Actualizar
        </Button>
      </div>

      {stale && (
        <div className="rounded-lg border border-[var(--warn)] bg-[var(--warn-soft)] px-4 py-3 text-sm text-[var(--ink)]" role="alert">
          <div className="flex items-start gap-2">
            <ShieldAlert size={18} className="mt-0.5 shrink-0" aria-hidden="true" />
            <p>{status?.last_remote_at
              ? "Pasaron más de 24 horas desde la última copia remota confirmada. Revisá el estado y la conexión."
              : "Todavía no hay una copia remota confirmada. Revisá la configuración de respaldos."}</p>
          </div>
        </div>
      )}

      <div className="grid gap-3 sm:grid-cols-2">
        {local && (
          <div className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4">
            <div className="mb-2 flex items-center gap-2 text-sm font-semibold"><HardDrive size={17} aria-hidden="true" /> Última copia local</div>
            <p className="text-sm">{loading ? "Cargando…" : lastCopy(status?.last_local_at)}</p>
            <p className="mt-1 text-xs text-[var(--muted)]">Snapshot consistente en esta PC.</p>
          </div>
        )}
        <div className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4">
          <div className="mb-2 flex items-center gap-2 text-sm font-semibold"><Cloud size={17} aria-hidden="true" /> Última copia remota confirmada</div>
          <p className="text-sm">{loading ? "Cargando…" : lastCopy(status?.last_remote_at)}</p>
          <p className="mt-1 text-xs text-[var(--muted)]">Solo cuenta cuando Storage y el manifiesto confirman la copia.</p>
        </div>
      </div>

      {local && (
        <p className="text-sm">
          Pendientes de subir: <strong>{status?.ready ? status.pending : "—"}</strong>
          {!status?.ready && <span className="ml-2 text-[var(--muted)]">El motor de respaldos todavía no informa la cola.</span>}
        </p>
      )}
      {status?.last_error && <p className="text-sm text-[var(--danger)]" role="alert">Último problema: {status.last_error}</p>}
      {error && <p className="text-sm text-[var(--danger)]" role="alert">{error}</p>}
      {notice && <p className="text-sm text-[var(--ok)]" role="status">{notice}</p>}

      <div className="flex flex-wrap gap-2 border-t border-[var(--line)] pt-4">
        {local && (
          <Button onClick={() => void runBackup()} disabled={!status?.ready || running} title={!status?.ready ? "Motor de respaldos no listo" : undefined}>
            <HardDrive size={16} aria-hidden="true" /> {running ? "Solicitando…" : "Respaldar ahora"}
          </Button>
        )}
        {local && status?.ready && (
          <Button variant="danger" onClick={() => void openRestore()} disabled={running || Boolean(restoringId)}>
            <RotateCcw size={16} aria-hidden="true" /> Restaurar copia
          </Button>
        )}
        <Button variant="secondary" onClick={() => setGuideOpen(true)}>
          <BookOpen size={16} aria-hidden="true" /> Guía de recuperación
        </Button>
      </div>
      {local && !status?.ready && (
        <p className="text-xs text-[var(--muted)]">La acción de respaldo se habilita cuando el motor local tiene la clave de cifrado y la cola operativa.</p>
      )}

      <Dialog open={guideOpen} title="Recuperar una copia" subtitle="Solo administración, en el equipo de recepción" onClose={() => setGuideOpen(false)}>
        <ol className="list-decimal space-y-3 pl-5 text-sm leading-relaxed">
          <li>Detené la operación y avisá al personal. No hagas ingresos, cierres ni cambios mientras se restaura.</li>
          <li>Elegí una copia cuya fecha y origen reconozcas. Conservá la clave de cifrado fuera de esta PC.</li>
          <li>Usá «Restaurar copia» en esta pantalla: el sistema valida integridad y checksum antes de reemplazar la base.</li>
          <li>Volvé a iniciar sesión. Comprobá habitaciones, cuentas, historial y tickets.</li>
          <li>La sincronización se reinicia sin reenviar la outbox antigua.</li>
        </ol>
        <p className="mt-4 rounded-lg bg-[var(--warn-soft)] p-3 text-sm">
          {local
            ? "No reemplaces manualmente el archivo SQLite mientras la aplicación está abierta."
            : "Para restaurar, usá el equipo de recepción con una cuenta administradora. Desde esta PC solo se consulta el estado."}
        </p>
        <Button variant="secondary" className="mt-4 w-full" onClick={() => setGuideOpen(false)}>Entendido</Button>
      </Dialog>

      <Dialog
        open={restoreOpen}
        title="Restaurar una copia local"
        subtitle="Reemplaza la base operativa de esta PC. Acción irreversible sin otra copia previa."
        onClose={() => {
          if (restoringId) return;
          setRestoreOpen(false);
          setConfirmId(null);
        }}
        size="lg"
        dismissible={!restoringId}
      >
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <div className="overflow-y-auto p-6 scrollbar-thin">
            {listLoading && <p className="text-sm text-[var(--muted)]">Cargando copias…</p>}
            {!listLoading && items.length === 0 && (
              <p className="text-sm text-[var(--muted)]">Todavía no hay copias locales en la cola.</p>
            )}
            <ul className="space-y-3">
              {items.map((item) => (
                <li key={item.backup_id} className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div>
                      <p className="font-mono text-sm tabular-nums">{formatDateTime(item.created_at)}</p>
                      <p className="mt-1 text-xs text-[var(--muted)]">
                        {statusLabel(item.status)} · esquema {item.schema_version} · {item.size_bytes.toLocaleString("es-PY")} bytes
                      </p>
                      <p className="mt-1 font-mono text-[11px] text-[var(--muted)]">{item.backup_id}</p>
                    </div>
                    {confirmId === item.backup_id ? (
                      <div className="flex flex-wrap gap-2">
                        <Button
                          variant="danger"
                          size="sm"
                          disabled={Boolean(restoringId)}
                          onClick={() => void restoreBackup(item.backup_id)}
                        >
                          {restoringId === item.backup_id ? "Restaurando…" : "Confirmar restauración"}
                        </Button>
                        <Button variant="secondary" size="sm" disabled={Boolean(restoringId)} onClick={() => setConfirmId(null)}>
                          Cancelar
                        </Button>
                      </div>
                    ) : (
                      <Button variant="danger" size="sm" disabled={Boolean(restoringId)} onClick={() => setConfirmId(item.backup_id)}>
                        Restaurar
                      </Button>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </Dialog>
    </section>
  );
}
