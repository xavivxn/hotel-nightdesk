import { formatDuration, formatMoney, statusLabel } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { BoardRoom } from "@/lib/types";
import { Ban, BrushCleaning, CalendarClock, Check, Timer } from "lucide-react";
import type { LucideIcon } from "lucide-react";

const fills: Record<string, string> = {
  available: "room-available",
  occupied: "room-occupied",
  dirty: "room-dirty",
  blocked: "room-blocked",
  reserved: "room-reserved",
};

const stamps: Record<string, string> = {
  available: "border-0 bg-[var(--ok)] text-[var(--ok-ink)]",
  occupied: "border-0 bg-[var(--warn)] text-[var(--warn-ink)]",
  dirty: "border-0 bg-[var(--dirty)] text-[var(--dirty-ink)]",
  blocked: "border-0 bg-[var(--danger)] text-[var(--danger-ink)]",
  reserved: "border-0 bg-[var(--info)] text-[var(--info-ink)]",
};

const icons: Record<string, LucideIcon> = {
  available: Check,
  occupied: Timer,
  dirty: BrushCleaning,
  blocked: Ban,
  reserved: CalendarClock,
};

const marks: Partial<Record<string, { Icon: LucideIcon; className: string }>> = {
  available: { Icon: Check, className: "text-[var(--ok)]" },
  dirty: { Icon: BrushCleaning, className: "text-[var(--dirty)]" },
  reserved: { Icon: CalendarClock, className: "text-[var(--info)]" },
  blocked: { Icon: Ban, className: "text-[var(--danger)]" },
};

const legendStatuses = ["available", "occupied", "dirty", "reserved", "blocked"] as const;

export function RoomStatusLegend() {
  return (
    <ul className="mt-4 flex flex-wrap gap-2">
      {legendStatuses.map((status) => {
        const Icon = icons[status];
        return (
          <li key={status} className={cn("stamp", stamps[status])}>
            <Icon size={11} />
            {statusLabel(status)}
          </li>
        );
      })}
    </ul>
  );
}

export function RoomCard({
  item,
  currency,
  onClick,
}: {
  item: BoardRoom;
  currency: string;
  onClick: () => void;
}) {
  const status = item.display_status;
  const Icon = icons[status] ?? Check;
  const mark = marks[status];
  return (
    <button
      onClick={onClick}
      className={cn(
        "card room-card flex min-h-[156px] flex-col items-start rounded-lg p-4 pl-7 text-left transition-colors hover:bg-[var(--surface-2)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent)]",
        fills[status] ?? fills.available,
      )}
    >
      {mark ? (
        <mark.Icon
          aria-hidden
          size={40}
          strokeWidth={1.6}
          className={cn("pointer-events-none absolute right-3 bottom-3 opacity-80", mark.className)}
        />
      ) : null}
      <div className="relative flex w-full items-start justify-between gap-3">
        <div>
          <p className="font-mono text-3xl leading-none font-semibold tracking-tight">{item.room.number}</p>
          <p className="mt-1 text-[11px] font-medium uppercase tracking-[0.08em] text-[var(--muted)]">
            {item.room.room_type}
          </p>
        </div>
        <span className={cn("stamp shrink-0", stamps[status] ?? stamps.available)}>
          <Icon size={11} />
          {statusLabel(status)}
        </span>
      </div>
      <div className={cn("relative mt-auto w-full pt-6", mark && "pr-12")}>
        {item.stay ? (
          <>
            <p className="truncate font-medium">{item.stay.guest_name}</p>
            <div className="mt-1 flex items-center justify-between text-sm text-[var(--muted)]">
              <span>{item.stay.converted_to_overnight ? "Pernocte" : item.stay.rate_plan_name}</span>
              <span className="font-mono tabular-nums">{formatDuration(item.elapsed_minutes ?? 0)}</span>
            </div>
            {item.estimated_total_cents != null ? (
              <p className="mt-2 font-mono text-lg font-semibold tabular-nums">
                {formatMoney(item.estimated_total_cents, currency)}
              </p>
            ) : null}
          </>
        ) : item.reservation ? (
          <>
            <p className="truncate font-medium">{item.reservation.guest_name}</p>
            <p className="mt-1 text-sm text-[var(--muted)]">Llegada reservada · {item.reservation.rate_plan_name}</p>
          </>
        ) : (
          <p className="text-sm text-[var(--muted)]">
            {status === "dirty" ? "Pendiente de aseo" : status === "blocked" ? "Fuera de servicio" : "Lista para check-in"}
          </p>
        )}
      </div>
    </button>
  );
}
