import { RoomCard } from "@/components/board/RoomCard";
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

  const occupied = board.filter((r) => r.display_status === "occupied").length;

  return (
    <div className="px-6 py-6 lg:px-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="text-xs uppercase tracking-[0.22em] text-[var(--muted)]">Recepción</p>
          <h1 className="font-display text-4xl">Tablero</h1>
        </div>
        <div className="flex items-center gap-3">
          <div className="rounded-2xl border border-[var(--line)] bg-[var(--surface)] px-4 py-3 text-sm">
            <span className="text-[var(--muted)]">Ocupación</span>
            <p className="text-lg font-semibold">
              {occupied}/{board.length}
            </p>
          </div>
          <label className="relative block w-72">
            <Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--muted)]" size={16} />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Buscar habitación o huésped"
              className="h-12 w-full rounded-2xl border border-[var(--line)] bg-[var(--surface)] pl-10 pr-4 outline-none focus:border-[var(--accent)]"
            />
          </label>
        </div>
      </header>
      {error ? <p className="mt-4 text-[var(--danger)]">{error}</p> : null}
      <div className="mt-6 grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-4">
        {filtered.map((item) => (
          <RoomCard
            key={item.room.id}
            item={item}
            currency={settings.currency_symbol}
            onClick={() => setSelected(item)}
          />
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
