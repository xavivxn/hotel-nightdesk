import { previewBill, theoreticalNightEnd, dormidaWindowOpen, dormidaUnavailableMessage } from "./billing";
import { mockAuth } from "./mock-auth";
import { buildSeedProducts } from "./products";
import { fail as throwApi } from "./errors";
import type {
  DailyReport,
  AnalyticsSummary,
  AppSettings,
  BoardRoom,
  Charge,
  CheckInPayload,
  CheckOutPayload,
  CheckOutResult,
  ContractInfo,
  CreateReservationPayload,
  HistoryStay,
  Payment,
  Product,
  RatePlan,
  Reservation,
  Room,
  Stay,
  BackupListItem,
  BackupRunResult,
  BackupStatus,
} from "./types";

type Db = {
  rooms: Room[];
  rates: RatePlan[];
  products: Product[];
  guests: { id: number; name: string; document: string | null; phone: string | null }[];
  reservations: Reservation[];
  stays: Stay[];
  charges: Charge[];
  payments: Payment[];
  closed_bills: Record<string, ReturnType<typeof previewBill>>;
  settings: AppSettings;
  ids: { room: number; rate: number; product: number; guest: number; reservation: number; stay: number; charge: number; payment: number };
};

const KEY = "nightdesk.mock.v6";

function nowIso() {
  return new Date().toISOString();
}

