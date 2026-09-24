import { Button } from "@/components/ui/Button";
import { Drawer } from "@/components/ui/Drawer";
import { Field, Input, Select, reportInputIssue } from "@/components/ui/Field";
import { api } from "@/lib/api";
import {
  FIELD_EMPTY,
  formatMoney,
  guaraniesToInput,
  parseIntegerField,
  parseMoneyInteger,
  rateKindLabel,
  requireTrimmed,
  statusLabel,
} from "@/lib/format";
import type { AppSettings, RateKind, RatePlan, Room } from "@/lib/types";
import { useEffect, useRef, useState } from "react";

type RoomDraft = {
  id: number;
  number: string;
  room_type: string;
  floor: string;
  notes: string | null;
  version: number;
};

type RateDraft = {
  id: number;
  name: string;
  kind: RateKind;
  base_amount: string;
  extra_amount: string;
  included_hours: string;
  grace_minutes: string;
  night_cutoff_hour: string;
  active: boolean;
  version: number;
};

function roomDraft(room?: Room): RoomDraft {
  return {
    id: room?.id ?? 0,
    number: room?.number ?? "",
    room_type: room?.room_type ?? "Estándar",
    floor: String(room?.floor ?? 1),
    notes: room?.notes ?? null,
    version: room?.version ?? 1,
  };
}

function rateDraft(rate?: RatePlan): RateDraft {
  return {
    id: rate?.id ?? 0,
    name: rate?.name ?? "",
    kind: rate?.kind ?? "hourly",
    base_amount: rate ? guaraniesToInput(rate.base_amount_cents) : "",
    extra_amount: rate ? guaraniesToInput(rate.extra_hour_cents) : "",
    included_hours: String(rate?.included_hours ?? 1),
    grace_minutes: String(rate?.grace_minutes ?? 5),
    night_cutoff_hour: String(rate?.night_cutoff_hour ?? 10),
    active: rate?.active ?? true,
    version: rate?.version ?? 1,
  };
}

