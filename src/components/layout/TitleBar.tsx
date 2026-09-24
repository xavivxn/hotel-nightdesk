import { Heart, Minus, Square, X } from "lucide-react";
import { useEffect, useState } from "react";

function isTauri() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function isWindows() {
  return typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");
}

export function TitleBar() {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    setVisible(isTauri() && isWindows());
  }, []);

  async function win() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow();
  }

  if (!visible) return null;

  return (
    <div
      data-tauri-drag-region
      className="app-chrome relative z-50 flex h-11 shrink-0 items-center justify-end border-b border-[var(--line)] bg-[var(--surface)]"
    >
      <div data-tauri-drag-region className="flex h-full min-w-0 flex-1 items-center pl-4">
        <p className="pointer-events-none flex min-w-0 items-center gap-2 truncate text-[12px]">
          <Heart size={12} aria-hidden="true" className="shrink-0 fill-[var(--accent)] text-[var(--accent)]" />
          <span className="font-semibold uppercase tracking-[0.12em] text-[var(--ink)]">Love Nestt</span>
          <span className="text-[var(--muted)]">· Sistema de Recepción</span>
        </p>
      </div>
      <button
        type="button"
        aria-label="Minimizar"
        data-tauri-drag-region="false"
        className="flex h-full w-11 cursor-pointer items-center justify-center text-[var(--muted)] hover:bg-[var(--surface-2)] hover:text-[var(--ink)]"
        onClick={async () => {
          await (await win()).minimize();
        }}
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        aria-label="Pantalla completa"
        data-tauri-drag-region="false"
        className="flex h-full w-11 cursor-pointer items-center justify-center text-[var(--muted)] hover:bg-[var(--surface-2)] hover:text-[var(--ink)]"
        onClick={async () => {
          const window = await win();
          const full = await window.isFullscreen();
          await window.setFullscreen(!full);
        }}
      >
        <Square size={12} />
      </button>
      <button
        type="button"
        aria-label="Cerrar"
        data-tauri-drag-region="false"
        className="flex h-full w-11 cursor-pointer items-center justify-center text-[var(--muted)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
        onClick={async () => {
          await (await win()).close();
        }}
      >
        <X size={14} />
      </button>
    </div>
  );
}
