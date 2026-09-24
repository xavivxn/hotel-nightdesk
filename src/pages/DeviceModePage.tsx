import { useState } from "react";
import { LoveNestLogoLayered } from "@/components/layout/BrandLogo";
import { DutyClock } from "@/components/layout/DutyClock";
import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import type { DeviceMode } from "@/lib/types";
import { cn } from "@/lib/utils";

export function DeviceModePage({ onChosen }: { onChosen: (mode: DeviceMode) => void }) {
  const [mode, setMode] = useState<DeviceMode | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function confirm() {
    if (!mode) return;
    setBusy(true);
    setError(null);
    try {
      await api.deviceModeSet(mode);
      onChosen(mode);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="login-stage login-stage--split h-full min-h-0">
      <aside className="login-brand">
        <div className="login-brand-bg" aria-hidden="true" />
        <div className="login-logo-slot">
          <div className="login-logo-glow" />
          <LoveNestLogoLayered className="login-logo" />
        </div>
        <div className="login-brand-clock">
          <DutyClock />
        </div>
        <p className="login-brand-foot font-mono tabular-nums">
          v{__APP_VERSION__} · Primer arranque
        </p>
      </aside>

      <section className="login-form-panel">
        <div className="login-card w-full space-y-4">
          <div>
            <p className="page-kicker">Primer arranque</p>
            <h1 className="page-title">¿Cómo se usa esta PC?</h1>
            <p className="mt-2 text-sm text-[var(--muted)]">Esta elección queda en el equipo.</p>
          </div>

          <div className="grid gap-3">
            <button
              type="button"
              onClick={() => setMode("reception")}
              className={cn(
                "rounded-lg border p-4 text-left",
                mode === "reception"
                  ? "border-[var(--accent)] bg-[var(--accent-soft)]"
                  : "border-[var(--line)] bg-[var(--surface)] hover:bg-[var(--surface-2)]",
              )}
            >
              <strong className="block text-[var(--ink)]">Recepción</strong>
              <span className="mt-1 block text-sm text-[var(--muted)]">
                Opera el motel en esta PC: tablero, cuentas e impresión.
              </span>
            </button>
            <button
              type="button"
              onClick={() => setMode("remote")}
              className={cn(
                "rounded-lg border p-4 text-left",
                mode === "remote"
                  ? "border-[var(--accent)] bg-[var(--accent-soft)]"
                  : "border-[var(--line)] bg-[var(--surface)] hover:bg-[var(--surface-2)]",
              )}
            >
              <strong className="block text-[var(--ink)]">Administración remota</strong>
              <span className="mt-1 block text-sm text-[var(--muted)]">
                Consulta y catálogo a distancia. Requiere internet.
              </span>
            </button>
          </div>

          {error ? (
            <p role="alert" className="login-error text-sm text-[var(--danger)]">
              {error}
            </p>
          ) : null}

          <Button className="login-submit w-full" disabled={!mode || busy} onClick={() => void confirm()}>
            {busy ? "Guardando…" : "Continuar"}
          </Button>
        </div>
      </section>
    </div>
  );
}
