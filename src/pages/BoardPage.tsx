import { BedDouble, BrushCleaning, CalendarClock, Check, X } from "lucide-react";
import { RoomCard, RoomStatusLegend } from "@/components/board/RoomCard";
import { RoomDrawer } from "@/components/board/RoomDrawer";
import { FlipStat } from "@/components/ui/FlipStat";
import { api } from "@/lib/api";
import { playStatusPulse, runViewTransition } from "@/lib/board-motion";
import type { AppSettings, BoardRoom, DeviceMode, RatePlan } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";

export function BoardPage({
  settings,
  deviceMode = "reception",
}: {
  settings: AppSettings;
  deviceMode?: DeviceMode;
}) {
  const [board, setBoard] = useState<BoardRoom[]>([]);
  const [rates, setRates] = useState<RatePlan[]>([]);
  const [statusFilter, setStatusFilter] = useState<string[]>([]);
  const [selected, setSelected] = useState<BoardRoom | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [liveNote, setLiveNote] = useState<string | null>(null);

  const cardEls = useRef(new Map<number, HTMLButtonElement>());
  const statEls = useRef(new Map<string, HTMLElement>());
  const prevStatus = useRef(new Map<number, string>());
  const primed = useRef(false);

  async function load() {
    try {
      const [rooms, plans] = await Promise.all([api.listBoard(), api.listRatePlans(true)]);
      setError(null);
      setBoard(rooms);
      setRates(plans);
      setSelected((current) =>
        current ? (rooms.find((r) => r.room.id === current.room.id) ?? null) : null,
      );
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

  useEffect(() => {
    if (!primed.current) {
      for (const room of board) {
        prevStatus.current.set(room.room.id, room.display_status);
      }
      primed.current = board.length > 0;
      return;
    }
    for (const room of board) {
      const prev = prevStatus.current.get(room.room.id);
      if (prev && prev !== room.display_status) {
        const card = cardEls.current.get(room.room.id);
        const stat = statEls.current.get(room.display_status);
        if (card && stat) playStatusPulse(card, stat, room.display_status);
      }
      prevStatus.current.set(room.room.id, room.display_status);
    }
  }, [board]);

  const filtered = useMemo(() => {
    return board
      .filter((item) => !statusFilter.length || statusFilter.includes(item.display_status))
      .slice()
      .sort((a, b) => a.room.number.localeCompare(b.room.number, undefined, { numeric: true }));
  }, [board, statusFilter]);

  const occupied = board.filter((r) => r.display_status === "occupied").length;

  const onStatusChanged = useCallback((_roomId: number, _status: string, el: HTMLButtonElement) => {
    cardEls.current.set(_roomId, el);
  }, []);

  function openRoom(item: BoardRoom) {
    setSelected(item);
  }

  function setFilter(next: string[]) {
    runViewTransition(() => setStatusFilter(next));
  }

  const stats = [
    { status: "available", label: "Disponibles", hint: "Listas para recibir", icon: Check },
    { status: "occupied", label: "Ocupadas", hint: "Estadías en curso", icon: BedDouble },
    { status: "dirty", label: "Por limpiar", hint: "Pendientes de aseo", icon: BrushCleaning },
    { status: "reserved", label: "Reservadas", hint: "Próximas llegadas", icon: CalendarClock },
  ] as const;

  return (
    <div className="board-page board-page--enter px-6 py-6 lg:px-8">
      <header className="board-header">
        <div>
          <p className="page-kicker">Recepción / Vista general</p>
          <h1 className="page-title">Tablero de habitaciones</h1>
          {deviceMode === "remote" && (
            <p className="text-sm text-[var(--muted)]">
              Solo lectura · {liveNote ?? "en vivo vía Supabase"}
            </p>
          )}
        </div>
        <p className="occupancy-compact">
          {occupied} de {board.length} ocupadas
        </p>
      </header>
      <div className="board-stats" aria-label="Resumen de habitaciones">
        {stats.map(({ status, label, hint, icon: Icon }, i) => (
          <button
            key={status}
            type="button"
            ref={(node) => {
              if (node) statEls.current.set(status, node);
              else statEls.current.delete(status);
            }}
            className={cn("stat-card", `room-${status}`, "stat-card--press")}
            style={{ "--enter-delay": `${80 + i * 60}ms` } as CSSProperties}
            aria-pressed={statusFilter.includes(status)}
            onClick={() =>
              setFilter(
                statusFilter.length === 1 && statusFilter[0] === status ? [] : [status],
              )
            }
          >
            <div className="stat-icon">
              <Icon size={20} />
            </div>
            <div>
              <p>{label}</p>
              <span>{hint}</span>
            </div>
            <FlipStat value={board.filter((room) => room.display_status === status).length} />
          </button>
        ))}
      </div>
      <div className="board-toolbar">
        <span className="toolbar-count">{filtered.length} habitaciones</span>
        {statusFilter.length > 0 ? (
          <button type="button" className="clear-filters" onClick={() => setFilter([])}>
            <X size={14} /> Limpiar filtros
          </button>
        ) : null}
      </div>
      <RoomStatusLegend />
      {error ? <p className="mt-4 text-[var(--danger)]">{error}</p> : null}
      <div className="mt-6">
        {filtered.length === 0 && !error ? (
          <p className="mt-8 text-sm text-[var(--muted)]">
            {statusFilter.length ? "No hay habitaciones para este filtro." : "No hay habitaciones."}
          </p>
        ) : (
          <div className="room-grid">
            {filtered.map((item, i) => (
              <RoomCard
                key={item.room.id}
                item={item}
                currency={settings.currency_symbol}
                enterDelayMs={Math.min(i * 45, 420)}
                onStatusChanged={onStatusChanged}
                ref={(node) => {
                  if (node) cardEls.current.set(item.room.id, node);
                  else cardEls.current.delete(item.room.id);
                }}
                onClick={() => openRoom(item)}
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
