import { Minus, Square, X } from "lucide-react";
import { useEffect, useState } from "react";

function isTauri() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function TitleBar() {
  const [active, setActive] = useState(false);

  useEffect(() => {
    setActive(isTauri());
  }, []);

  if (!active) return null;

  async function win() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow();
  }

  return (
    <div
      data-tauri-drag-region
      className="flex h-9 shrink-0 items-stretch justify-end border-b border-[var(--line)] bg-[var(--surface)]"
    >
      <button
        type="button"
        aria-label="Minimizar"
        data-tauri-drag-region="false"
        className="flex w-11 items-center justify-center text-[var(--muted)] hover:bg-[var(--surface-2)] hover:text-[var(--ink)]"
        onClick={async () => {
          await (await win()).minimize();
        }}
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        aria-label="Maximizar"
        data-tauri-drag-region="false"
        className="flex w-11 items-center justify-center text-[var(--muted)] hover:bg-[var(--surface-2)] hover:text-[var(--ink)]"
        onClick={async () => {
          await (await win()).toggleMaximize();
        }}
      >
        <Square size={12} />
      </button>
      <button
        type="button"
        aria-label="Cerrar"
        data-tauri-drag-region="false"
        className="flex w-11 items-center justify-center text-[var(--muted)] hover:bg-[var(--danger-soft)] hover:text-[var(--danger)]"
        onClick={async () => {
          await (await win()).close();
        }}
      >
        <X size={14} />
      </button>
    </div>
  );
}
