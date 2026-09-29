import { Download, LoaderCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { api } from "@/lib/api";
import { toApiError } from "@/lib/errors";
import type { AppUpdateInfo, AppUpdateProgress } from "@/lib/types";

/** Deja arrancar sync y respaldos antes de la primera consulta. */
const FIRST_CHECK_MS = 20_000;
const CHECK_EVERY_MS = 4 * 60 * 60 * 1000;

type Phase = "idle" | "downloading" | "installing" | "error";

function formatMb(bytes: number) {
  return `${(bytes / 1024 / 1024).toLocaleString("es-AR", { maximumFractionDigits: 1 })} MB`;
}

/** Botón de la barra de título: aparece solo cuando hay una versión nueva publicada. */
export function UpdateButton() {
  const [update, setUpdate] = useState<AppUpdateInfo | null>(null);
  const [open, setOpen] = useState(false);
  const [phase, setPhase] = useState<Phase>("idle");
  const [progress, setProgress] = useState<AppUpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = phase === "downloading" || phase === "installing";
  const busyRef = useRef(false);
  busyRef.current = busy;

  const check = useCallback(async () => {
    if (busyRef.current) return;
    try {
      setUpdate(await api.updateCheck());
    } catch {
      // Sin internet o sin versión publicada: silencio; el próximo intento vuelve a consultar.
    }
  }, []);

  useEffect(() => {
    const first = window.setTimeout(check, FIRST_CHECK_MS);
    const every = window.setInterval(check, CHECK_EVERY_MS);
    window.addEventListener("online", check);
    return () => {
      window.clearTimeout(first);
      window.clearInterval(every);
      window.removeEventListener("online", check);
    };
  }, [check]);

  async function install() {
    setError(null);
    setProgress(null);
    setPhase("downloading");
    try {
      await api.updateInstall((next) => {
        setProgress(next);
        if (next.stage === "installing") setPhase("installing");
      });
      // En Windows el instalador cierra la app antes de llegar acá.
    } catch (err) {
      setPhase("error");
      setError(toApiError(err).message);
    }
  }

  function close() {
    if (busy) return;
    setOpen(false);
    if (phase === "error") setPhase("idle");
  }

  if (!update) return null;

  const percent =
    progress?.total && progress.total > 0
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;

  return (
    <>
      <button
        type="button"
        data-tauri-drag-region="false"
        title={`Versión ${update.version} disponible`}
        onClick={() => setOpen(true)}
        className="flex h-full shrink-0 cursor-pointer items-center gap-2 border-l border-[var(--line)] bg-[var(--accent-soft)] px-4 text-[12px] font-semibold text-[var(--accent)] hover:bg-[var(--surface-2)]"
      >
        <Download size={14} aria-hidden="true" />
        Actualizar
        <span className="font-mono text-[11px] font-medium opacity-80">{update.version}</span>
      </button>

      <Dialog
        open={open}
        title="Actualizar la app"
        subtitle={`Versión ${update.current_version} → ${update.version}`}
        onClose={close}
        dismissible={!busy}
      >
        {phase === "idle" || phase === "error" ? (
          <div className="space-y-4 text-sm">
            <p>
              La app se va a cerrar, instalar la versión nueva y volver a abrir sola. Tarda alrededor de un
              minuto y los datos de recepción no se tocan.
            </p>
            <p className="text-[var(--muted)]">Hacelo cuando no estés cobrando ni haciendo un check-in.</p>
            {update.notes ? (
              <div className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] px-4 py-3">
                <p className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-[var(--muted)]">Novedades</p>
                <p className="whitespace-pre-line">{update.notes}</p>
              </div>
            ) : null}
            {error ? (
              <p role="alert" className="rounded-lg bg-[var(--danger-soft)] px-4 py-3 text-[var(--danger)]">
                No se pudo actualizar: {error}
              </p>
            ) : null}
            <div className="flex justify-end gap-2 pt-2">
              <Button variant="secondary" onClick={close}>
                Más tarde
              </Button>
              <Button onClick={install}>
                <Download size={16} aria-hidden="true" />
                {phase === "error" ? "Reintentar" : "Actualizar ahora"}
              </Button>
            </div>
          </div>
        ) : (
          <div className="space-y-3 text-sm" aria-live="polite">
            <div className="flex items-center gap-2 font-semibold">
              <LoaderCircle size={16} aria-hidden="true" className="motion-safe:animate-spin" />
              {phase === "installing" ? "Instalando…" : "Descargando…"}
              {phase === "downloading" ? (
                <span className="ml-auto font-mono font-medium text-[var(--muted)]">
                  {percent !== null ? `${percent} %` : progress ? formatMb(progress.downloaded) : ""}
                </span>
              ) : null}
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-[var(--surface-2)]">
              <div
                className="h-full rounded-full bg-[var(--accent)] transition-[width] duration-200"
                style={{ width: `${phase === "installing" ? 100 : (percent ?? 0)}%` }}
              />
            </div>
            <p className="text-[var(--muted)]">
              {phase === "installing"
                ? "La app se va a cerrar y volver a abrir sola. No apagues la PC."
                : "No cierres la app mientras descarga."}
            </p>
          </div>
        )}
      </Dialog>
    </>
  );
}
