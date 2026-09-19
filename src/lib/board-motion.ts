/** Board motion helpers — transform/opacity first; View Transition when available. */

export function prefersReducedMotion(): boolean {
  return typeof window !== "undefined"
    && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

const STATUS_GLOW: Record<string, string> = {
  available: "color-mix(in srgb, var(--ok) 55%, transparent)",
  occupied: "color-mix(in srgb, var(--warn) 55%, transparent)",
  dirty: "color-mix(in srgb, var(--dirty) 50%, transparent)",
  reserved: "color-mix(in srgb, var(--info) 55%, transparent)",
  blocked: "color-mix(in srgb, var(--danger) 50%, transparent)",
};

export function statusGlow(status: string): string {
  return STATUS_GLOW[status] ?? STATUS_GLOW.available;
}

export function runViewTransition(update: () => void): void {
  if (prefersReducedMotion()) {
    update();
    return;
  }
  const doc = document as Document & {
    startViewTransition?: (cb: () => void) => { finished: Promise<void> };
  };
  if (typeof doc.startViewTransition === "function") {
    doc.startViewTransition(update);
    return;
  }
  update();
}

/** Soft halo that travels from a room card to its summary stat. */
export function playStatusPulse(fromEl: HTMLElement, toEl: HTMLElement, status: string): void {
  if (prefersReducedMotion()) return;

  const from = fromEl.getBoundingClientRect();
  const to = toEl.getBoundingClientRect();
  const size = Math.max(28, Math.min(from.width, from.height) * 0.35);
  const startX = from.left + from.width / 2 - size / 2;
  const startY = from.top + from.height / 2 - size / 2;
  const endX = to.left + to.width / 2 - size / 2;
  const endY = to.top + to.height / 2 - size / 2;

  const halo = document.createElement("div");
  halo.className = "board-status-halo";
  halo.style.width = `${size}px`;
  halo.style.height = `${size}px`;
  halo.style.left = `${startX}px`;
  halo.style.top = `${startY}px`;
  halo.style.setProperty("--halo-color", statusGlow(status));
  document.body.appendChild(halo);

  const anim = halo.animate(
    [
      {
        transform: "translate(0, 0) scale(0.7)",
        opacity: 0,
        offset: 0,
      },
      {
        transform: "translate(0, 0) scale(1)",
        opacity: 0.55,
        offset: 0.18,
      },
      {
        transform: `translate(${endX - startX}px, ${endY - startY}px) scale(0.85)`,
        opacity: 0.35,
        offset: 0.78,
      },
      {
        transform: `translate(${endX - startX}px, ${endY - startY}px) scale(0.4)`,
        opacity: 0,
        offset: 1,
      },
    ],
    {
      duration: 420,
      easing: "cubic-bezier(0.22, 1, 0.36, 1)",
      fill: "forwards",
    },
  );

  void anim.finished.then(() => halo.remove()).catch(() => halo.remove());

  toEl.classList.add("stat-card--pulse");
  window.setTimeout(() => toEl.classList.remove("stat-card--pulse"), 480);
}
