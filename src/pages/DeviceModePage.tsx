import { useState } from "react";
import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import type { DeviceMode } from "@/lib/types";
import { cn } from "@/lib/utils";

export function DeviceModePage({ onChosen }: { onChosen: (mode: DeviceMode) => void }) {
  const [mode, setMode] = useState<DeviceMode | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  return (
    <div className="grid h-full min-h-0 place-items-center overflow-auto bg-[var(--bg)] p-6">
      <div className="w-full max-w-md space-y-5">
        <header className="text-center">
          <h1 className="page-title">¿Cómo se usa esta PC?</h1>
          <p className="mt-2 text-sm text-[var(--muted)]">Esta elección queda en el equipo. No se sincroniza.</p>
        </header>
        <div className="grid gap-3">
          <button
            type="button"
            onClick={() => setMode("reception")}
            className={cn(
              "rounded-lg border p-4 text-left transition",
              mode === "reception"
                ? "border-[var(--accent)] bg-[var(--accent-soft)]"
                : "border-[var(--line)] bg-[var(--surface)] hover:bg-[var(--surface-2)]",
            )}
          >
            <strong className="block text-[var(--ink)]">Recepción</strong>
            <span className="mt-1 block text-sm text-[var(--muted)]">
              Opera el motel offline (SQLite, impresora, login local).
            </span>
          </button>
          <button
            type="button"
            onClick={() => setMode("remote")}
            className={cn(
              "rounded-lg border p-4 text-left transition",
              mode === "remote"
                ? "border-[var(--accent)] bg-[var(--accent-soft)]"
                : "border-[var(--line)] bg-[var(--surface)] hover:bg-[var(--surface-2)]",
            )}
          >
            <strong className="block text-[var(--ink)]">Administración remota</strong>
            <span className="mt-1 block text-sm text-[var(--muted)]">
              Consulta y catálogo vía Supabase. Requiere internet.
            </span>
          </button>
        </div>
        {error && <p role="alert" className="text-sm text-[var(--danger)]">{error}</p>}
        <Button
          className="w-full"
          disabled={!mode || busy}
          onClick={async () => {
            if (!mode) return;
            setBusy(true);
            setError(null);
            try {
              await api.deviceModeSet(mode);
              onChosen(mode);
            } catch (e) {
              setError(String(e));
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? "Guardando…" : "Continuar"}
        </Button>
      </div>
    </div>
  );
}
