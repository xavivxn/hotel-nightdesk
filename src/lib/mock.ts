import { previewBill } from "./billing";
import type {
  AppSettings,
  BoardRoom,
  Charge,
  CheckInPayload,
  CheckOutPayload,
  CheckOutResult,
  HistoryStay,
  Payment,
  RatePlan,
  Reservation,
  Room,
  Stay,
} from "./types";

type Db = {
  rooms: Room[];
  rates: RatePlan[];
  guests: { id: number; name: string; document: string | null; phone: string | null }[];
  reservations: Reservation[];
  stays: Stay[];
  charges: Charge[];
  payments: Payment[];
  settings: AppSettings;
  ids: { room: number; rate: number; guest: number; reservation: number; stay: number; charge: number; payment: number };
};

const KEY = "nightdesk.mock.v1";

function nowIso() {
  const date = new Date();
  const pad = (n: number) => n.toString().padStart(2, "0");
  const offset = -date.getTimezoneOffset();
  const sign = offset >= 0 ? "+" : "-";
  const hours = pad(Math.floor(Math.abs(offset) / 60));
  const minutes = pad(Math.abs(offset) % 60);
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}${sign}${hours}:${minutes}`;
}

function seed(): Db {
  const seedRooms: Array<[string, string, number]> = [
    ["101", "Estándar", 1],
    ["102", "Estándar", 1],
    ["103", "Estándar", 1],
    ["104", "Estándar", 1],
    ["105", "Estándar", 1],
    ["106", "Suite", 1],
    ["201", "Estándar", 2],
    ["202", "Estándar", 2],
    ["203", "Estándar", 2],
    ["204", "Suite", 2],
  ];
  const rooms: Room[] = seedRooms.map(([number, room_type, floor], i) => ({
    id: i + 1,
    number,
    room_type,
    floor: Number(floor),
    status: "available",
    notes: null,
  }));
  const rates: RatePlan[] = [
    {
      id: 1,
      name: "3 horas",
      kind: "hourly",
      base_amount_cents: 1_800_000,
      extra_hour_cents: 500_000,
      included_hours: 3,
      grace_minutes: 10,
      night_cutoff_hour: 12,
      active: true,
    },
    {
      id: 2,
      name: "Noche",
      kind: "night",
      base_amount_cents: 4_500_000,
      extra_hour_cents: 600_000,
      included_hours: 24,
      grace_minutes: 15,
      night_cutoff_hour: 12,
      active: true,
    },
    {
      id: 3,
      name: "Pernocte",
      kind: "overnight",
      base_amount_cents: 3_200_000,
      extra_hour_cents: 500_000,
      included_hours: 12,
      grace_minutes: 15,
      night_cutoff_hour: 12,
      active: true,
    },
  ];
  return {
    rooms,
    rates,
    guests: [],
    reservations: [],
    stays: [],
    charges: [],
    payments: [],
    settings: {
      business_name: "Nightdesk Inn",
      address: "Av. Principal 100",
      phone: "",
      tax_percent: 0,
      currency_symbol: "$",
      theme: "dark",
      receipt_footer: "Gracias por su visita",
      printer_enabled: false,
      printer_path: "",
      printer_name: "",
      paper_width: 80,
      auto_print_on_checkout: true,
      pin_hash: "",
      has_pin: false,
    },
    ids: { room: 10, rate: 3, guest: 0, reservation: 0, stay: 0, charge: 0, payment: 0 },
  };
}

function load(): Db {
  const raw = localStorage.getItem(KEY);
  if (!raw) {
    const db = seed();
    save(db);
    return db;
  }
  try {
    return JSON.parse(raw) as Db;
  } catch {
    const db = seed();
    save(db);
    return db;
  }
}

function save(db: Db) {
  localStorage.setItem(KEY, JSON.stringify(db));
}

function fail(message: string): never {
  throw message;
}

function todayHold(db: Db, roomId: number) {
  const day = nowIso().slice(0, 10);
  return db.reservations.find(
    (r) => r.room_id === roomId && r.status === "hold" && r.expected_arrival_at.slice(0, 10) === day,
  );
}

function openStay(db: Db, roomId: number) {
  return db.stays.find((s) => s.room_id === roomId && s.status === "open");
}

function stayBill(db: Db, stay: Stay) {
  const rate = db.rates.find((r) => r.id === stay.rate_plan_id) ?? fail("Tarifa no encontrada");
  const overnight = stay.overnight_rate_plan_id
    ? db.rates.find((r) => r.id === stay.overnight_rate_plan_id)
    : db.rates.find((r) => r.kind === "overnight" && r.active);
  const night = db.rates.find((r) => r.kind === "night" && r.active);
  const manual = db.charges
    .filter((c) => c.stay_id === stay.id && (c.kind === "surcharge" || c.kind === "discount"))
    .map((c) => ({ kind: c.kind, description: c.description, amount_cents: c.amount_cents }));
  return previewBill({
    stayId: stay.id,
    checkIn: new Date(stay.check_in_at),
    now: stay.check_out_at ? new Date(stay.check_out_at) : new Date(),
    rate,
    converted: stay.converted_to_overnight,
    overnightPlan: overnight,
    nightPlan: night,
    manualLines: manual,
    taxPercent: db.settings.tax_percent,
  });
}

function hashPin(pin: string) {
  return `mock:${pin}`;
}

export async function mockInvoke<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  const db = load();
  const result = handle(db, name, args) as T;
  save(db);
  return result;
}

function handle(db: Db, name: string, args: Record<string, unknown>): unknown {
  switch (name) {
    case "list_board":
      return db.rooms.map((room) => {
        const stay = openStay(db, room.id) ?? null;
        const reservation = stay ? null : todayHold(db, room.id) ?? null;
        let display_status = "available";
        if (stay) display_status = "occupied";
        else if (room.status === "blocked") display_status = "blocked";
        else if (room.status === "dirty") display_status = "dirty";
        else if (reservation) display_status = "reserved";
        const bill = stay ? stayBill(db, stay) : null;
        const elapsed = stay
          ? Math.floor((Date.now() - new Date(stay.check_in_at).getTime()) / 60000)
          : null;
        const item: BoardRoom = {
          room,
          display_status,
          stay,
          reservation,
          estimated_total_cents: bill?.total_cents ?? null,
          elapsed_minutes: elapsed,
        };
        return item;
      });
    case "list_rooms":
      return db.rooms;
    case "save_room": {
      const payload = args.payload as { id?: number; number: string; room_type: string; floor: number; notes?: string | null };
      if (payload.id) {
        const room = db.rooms.find((r) => r.id === payload.id) ?? fail("Habitación no encontrada");
        Object.assign(room, { number: payload.number, room_type: payload.room_type, floor: payload.floor, notes: payload.notes ?? null });
        return room;
      }
      db.ids.room += 1;
      const room: Room = {
        id: db.ids.room,
        number: payload.number,
        room_type: payload.room_type,
        floor: payload.floor,
        status: "available",
        notes: payload.notes ?? null,
      };
      db.rooms.push(room);
      return room;
    }
    case "set_room_status": {
      const room = db.rooms.find((r) => r.id === args.room_id) ?? fail("Habitación no encontrada");
      if (openStay(db, room.id)) fail("No se puede cambiar el estado de una habitación ocupada");
      room.status = String(args.status);
      return room;
    }
    case "list_rate_plans":
      return args.active_only ? db.rates.filter((r) => r.active) : db.rates;
    case "save_rate_plan": {
      const payload = args.payload as RatePlan & { id?: number };
      if (payload.id) {
        const rate = db.rates.find((r) => r.id === payload.id) ?? fail("Tarifa no encontrada");
        Object.assign(rate, payload);
        return rate;
      }
      db.ids.rate += 1;
      const rate = { ...payload, id: db.ids.rate };
      db.rates.push(rate);
      return rate;
    }
    case "check_in": {
      const payload = args.payload as CheckInPayload;
      if (!payload.guest_name.trim()) fail("El nombre del huésped es obligatorio");
      const room = db.rooms.find((r) => r.id === payload.room_id) ?? fail("Habitación no encontrada");
      if (openStay(db, room.id)) fail("La habitación ya está ocupada");
      if (room.status === "blocked") fail("La habitación está bloqueada");
      const hold = todayHold(db, room.id);
      if (hold && payload.reservation_id !== hold.id) {
        fail(`La habitación ${room.number} tiene una reserva de ${hold.guest_name} para hoy`);
      }
      const rate = db.rates.find((r) => r.id === payload.rate_plan_id) ?? fail("Tarifa no encontrada");
      let guestId = 0;
      let reservationId: number | null = null;
      if (payload.reservation_id) {
        const res = db.reservations.find((r) => r.id === payload.reservation_id) ?? fail("Reserva no encontrada");
        res.status = "checked_in";
        guestId = res.guest_id;
        reservationId = res.id;
      } else {
        db.ids.guest += 1;
        guestId = db.ids.guest;
        db.guests.push({
          id: guestId,
          name: payload.guest_name.trim(),
          document: payload.document ?? null,
          phone: payload.phone ?? null,
        });
      }
      const guest = db.guests.find((g) => g.id === guestId)!;
      db.ids.stay += 1;
      const stay: Stay = {
        id: db.ids.stay,
        room_id: room.id,
        room_number: room.number,
        guest_id: guest.id,
        guest_name: guest.name,
        guest_document: guest.document,
        guest_phone: guest.phone,
        rate_plan_id: rate.id,
        rate_plan_name: rate.name,
        rate_kind: rate.kind,
        reservation_id: reservationId,
        check_in_at: nowIso(),
        expected_checkout_at: new Date(Date.now() + (payload.expected_hours ?? rate.included_hours) * 3600000).toISOString(),
        check_out_at: null,
        status: "open",
        converted_to_overnight: false,
        overnight_rate_plan_id: null,
        notes: null,
      };
      db.stays.push(stay);
      room.status = "occupied";
      return stay;
    }
    case "preview_bill": {
      const stay = db.stays.find((s) => s.id === args.stay_id) ?? fail("Estadía no encontrada");
      return stayBill(db, stay);
    }
    case "get_stay_detail": {
      const stay = db.stays.find((s) => s.id === args.stay_id) ?? fail("Estadía no encontrada");
      return [stay, stayBill(db, stay), db.charges.filter((c) => c.stay_id === stay.id), db.payments.filter((p) => p.stay_id === stay.id)];
    }
    case "convert_to_overnight": {
      const stay = db.stays.find((s) => s.id === args.stay_id) ?? fail("Estadía no encontrada");
      const overnight = db.rates.find((r) => r.kind === "overnight" && r.active) ?? db.rates.find((r) => r.kind === "night" && r.active);
      if (!overnight) fail("No hay una tarifa de pernocte o noche activa");
      stay.converted_to_overnight = true;
      stay.overnight_rate_plan_id = overnight.id;
      return stay;
    }
    case "add_charge": {
      const payload = args.payload as { stay_id: number; kind: string; description: string; amount_cents: number };
      const kind = payload.kind === "discount" || payload.amount_cents < 0 ? "discount" : "surcharge";
      const amount = kind === "discount" ? -Math.abs(payload.amount_cents) : Math.abs(payload.amount_cents);
      db.ids.charge += 1;
      const charge: Charge = {
        id: db.ids.charge,
        stay_id: payload.stay_id,
        kind,
        description: payload.description,
        amount_cents: amount,
        created_at: nowIso(),
      };
      db.charges.push(charge);
      return charge;
    }
    case "delete_charge":
      db.charges = db.charges.filter((c) => c.id !== args.charge_id || (c.kind !== "surcharge" && c.kind !== "discount"));
      return null;
    case "check_out": {
      const payload = args.payload as CheckOutPayload;
      const stay = db.stays.find((s) => s.id === payload.stay_id) ?? fail("Estadía no encontrada");
      const bill = stayBill(db, stay);
      if (payload.amount_cents < bill.total_cents) fail("El monto cobrado es menor al total");
      stay.status = "closed";
      stay.check_out_at = nowIso();
      const room = db.rooms.find((r) => r.id === stay.room_id)!;
      room.status = "dirty";
      db.ids.payment += 1;
      db.payments.push({
        id: db.ids.payment,
        stay_id: stay.id,
        method: payload.method,
        amount_cents: payload.amount_cents,
        created_at: stay.check_out_at,
      });
      const result: CheckOutResult = {
        stay,
        bill,
        print_error: payload.print && db.settings.printer_enabled ? "Simulación: no hay impresora en el navegador" : null,
      };
      return result;
    }
    case "list_reservations":
      return [...db.reservations].sort((a, b) => b.expected_arrival_at.localeCompare(a.expected_arrival_at));
    case "create_reservation": {
      const payload = args.payload as {
        guest_name: string;
        document?: string | null;
        phone?: string | null;
        room_id: number;
        rate_plan_id: number;
        expected_arrival_at: string;
        expected_nights: number;
        notes?: string | null;
      };
      const room = db.rooms.find((r) => r.id === payload.room_id) ?? fail("Habitación no encontrada");
      const rate = db.rates.find((r) => r.id === payload.rate_plan_id) ?? fail("Tarifa no encontrada");
      db.ids.guest += 1;
      db.guests.push({
        id: db.ids.guest,
        name: payload.guest_name,
        document: payload.document ?? null,
        phone: payload.phone ?? null,
      });
      db.ids.reservation += 1;
      const res: Reservation = {
        id: db.ids.reservation,
        guest_id: db.ids.guest,
        guest_name: payload.guest_name,
        guest_document: payload.document ?? null,
        guest_phone: payload.phone ?? null,
        room_id: room.id,
        room_number: room.number,
        rate_plan_id: rate.id,
        rate_plan_name: rate.name,
        expected_arrival_at: payload.expected_arrival_at,
        expected_nights: payload.expected_nights,
        status: "hold",
        notes: payload.notes ?? null,
      };
      db.reservations.push(res);
      return res;
    }
    case "set_reservation_status": {
      const res = db.reservations.find((r) => r.id === args.reservation_id) ?? fail("Reserva no encontrada");
      res.status = String(args.status);
      return res;
    }
    case "check_in_reservation": {
      const res = db.reservations.find((r) => r.id === args.reservation_id) ?? fail("Reserva no encontrada");
      return handle(db, "check_in", {
        payload: {
          room_id: res.room_id,
          guest_name: res.guest_name,
          document: res.guest_document,
          phone: res.guest_phone,
          rate_plan_id: res.rate_plan_id,
          reservation_id: res.id,
        } satisfies CheckInPayload,
      });
    }
    case "list_history": {
      const day =
        (args.date as string | undefined) ??
        nowIso().slice(0, 10);
      return db.stays
        .filter((s) => s.status === "closed" && (s.check_out_at ?? s.check_in_at).slice(0, 10) === day)
        .map((stay) => {
          const item: HistoryStay = {
            stay,
            total_cents: stayBill(db, stay).total_cents,
            payment_method: db.payments.find((p) => p.stay_id === stay.id)?.method ?? null,
          };
          return item;
        });
    }
    case "get_settings":
      return { ...db.settings, pin_hash: "", has_pin: Boolean(db.settings.pin_hash) };
    case "save_settings": {
      const payload = args.payload as AppSettings;
      const newPin = args.new_pin;
      const previousHash = db.settings.pin_hash;
      db.settings = { ...payload, pin_hash: previousHash };
      if (typeof newPin === "string") {
        db.settings.pin_hash = newPin ? hashPin(newPin) : "";
      }
      db.settings.has_pin = Boolean(db.settings.pin_hash);
      return { ...db.settings, pin_hash: "", has_pin: Boolean(db.settings.pin_hash) };
    }
    case "verify_pin":
      if (!db.settings.pin_hash) return true;
      return db.settings.pin_hash === hashPin(String(args.pin));
    case "pin_required":
      return Boolean(db.settings.pin_hash);
    case "print_test":
      return db.settings.printer_enabled ? "Simulación: ticket de prueba archivado" : null;
    case "reprint_receipt":
      return db.settings.printer_enabled ? "Simulación: reimpresión en el navegador" : null;
    default:
      fail(`Comando no implementado: ${name}`);
  }
}
