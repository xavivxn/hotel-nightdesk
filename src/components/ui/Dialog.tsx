import { cn } from "@/lib/utils";
import { X } from "lucide-react";
import { useEffect, type ReactNode } from "react";

export function Dialog({
  open,
  title,
  subtitle,
  onClose,
  children,
  size = "sm",
  dismissible = true,
}: {
  open: boolean;
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  size?: "sm" | "lg";
  dismissible?: boolean;
}) {
  useEffect(() => {
    if (!open) return;
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      window.removeEventListener("keydown", onKey);
      document.body.style.overflow = previous;
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center p-4">
      {dismissible ? (
        <button
          className="absolute inset-0 bg-black/50 backdrop-blur-[2px]"
          onClick={onClose}
          aria-label="Cerrar"
        />
      ) : (
        <div className="absolute inset-0 bg-black/50 backdrop-blur-[2px]" />
      )}
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="nightdesk-dialog-title"
        className={cn(
          "relative z-10 flex w-full flex-col overflow-hidden rounded-lg border border-[var(--line)] bg-[var(--surface)] shadow-2xl",
          size === "lg"
            ? "h-[min(92vh,820px)] max-w-[960px]"
            : "max-h-[min(92vh,720px)] max-w-md",
        )}
      >
        <header className="flex shrink-0 items-start justify-between gap-4 border-b border-[var(--line)] px-6 py-5">
          <div>
            <p id="nightdesk-dialog-title" className="text-xl font-semibold tracking-tight">
              {title}
            </p>
            {subtitle ? <p className="mt-1 text-sm text-[var(--muted)]">{subtitle}</p> : null}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="inline-flex h-11 w-11 shrink-0 items-center justify-center rounded-lg text-[var(--muted)] hover:bg-[var(--surface-2)]"
            aria-label="Cerrar panel"
          >
            <X size={18} />
          </button>
        </header>
        <div
          className={cn(
            "min-h-0 flex-1",
            size === "lg" ? "flex flex-col overflow-hidden" : "overflow-y-auto p-6 scrollbar-thin",
          )}
        >
          {children}
        </div>
      </div>
    </div>
  );
}
