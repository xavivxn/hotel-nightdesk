import { formatDuration, formatMoney, statusLabel } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { BoardRoom } from "@/lib/types";
import { Ban, Bath, BrushCleaning, CalendarClock, Check, Crown, Hourglass, Timer } from "lucide-react";
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
  dirty: "stamp-dirty",
  blocked: "stamp-blocked",
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
  if (stay.converted_to_overnight || !stay.expected_checkout_at) return elapsed;
  const end = new Date(stay.expected_checkout_at).getTime();
  if (Number.isNaN(end)) return elapsed;
  const minutes = Math.round((end - Date.now()) / 60000);
  return minutes >= 0
    ? { label: "Restan", value: formatDuration(minutes), overdue: false }
    : { label: "Excedido", value: formatDuration(-minutes), overdue: true };
}

export function RoomStatusLegend() {
  return (
    <ul className="mt-3 flex flex-wrap gap-2" aria-label="Leyenda de estados">
      {legendStatuses.map((status) => {
        const Icon = icons[status];
        return (
          <li key={status}>
            <span className={cn("stamp status-key", stamps[status])}>
              <Icon size={14} />
              {statusLabel(status)}
            </span>
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

function roomKind(type: string) {
  const value = type.trim().toLowerCase();
  const isJacuzzi = /jacc?uz+i/.test(value);
  const isStandard = value === "estándar" || value === "estandar" || value === "normal";
  return { isJacuzzi, isSuite: !isStandard && !isJacuzzi };
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
  const time = timeStatus(item);
  const { isJacuzzi, isSuite } = roomKind(item.room.room_type);
  const showJacuzzi = isJacuzzi && !item.stay;
  const total = item.stay && Number.isFinite(Number(item.estimated_total_cents))
    ? Number(item.estimated_total_cents)
    : null;
  return (
    <button
      onClick={onClick}
      className={cn(
        "room-card focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent)]",
        fills[status] ?? fills.available,
      )}
    >
      {status === "dirty" ? <span className="room-dirty-sheen" aria-hidden="true" /> : null}
      {isJacuzzi && item.stay ? <span className="sr-only">Jacuzzi</span> : null}
      <div className="room-head">
        <span className="room-head-main">
          <span className="room-head-number">{item.room.number}</span>
          {showJacuzzi || isSuite ? (
            <span className="room-head-attr">
              {showJacuzzi ? (
                <span className="room-jacuzzi-label">
                  <Bath size={10} className="shrink-0" aria-hidden="true" />
                  Jacuzzi
                </span>
              ) : (
                <span className="room-suite-label">
                  <Crown size={10} className="shrink-0" aria-hidden="true" />
                  {item.room.room_type}
                </span>
              )}
            </span>
          ) : null}
        </span>
        <span className="room-head-status">
          <Icon size={12} className="shrink-0" />
          <span>{statusLabel(status)}</span>
        </span>
      </div>
      <div className="room-body">
        {item.stay ? (
          <div className={cn("room-panel", time?.overdue && "is-overdue")}>
            <p className="room-timer-label">{time?.label}</p>
            <p className="room-timer">
              <Hourglass size={14} className="room-hourglass shrink-0" />
              <span>{time?.value}</span>
            </p>
            <p className="room-sub">
              {item.stay.converted_to_overnight ? "Dormida" : item.stay.rate_plan_name}
            </p>
            {total != null ? (
              <p className="room-tariff">
                Total <b>{formatMoney(total, currency)}</b>
              </p>
            ) : null}
          </div>
        ) : (
          <p className="room-note">
            <Icon size={15} className="room-status-icon shrink-0" />
            <span>{notes[status] ?? notes.available}</span>
          </p>
        )}
      </div>
    </button>
  );
}
