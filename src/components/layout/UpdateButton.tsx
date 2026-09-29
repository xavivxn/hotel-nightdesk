import { Download, LoaderCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import { toApiError } from "@/lib/errors";
import type { AppUpdateInfo, AppUpdateProgress } from "@/lib/types";

/** Short wait so sync and backups start first. */
const FIRST_CHECK_MS = 5_000;
const CHECK_EVERY_MS = 4 * 60 * 60 * 1000;

type Phase = "idle" | "downloading" | "installing" | "error";

/**
 * Board header button. Hidden until a newer version is published in the `updates` bucket;
 * one click downloads, verifies and installs it, and the app reopens by itself.
 */
export function UpdateButton() {
  const [update, setUpdate] = useState<AppUpdateInfo | null>(null);
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

  if (!update) return null;

  const percent =
    progress?.total && progress.total > 0
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;
  const label =
    phase === "installing"
      ? "Instalando…"
      : phase === "downloading"
        ? `Descargando${percent !== null ? ` ${percent} %` : "…"}`
        : phase === "error"
          ? "Reintentar actualización"
          : "Actualizar";

  return (
    <div className="flex flex-col items-end gap-1" aria-live="polite">
      <Button
        onClick={install}
        disabled={busy}
        title={`Versión ${update.current_version} → ${update.version}. La app se cierra, se actualiza y vuelve a abrir sola.`}
      >
        {busy ? <LoaderCircle size={16} aria-hidden="true" className="motion-safe:animate-spin" /> : <Download size={16} aria-hidden="true" />}
        {label}
        {phase === "idle" || phase === "error" ? (
          <span className="font-mono text-xs font-medium opacity-80">{update.version}</span>
        ) : null}
      </Button>
      {phase === "installing" ? (
        <p className="text-xs text-[var(--muted)]">La app se cierra y vuelve a abrir sola.</p>
      ) : null}
      {error ? <p role="alert" className="max-w-xs text-right text-xs text-[var(--danger)]">No se pudo actualizar: {error}</p> : null}
    </div>
  );
}
