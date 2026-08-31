import { RoomCard, RoomStatusLegend } from "@/components/board/RoomCard";
import { RoomDrawer } from "@/components/board/RoomDrawer";
import { api } from "@/lib/api";
import type { AppSettings, BoardRoom, RatePlan } from "@/lib/types";
import { Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

export function BoardPage({ settings }: { settings: AppSettings }) {
  const [board, setBoard] = useState<BoardRoom[]>([]);
  const [rates, setRates] = useState<RatePlan[]>([]);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<BoardRoom | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function load() {
    try {
      const [rooms, plans] = await Promise.all([api.listBoard(), api.listRatePlans(true)]);
      setBoard(rooms);
      setRates(plans);
      setSelected((current) => current ? rooms.find((r) => r.room.id === current.room.id) ?? null : null);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    load();
    const id = window.setInterval(load, 20000);
    return () => window.clearInterval(id);
  }, []);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return board;
    return board.filter((item) =>
      [item.room.number, item.stay?.guest_name, item.reservation?.guest_name]
        .filter(Boolean)
        .some((value) => String(value).toLowerCase().includes(q)),
    );
  }, [board, query]);

  const grouped = useMemo(() => {
    const map = new Map<number, BoardRoom[]>();
    for (const item of filtered) {
      const floor = item.room.floor;
      const list = map.get(floor) ?? [];
      list.push(item);
      map.set(floor, list);
    }
    return [...map.entries()].sort((a, b) => a[0] - b[0]);
  }, [filtered]);

  const occupied = board.filter((r) => r.display_status === "occupied").length;
  const occupancyPct = board.length ? Math.round((occupied / board.length) * 100) : 0;

  return (
    <div className="px-6 py-6 lg:px-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="page-kicker">Recepción</p>
          <h1 className="page-title">Tablero</h1>
        </div>
        <div className="flex items-center gap-3">
          <div className="min-w-[148px] rounded-lg border border-[var(--line)] bg-[var(--surface)] px-3 py-2">
            <div className="flex items-baseline justify-between gap-3">
              <span className="text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">Ocupación</span>
              <span className="font-mono text-sm font-semibold tabular-nums">
                {occupied}/{board.length}
              </span>
            </div>
            <div className="mt-2 h-1 overflow-hidden rounded-full bg-[var(--surface-2)]">
              <div className="h-full bg-[var(--warn)]" style={{ width: `${occupancyPct}%` }} />
            </div>
          </div>
          <label className="relative block w-72">
            <Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--muted)]" size={16} />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Buscar habitación o huésped"
              className="h-11 w-full rounded-lg border border-[var(--line)] bg-[var(--surface)] pl-10 pr-4 outline-none focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent-soft)]"
            />
          </label>
        </div>
      </header>
      <RoomStatusLegend />
      {error ? <p className="mt-4 text-[var(--danger)]">{error}</p> : null}
      <div className="mt-2">
        {grouped.map(([floor, rooms]) => (
          <section key={floor} className="mt-6">
            <div className="mb-3 flex items-center gap-3">
              <p className="font-mono text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--muted)]">
                Piso {floor}
              </p>
              <div className="h-px flex-1 bg-[var(--line)]" />
              <p className="font-mono text-[11px] tabular-nums text-[var(--muted)]">{rooms.length}</p>
            </div>
            <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
              {rooms.map((item) => (
                <RoomCard
                  key={item.room.id}
                  item={item}
                  currency={settings.currency_symbol}
                  onClick={() => setSelected(item)}
                />
              ))}
            </div>
          </section>
        ))}
      </div>
      <RoomDrawer
        item={selected}
        rates={rates}
        settings={settings}
        onClose={() => setSelected(null)}
        onChanged={load}
      />
    </div>
  );
}