export function RoomsPage({ settings }: { settings: AppSettings }) {
  const [rooms, setRooms] = useState<Room[]>([]);
  const [rates, setRates] = useState<RatePlan[]>([]);
  const [roomForm, setRoomForm] = useState<RoomDraft | null>(null);
  const [rateForm, setRateForm] = useState<RateDraft | null>(null);
  const [error, setError] = useState<string | null>(null);
  const roomNumberRef = useRef<HTMLInputElement>(null);
  const roomTypeRef = useRef<HTMLInputElement>(null);
  const roomFloorRef = useRef<HTMLInputElement>(null);
  const rateNameRef = useRef<HTMLInputElement>(null);
  const rateBaseRef = useRef<HTMLInputElement>(null);
  const rateExtraRef = useRef<HTMLInputElement>(null);
  const rateHoursRef = useRef<HTMLInputElement>(null);
  const rateGraceRef = useRef<HTMLInputElement>(null);
  const rateCutoffRef = useRef<HTMLInputElement>(null);

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
          <Button variant="secondary" onClick={() => setRateForm(rateDraft())}>
            Nueva tarifa
          </Button>
          <Button onClick={() => setRoomForm(roomDraft())}>
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
                  <Button size="sm" variant="ghost" onClick={() => setRoomForm(roomDraft(room))}>Editar</Button>
                </div>
              </div>
            ))}
          </div>
        </section>
        <section className="card rounded-lg p-4">
          <h2 className="mb-3 text-lg font-semibold tracking-tight">Tarifas</h2>
          <div className="space-y-2">
            {rates.map((rate) => (
              <button key={rate.id} className="flex w-full items-center justify-between rounded-lg bg-[var(--surface-2)] px-3 py-3 text-left" onClick={() => setRateForm(rateDraft(rate))}>
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
              <Input ref={roomNumberRef} value={roomForm.number} onChange={(e) => setRoomForm({ ...roomForm, number: e.target.value })} />
            </Field>
            <Field label="Tipo">
              <Input ref={roomTypeRef} value={roomForm.room_type} onChange={(e) => setRoomForm({ ...roomForm, room_type: e.target.value })} />
            </Field>
            <Field label="Piso">
              <Input
                ref={roomFloorRef}
                inputMode="numeric"
                value={roomForm.floor}
                onChange={(e) => setRoomForm({ ...roomForm, floor: e.target.value })}
              />
            </Field>
            <Button className="w-full" onClick={async () => {
              const number = requireTrimmed(roomForm.number);
              const room_type = requireTrimmed(roomForm.room_type);
              if (!number) {
                reportInputIssue(roomNumberRef.current, FIELD_EMPTY);
                return;
              }
              if (!room_type) {
                reportInputIssue(roomTypeRef.current, FIELD_EMPTY);
                return;
              }
              const floor = parseIntegerField(roomForm.floor, { min: 0, max: 99 });
              if (floor == null) {
                reportInputIssue(roomFloorRef.current, "Ingresá un piso válido (0 a 99).");
                return;
              }
              setError(null);
              await api.saveRoom({
                id: roomForm.id || null,
                number,
                room_type,
                floor,
                notes: roomForm.notes,
                expected_version: roomForm.id ? roomForm.version : null,
              });
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
              <Input ref={rateNameRef} value={rateForm.name} onChange={(e) => setRateForm({ ...rateForm, name: e.target.value })} />
            </Field>
            <Field label="Tipo">
              <Select value={rateForm.kind} onChange={(e) => setRateForm({ ...rateForm, kind: e.target.value as RateKind })}>
                <option value="hourly">Por hora</option>
                <option value="night">Por noche</option>
                <option value="overnight">Dormida</option>
              </Select>
            </Field>
            <Field label="Monto base (Gs.)">
              <Input
                ref={rateBaseRef}
                inputMode="numeric"
                value={rateForm.base_amount}
                onChange={(e) => setRateForm({ ...rateForm, base_amount: e.target.value })}
              />
            </Field>
            <Field label="Adicional 30 min (Gs.)">
              <Input
                ref={rateExtraRef}
                inputMode="numeric"
                value={rateForm.extra_amount}
                onChange={(e) => setRateForm({ ...rateForm, extra_amount: e.target.value })}
              />
            </Field>
            <div className="grid grid-cols-3 gap-2">
              <Field label="Horas incl.">
                <Input
                  ref={rateHoursRef}
                  inputMode="numeric"
                  value={rateForm.included_hours}
                  onChange={(e) => setRateForm({ ...rateForm, included_hours: e.target.value })}
                />
              </Field>
              <Field label="Gracia (min)">
                <Input
                  ref={rateGraceRef}
                  inputMode="numeric"
                  value={rateForm.grace_minutes}
                  onChange={(e) => setRateForm({ ...rateForm, grace_minutes: e.target.value })}
                />
              </Field>
              <Field label="Corte">
                <Input
                  ref={rateCutoffRef}
                  inputMode="numeric"
                  value={rateForm.night_cutoff_hour}
                  onChange={(e) => setRateForm({ ...rateForm, night_cutoff_hour: e.target.value })}
                />
              </Field>
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={rateForm.active} onChange={(e) => setRateForm({ ...rateForm, active: e.target.checked })} />
              Activa
            </label>
            <Button className="w-full" onClick={async () => {
              const name = requireTrimmed(rateForm.name);
              if (!name) {
                reportInputIssue(rateNameRef.current, FIELD_EMPTY);
                return;
              }
              const base = parseMoneyInteger(rateForm.base_amount);
              if (base == null || base < 0) {
                reportInputIssue(rateBaseRef.current, "Ingresá un monto entero en guaraníes.");
                return;
              }
              const extra = parseMoneyInteger(rateForm.extra_amount);
              if (extra == null || extra < 0) {
                reportInputIssue(rateExtraRef.current, "Ingresá un monto entero en guaraníes.");
                return;
              }
              const included_hours = parseIntegerField(rateForm.included_hours, { min: 1, max: 72 });
              if (included_hours == null) {
                reportInputIssue(rateHoursRef.current, "Ingresá horas incluidas (1 o más).");
                return;
              }
              const grace_minutes = parseIntegerField(rateForm.grace_minutes, { min: 0, max: 180 });
              if (grace_minutes == null) {
                reportInputIssue(rateGraceRef.current, "Ingresá minutos de gracia.");
                return;
              }
              const night_cutoff_hour = parseIntegerField(rateForm.night_cutoff_hour, { min: 0, max: 23 });
              if (night_cutoff_hour == null) {
                reportInputIssue(rateCutoffRef.current, "Ingresá la hora de corte (0 a 23).");
                return;
              }
              setError(null);
              await api.saveRatePlan({
                id: rateForm.id || null,
                name,
                kind: rateForm.kind,
                base_amount_cents: base,
                extra_hour_cents: extra,
                included_hours,
                grace_minutes,
                night_cutoff_hour,
                active: rateForm.active,
                expected_version: rateForm.id ? rateForm.version : null,
              });
              setRateForm(null);
              await load();
            }}>Guardar tarifa</Button>
          </div>
        ) : null}
      </Drawer>
    </div>
  );
}
