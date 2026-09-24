import { useRef, useState, type FormEvent } from "react";
import { LoveNestLogoLayered } from "@/components/layout/BrandLogo";
import { DutyClock } from "@/components/layout/DutyClock";
import { Button } from "@/components/ui/Button";
import { Field, Input, PasswordInput, reportInputIssue } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { FIELD_EMPTY, requireTrimmed } from "@/lib/format";

export function RemoteConfigPage({ onConfigured }: { onConfigured: () => void }) {
  const [projectUrl, setProjectUrl] = useState("");
  const [anonKey, setAnonKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const urlRef = useRef<HTMLInputElement>(null);
  const keyRef = useRef<HTMLInputElement>(null);

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    const url = requireTrimmed(projectUrl);
    const key = requireTrimmed(anonKey);
    if (!url) {
      reportInputIssue(urlRef.current, FIELD_EMPTY);
      return;
    }
    if (!key) {
      reportInputIssue(keyRef.current, FIELD_EMPTY);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.remoteConfigure({ project_url: url, anon_key: key });
      setAnonKey("");
      onConfigured();
    } catch (err) {
      setError(String(err));
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
          v{__APP_VERSION__} · Administración remota
        </p>
      </aside>

      <section className="login-form-panel">
        <form className="login-card w-full space-y-4" onSubmit={(e) => void handleSubmit(e)}>
          <div>
            <p className="page-kicker">Administración remota</p>
            <h1 className="page-title">Conectar Supabase</h1>
            <p className="mt-2 text-sm text-[var(--muted)]">
              URL y clave anónima se guardan en este equipo. No van al repositorio ni a ajustes visibles.
            </p>
          </div>
          <Field label="URL del proyecto">
            <Input
              ref={urlRef}
              required
              autoFocus
              value={projectUrl}
              onChange={(e) => setProjectUrl(e.target.value)}
              placeholder="https://xxxx.supabase.co"
            />
          </Field>
          <Field label="Clave anónima (anon)">
            <PasswordInput
              ref={keyRef}
              required
              show={showKey}
              onToggle={() => setShowKey((v) => !v)}
              value={anonKey}
              onChange={(e) => setAnonKey(e.target.value)}
            />
          </Field>
          {error ? (
            <p role="alert" className="login-error text-sm text-[var(--danger)]">
              {error}
            </p>
          ) : null}
          <Button className="login-submit w-full" disabled={busy} type="submit">
            {busy ? "Guardando…" : "Guardar y continuar"}
          </Button>
        </form>
      </section>
    </div>
  );
}
