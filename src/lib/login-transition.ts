/**
 * Centralized timings for the login → dashboard choreography.
 *
 * On auth OK the login chrome stays until reveal. Then in one step:
 *   - form + brand panel clear (shell is already painted underneath)
 *   - logo FLIPs to the sidebar (~630 ms)
 *
 * Reduced motion skips the flight and enters the shell directly.
 */

export type LoginPhase = "idle" | "loading" | "success" | "transitioning" | "error";

/** Visual state driven by the LoginPage itself. E4 (reveal) comes from App via `handoff`. */
export type LoginSuccessVisual = "idle" | "form-exit" | "recompose" | "hold";

export type HandoffPhase = "hold" | "reveal";

export const LOGIN_MOTION = {
  btnPressMs: 90,
  formExitMs: 0,
  /** No pause before hold — App starts reveal as soon as settings are loaded. */
  recomposeLeadMs: 0,
  recomposeMs: 0,
  layerStaggerMs: 70,
  revealMs: 630,
  errorShakeMs: 250,
  shellStaggerMs: 90,
  /** If settings take longer than this while holding, show "Cargando tu espacio…". */
  slowLoadMs: 2500,
} as const;

/** Intrinsic aspect ratio (w / h) of love-nest-logo-borderless.png (1005 × 868). */
export const LOGO_ASPECT = 1005 / 868;

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
 * Marks the login choreography ready for E4. Chrome stays visible until App
 * sets handoff to "reveal" (clear + FLIP in the same frame — no empty hold).
 */
export async function runLoginSuccessSequence(signal?: AbortSignal): Promise<void> {
  if (signal?.aborted) {
    throw new DOMException("Aborted", "AbortError");
  }
}

/** Rect of an image drawn with `object-fit: contain` inside `box`. */
export function containedRect(box: DOMRect, aspect: number): DOMRect {
  let width = box.width;
  let height = width / aspect;
  if (height > box.height) {
    height = box.height;
    width = height * aspect;
  }
  return new DOMRect(
    box.left + (box.width - width) / 2,
    box.top + (box.height - height) / 2,
    width,
    height,
  );
}

/** Where the logo lands: the sidebar brand image, as drawn (not its layout box). */
export function measureBrandTarget(): DOMRect | null {
  if (typeof document === "undefined") return null;
  const img = document.querySelector<HTMLImageElement>("[data-brand-target] img");
  if (!img) return null;
  const box = img.getBoundingClientRect();
  if (box.width <= 0 || box.height <= 0) return null;
  const aspect = img.naturalWidth > 0 && img.naturalHeight > 0
    ? img.naturalWidth / img.naturalHeight
    : LOGO_ASPECT;
  return containedRect(box, aspect);
}

export type FlipTransform = { x: number; y: number; scale: number };

/** FLIP: translate + uniform scale (about the center) that maps `from` onto `to`. */
export function flipTransform(from: DOMRect, to: DOMRect): FlipTransform {
  const scale = from.width > 0 ? to.width / from.width : 1;
  return {
    x: to.left + to.width / 2 - (from.left + from.width / 2),
    y: to.top + to.height / 2 - (from.top + from.height / 2),
    scale,
  };
}

export const SHELL_ENTER_KEY = "nightdesk-shell-enter";

/** Used by the reduced-motion / no-handoff path so AppShell still plays its short entrance. */
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
