import { cn } from "@/lib/utils";
import { X } from "lucide-react";
import type { ReactNode } from "react";

export function Drawer({
  open,
  title,
  subtitle,
  onClose,
  children,
  wide,
}: {
  open: boolean;
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
}) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-40 flex justify-end">
      <button className="absolute inset-0 bg-black/40 backdrop-blur-[2px]" onClick={onClose} aria-label="Cerrar" />
      <aside
        className={cn(
          "relative z-10 flex h-full w-full flex-col border-l border-[var(--line)] bg-[var(--surface)] shadow-2xl",
          wide ? "max-w-xl" : "max-w-md",
        )}
      >
        <header className="flex items-start justify-between gap-4 border-b border-[var(--line)] px-6 py-5">
          <div>
            <p className="font-display text-2xl">{title}</p>
            {subtitle ? <p className="mt-1 text-sm text-[var(--muted)]">{subtitle}</p> : null}
          </div>
          <button
            onClick={onClose}
            className="rounded-full p-2 text-[var(--muted)] hover:bg-[var(--surface-2)]"
            aria-label="Cerrar panel"
          >
            <X size={18} />
          </button>
        </header>
        <div className="flex-1 overflow-y-auto p-6 scrollbar-thin">{children}</div>
      </aside>
    </div>
  );
}
