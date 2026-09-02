import { formatDuration, formatMoney, statusLabel } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { BoardRoom } from "@/lib/types";
import { Ban, BrushCleaning, CalendarClock, Check, Crown, Hourglass, Timer } from "lucide-react";
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

const legendStatuses = ["available", "occupied", "dirty", "reserved", "blocked"] as const;

type TimeStatus = { label: string; value: string; overdue: boolean };

function timeStatus(item: BoardRoom): TimeStatus | null {
  const stay = item.stay;
  if (!stay) return null;
  const elapsed = { label: "Lleva", value: formatDuration(item.elapsed_minutes ?? 0), overdue: false };
  // Al convertir a pernocte no se recalcula expected_checkout_at: el límite por hora deja de aplicar.
  if (stay.converted_to_overnight || !stay.expected_checkout_at) return elapsed;
  const end = new Date(stay.expected_checkout_at).getTime();
  if (Number.isNaN(end)) return elapsed;
  const minutes = Math.round((end - Date.now()) / 60000);
  return minutes >= 0
    ? { label: "Restan", value: formatDuration(minutes), overdue: false }
    : { label: "Excedido", value: formatDuration(-minutes), overdue: true };
}

export function RoomStatusLegend({
  active,
  onToggle,
}: {
  active: string[];
  onToggle: (status: string) => void;
}) {
  const filtering = active.length > 0;
  return (
    <ul className="mt-4 flex flex-wrap gap-2" aria-label="Filtrar por estado">
      {legendStatuses.map((status) => {
        const Icon = icons[status];
        const selected = active.includes(status);
        return (
          <li key={status}>
            <button
              type="button"
              aria-pressed={selected}
              onClick={() => onToggle(status)}
              className={cn(
                "stamp cursor-pointer transition-opacity focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent)]",
                stamps[status],
                filtering && !selected && "opacity-35",
              )}
            >
              <Icon size={11} />
              {statusLabel(status)}
            </button>
          </li>
        );
      })}
    </ul>
  );
}

const notes: Record<string, string> = {
  available: "Lista para check-in",
  dirty: "Pendiente de aseo",
  blocked: "Fuera de servicio",
  reserved: "Llegada reservada",
};

export function RoomCard({
  item,
  currency,
  baseRateCents,
  onClick,
}: {
  item: BoardRoom;
  currency: string;
  baseRateCents: number | null;
  onClick: () => void;
}) {
  const status = item.display_status;
  const Icon = icons[status] ?? Check;
  const time = timeStatus(item);
  const isSuite = item.room.room_type.trim().toLowerCase() !== "estándar";
  const amount = item.stay ? item.estimated_total_cents : baseRateCents;
  return (
    <button
      onClick={onClick}
      className={cn(
        "room-card focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent)]",
        fills[status] ?? fills.available,
        isSuite && "room-suite",
      )}
    >
      <div className="room-head">
        <span className="room-head-number">{item.room.number}</span>
        <span className="room-head-status">
          {isSuite ? <Crown size={12} className="shrink-0" /> : null}
          <Icon size={12} className="shrink-0" />
          <span className="truncate">{statusLabel(status)}</span>
        </span>
      </div>
      <div className="room-body">
        {item.stay ? (
          <div className={cn("room-panel", time?.overdue && "is-overdue")}>
            <p className="room-timer-label">{time?.label}</p>
            <p className="room-timer">
              <Hourglass size={14} className="shrink-0" />
              <span className="truncate">{time?.value}</span>
            </p>
            <p className="room-sub">
              {item.stay.converted_to_overnight ? "Pernocte" : item.stay.rate_plan_name}
            </p>
          </div>
        ) : (
          <>
            {isSuite ? (
              <p className="room-suite-label">
                <Crown size={13} className="shrink-0" />
                {item.room.room_type}
              </p>
            ) : null}
            <p className="room-note">{notes[status] ?? notes.available}</p>
          </>
        )}
        {amount != null ? (
          <p className="room-tariff">
            {item.stay ? "Total" : "Tarifa"} <b>{formatMoney(amount, currency)}</b>
          </p>
        ) : null}
      </div>
    </button>
  );
}
