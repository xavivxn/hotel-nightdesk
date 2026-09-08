import { BedDouble, BrushCleaning, CalendarClock, Check, X } from "lucide-react";
import { RoomCard, RoomStatusLegend } from "@/components/board/RoomCard";
import { RoomDrawer } from "@/components/board/RoomDrawer";
import { api } from "@/lib/api";
import type { AppSettings, BoardRoom, RatePlan } from "@/lib/types";
import { useEffect, useMemo, useState } from "react";

export function BoardPage({ settings }: { settings: AppSettings }) {
  const [board, setBoard] = useState<BoardRoom[]>([]);
  const [rates, setRates] = useState<RatePlan[]>([]);
  const [statusFilter, setStatusFilter] = useState<string[]>([]);
  const [selected, setSelected] = useState<BoardRoom | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function load() {
    try {
      const [rooms, plans] = await Promise.all([api.listBoard(), api.listRatePlans(true)]);
      setError(null);
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
    return board.filter((item) => !statusFilter.length || statusFilter.includes(item.display_status));
  }, [board, statusFilter]);

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
  const baseRateCents = rates[0]?.base_amount_cents ?? null;

  return (
    <div className="board-page px-6 py-6 lg:px-8">
      <header className="board-header">
        <div><p className="page-kicker">Recepción / Vista general</p><h1 className="page-title">Tablero de habitaciones</h1><p className="page-description">Cada habitación, cada movimiento. Todo bajo control.</p></div>
        <div className="occupancy-overview"><span className="occupancy-number">{occupancyPct}<small>%</small></span><div><p>Ocupación actual</p><span>{occupied} de {board.length} habitaciones</span><div className="occupancy-track"><div style={{ width: `${occupancyPct}%` }} /></div></div></div>
      </header>
      <div className="board-stats" aria-label="Resumen de habitaciones">
        {[
          { status: "available", label: "Disponibles", hint: "Listas para recibir", icon: Check },
          { status: "occupied", label: "Ocupadas", hint: "Estadías en curso", icon: BedDouble },
          { status: "dirty", label: "Por limpiar", hint: "Pendientes de aseo", icon: BrushCleaning },
          { status: "reserved", label: "Reservadas", hint: "Próximas llegadas", icon: CalendarClock },
        ].map(({ status, label, hint, icon: Icon }) => <button key={status} type="button" className={`stat-card room-${status}`} aria-pressed={statusFilter.includes(status)} onClick={() => setStatusFilter(current => current.length === 1 && current[0] === status ? [] : [status])}><div className="stat-icon"><Icon size={20} /></div><div><p>{label}</p><span>{hint}</span></div><strong>{board.filter(room => room.display_status === status).length}</strong></button>)}
      </div>
      <div className="board-toolbar">
        <span className="toolbar-count">{filtered.length} habitaciones</span>
        {statusFilter.length > 0 ? <button className="clear-filters" onClick={() => { setStatusFilter([]); }}><X size={14} /> Limpiar filtros</button> : null}
      </div>
      <RoomStatusLegend
        active={statusFilter}
        onToggle={(status) => {
          setStatusFilter((current) =>
            current.includes(status) ? current.filter((item) => item !== status) : [...current, status],
          );
        }}
      />
      {error ? <p className="mt-4 text-[var(--danger)]">{error}</p> : null}
      <div className="mt-2">
        {grouped.length === 0 && !error ? (
          <p className="mt-8 text-sm text-[var(--muted)]">
            {statusFilter.length
              ? "No hay habitaciones para este filtro."
              : "No hay habitaciones."}
          </p>
        ) : null}
        {grouped.map(([floor, rooms]) => (
          <section key={floor} className="mt-6">
            <div className="floor-heading mb-3 flex items-center gap-3">
              <p className="font-mono text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--muted)]">
                Piso {floor}
              </p>
              <div className="h-px flex-1 bg-[var(--line)]" />
              <p className="font-mono text-[11px] tabular-nums text-[var(--muted)]">{rooms.length} habitaciones</p>
            </div>
            <div className="room-grid">
              {rooms.map((item) => (
                <RoomCard
                  key={item.room.id}
                  item={item}
                  currency={settings.currency_symbol}
                  baseRateCents={baseRateCents}
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
