import { useEffect, useRef, useState, type FormEvent } from "react";
import { LoveNestLogo } from "@/components/layout/BrandLogo";
import { Button } from "@/components/ui/Button";
import { Input, PasswordInput } from "@/components/ui/Field";
import { api } from "@/lib/api";
import {
  LOGIN_MOTION,
  markShellEnter,
  prefersReducedMotion,
  runLoginSuccessSequence,
  type LoginPhase,
  type LoginSuccessVisual,
  wait,
} from "@/lib/login-transition";
import type { SessionInfo } from "@/lib/types";
import { cn } from "@/lib/utils";

export function LoginPage({
  setup,
  remote = false,
  notice,
  onLogin,
}: {
  setup: boolean;
  remote?: boolean;
  notice: string | null;
  onLogin: (session: SessionInfo) => void;
}) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [legacyPin, setLegacyPin] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [showPin, setShowPin] = useState(false);
  const [phase, setPhase] = useState<LoginPhase>("idle");
  const [visual, setVisual] = useState<LoginSuccessVisual>("idle");
  const [error, setError] = useState<string | null>(null);
  const [needsSetup, setNeedsSetup] = useState(setup && !remote);
  const [btnPressed, setBtnPressed] = useState(false);
  const abortRef = useRef<AbortController | null>(null);

  useEffect(() => () => abortRef.current?.abort(), []);

  const locked = phase === "loading" || phase === "success" || phase === "transitioning";

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (locked) return;

    abortRef.current?.abort();
    const ac = new AbortController();
    abortRef.current = ac;

    setError(null);
    setBtnPressed(true);
    setPhase("loading");
    try {
      await wait(LOGIN_MOTION.btnPressMs, ac.signal);
      setBtnPressed(false);

      if (needsSetup) {
        if (password !== confirm) throw "Las contraseñas no coinciden";
        await api.setupAdmin({ username, password }, legacyPin);
        setNeedsSetup(false);
      }
      const session = await api.login({ username, password });

      if (prefersReducedMotion()) {
        markShellEnter();
        setPhase("transitioning");
        onLogin(session);
        return;
      }

      setPhase("success");
      await runLoginSuccessSequence(setVisual, ac.signal);
      setPhase("transitioning");
      markShellEnter();
      onLogin(session);
    } catch (err) {
      if (err instanceof DOMException && err.name === "AbortError") return;
      setBtnPressed(false);
      setVisual("idle");
      setPhase("error");
      setError(String(err));
      try {
        await wait(LOGIN_MOTION.errorShakeMs, ac.signal);
      } catch {
        /* aborted */
      }
      setPhase("idle");
    } finally {
      setPassword("");
      setConfirm("");
      setLegacyPin("");
    }
  }

  return (
    <div
      className={cn(
        "login-stage grid h-full min-h-0 place-items-center overflow-auto p-6",
        phase === "transitioning" && "login-stage--handoff",
        visual === "form-exit" && "login-stage--form-exit",
        visual === "logo-pulse" && "login-stage--logo-pulse",
      )}
    >
      <div className="login-stack w-full max-w-md">
        <div className="login-logo-slot" aria-hidden={false}>
          <div className="login-logo-glow" />
          <LoveNestLogo className="login-logo" />
        </div>

        <form
          className={cn(
            "card login-card w-full space-y-4 rounded-lg p-8",
            phase === "error" && "login-card--shake",
          )}
          onSubmit={handleSubmit}
          aria-busy={phase === "loading"}
        >
          <p className="page-kicker">{remote ? "Administración remota" : "Acceso local"}</p>
          <h1 className="page-title">{needsSetup ? "Crear administrador" : "Iniciar sesión"}</h1>
          <p className="text-sm text-[var(--muted)]">
            {remote
              ? "Ingresá con el email y contraseña de Supabase Auth. Requiere internet."
              : needsSetup
                ? "Configurá la primera cuenta de este equipo. Después podrás crear usuarios de recepción."
                : "Ingresá con tu cuenta individual. Funciona sin internet."}
          </p>
          {notice ? (
            <p role="status" className="text-sm text-[var(--muted)]">
              {notice}
            </p>
          ) : null}
          <label className="block text-sm">
            {remote ? "Email" : "Usuario"}
            <Input
              autoFocus
              required
              maxLength={64}
              autoComplete="username"
              value={username}
              disabled={locked}
              onChange={(e) => setUsername(e.target.value)}
            />
          </label>
          <label className="block text-sm">
            Contraseña
            <PasswordInput
              required
              show={showPassword}
              onToggle={() => setShowPassword((v) => !v)}
              autoComplete={needsSetup ? "new-password" : "current-password"}
              value={password}
              disabled={locked}
              onChange={(e) => setPassword(e.target.value)}
            />
          </label>
          {needsSetup ? (
            <>
              <label className="block text-sm">
                Repetir contraseña
                <PasswordInput
                  required
                  show={showConfirm}
                  onToggle={() => setShowConfirm((v) => !v)}
                  autoComplete="new-password"
                  value={confirm}
                  disabled={locked}
                  onChange={(e) => setConfirm(e.target.value)}
                />
              </label>
              <label className="block text-sm">
                PIN anterior (si el equipo tenía uno)
                <PasswordInput
                  show={showPin}
                  onToggle={() => setShowPin((v) => !v)}
                  value={legacyPin}
                  disabled={locked}
                  onChange={(e) => setLegacyPin(e.target.value)}
                />
              </label>
            </>
          ) : null}
          {error ? (
            <p role="alert" className="login-error text-sm text-[var(--danger)]">
              {error}
            </p>
          ) : null}
          <Button
            className={cn("login-submit w-full", btnPressed && "login-submit--press")}
            disabled={locked}
            type="submit"
          >
            {phase === "loading" ? (
              <>
                <span className="login-spinner" aria-hidden />
                Validando…
              </>
            ) : needsSetup ? (
              "Crear y entrar"
            ) : (
              "Entrar"
            )}
          </Button>
        </form>
      </div>
    </div>
  );
}
