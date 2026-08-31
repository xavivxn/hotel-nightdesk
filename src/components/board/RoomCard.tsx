import { formatDuration, formatMoney, statusLabel } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { BoardRoom } from "@/lib/types";

const tones: Record<string, string> = {
  available: "border-[color-mix(in_srgb,var(--ok)_55%,var(--line))] bg-[var(--ok-soft)]/40",
  occupied: "border-[color-mix(in_srgb,var(--warn)_60%,var(--line))] bg-[var(--warn-soft)]/50",
  dirty: "border-[var(--line)] bg-[var(--surface-2)]",
  blocked: "border-[color-mix(in_srgb,var(--danger)_50%,var(--line))] bg-[var(--danger-soft)]/40",
  reserved: "border-[color-mix(in_srgb,var(--info)_55%,var(--line))] bg-[var(--info-soft)]/45",
};

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
  return (
    <button
      onClick={onClick}
      className={cn(
        "card group relative flex min-h-[168px] flex-col items-start rounded-2xl p-4 text-left transition hover:-translate-y-0.5 hover:border-[var(--accent)]",
        tones[status] ?? tones.available,
      )}
    >
      <div className="flex w-full items-start justify-between gap-3">
        <div>
          <p className="font-display text-3xl leading-none tracking-tight">{item.room.number}</p>
          <p className="mt-1 text-xs uppercase tracking-[0.16em] text-[var(--muted)]">
            {item.room.room_type} · Piso {item.room.floor}
          </p>
        </div>
        <span className="rounded-full bg-[var(--surface)]/80 px-2.5 py-1 text-[11px] font-semibold uppercase tracking-wide">
          {statusLabel(status)}
        </span>
      </div>
      <div className="mt-auto w-full pt-6">
        {item.stay ? (
          <>
            <p className="truncate font-medium">{item.stay.guest_name}</p>
            <div className="mt-1 flex items-center justify-between text-sm text-[var(--muted)]">
              <span>{item.stay.converted_to_overnight ? "Pernocte" : item.stay.rate_plan_name}</span>
              <span>{formatDuration(item.elapsed_minutes ?? 0)}</span>
            </div>
            {item.estimated_total_cents != null ? (
              <p className="mt-2 text-lg font-semibold">{formatMoney(item.estimated_total_cents, currency)}</p>
            ) : null}
          </>
        ) : item.reservation ? (
          <>
            <p className="truncate font-medium">{item.reservation.guest_name}</p>
            <p className="mt-1 text-sm text-[var(--muted)]">Llegada reservada · {item.reservation.rate_plan_name}</p>
          </>
        ) : (
          <p className="text-sm text-[var(--muted)]">Lista para check-in</p>
        )}
      </div>
    </button>
  );
}
