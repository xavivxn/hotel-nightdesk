import { Button } from "@/components/ui/Button";
import { Drawer } from "@/components/ui/Drawer";
import { Field, Input, Select } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { centsToInput, formatMoney, pesosToCents, rateKindLabel, statusLabel } from "@/lib/format";
import type { AppSettings, RateKind, RatePlan, Room } from "@/lib/types";
import { useEffect, useState } from "react";

export function RoomsPage({ settings }: { settings: AppSettings }) {
  const [rooms, setRooms] = useState<Room[]>([]);
  const [rates, setRates] = useState<RatePlan[]>([]);
  const [roomForm, setRoomForm] = useState<Room | null>(null);
  const [rateForm, setRateForm] = useState<RatePlan | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function load() {
    const [roomList, rateList] = await Promise.all([api.listRooms(), api.listRatePlans(false)]);
    setRooms(roomList);
    setRates(rateList);
  }

  useEffect(() => {
    load().catch((e) => setError(String(e)));
  }, []);

  return (
    <div className="px-6 py-6 lg:px-8">
      <header className="flex items-end justify-between">
        <div>
          <p className="page-kicker">Catálogo</p>
          <h1 className="page-title">Habitaciones y tarifas</h1>
        </div>
        <div className="flex gap-2">
          <Button variant="secondary" onClick={() => setRateForm({
            id: 0,
            name: "",
            kind: "hourly",
            base_amount_cents: 0,
            extra_hour_cents: 0,
            included_hours: 3,
            grace_minutes: 10,
            night_cutoff_hour: 12,
            active: true,
          })}>
            Nueva tarifa
          </Button>
          <Button onClick={() => setRoomForm({ id: 0, number: "", room_type: "Estándar", floor: 1, status: "available", notes: null })}>
            Nueva habitación
          </Button>
        </div>
      </header>
      {error ? <p className="mt-4 text-[var(--danger)]">{error}</p> : null}
      <div className="mt-6 grid gap-6 xl:grid-cols-2">
        <section className="card rounded-lg p-4">
          <h2 className="mb-3 text-lg font-semibold tracking-tight">Habitaciones</h2>
          <div className="space-y-2">
            {rooms.map((room) => (
              <div key={room.id} className="flex items-center justify-between rounded-lg bg-[var(--surface-2)] px-3 py-3">
                <div>
                  <p className="font-semibold"><span className="font-mono tabular-nums">{room.number}</span> · {room.room_type}</p>
                  <p className="text-xs text-[var(--muted)]">Piso {room.floor} · {statusLabel(room.status)}</p>
                </div>
                <div className="flex gap-2">
                  {room.status === "dirty" ? (
                    <Button size="sm" variant="secondary" onClick={async () => { await api.setRoomStatus(room.id, "available"); await load(); }}>
                      Limpia
                    </Button>
                  ) : null}
                  {room.status === "blocked" ? (
                    <Button size="sm" variant="secondary" onClick={async () => { await api.setRoomStatus(room.id, "available"); await load(); }}>
                      Desbloquear
                    </Button>
                  ) : room.status === "available" ? (
                    <Button size="sm" variant="ghost" onClick={async () => { await api.setRoomStatus(room.id, "blocked"); await load(); }}>
                      Bloquear
                    </Button>
                  ) : null}
                  <Button size="sm" variant="ghost" onClick={() => setRoomForm(room)}>Editar</Button>
                </div>
              </div>
            ))}
          </div>
        </section>
        <section className="card rounded-lg p-4">
          <h2 className="mb-3 text-lg font-semibold tracking-tight">Tarifas</h2>
          <div className="space-y-2">
            {rates.map((rate) => (
              <button key={rate.id} className="flex w-full items-center justify-between rounded-lg bg-[var(--surface-2)] px-3 py-3 text-left" onClick={() => setRateForm(rate)}>
                <div>
                  <p className="font-semibold">{rate.name}</p>
                  <p className="text-xs text-[var(--muted)]">
                    {rateKindLabel(rate.kind)} · <span className="font-mono tabular-nums">{formatMoney(rate.base_amount_cents, settings.currency_symbol)}</span>
                    {rate.active ? "" : " · inactiva"}
                  </p>
                </div>
              </button>
            ))}
          </div>
        </section>
      </div>
      <Drawer open={Boolean(roomForm)} title={roomForm?.id ? "Editar habitación" : "Nueva habitación"} onClose={() => setRoomForm(null)}>
        {roomForm ? (
          <div className="space-y-4">
            <Field label="Número">
              <Input value={roomForm.number} onChange={(e) => setRoomForm({ ...roomForm, number: e.target.value })} />
            </Field>
            <Field label="Tipo">
              <Input value={roomForm.room_type} onChange={(e) => setRoomForm({ ...roomForm, room_type: e.target.value })} />
            </Field>
            <Field label="Piso">
              <Input type="number" value={roomForm.floor} onChange={(e) => setRoomForm({ ...roomForm, floor: Number(e.target.value) })} />
            </Field>
            <Button className="w-full" onClick={async () => {
              await api.saveRoom({ ...roomForm, id: roomForm.id || null });
              setRoomForm(null);
              await load();
            }}>Guardar</Button>
          </div>
        ) : null}
      </Drawer>
      <Drawer open={Boolean(rateForm)} title={rateForm?.id ? "Editar tarifa" : "Nueva tarifa"} onClose={() => setRateForm(null)}>
        {rateForm ? (
          <div className="space-y-4">
            <Field label="Nombre">
              <Input value={rateForm.name} onChange={(e) => setRateForm({ ...rateForm, name: e.target.value })} />
            </Field>
            <Field label="Tipo">
              <Select value={rateForm.kind} onChange={(e) => setRateForm({ ...rateForm, kind: e.target.value as RateKind })}>
                <option value="hourly">Por hora</option>
                <option value="night">Por noche</option>
                <option value="overnight">Pernocte</option>
              </Select>
            </Field>
            <Field label="Monto base">
              <Input defaultValue={centsToInput(rateForm.base_amount_cents)} onBlur={(e) => setRateForm({ ...rateForm, base_amount_cents: pesosToCents(e.target.value) })} />
            </Field>
            <Field label="Hora extra">
              <Input defaultValue={centsToInput(rateForm.extra_hour_cents)} onBlur={(e) => setRateForm({ ...rateForm, extra_hour_cents: pesosToCents(e.target.value) })} />
            </Field>
            <div className="grid grid-cols-3 gap-2">
              <Field label="Horas incl.">
                <Input type="number" value={rateForm.included_hours} onChange={(e) => setRateForm({ ...rateForm, included_hours: Number(e.target.value) })} />
              </Field>
              <Field label="Gracia (min)">
                <Input type="number" value={rateForm.grace_minutes} onChange={(e) => setRateForm({ ...rateForm, grace_minutes: Number(e.target.value) })} />
              </Field>
              <Field label="Corte">
                <Input type="number" value={rateForm.night_cutoff_hour} onChange={(e) => setRateForm({ ...rateForm, night_cutoff_hour: Number(e.target.value) })} />
              </Field>
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={rateForm.active} onChange={(e) => setRateForm({ ...rateForm, active: e.target.checked })} />
              Activa
            </label>
            <Button className="w-full" onClick={async () => {
              await api.saveRatePlan({ ...rateForm, id: rateForm.id || null });
              setRateForm(null);
              await load();
            }}>Guardar tarifa</Button>
          </div>
        ) : null}
      </Drawer>
    </div>
  );
}