function localDay(iso?: string) {
  const date = iso ? new Date(iso) : new Date();
  if (Number.isNaN(date.getTime()) && iso) return iso.slice(0, 10);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function seedRooms(): Array<[string, string, number]> {
  const rooms: Array<[string, string, number]> = [];
  for (let n = 1; n <= 23; n++) {
    const number = String(n).padStart(2, "0");
    const floor = Math.ceil(n / 9);
    const room_type = n <= 4 ? "Jacuzzi" : "Normal";
    rooms.push([number, room_type, floor]);
  }
  return rooms;
}

function seed(): Db {
  const rooms: Room[] = seedRooms().map(([number, room_type, floor], i) => ({
    id: i + 1,
    number,
    room_type,
    floor: Number(floor),
    status: "available",
    notes: null,
    active: true,
    version: 1,
  }));
  const rates: RatePlan[] = [
    {
      id: 1,
      name: "1 hora",
      kind: "hourly",
      base_amount_cents: 45_000,
      extra_hour_cents: 15_000,
      included_hours: 1,
      grace_minutes: 5,
      night_cutoff_hour: 10,
      active: true,
      version: 1,
    },
    {
      id: 2,
      name: "Dormida",
      kind: "overnight",
      base_amount_cents: 120_000,
      extra_hour_cents: 15_000,
      included_hours: 12,
      grace_minutes: 5,
      night_cutoff_hour: 10,
      active: true,
      version: 1,
    },
  ];
  return {
    rooms,
    rates,
    products: buildSeedProducts().map((product) => ({ ...product, version: 1 })),
    guests: [],
    reservations: [],
    stays: [],
    charges: [],
    payments: [],
    closed_bills: {},
    settings: {
      business_name: "MotelApp",
      address: "Av. Principal 100",
      phone: "",
      tax_percent: 0,
      currency_symbol: "Gs.",
      theme: "light",
      receipt_footer: "Gracias por su visita",
      printer_enabled: false,
      printer_path: "",
      printer_name: "",
      paper_width: 80,
      auto_print_on_checkout: true,
      require_guest_name: false,
      pin_hash: "",
      has_pin: false,
    },
    ids: { room: 23, rate: 3, product: 41, guest: 0, reservation: 0, stay: 0, charge: 0, payment: 0 },
  };
}

function normalizeRoomConfiguration(db: Db): Db {
  db.rooms = db.rooms.map((room) => ({
    ...room,
    active: typeof room.active === "boolean" ? room.active : true,
  }));

  const retireCount = Math.max(db.rooms.filter((room) => room.active).length - 23, 0);
  const candidates = db.rooms
    .filter((room) => room.active)
    .filter((room) => !db.stays.some((stay) => stay.room_id === room.id && stay.status === "open"))
    .filter((room) => !db.reservations.some((res) => res.room_id === room.id && res.status === "hold"))
    .sort((a, b) => Number(b.number) - Number(a.number) || b.id - a.id)
    .slice(0, retireCount);

  for (const room of candidates) {
    room.active = false;
  }
  return db;
}

function load(): Db {
  const raw = localStorage.getItem(KEY);
  if (!raw) {
    const db = seed();
    save(db);
    return db;
  }
  try {
    const db = JSON.parse(raw) as Db;
    db.closed_bills ??= {};
    db.products ??= buildSeedProducts().map((product) => ({ ...product, version: 1 }));
    db.rooms = (db.rooms ?? []).map((room) => ({ ...room, version: room.version ?? 1 }));
    db.rates = (db.rates ?? []).map((rate) => ({ ...rate, version: rate.version ?? 1 }));
    db.products = db.products.map((product) => ({ ...product, version: product.version ?? 1 }));
    db.ids ??= { room: 23, rate: 3, product: db.products.reduce((max, item) => Math.max(max, item.id), 0), guest: 0, reservation: 0, stay: 0, charge: 0, payment: 0 };
    db.ids.product ??= db.products.reduce((max, item) => Math.max(max, item.id), 0);
    return normalizeRoomConfiguration(db);
  } catch {
    const db = seed();
    save(db);
    return db;
  }
}

function save(db: Db) {
  localStorage.setItem(KEY, JSON.stringify(db));
}

function fail(codeOrMessage: string, message?: string): never {
  if (message === undefined) throwApi("validation", codeOrMessage);
  throwApi(codeOrMessage, message);
}

function acceptReserved(payload: { operation_id?: string | null; expected_version?: number | null }) {
  if (payload.operation_id != null && payload.operation_id.trim() === "") {
    fail("validation", "operation_id no puede estar vacío");
  }
  if (payload.expected_version != null && payload.expected_version < 0) {
    fail("validation", "expected_version no puede ser negativo");
  }
}

function todayHold(db: Db, roomId: number) {
  const day = localDay();
  return db.reservations.find(
    (r) => r.room_id === roomId && r.status === "hold" && r.expected_arrival_at.slice(0, 10) === day,
  );
}

function openStay(db: Db, roomId: number) {
  return db.stays.find((s) => s.room_id === roomId && s.status === "open");
}

function stayBill(db: Db, stay: Stay) {
  if (stay.status === "closed" && db.closed_bills[String(stay.id)]) {
    return db.closed_bills[String(stay.id)];
  }
  const rate = db.rates.find((r) => r.id === stay.rate_plan_id) ?? fail("Tarifa no encontrada");
  const overnight = stay.overnight_rate_plan_id
    ? db.rates.find((r) => r.id === stay.overnight_rate_plan_id)
    : db.rates.find((r) => r.kind === "overnight" && r.active);
  const night = db.rates.find((r) => r.kind === "night" && r.active);
  const manual = db.charges
    .filter((c) => c.stay_id === stay.id && (c.kind === "surcharge" || c.kind === "discount") && !c.deleted_at)
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

function mockAnalytics(db: Db, args: Record<string, unknown>): AnalyticsSummary {
  const from = String(args.from ?? "");
  const to = String(args.to ?? "");
  const roomType = String(args.room_type ?? "all").toLowerCase();
  const start = new Date(`${from}T00:00:00`);
  const end = new Date(`${to}T00:00:00`);
  if (!/^\d{4}-\d{2}-\d{2}$/.test(from) || !/^\d{4}-\d{2}-\d{2}$/.test(to) ||
      Number.isNaN(+start) || Number.isNaN(+end) || localDay(start.toISOString()) !== from ||
      localDay(end.toISOString()) !== to || start > end || to > localDay() ||
      (Date.UTC(end.getFullYear(), end.getMonth(), end.getDate()) - Date.UTC(start.getFullYear(), start.getMonth(), start.getDate())) / 86400000 >= 366) {
    fail("validation", "Elegí un rango válido de hasta 366 días, sin fechas futuras");
  }
  if (!["all", "normal", "jacuzzi"].includes(roomType)) fail("validation", "Tipo de habitación inválido");

  const rooms = db.rooms.filter(r => roomType === "all" || r.room_type.toLowerCase() === roomType);
  const roomById = new Map(rooms.map(r => [r.id, r]));
  const daily = [] as AnalyticsSummary["daily"];
  for (const day = new Date(start); day <= end; day.setDate(day.getDate() + 1)) {
    daily.push({ date: localDay(day.toISOString()), revenue_cents: 0, closed_accounts: 0, check_ins: 0, reservation_arrivals: 0, reservation_cancellations: 0, no_shows: 0 });
  }
  const dayByDate = new Map(daily.map(d => [d.date, d]));
  const byRoom = new Map<string, AnalyticsSummary["by_room"][number]>();
  const byType = new Map<string, AnalyticsSummary["by_room_type"][number]>();
  const extras = new Map<string, AnalyticsSummary["top_extras"][number]>();
  const checkInHours = Array.from({ length: 24 }, (_, hour) => ({ hour, count: 0 }));
  let reservationArrivals = 0, reservationCancellations = 0, noShows = 0;
  let total = 0, closed = 0, lodging = 0, extraTotal = 0, discount = 0, tax = 0, stayMinutes = 0;
  for (const stay of db.stays) {
    const room = roomById.get(stay.room_id);
    if (!room) continue;
    const checkInDay = dayByDate.get(localDay(stay.check_in_at));
    if (checkInDay) {
      checkInDay.check_ins += 1;
      checkInHours[new Date(stay.check_in_at).getHours()].count += 1;
    }
    if (stay.status !== "closed" || !stay.check_out_at) continue;
    const day = dayByDate.get(localDay(stay.check_out_at));
    if (!day) continue;
    const bill = db.closed_bills[String(stay.id)];
    if (!bill) fail("storage", "Cuenta cerrada sin detalle histórico; requiere revisión");
    if (bill.stay_id !== stay.id ||
        bill.lines.reduce((sum, line) => sum + line.amount_cents, 0) !== bill.subtotal_cents ||
        bill.subtotal_cents + bill.tax_cents !== bill.total_cents ||
        bill.lines.some(line => !["stay", "extra_hour", "surcharge", "product", "discount"].includes(line.kind))) {
      fail("storage", "Detalle histórico de la cuenta inconsistente; requiere revisión");
    }
    total += bill.total_cents;
    closed += 1;
    day.revenue_cents += bill.total_cents;
    day.closed_accounts += 1;
    stayMinutes += Math.max(0, Math.trunc((+new Date(stay.check_out_at) - +new Date(stay.check_in_at)) / 60000));
    tax += bill.tax_cents;
    for (const line of bill.lines) {
      if (line.kind === "stay" || line.kind === "extra_hour") lodging += line.amount_cents;
      else if (line.kind === "surcharge" || line.kind === "product") {
        extraTotal += line.amount_cents;
        const key = line.description.trim() || "Cargo sin descripción";
        const item = extras.get(key) ?? { description: key, count: 0, revenue_cents: 0 };
        item.count += 1;
        item.revenue_cents += line.amount_cents;
        extras.set(key, item);
      } else if (line.kind === "discount") discount += line.amount_cents;
    }
    const roomItem = byRoom.get(room.number) ?? { room_number: room.number, room_type: room.room_type, revenue_cents: 0, closed_accounts: 0 };
    roomItem.revenue_cents += bill.total_cents;
    roomItem.closed_accounts += 1;
    byRoom.set(room.number, roomItem);
    const typeItem = byType.get(room.room_type) ?? { room_type: room.room_type, revenue_cents: 0, closed_accounts: 0 };
    typeItem.revenue_cents += bill.total_cents;
    typeItem.closed_accounts += 1;
    byType.set(room.room_type, typeItem);
  }
  for (const reservation of db.reservations) {
    if (!roomById.has(reservation.room_id)) continue;
    const day = dayByDate.get(localDay(reservation.expected_arrival_at));
    if (!day) continue;
    day.reservation_arrivals += 1;
    reservationArrivals += 1;
    if (reservation.status === "cancelled") { day.reservation_cancellations += 1; reservationCancellations += 1; }
    if (reservation.status === "no_show") { day.no_shows += 1; noShows += 1; }
  }
  const current = new Map(["available", "occupied", "dirty", "reserved", "blocked"].map(status => [status, 0]));
  for (const room of rooms.filter(r => r.active)) {
    const status = openStay(db, room.id) ? "occupied" : room.status === "blocked" ? "blocked" :
      room.status === "dirty" ? "dirty" : todayHold(db, room.id) ? "reserved" : "available";
    current.set(status, (current.get(status) ?? 0) + 1);
  }
  return {
    from, to, generated_at: nowIso(), total_revenue_cents: total, closed_accounts: closed,
    average_ticket_cents: closed ? Math.trunc(total / closed) : 0,
    lodging_cents: lodging, extras_cents: extraTotal, discount_cents: discount, tax_cents: tax,
    average_stay_minutes: closed ? Math.trunc(stayMinutes / closed) : null,
    reservation_arrivals: reservationArrivals, reservation_cancellations: reservationCancellations,
    no_shows: noShows, check_in_hours: checkInHours,
    current_rooms: [...current].map(([status, count]) => ({ status, count })),
    daily,
    by_room_type: [...byType.values()].sort((a, b) => b.revenue_cents - a.revenue_cents),
    by_room: [...byRoom.values()].sort((a, b) => b.revenue_cents - a.revenue_cents),
    top_extras: [...extras.values()].sort((a, b) => b.revenue_cents - a.revenue_cents).slice(0, 10),
  };
}

function hashPin(pin: string) {
  return `mock:${pin}`;
}

export async function mockInvoke<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  const authResult = await mockAuth(name, args);
  if (name.startsWith("auth_") || name === "list_users" || name === "set_user_active" || name === "delete_user") return authResult as T;
  const db = load();
  const result = handle(db, name, args) as T;
  save(db);
  return result;
}

function handle(db: Db, name: string, args: Record<string, unknown>): unknown {
  switch (name) {
    case "analytics_summary":
      return mockAnalytics(db, args);
    case "daily_report": {
      const date = String(args.date ?? "");
      const start = new Date(`${date}T00:00:00`);
      if (!/^\d{4}-\d{2}-\d{2}$/.test(date) || Number.isNaN(+start) || localDay(start.toISOString()) !== date || date > localDay()) fail("Elegí una fecha válida, no futura");
      const end = new Date(start); end.setDate(end.getDate() + 1);
      const now = new Date(), cutoff = new Date(Math.min(+end, +now));
      const stays = db.stays.filter(s => +new Date(s.check_in_at) < +cutoff && (!s.check_out_at || +new Date(s.check_out_at) > +start) || !!s.check_out_at && localDay(s.check_out_at) === date);
      const accounts = stays.map(s => {
        const closed = !!s.check_out_at && +new Date(s.check_out_at) >= +start && +new Date(s.check_out_at) < +end && +new Date(s.check_out_at) <= +now;
        return { stay_id: s.id, room_number: s.room_number, check_in_at: s.check_in_at, check_out_at: s.check_out_at,
          closed_on_day: closed, open_at_cutoff: +new Date(s.check_in_at) < +cutoff && (!s.check_out_at || +new Date(s.check_out_at) >= +cutoff), total_cents: closed ? stayBill(db, s).total_cents : null };
      });
      const adjustments = db.charges.filter(c => (c.kind === "surcharge" || c.kind === "discount") && !c.deleted_at && localDay(c.created_at) === date && +new Date(c.created_at) <= +now);
      return { date, generated_at: now.toISOString(), cutoff_at: cutoff.toISOString(), timezone: "Hora local del navegador (demostración)",
        occupied_rooms: new Set(stays.filter(s => +new Date(s.check_in_at) < +cutoff && (!s.check_out_at || +new Date(s.check_out_at) > +start)).map(s => s.room_id)).size,
        closed_total_cents: accounts.reduce((sum, a) => sum + (a.total_cents ?? 0), 0),
        adjustments_total_cents: adjustments.reduce((sum, c) => sum + c.amount_cents, 0), accounts, adjustments } satisfies DailyReport;
    }
    case "contract_info": {
      const info: ContractInfo = {
        contract_version: 1,
        app_version: "0.1.0",
        schema_migrations: ["001_init", "002_products", "003_rooms_scope", "004_account_closure", "005_auth", "006_stay_integrity"],
      };
      return info;
    }
    case "list_board":
      return db.rooms.filter((room) => room.active).map((room) => {
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
      return db.rooms.filter((room) => room.active);
    case "save_room": {
      const payload = args.payload as { id?: number; number: string; room_type: string; floor: number; notes?: string | null; operation_id?: string | null; expected_version?: number | null };
      acceptReserved(payload);
      const number = String(payload.number ?? "").trim();
      const room_type = String(payload.room_type ?? "").trim();
      if (!number) fail("El número de habitación es obligatorio");
      if (!Number.isFinite(Number(payload.floor))) fail("Ingresá un piso válido");
      const floor = Math.trunc(Number(payload.floor));
      if (payload.id) {
        const room = db.rooms.find((r) => r.id === payload.id) ?? fail("Habitación no encontrada");
        if (payload.expected_version != null && payload.expected_version !== room.version) {
          fail("conflict", "La ficha cambió; recargá antes de guardar");
        }
        Object.assign(room, { number, room_type, floor, notes: payload.notes ?? null, version: (room.version ?? 1) + 1 });
        return room;
      }
      db.ids.room += 1;
      const room: Room = {
        id: db.ids.room,
        number,
        room_type,
        floor,
        status: "available",
        notes: payload.notes ?? null,
        active: true,
        version: 1,
      };
      db.rooms.push(room);
      return room;
    }
    case "set_room_status": {
      const room = db.rooms.find((r) => r.id === args.room_id) ?? fail("Habitación no encontrada");
      if (!room.active) fail("La habitación ya no está habilitada");
      if (openStay(db, room.id)) fail("No se puede cambiar el estado de una habitación ocupada");
      room.status = String(args.status);
      return room;
    }
    case "list_rate_plans":
      return args.active_only ? db.rates.filter((r) => r.active) : db.rates;
    case "save_rate_plan": {
      const payload = args.payload as RatePlan & { id?: number; operation_id?: string | null; expected_version?: number | null };
      acceptReserved(payload);
      const name = String(payload.name ?? "").trim();
      if (!name) fail("El nombre de la tarifa es obligatorio");
      const numeric = [payload.base_amount_cents, payload.extra_hour_cents, payload.included_hours, payload.grace_minutes, payload.night_cutoff_hour];
      if (numeric.some((value) => !Number.isFinite(Number(value)))) fail("Ingresá números válidos en la tarifa");
      const normalized = {
        name,
        kind: payload.kind,
        base_amount_cents: Math.round(Number(payload.base_amount_cents)),
        extra_hour_cents: Math.round(Number(payload.extra_hour_cents)),
        included_hours: Math.max(1, Math.trunc(Number(payload.included_hours))),
        grace_minutes: Math.max(0, Math.trunc(Number(payload.grace_minutes))),
        night_cutoff_hour: Math.min(23, Math.max(0, Math.trunc(Number(payload.night_cutoff_hour)))),
        active: payload.active,
      };
      if (payload.id) {
        const rate = db.rates.find((r) => r.id === payload.id) ?? fail("Tarifa no encontrada");
        if (payload.expected_version != null && payload.expected_version !== rate.version) {
          fail("conflict", "La ficha cambió; recargá antes de guardar");
        }
        Object.assign(rate, normalized, { version: (rate.version ?? 1) + 1 });
        return rate;
      }
      db.ids.rate += 1;
      const rate = { ...normalized, id: db.ids.rate, version: 1 };
      db.rates.push(rate);
      return rate;
    }
    case "check_in": {
      const payload = args.payload as CheckInPayload;
      acceptReserved(payload);
      const room = db.rooms.find((r) => r.id === payload.room_id) ?? fail("not_found", "Habitación no encontrada");
      if (!room.active) fail("La habitación ya no está habilitada");
      if (openStay(db, room.id)) fail("conflict", "La habitación ya está ocupada");
      if (room.status === "blocked") fail("La habitación está bloqueada");
      if (room.status === "dirty") fail("La habitación necesita limpieza antes del check-in");
      const hold = todayHold(db, room.id);
      if (hold && payload.reservation_id !== hold.id) {
        fail("conflict", `La habitación ${room.number} tiene una reserva para hoy`);
      }
      const rate = db.rates.find((r) => r.id === payload.rate_plan_id) ?? fail("not_found", "Tarifa no encontrada");
      if ((rate.kind === "overnight" || rate.kind === "night") && !dormidaWindowOpen(new Date(), rate.night_cutoff_hour)) {
        fail(dormidaUnavailableMessage(new Date(), rate.night_cutoff_hour));
      }
      let guestId = 0;
      let reservationId: number | null = null;
      if (payload.reservation_id) {
        const res = db.reservations.find((r) => r.id === payload.reservation_id) ?? fail("Reserva no encontrada");
        if (res.room_id !== room.id || res.rate_plan_id !== rate.id) fail("La reserva no coincide con la habitación o tarifa seleccionada");
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
        expected_checkout_at:
          rate.kind === "hourly"
            ? new Date(Date.now() + (payload.expected_hours ?? Math.max(1, rate.included_hours)) * 3600000).toISOString()
            : theoreticalNightEnd(new Date(), rate.night_cutoff_hour).toISOString(),
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
      return [stay, stayBill(db, stay), db.charges.filter((c) => c.stay_id === stay.id && !c.deleted_at), db.payments.filter((p) => p.stay_id === stay.id)];
    }
    case "convert_to_overnight": {
      const stay = db.stays.find((s) => s.id === args.stay_id) ?? fail("Estadía no encontrada");
      const overnight = db.rates.find((r) => r.kind === "overnight" && r.active) ?? db.rates.find((r) => r.kind === "night" && r.active);
      if (!overnight) fail("No hay una tarifa de dormida activa");
      if (!dormidaWindowOpen(new Date(), overnight.night_cutoff_hour)) {
        fail(dormidaUnavailableMessage(new Date(), overnight.night_cutoff_hour));
      }
      stay.converted_to_overnight = true;
      stay.overnight_rate_plan_id = overnight.id;
      stay.expected_checkout_at = theoreticalNightEnd(
        new Date(stay.check_in_at),
        overnight.night_cutoff_hour,
      ).toISOString();
      return stay;
    }
    case "add_charge": {
      const payload = args.payload as { stay_id: number; kind: string; description: string; amount_cents: number; operation_id?: string | null; expected_version?: number | null };
      acceptReserved(payload);
      const stay = db.stays.find((s) => s.id === payload.stay_id) ?? fail("not_found", "Estadía no encontrada");
      if (stay.status !== "open") fail("conflict", "No se pueden agregar cargos a una estadía cerrada");
      const kind = payload.kind === "discount" || payload.amount_cents < 0 ? "discount" : "surcharge";
      const amount = kind === "discount" ? -Math.abs(payload.amount_cents) : Math.abs(payload.amount_cents);
      if (!payload.description.trim()) fail("La descripción del cargo es obligatoria");
      if (amount === 0) fail("El importe del cargo debe ser distinto de cero");
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
    case "list_products": {
      const activeOnly = args.active_only !== false;
      return (db.products ?? []).filter((p) => (activeOnly ? p.active : true));
    }
    case "save_product": {
      const payload = args.payload as { id?: number | null; name: string; category: string; price_cents: number; active?: boolean; sort_order?: number; operation_id?: string | null; expected_version?: number | null };
      acceptReserved(payload);
      const name = String(payload.name ?? "").trim();
      const category = String(payload.category ?? "").trim();
      const price = Number(payload.price_cents);
      if (!name) fail("El nombre del producto es obligatorio");
      if (!category) fail("La categoría del producto es obligatoria");
      if (!["bebidas", "snacks", "tabaco", "higiene", "adulto", "licores"].includes(category)) fail("La categoría del producto no es válida");
      if (!Number.isInteger(price) || price <= 0) fail("El precio debe ser un número entero mayor que 0 Gs.");
      if (payload.id) {
        const product = db.products.find((item) => item.id === payload.id) ?? fail("Producto no encontrado");
        if (payload.expected_version != null && payload.expected_version !== product.version) {
          fail("conflict", "La ficha cambió; recargá antes de guardar");
        }
        Object.assign(product, { name, category, price_cents: price, active: payload.active ?? product.active, sort_order: payload.sort_order ?? product.sort_order, version: (product.version ?? 1) + 1 });
        return product;
      }
      db.ids.product = Math.max(db.ids.product ?? 0, ...db.products.map((item) => item.id)) + 1;
      const product: Product = { id: db.ids.product, name, category, price_cents: price, active: payload.active ?? true, sort_order: payload.sort_order ?? db.ids.product, version: 1 };
      db.products.push(product);
      return product;
    }
    case "set_product_active": {
      const product = db.products.find((item) => item.id === Number(args.product_id)) ?? fail("Producto no encontrado");
      product.active = Boolean(args.active);
      product.version = (product.version ?? 1) + 1;
      return product;
    }
    case "add_product_charge": {
      const payload = args.payload as { stay_id: number; product_id: number; operation_id?: string | null; expected_version?: number | null };
      acceptReserved(payload);
      const stay = db.stays.find((s) => s.id === payload.stay_id) ?? fail("not_found", "Estadía no encontrada");
      if (stay.status !== "open") fail("conflict", "No se pueden agregar cargos a una estadía cerrada");
      const product = (db.products ?? []).find((p) => p.id === payload.product_id) ?? fail("not_found", "Producto no encontrado");
      if (!product.active) fail("El producto no está activo");
      db.ids.charge += 1;
      const charge: Charge = {
        id: db.ids.charge,
        stay_id: payload.stay_id,
        kind: "surcharge",
        description: product.name,
        amount_cents: product.price_cents,
        created_at: nowIso(),
      };
      db.charges.push(charge);
      return charge;
    }
    case "delete_charge":
      {
        const charge = db.charges.find((item) => item.id === args.charge_id && !item.deleted_at);
        const stay = db.stays.find((item) => item.id === charge?.stay_id);
        if (!charge || !stay) fail("not_found", "Cargo no encontrado");
        if (stay.status !== "open") fail("conflict", "No se pueden modificar cargos de una cuenta cerrada");
        if (charge.kind === "surcharge" || charge.kind === "discount") {
          charge.deleted_at = nowIso();
        }
      }
      return null;
    case "check_out": {
      const payload = args.payload as CheckOutPayload;
      acceptReserved(payload);
      const stay = db.stays.find((s) => s.id === payload.stay_id) ?? fail("not_found", "Estadía no encontrada");
      if (stay.status !== "open") fail("conflict", "La estadía ya está cerrada");
      const bill = stayBill(db, stay);
      stay.status = "closed";
      stay.check_out_at = nowIso();
      const room = db.rooms.find((r) => r.id === stay.room_id)!;
      room.status = "dirty";
      db.closed_bills[String(stay.id)] = bill;
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
      const payload = args.payload as CreateReservationPayload;
      acceptReserved(payload);
      const room = db.rooms.find((r) => r.id === payload.room_id) ?? fail("not_found", "Habitación no encontrada");
      if (!room.active) fail("La habitación ya no está habilitada");
      const rate = db.rates.find((r) => r.id === payload.rate_plan_id) ?? fail("not_found", "Tarifa no encontrada");
      if (payload.expected_nights < 1) fail("La reserva debe tener al menos una noche");
      const open = openStay(db, room.id);
      if (open && localDay(payload.expected_arrival_at) === localDay(open.check_in_at)) {
        fail("conflict", "La habitación está ocupada en esa fecha");
      }
      if (db.reservations.some((r) => r.room_id === room.id && r.status === "hold" && r.expected_arrival_at.slice(0, 10) === payload.expected_arrival_at.slice(0, 10))) {
        fail("conflict", "Ya existe una reserva vigente para esa habitación y fecha");
      }
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
      const day = (args.date as string | undefined) ?? localDay();
      return db.stays
        .filter((s) => s.status === "closed" && localDay(s.check_out_at ?? s.check_in_at) === day)
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
      return {
        ...db.settings,
        require_guest_name: db.settings.require_guest_name ?? false,
        pin_hash: "",
        has_pin: Boolean(db.settings.pin_hash),
      };
    case "save_settings": {
      const payload = args.payload as AppSettings;
      if (!Number.isFinite(payload.tax_percent) || payload.tax_percent < 0 || payload.tax_percent > 100) {
        fail("Ingresá un IVA entre 0 y 100");
      }
      const newPin = args.new_pin;
      const previousHash = db.settings.pin_hash;
      db.settings = {
        ...payload,
        business_name: payload.business_name.trim(),
        address: payload.address.trim(),
        phone: payload.phone.trim(),
        currency_symbol: payload.currency_symbol.trim(),
        receipt_footer: payload.receipt_footer.trim(),
        printer_path: payload.printer_path.trim(),
        printer_name: payload.printer_name.trim(),
        pin_hash: previousHash,
      };
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
    case "list_printers":
      return [];
    case "reprint_receipt":
      if (!db.stays.some(s => s.id === Number(args.stay_id) && s.status === "closed")) fail("Solo se reimprimen cuentas cerradas");
      return db.settings.printer_enabled ? "Simulación: reimpresión en el navegador" : null;
    case "device_mode_get": {
      const mode = localStorage.getItem("nightdesk.device_mode");
      return mode === "reception" || mode === "remote" ? mode : null;
    }
    case "device_mode_set": {
      const payload = args.payload as { mode: string };
      if (localStorage.getItem("nightdesk.device_mode")) fail("El modo de este equipo ya está definido");
      if (payload.mode !== "reception" && payload.mode !== "remote") fail("Elegí Recepción o Administración remota");
      localStorage.setItem("nightdesk.device_mode", payload.mode);
      return;
    }
    case "remote_configure": {
      const payload = args.payload as { project_url: string; anon_key: string };
      if (!payload.project_url?.trim() || !payload.anon_key?.trim()) fail("Ingresá la URL del proyecto y la clave anónima");
      localStorage.setItem(
        "nightdesk.remote_config",
        JSON.stringify({ project_url: payload.project_url.trim(), anon_key: payload.anon_key.trim() }),
      );
      return;
    }
    case "remote_configured":
      return Boolean(localStorage.getItem("nightdesk.remote_config"));
    case "remote_get_config": {
      const raw = localStorage.getItem("nightdesk.remote_config");
      return raw ? JSON.parse(raw) : null;
    }
    case "hash_password": {
      const payload = args.payload as { password: string };
      return { hash: `mock-argon2:${payload.password}` };
    }
    case "sync_status":
      return {
        connected: false,
        pending_outbox: 0,
        last_push_at: null,
        last_pull_at: null,
        last_error: null,
        configured: Boolean(localStorage.getItem("nightdesk.device_config")),
        realtime_connected: false,
      };
    case "sync_pull_now":
      window.dispatchEvent(new Event("sync:catalog-updated"));
      return;
    case "sync_configure_device": {
      const payload = args.payload as {
        project_url: string;
        anon_key: string;
        device_email: string;
        device_password: string;
      };
      if (!payload.project_url || !payload.anon_key || !payload.device_email || !payload.device_password) {
        fail("Completá URL, clave anónima y credenciales del dispositivo");
      }
      localStorage.setItem("nightdesk.device_config", "1");
      return;
    }
    case "backup_status": {
      const queue = mockBackupQueue();
      const lastLocal = queue[0]?.created_at ?? null;
      const lastRemote = queue.find((b) => b.uploaded_at)?.uploaded_at ?? null;
      const pending = queue.filter((b) => b.status === "pending_upload" || b.status === "failed").length;
      return {
        last_local_at: lastLocal,
        last_remote_at: lastRemote,
        pending,
        last_error: null,
        ready: true,
      } satisfies BackupStatus;
    }
    case "backup_run_now": {
      const id = crypto.randomUUID();
      const at = nowIso();
      const item: BackupListItem = {
        backup_id: id,
        created_at: at,
        status: "pending_upload",
        size_bytes: 12_345,
        checksum: "mock",
        schema_version: "016",
        uploaded_at: null,
        remote_path: null,
        source: "local",
      };
      const queue = mockBackupQueue();
      queue.unshift(item);
      localStorage.setItem(BACKUP_KEY, JSON.stringify(queue.slice(0, 7)));
      return { backup_id: id } satisfies BackupRunResult;
    }
    case "backup_list":
      return mockBackupQueue().map((item) => ({ ...item, source: item.source ?? "local" }));
    case "backup_import_key": {
      const payload = args.payload as { key_hex?: string } | undefined;
      const hex = String(payload?.key_hex ?? "").replace(/\s/g, "");
      if (!/^[0-9a-fA-F]{64}$/.test(hex)) fail("validation", "La clave debe ser 64 caracteres hexadecimales");
      return;
    }
    case "backup_restore": {
      const payload = args.payload as { backup_id?: string; source?: string } | undefined;
      const backupId = String(payload?.backup_id ?? args.backup_id ?? "");
      const source = payload?.source ?? "local";
      if (source !== "local" && source !== "remote") fail("validation", "Origen de respaldo inválido");
      const found = mockBackupQueue().find((b) => b.backup_id === backupId);
      if (!found) fail("not_found", source === "remote" ? "No se encontró ese respaldo remoto" : "No se encontró ese respaldo local");
      return;
    }
    default:
      fail(`Comando no implementado: ${name}`);
  }
}

const BACKUP_KEY = "nightdesk.mock.backups";

function mockBackupQueue(): BackupListItem[] {
  try {
    const raw = localStorage.getItem(BACKUP_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as BackupListItem[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}
