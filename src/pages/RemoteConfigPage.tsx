import { useState } from "react";
import { Button } from "@/components/ui/Button";
import { Input, PasswordInput } from "@/components/ui/Field";
import { api } from "@/lib/api";

export function RemoteConfigPage({ onConfigured }: { onConfigured: () => void }) {
  const [projectUrl, setProjectUrl] = useState("");
  const [anonKey, setAnonKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  return (
    <div className="grid h-full min-h-0 place-items-center overflow-auto p-6">
      <form
        className="card w-full max-w-md space-y-4 rounded-lg p-8"
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError(null);
          try {
            await api.remoteConfigure({ project_url: projectUrl, anon_key: anonKey });
            setAnonKey("");
            onConfigured();
          } catch (err) {
            setError(String(err));
          } finally {
            setBusy(false);
          }
        }}
      >
        <p className="page-kicker">Administración remota</p>
        <h1 className="page-title">Conectar Supabase</h1>
        <p className="text-sm text-[var(--muted)]">
          URL y clave anónima se guardan en este equipo. No van al repositorio ni a ajustes visibles.
        </p>
        <label className="block text-sm">
          URL del proyecto
          <Input
            required
            autoFocus
            value={projectUrl}
            onChange={(e) => setProjectUrl(e.target.value)}
            placeholder="https://xxxx.supabase.co"
          />
        </label>
        <label className="block text-sm">
          Clave anónima (anon)
          <PasswordInput
            required
            show={showKey}
            onToggle={() => setShowKey((v) => !v)}
            value={anonKey}
            onChange={(e) => setAnonKey(e.target.value)}
          />
        </label>
        {error && <p role="alert" className="text-sm text-[var(--danger)]">{error}</p>}
        <Button className="w-full" disabled={busy} type="submit">
          {busy ? "Guardando…" : "Guardar y continuar"}
        </Button>
      </form>
    </div>
  );
}
