import { useState } from "react";
import { LoveNestLogo } from "@/components/layout/BrandLogo";
import { Button } from "@/components/ui/Button";
import { Input, PasswordInput } from "@/components/ui/Field";
import { api } from "@/lib/api";
import type { SessionInfo } from "@/lib/types";

export function LoginPage({ setup, notice, onLogin }: { setup: boolean; notice: string | null; onLogin: (session: SessionInfo) => void }) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [legacyPin, setLegacyPin] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [showPin, setShowPin] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [needsSetup, setNeedsSetup] = useState(setup);
  return <div className="grid h-full min-h-0 place-items-center overflow-auto p-6"><form className="card w-full max-w-md space-y-4 rounded-lg p-8" onSubmit={async e => {
    e.preventDefault(); setError(null); setBusy(true);
    try {
      if (needsSetup) {
        if (password !== confirm) throw "Las contraseñas no coinciden";
        await api.setupAdmin({ username, password }, legacyPin);
        setNeedsSetup(false);
      }
      onLogin(await api.login({ username, password }));
    } catch (e) { setError(String(e)); } finally { setPassword(""); setConfirm(""); setLegacyPin(""); setBusy(false); }
  }}>
    <LoveNestLogo className="login-logo" />
    <p className="page-kicker">Acceso local</p>
    <h1 className="page-title">{needsSetup ? "Crear administrador" : "Iniciar sesión"}</h1>
    <p className="text-sm text-[var(--muted)]">{needsSetup ? "Configurá la primera cuenta de este equipo. Después podrás crear usuarios de recepción." : "Ingresá con tu cuenta individual. Funciona sin internet."}</p>
    {notice && <p role="status" className="text-sm text-[var(--muted)]">{notice}</p>}
    <label className="block text-sm">Usuario<Input autoFocus required maxLength={64} autoComplete="username" value={username} onChange={e => setUsername(e.target.value)} /></label>
    <label className="block text-sm">Contraseña<PasswordInput required show={showPassword} onToggle={() => setShowPassword(v => !v)} autoComplete={needsSetup ? "new-password" : "current-password"} value={password} onChange={e => setPassword(e.target.value)} /></label>
    {needsSetup && <><label className="block text-sm">Repetir contraseña<PasswordInput required show={showConfirm} onToggle={() => setShowConfirm(v => !v)} autoComplete="new-password" value={confirm} onChange={e => setConfirm(e.target.value)} /></label><label className="block text-sm">PIN anterior (si el equipo tenía uno)<PasswordInput show={showPin} onToggle={() => setShowPin(v => !v)} value={legacyPin} onChange={e => setLegacyPin(e.target.value)} /></label></>}
    {error && <p role="alert" className="text-sm text-[var(--danger)]">{error}</p>}
    <Button className="w-full" disabled={busy} type="submit">{busy ? "Validando…" : needsSetup ? "Crear y entrar" : "Entrar"}</Button>
  </form></div>;
}
