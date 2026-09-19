import { BedDouble, BrushCleaning, CalendarClock, Check, X } from "lucide-react";
import { RoomCard, RoomStatusLegend } from "@/components/board/RoomCard";
import { RoomDrawer } from "@/components/board/RoomDrawer";
import { api } from "@/lib/api";
import type { AppSettings, BoardRoom, DeviceMode, RatePlan } from "@/lib/types";
import { useEffect, useMemo, useState } from "react";

export function BoardPage({ settings, deviceMode = "reception" }: { settings: AppSettings; deviceMode?: DeviceMode }) {
  const [board, setBoard] = useState<BoardRoom[]>([]);
  const [rates, setRates] = useState<RatePlan[]>([]);
  const [statusFilter, setStatusFilter] = useState<string[]>([]);
  const [selected, setSelected] = useState<BoardRoom | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [liveNote, setLiveNote] = useState<string | null>(null);

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
    void load();
    const id = window.setInterval(load, deviceMode === "remote" ? 60000 : 20000);
    function onCatalog() {
      void load();
    }
    window.addEventListener("sync:catalog-updated", onCatalog);
    let unsub = () => {};
    if (deviceMode === "remote") {
      void api.subscribeOperational(() => {
        setLiveNote(`Actualizado ${new Date().toLocaleTimeString()}`);
        void load();
      }).then((fn) => {
        unsub = fn;
      });
    }
    return () => {
      window.clearInterval(id);
      window.removeEventListener("sync:catalog-updated", onCatalog);
      unsub();
    };
  }, [deviceMode]);

  const filtered = useMemo(() => {
    return board
      .filter((item) => !statusFilter.length || statusFilter.includes(item.display_status))
      .slice()
      .sort((a, b) => a.room.number.localeCompare(b.room.number, undefined, { numeric: true }));
  }, [board, statusFilter]);

  const occupied = board.filter((r) => r.display_status === "occupied").length;

  return (
    <div className="board-page px-6 py-6 lg:px-8">
      <header className="board-header">
        <div>
          <p className="page-kicker">Recepción / Vista general</p>
          <h1 className="page-title">Tablero de habitaciones</h1>
          {deviceMode === "remote" && (
            <p className="text-sm text-[var(--muted)]">Solo lectura · {liveNote ?? "en vivo vía Supabase"}</p>
          )}
        </div>
        <p className="occupancy-compact">{occupied} de {board.length} ocupadas</p>
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
      <RoomStatusLegend />
      {error ? <p className="mt-4 text-[var(--danger)]">{error}</p> : null}
      <div className="mt-6">
        {filtered.length === 0 && !error ? (
          <p className="mt-8 text-sm text-[var(--muted)]">
            {statusFilter.length
              ? "No hay habitaciones para este filtro."
              : "No hay habitaciones."}
          </p>
        ) : (
          <div className="room-grid">
            {filtered.map((item) => (
              <RoomCard
                key={item.room.id}
                item={item}
                currency={settings.currency_symbol}
                onClick={() => setSelected(item)}
              />
            ))}
          </div>
        )}
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
