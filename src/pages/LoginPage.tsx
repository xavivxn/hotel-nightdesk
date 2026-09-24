import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { Info, TriangleAlert } from "lucide-react";
import { LoveNestLogoLayered } from "@/components/layout/BrandLogo";
import { DutyClock } from "@/components/layout/DutyClock";
import { Button } from "@/components/ui/Button";
import { Input, PasswordInput, reportInputIssue } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { FIELD_EMPTY, requireTrimmed } from "@/lib/format";
import {
  LOGIN_MOTION,
  flipTransform,
  markShellEnter,
  measureBrandTarget,
  prefersReducedMotion,
  runLoginSuccessSequence,
  type HandoffPhase,
  type LoginPhase,
  wait,
} from "@/lib/login-transition";
import type { SessionInfo } from "@/lib/types";
import { cn } from "@/lib/utils";

export type LoginHandoff = {
  phase: HandoffPhase;
  /** Settings are taking long: show a hint under the logo while holding. */
  slow: boolean;
  /** Called once the logo landed on the sidebar (E4 done). */
  onDone: () => void;
};

export function LoginPage({
  setup,
  remote = false,
  notice,
  handoff,
  onLogin,
  onSequenceDone,
}: {
  setup: boolean;
  remote?: boolean;
  notice: string | null;
  /** Present while App keeps this page mounted as an overlay above the shell. */
  handoff?: LoginHandoff;
  /** `animated` is false under reduced motion: App should mount the shell directly. */
  onLogin: (session: SessionInfo, animated: boolean) => void;
  /** E2 → E3 finished; App may start E4 as soon as settings are loaded. */
  onSequenceDone: () => void;
}) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [legacyPin, setLegacyPin] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [showPin, setShowPin] = useState(false);
  const [capsOn, setCapsOn] = useState(false);
  const [phase, setPhase] = useState<LoginPhase>("idle");
  const [error, setError] = useState<string | null>(null);
  const [needsSetup, setNeedsSetup] = useState(setup && !remote);
  const [btnPressed, setBtnPressed] = useState(false);
  const abortRef = useRef<AbortController | null>(null);
  const slotRef = useRef<HTMLDivElement>(null);
  const logoRef = useRef<HTMLDivElement>(null);
  const usernameRef = useRef<HTMLInputElement>(null);
  const onDoneRef = useRef<(() => void) | undefined>(handoff?.onDone);
  onDoneRef.current = handoff?.onDone;

  const [noticeState, setNoticeState] = useState<"shown" | "leaving" | "hidden">(notice ? "shown" : "hidden");

  useEffect(() => () => abortRef.current?.abort(), []);

  const revealing = handoff?.phase === "reveal";

  // E4: FLIP logo to sidebar. Overlay chrome is gone; the shell is already painted underneath.
  useLayoutEffect(() => {
    if (!revealing) return;
    const slot = slotRef.current;
    const logo = logoRef.current;
    if (!slot || !logo) return;

    const target = measureBrandTarget();
    const ac = new AbortController();

    if (target) {
      const { x, y, scale } = flipTransform(logo.getBoundingClientRect(), target);
      // Explicit FLIP: lock start pose, then animate to the sidebar slot.
      slot.style.transition = "none";
      slot.style.setProperty("--flip-x", "0px");
      slot.style.setProperty("--flip-y", "0px");
      slot.style.setProperty("--flip-scale", "1");
      void slot.offsetWidth;
      slot.style.transition = "";
      slot.style.setProperty("--flip-x", `${x}px`);
      slot.style.setProperty("--flip-y", `${y}px`);
      slot.style.setProperty("--flip-scale", `${scale}`);
    }

    wait(LOGIN_MOTION.revealMs, ac.signal).then(
      () => onDoneRef.current?.(),
      () => {
        /* aborted */
      },
    );
    return () => {
      ac.abort();
      slot.style.transition = "";
      slot.style.removeProperty("--flip-x");
      slot.style.removeProperty("--flip-y");
      slot.style.removeProperty("--flip-scale");
    };
  }, [revealing]);

  useEffect(() => {
    setNoticeState(notice ? "shown" : "hidden");
    if (!notice) return;
    const leave = window.setTimeout(() => setNoticeState("leaving"), 4500);
    const hide = window.setTimeout(() => setNoticeState("hidden"), 4800);
    return () => {
      window.clearTimeout(leave);
      window.clearTimeout(hide);
    };
  }, [notice]);

  const locked = phase === "loading" || phase === "success" || phase === "transitioning";
  const modeLabel = remote ? "Administración remota" : "Recepción";

  function trackCaps(e: KeyboardEvent<HTMLInputElement>) {
    setCapsOn(e.getModifierState("CapsLock"));
  }

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

      const user = requireTrimmed(username);
      if (!user) {
        setPhase("idle");
        reportInputIssue(usernameRef.current, FIELD_EMPTY);
        return;
      }

      if (needsSetup) {
        if (password !== confirm) throw "Las contraseñas no coinciden";
        await api.setupAdmin({ username: user, password }, requireTrimmed(legacyPin) ?? "");
        setNeedsSetup(false);
      }
      const session = await api.login({ username: user, password });

      if (prefersReducedMotion()) {
        markShellEnter();
        setPhase("transitioning");
        onLogin(session, false);
        return;
      }

      // App mounts the shell underneath (blurred) and prefetches settings while E2/E3 play.
      setPhase("success");
      onLogin(session, true);
      await runLoginSuccessSequence(ac.signal);
      setPhase("transitioning");
      onSequenceDone();
    } catch (err) {
      if (err instanceof DOMException && err.name === "AbortError") return;
      setBtnPressed(false);
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
        "login-stage login-stage--split h-full min-h-0",
        handoff && "login-stage--overlay",
        revealing && "login-stage--reveal",
      )}
      aria-hidden={handoff ? true : undefined}
    >
      <aside className="login-brand">
        <div className="login-brand-bg" aria-hidden="true" />
        <div className="login-logo-slot" ref={slotRef}>
          <div className="login-logo-glow" />
          <LoveNestLogoLayered ref={logoRef} className="login-logo" />
        </div>
        <div className="login-brand-clock">
          <DutyClock />
        </div>
        {handoff?.phase === "hold" && handoff.slow ? (
          <p role="status" className="login-hold-note text-sm text-[var(--muted)]">
            Cargando tu espacio…
          </p>
        ) : null}
        <p className="login-brand-foot font-mono tabular-nums">
          v{__APP_VERSION__} · {modeLabel}
        </p>
      </aside>

      <section className="login-form-panel">
        <form
          className={cn("login-card w-full space-y-4", phase === "error" && "login-card--shake")}
          onSubmit={handleSubmit}
          aria-busy={phase === "loading"}
        >
          <div>
            <p className="page-kicker">{modeLabel}</p>
            <h1 className="page-title">{needsSetup ? "Crear administrador" : "Iniciar sesión"}</h1>
            <p className="mt-2 text-sm text-[var(--muted)]">
              {needsSetup
                ? "Configurá la primera cuenta de este equipo. Después podrás crear usuarios de recepción."
                : remote
                  ? "Ingresá con tu email."
                  : "Ingresá con tu usuario."}
            </p>
          </div>

          {notice && noticeState !== "hidden" ? (
            <p role="status" className={cn("login-notice", noticeState === "leaving" && "login-notice--leaving")}>
              <Info size={15} aria-hidden="true" className="mt-0.5 shrink-0 text-[var(--accent)]" />
              <span>{notice}</span>
            </p>
          ) : null}

          <LoginField label={remote ? "Email" : "Usuario"}>
            <Input
              ref={usernameRef}
              autoFocus
              required
              maxLength={64}
              autoComplete="username"
              value={username}
              disabled={locked}
              onChange={(e) => setUsername(e.target.value)}
            />
          </LoginField>
          <LoginField label="Contraseña">
            <PasswordInput
              required
              show={showPassword}
              onToggle={() => setShowPassword((v) => !v)}
              autoComplete={needsSetup ? "new-password" : "current-password"}
              value={password}
              disabled={locked}
              onKeyDown={trackCaps}
              onKeyUp={trackCaps}
              onBlur={() => setCapsOn(false)}
              onChange={(e) => setPassword(e.target.value)}
            />
          </LoginField>
          {needsSetup ? (
            <>
              <LoginField label="Repetir contraseña">
                <PasswordInput
                  required
                  show={showConfirm}
                  onToggle={() => setShowConfirm((v) => !v)}
                  autoComplete="new-password"
                  value={confirm}
                  disabled={locked}
                  onKeyDown={trackCaps}
                  onKeyUp={trackCaps}
                  onBlur={() => setCapsOn(false)}
                  onChange={(e) => setConfirm(e.target.value)}
                />
              </LoginField>
              <LoginField label="PIN anterior (si el equipo tenía uno)">
                <PasswordInput
                  show={showPin}
                  onToggle={() => setShowPin((v) => !v)}
                  value={legacyPin}
                  disabled={locked}
                  onChange={(e) => setLegacyPin(e.target.value)}
                />
              </LoginField>
            </>
          ) : null}
          {capsOn ? (
            <p role="status" className="flex items-center gap-1.5 text-sm font-medium text-[var(--warn)]">
              <TriangleAlert size={15} aria-hidden="true" /> Bloq Mayús está activado
            </p>
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
                Verificando…
              </>
            ) : needsSetup ? (
              "Crear y entrar"
            ) : (
              "Entrar"
            )}
          </Button>
        </form>
      </section>
    </div>
  );
}


function LoginField({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">
        {label}
      </span>
      {children}
    </label>
  );
}