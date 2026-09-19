/** Centralized timings for login → dashboard (transform/opacity only). */

export type LoginPhase = "idle" | "loading" | "success" | "transitioning" | "error";

export type LoginSuccessVisual = "idle" | "form-exit" | "logo-pulse";

export const LOGIN_MOTION = {
  formExitMs: 280,
  logoPulseMs: 400,
  glowMs: 480,
  dashEnterMs: 380,
  errorShakeMs: 250,
  btnPressMs: 90,
  shellStaggerMs: 60,
} as const;

export function prefersReducedMotion(): boolean {
  return typeof window !== "undefined"
    && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Single delayed wait; abortable so unmount cancels the sequence. */
export function wait(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new DOMException("Aborted", "AbortError"));
      return;
    }
    const id = window.setTimeout(() => resolve(), ms);
    const onAbort = () => {
      window.clearTimeout(id);
      reject(new DOMException("Aborted", "AbortError"));
    };
    signal?.addEventListener("abort", onAbort, { once: true });
  });
}

/**
 * Success choreography after auth resolves.
 * Form exits first; logo pulses; caller then commits session.
 */
export async function runLoginSuccessSequence(
  setVisual: (visual: LoginSuccessVisual) => void,
  signal?: AbortSignal,
): Promise<void> {
  setVisual("form-exit");
  await wait(LOGIN_MOTION.formExitMs, signal);
  setVisual("logo-pulse");
  await wait(Math.max(LOGIN_MOTION.logoPulseMs, LOGIN_MOTION.glowMs - 40), signal);
  setVisual("idle");
}

export const SHELL_ENTER_KEY = "nightdesk-shell-enter";

export function markShellEnter(): void {
  try {
    sessionStorage.setItem(SHELL_ENTER_KEY, "1");
  } catch {
    /* private mode */
  }
}

export function consumeShellEnter(): boolean {
  try {
    const v = sessionStorage.getItem(SHELL_ENTER_KEY);
    if (v) sessionStorage.removeItem(SHELL_ENTER_KEY);
    return v === "1";
  } catch {
    return false;
  }
}
