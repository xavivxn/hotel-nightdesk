import { Button } from "@/components/ui/Button";
import { Drawer } from "@/components/ui/Drawer";
import { Field, Input, Select } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { formatDateTime, localInputToRfc3339, statusLabel, toDateTimeLocal } from "@/lib/format";
import type { RatePlan, Reservation, Room } from "@/lib/types";
import { useEffect, useState } from "react";

export function ReservationsPage() {
  const [items, setItems] = useState<Reservation[]>([]);
  const [rooms, setRooms] = useState<Room[]>([]);
  const [rates, setRates] = useState<RatePlan[]>([]);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState({
    guest_name: "",
    document: "",
    phone: "",
    room_id: 0,
    rate_plan_id: 0,
    expected_arrival_at: toDateTimeLocal(),
    expected_nights: 1,
  });

  async function load() {
    const [reservations, roomList, rateList] = await Promise.all([
      api.listReservations(),
      api.listRooms(),
      api.listRatePlans(true),
    ]);
    setItems(reservations);
    setRooms(roomList);
    setRates(rateList);
    setForm((current) => ({
      ...current,
      room_id: current.room_id || roomList[0]?.id || 0,
      rate_plan_id: current.rate_plan_id || rateList.find((r) => r.kind === "night")?.id || rateList[0]?.id || 0,
    }));
  }

  useEffect(() => {
    load().catch((e) => setError(String(e)));
  }, []);

  return (
    <div className="px-6 py-6 lg:px-8">
      <header className="flex items-end justify-between gap-4">
        <div>
          <p className="page-kicker">Agenda</p>
          <h1 className="page-title">Reservas</h1>
        </div>
        <Button onClick={() => setOpen(true)}>Nueva reserva</Button>
      </header>
      {error ? <p className="mt-4 text-[var(--danger)]">{error}</p> : null}
      <div className="mt-6 overflow-hidden rounded-lg border border-[var(--line)] bg-[var(--surface)]">
        <table className="w-full text-left text-sm">
          <thead className="bg-[var(--surface-2)] text-xs uppercase tracking-[0.12em] text-[var(--muted)]">
            <tr>
              <th className="px-4 py-3">Habitación</th>
              <th className="px-4 py-3">Llegada</th>
              <th className="px-4 py-3">Noches</th>
              <th className="px-4 py-3">Estado</th>
              <th className="px-4 py-3"></th>
            </tr>
          </thead>
          <tbody>
            {items.map((item) => (
              <tr key={item.id} className="border-t border-[var(--line)]">
                <td className="px-4 py-3 font-mono tabular-nums">{item.room_number}</td>
                <td className="px-4 py-3">{formatDateTime(item.expected_arrival_at)}</td>
                <td className="px-4 py-3">{item.expected_nights}</td>
                <td className="px-4 py-3">{statusLabel(item.status)}</td>
                <td className="px-4 py-3 text-right">
                  {item.status === "hold" ? (
                    <div className="flex justify-end gap-2">
                      <Button size="sm" onClick={async () => { await api.checkInReservation(item.id); await load(); }}>
                        Check-in
                      </Button>
                      <Button size="sm" variant="secondary" onClick={async () => { await api.setReservationStatus(item.id, "cancelled"); await load(); }}>
                        Cancelar
                      </Button>
                      <Button size="sm" variant="ghost" onClick={async () => { await api.setReservationStatus(item.id, "no_show"); await load(); }}>
                        No show
                      </Button>
                    </div>
                  ) : null}
                </td>
              </tr>
            ))}
            {items.length === 0 ? (
              <tr>
                <td className="px-4 py-10 text-center text-[var(--muted)]" colSpan={5}>
                  No hay reservas. Creá una para bloquear una habitación el día de llegada.
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
      <Drawer open={open} title="Nueva reserva" onClose={() => setOpen(false)}>
        <div className="space-y-4">
          <Field label="Habitación">
            <Select value={form.room_id} onChange={(e) => setForm({ ...form, room_id: Number(e.target.value) })}>
              {rooms.map((room) => (
                <option key={room.id} value={room.id}>
                  {room.number} · {room.room_type}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Tarifa">
            <Select value={form.rate_plan_id} onChange={(e) => setForm({ ...form, rate_plan_id: Number(e.target.value) })}>
              {rates.map((rate) => (
                <option key={rate.id} value={rate.id}>
                  {rate.name}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Llegada">
            <Input
              type="datetime-local"
              value={form.expected_arrival_at}
              onChange={(e) => setForm({ ...form, expected_arrival_at: e.target.value })}
            />
          </Field>
          <Field label="Noches">
            <Input
              type="number"
              min={1}
              value={form.expected_nights}
              onChange={(e) => setForm({ ...form, expected_nights: Number(e.target.value) })}
            />
          </Field>
          <Button
            className="w-full"
            onClick={async () => {
              try {
                await api.createReservation({
                  ...form,
                  guest_name: "",
                  document: null,
                  phone: null,
                  expected_arrival_at: localInputToRfc3339(form.expected_arrival_at),
                });
                setOpen(false);
                await load();
              } catch (e) {
                setError(String(e));
              }
            }}
          >
            Guardar reserva
          </Button>
        </div>
      </Drawer>
    </div>
  );
}
