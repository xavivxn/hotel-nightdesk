import { createClient, type RealtimeChannel, type SupabaseClient } from "@supabase/supabase-js";
import { previewBill as previewBillLocal } from "./billing";
import { fail } from "./errors";
import type {
  AppSettings,
  AnalyticsSummary,
  BackupStatus,
  BillPreview,
  BoardRoom,
  Charge,
  ContractInfo,
  HistoryStay,
  LineItem,
  Payment,
  Product,
  RatePlan,
  Reservation,
  Room,
  SessionInfo,
  SessionUser,
  Stay,
} from "./types";

let client: SupabaseClient | null = null;
let authSession: SessionInfo | null = null;
let boardChannel: RealtimeChannel | null = null;
let settingsVersions: Record<string,number> | null = null;

const FORBIDDEN_OPS = new Set([
  "check_in",
  "check_out",
  "add_charge",
  "add_product_charge",
  "delete_charge",
  "convert_to_overnight",
  "set_room_status",
  "create_reservation",
  "set_reservation_status",
  "check_in_reservation",
  "save_daily_pdf",
  "verify_pin",
  "pin_required",
  "print_test",
  "list_printers",
  "reprint_receipt",
  "auth_setup",
  "auth_setup_required",
  "daily_report",
  "backup_run_now",
  "backup_list",
  "backup_import_key",
  "backup_restore",
]);

export function clearSupabaseClient() {
  if (boardChannel && client) void client.removeChannel(boardChannel);
  boardChannel = null;
  client = null;
  authSession = null;
  settingsVersions = null;
}

export function initSupabase(projectUrl: string, anonKey: string) {
  clearSupabaseClient();
  client = createClient(projectUrl, anonKey, {
    auth: { persistSession: false, autoRefreshToken: true, detectSessionInUrl: false },
  });
}

function sb(): SupabaseClient {
  if (!client) fail("forbidden", "Configurá la URL y la clave anónima de Supabase en este equipo");
  return client;
}

function mapAuthUser(user: { id: string; email?: string | null; app_metadata?: Record<string, unknown> }): SessionUser {
  const role = String(user.app_metadata?.role ?? "admin");
  return {
    id: 0,
    username: user.email ?? user.id,
    role: role === "recepcion" ? "recepcion" : "admin",
  };
}

function mapRoom(row: Record<string, unknown>): Room {
  return {
    id: Number(row.local_id ?? 0),
    number: String(row.number ?? ""),
    room_type: String(row.room_type ?? ""),
    floor: Number(row.floor ?? 0),
    status: String(row.status ?? "available"),
    notes: (row.notes as string | null) ?? null,
    active: Boolean(row.active ?? true),
    version: Number(row.version ?? 1),
  };
}

function mapRate(row: Record<string, unknown>): RatePlan {
  return {
    id: Number(row.local_id ?? 0),
    name: String(row.name ?? ""),
    kind: row.kind as RatePlan["kind"],
    base_amount_cents: Number(row.base_amount_cents ?? 0),
    extra_hour_cents: Number(row.extra_hour_cents ?? 0),
    included_hours: Number(row.included_hours ?? 1),
    grace_minutes: Number(row.grace_minutes ?? 0),
    night_cutoff_hour: Number(row.night_cutoff_hour ?? 10),
    active: Boolean(row.active ?? true),
    version: Number(row.version ?? 1),
  };
}

function mapProduct(row: Record<string, unknown>): Product {
  return {
    id: Number(row.local_id ?? 0),
    name: String(row.name ?? ""),
    category: String(row.category ?? ""),
    price_cents: Number(row.price_cents ?? 0),
    active: Boolean(row.active ?? true),
    sort_order: Number(row.sort_order ?? 0),
    version: Number(row.version ?? 1),
  };
}

async function settingsFromKv(): Promise<AppSettings> {
  const { data, error } = await sb().from("business_settings").select("key,value,version");
  if (error) fail("storage", error.message);
  settingsVersions = Object.fromEntries((data ?? []).map(r => [r.key, Number(r.version)]));
  for (const key of ["business_name","address","phone","tax_percent","currency_symbol","receipt_footer","require_guest_name"]) settingsVersions![key] ??= 0;
  const map = new Map((data ?? []).map((r) => [String((r as { key: string }).key), String((r as { value: string }).value)]));
  return {
    business_name: map.get("business_name") ?? "MotelApp",
    catalog_versions: {...settingsVersions},
    address: map.get("address") ?? "",
    phone: map.get("phone") ?? "",
    tax_percent: Number(map.get("tax_percent") ?? 0),
    currency_symbol: map.get("currency_symbol") ?? "Gs.",
    theme: "light",
    receipt_footer: map.get("receipt_footer") ?? "",
    printer_enabled: false,
    printer_path: "",
    printer_name: "",
    paper_width: 80,
    auto_print_on_checkout: false,
    require_guest_name: map.get("require_guest_name") === "true",
    pin_hash: "",
    has_pin: false,
  };
}

const BUSINESS_TIME_ZONE = "America/Asuncion";
const ANALYTICS_PAGE_SIZE = 500;
const DAY_MS = 24 * 60 * 60 * 1000;

function businessDate(timestamp: string): string {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) fail("storage", "La réplica contiene una fecha inválida");
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: BUSINESS_TIME_ZONE,
    year: "numeric", month: "2-digit", day: "2-digit",
  }).formatToParts(date);
  const part = (type: string) => parts.find((item) => item.type === type)?.value ?? "";
  return `${part("year")}-${part("month")}-${part("day")}`;
}

function businessHour(timestamp: string): number {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) fail("storage", "La réplica contiene una fecha inválida");
  const hourPart = new Intl.DateTimeFormat("en-US", {
    timeZone: BUSINESS_TIME_ZONE, hour: "2-digit", hourCycle: "h23",
  }).formatToParts(date).find((part) => part.type === "hour")?.value;
  const hour = Number(hourPart);
  if (!Number.isInteger(hour) || hour < 0 || hour > 23) {
    fail("storage", "La réplica contiene una hora inválida");
  }
  return hour;
}

function parseAnalyticsDate(value: unknown): number {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    fail("validation", "Elegí un rango de fechas válido");
  }
  const timestamp = Date.parse(`${value}T00:00:00.000Z`);
  if (!Number.isFinite(timestamp) || new Date(timestamp).toISOString().slice(0, 10) !== value) {
    fail("validation", "Elegí un rango de fechas válido");
  }
  return timestamp;
}

function checkedMoney(value: unknown): number {
  const amount = Number(value);
  if (!Number.isSafeInteger(amount)) fail("storage", "La réplica contiene un importe fuera de rango");
  return amount;
}

function addMoney(total: number, amount: number): number {
  const result = total + amount;
  if (!Number.isSafeInteger(result)) fail("storage", "El total del informe está fuera de rango");
  return result;
}

async function pagedRows(
  readPage: (from: number, to: number) => Promise<{ data: unknown[] | null; error: { message: string } | null }>,
): Promise<Record<string, unknown>[]> {
  const rows: Record<string, unknown>[] = [];
  for (let offset = 0; ; offset += ANALYTICS_PAGE_SIZE) {
    const { data, error } = await readPage(offset, offset + ANALYTICS_PAGE_SIZE - 1);
    if (error) fail("storage", `No se pudieron consultar los datos sincronizados: ${error.message}`);
    const page = (data ?? []) as Record<string, unknown>[];
    rows.push(...page);
    if (page.length < ANALYTICS_PAGE_SIZE) return rows;
  }
}

async function remoteAnalyticsSummary(args: Record<string, unknown>): Promise<AnalyticsSummary> {
  if (!authSession || authSession.user.role !== "admin") {
    fail("forbidden", "Esta operación requiere administración");
  }
  const { data: sessionData } = await sb().auth.getSession();
  if (!sessionData.session) fail("session_expired", "La sesión terminó. Volvé a ingresar");

  const from = String(args.from ?? "");
  const to = String(args.to ?? "");
  const fromMs = parseAnalyticsDate(from);
  const toMs = parseAnalyticsDate(to);
  if (toMs < fromMs || (toMs - fromMs) / DAY_MS >= 366) {
    fail("validation", "Elegí un rango de hasta 366 días");
  }
  if (to > businessDate(new Date().toISOString())) {
    fail("validation", "El análisis no admite fechas futuras");
  }
  if (args.room_type != null && typeof args.room_type !== "string") {
    fail("validation", "El tipo de habitación debe ser Normal o Jacuzzi");
  }
  const requestedType = String(args.room_type ?? "").trim().toLowerCase();
  if (requestedType && !["all", "normal", "jacuzzi"].includes(requestedType)) {
    fail("validation", "El tipo de habitación debe ser Normal o Jacuzzi");
  }
  const roomType = requestedType === "all" || !requestedType ? null : requestedType;

  // A one-day UTC margin on both sides covers local midnight and offset changes.
  // The business-date check below applies the exact requested range.
  const broadStart = new Date(fromMs - DAY_MS).toISOString();
  const broadEnd = new Date(toMs + 2 * DAY_MS).toISOString();
  const rooms = await pagedRows(async (start, end) => await sb().from("rooms")
    .select("uid,number,room_type,status,active")
    .order("uid")
    .range(start, end));
  const roomByUid = new Map(rooms.map((room) => [String(room.uid), room]));
  const includeRoom = (room: Record<string, unknown> | undefined) =>
    room !== undefined && (roomType === null || String(room.room_type).toLowerCase() === roomType);

  const currentCounts = new Map<string, number>([
    ["available", 0], ["occupied", 0], ["dirty", 0], ["reserved", 0], ["blocked", 0],
  ]);
  const openStays = await pagedRows(async (start, end) => await sb().from("stays")
    .select("uid,room_uid")
    .eq("status", "open")
    .order("uid")
    .range(start, end));
  const occupiedRoomUids = new Set(openStays.map((stay) => String(stay.room_uid)));
  const today = businessDate(new Date().toISOString());
  const todayMs = parseAnalyticsDate(today);
  const todayHolds = await pagedRows(async (start, end) => await sb().from("reservations")
    .select("uid,room_uid,expected_arrival_at")
    .eq("status", "hold")
    .gte("expected_arrival_at", new Date(todayMs - DAY_MS).toISOString())
    .lt("expected_arrival_at", new Date(todayMs + 2 * DAY_MS).toISOString())
    .order("uid")
    .range(start, end));
  const reservedRoomUids = new Set(todayHolds
    .filter((hold) => businessDate(String(hold.expected_arrival_at)) === today)
    .map((hold) => String(hold.room_uid)));
  for (const room of rooms) {
    if (!includeRoom(room) || room.active === false) continue;
    const uid = String(room.uid);
    const status = occupiedRoomUids.has(uid) ? "occupied"
      : room.status === "blocked" ? "blocked"
        : room.status === "dirty" ? "dirty"
          : reservedRoomUids.has(uid) ? "reserved" : "available";
    currentCounts.set(status, (currentCounts.get(status) ?? 0) + 1);
  }

  const daily = [] as AnalyticsSummary["daily"];
  const dailyByDate = new Map<string, AnalyticsSummary["daily"][number]>();
  for (let day = fromMs; day <= toMs; day += DAY_MS) {
    const date = new Date(day).toISOString().slice(0, 10);
    const item = {
      date, revenue_cents: 0, closed_accounts: 0, check_ins: 0,
      reservation_arrivals: 0, reservation_cancellations: 0, no_shows: 0,
    };
    daily.push(item);
    dailyByDate.set(date, item);
  }

  const closedRows = await pagedRows(async (start, end) => await sb().from("stays")
    .select("uid,room_uid,check_in_at,check_out_at,closed_total_cents,closed_line_count")
    .eq("status", "closed")
    .gte("check_out_at", broadStart)
    .lt("check_out_at", broadEnd)
    .order("uid")
    .range(start, end));
  const closed = closedRows.filter((stay) =>
    includeRoom(roomByUid.get(String(stay.room_uid))) &&
    dailyByDate.has(businessDate(String(stay.check_out_at))));
  const closedUids = closed.map((stay) => String(stay.uid));
  const chargesByStay = new Map<string, Record<string, unknown>[]>();
  // Restrict every request to the selected closed stays; keep URLs short and
  // paginate within each batch so PostgREST's row cap never truncates totals.
  for (let index = 0; index < closedUids.length; index += 80) {
    const batch = closedUids.slice(index, index + 80);
    const charges = await pagedRows(async (start, end) => await sb().from("charges")
      .select("uid,stay_uid,kind,description,amount_cents")
      .in("stay_uid", batch)
      .is("deleted_at", null)
      .order("uid")
      .range(start, end));
    for (const charge of charges) {
      const uid = String(charge.stay_uid);
      const lines = chargesByStay.get(uid) ?? [];
      lines.push(charge);
      chargesByStay.set(uid, lines);
    }
  }

  const byType = new Map<string, AnalyticsSummary["by_room_type"][number]>();
  const byRoom = new Map<string, AnalyticsSummary["by_room"][number]>();
  const topExtras = new Map<string, AnalyticsSummary["top_extras"][number]>();
  let total = 0;
  let lodging = 0;
  let extras = 0;
  let discount = 0;
  let tax = 0;
  let durationMinutes = 0;
  for (const stay of closed) {
    const uid = String(stay.uid);
    const lines = chargesByStay.get(uid) ?? [];
    if (stay.closed_total_cents == null || stay.closed_line_count == null) {
      fail("storage", "Hay cuentas cerradas sin confirmación de detalle. Esperá la sincronización de recepción y volvé a cargar Análisis");
    }
    const expectedTotal = checkedMoney(stay.closed_total_cents);
    const expectedLines = Number(stay.closed_line_count);
    if (!Number.isSafeInteger(expectedLines) || expectedLines < 0 || lines.length !== expectedLines) {
      fail("storage", "El detalle de cierre todavía no está completo en la réplica. Esperá la sincronización y volvé a cargar Análisis");
    }
    if (!lines.some((line) => line.kind === "stay" || line.kind === "extra_hour" || line.kind === "tax")) {
      fail("storage", "Hay cuentas cerradas sin detalle de cierre sincronizado. Esperá la sincronización y volvé a cargar Análisis");
    }
    const room = roomByUid.get(String(stay.room_uid))!;
    const date = businessDate(String(stay.check_out_at));
    const day = dailyByDate.get(date)!;
    const typeName = String(room.room_type ?? "Sin tipo");
    const roomNumber = String(room.number ?? "");
    const type = byType.get(typeName) ?? { room_type: typeName, revenue_cents: 0, closed_accounts: 0 };
    const roomItem = byRoom.get(roomNumber) ?? {
      room_number: roomNumber, room_type: typeName, revenue_cents: 0, closed_accounts: 0,
    };
    let stayTotal = 0;
    for (const line of lines) {
      const amount = checkedMoney(line.amount_cents);
      stayTotal = addMoney(stayTotal, amount);
      switch (String(line.kind)) {
        case "stay":
        case "extra_hour": lodging = addMoney(lodging, amount); break;
        case "surcharge": {
          extras = addMoney(extras, amount);
          const description = String(line.description ?? "").trim() || "Cargo sin descripción";
          const item = topExtras.get(description) ?? { description, count: 0, revenue_cents: 0 };
          item.count += 1;
          item.revenue_cents = addMoney(item.revenue_cents, amount);
          topExtras.set(description, item);
          break;
        }
        case "discount": discount = addMoney(discount, amount); break;
        case "tax": tax = addMoney(tax, amount); break;
        default: fail("storage", "Una cuenta cerrada tiene un tipo de cargo inválido");
      }
    }
    if (stayTotal !== expectedTotal) {
      fail("storage", "El total del cierre todavía no coincide con la réplica. Esperá la sincronización y volvé a cargar Análisis");
    }
    total = addMoney(total, stayTotal);
    day.revenue_cents = addMoney(day.revenue_cents, stayTotal);
    day.closed_accounts += 1;
    type.revenue_cents = addMoney(type.revenue_cents, stayTotal);
    type.closed_accounts += 1;
    roomItem.revenue_cents = addMoney(roomItem.revenue_cents, stayTotal);
    roomItem.closed_accounts += 1;
    byType.set(typeName, type);
    byRoom.set(roomNumber, roomItem);
    const checkIn = Date.parse(String(stay.check_in_at));
    const checkOut = Date.parse(String(stay.check_out_at));
    if (!Number.isFinite(checkIn) || !Number.isFinite(checkOut) || checkOut < checkIn) {
      fail("storage", "Hay cuentas cerradas con fechas de estadía inconsistentes");
    }
    durationMinutes += Math.floor((checkOut - checkIn) / 60000);
  }
  if (addMoney(addMoney(addMoney(lodging, extras), discount), tax) !== total) {
    fail("storage", "El detalle de ingresos sincronizado no coincide con el total");
  }

  const checkIns = await pagedRows(async (start, end) => await sb().from("stays")
    .select("uid,room_uid,check_in_at")
    .gte("check_in_at", broadStart)
    .lt("check_in_at", broadEnd)
    .order("uid")
    .range(start, end));
  const checkInHours = Array.from({ length: 24 }, (_, hour) => ({ hour, count: 0 }));
  for (const stay of checkIns) {
    if (!includeRoom(roomByUid.get(String(stay.room_uid)))) continue;
    const checkInAt = String(stay.check_in_at);
    const day = dailyByDate.get(businessDate(checkInAt));
    if (day) {
      day.check_ins += 1;
      checkInHours[businessHour(checkInAt)].count += 1;
    }
  }

  // Reservation outcomes are attributed to the scheduled arrival date. The
  // schema has no cancellation timestamp, so this is not a cancellation-date trend.
  const reservationRows = await pagedRows(async (start, end) => await sb().from("reservations")
    .select("uid,room_uid,expected_arrival_at,status")
    .gte("expected_arrival_at", broadStart)
    .lt("expected_arrival_at", broadEnd)
    .order("uid")
    .range(start, end));
  let reservationArrivals = 0;
  let reservationCancellations = 0;
  let noShows = 0;
  for (const reservation of reservationRows) {
    if (!includeRoom(roomByUid.get(String(reservation.room_uid)))) continue;
    const day = dailyByDate.get(businessDate(String(reservation.expected_arrival_at)));
    if (!day) continue;
    reservationArrivals += 1;
    day.reservation_arrivals += 1;
    if (reservation.status === "cancelled") {
      reservationCancellations += 1;
      day.reservation_cancellations += 1;
    } else if (reservation.status === "no_show") {
      noShows += 1;
      day.no_shows += 1;
    }
  }

  return {
    from, to, generated_at: new Date().toISOString(),
    total_revenue_cents: total,
    closed_accounts: closed.length,
    average_ticket_cents: closed.length ? Math.trunc(total / closed.length) : 0,
    lodging_cents: lodging,
    extras_cents: extras,
    discount_cents: discount,
    tax_cents: tax,
    average_stay_minutes: closed.length ? Math.trunc(durationMinutes / closed.length) : null,
    reservation_arrivals: reservationArrivals,
    reservation_cancellations: reservationCancellations,
    no_shows: noShows,
    check_in_hours: checkInHours,
    current_rooms: [...currentCounts].map(([status, count]) => ({ status, count })),
    daily,
    by_room_type: [...byType.values()].sort((a, b) => b.revenue_cents - a.revenue_cents),
    by_room: [...byRoom.values()].sort((a, b) => b.revenue_cents - a.revenue_cents || a.room_number.localeCompare(b.room_number)),
    top_extras: [...topExtras.values()].sort((a, b) => b.revenue_cents - a.revenue_cents || a.description.localeCompare(b.description)).slice(0, 10),
  };
}

export async function supabaseInvoke<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  if (FORBIDDEN_OPS.has(name)) fail("forbidden", "Esta operación solo está disponible en recepción");

  switch (name) {
    case "auth_login": {
      const payload = args.payload as { username: string; password: string };
      const email = payload.username.trim();
      const { data, error } = await sb().auth.signInWithPassword({ email, password: payload.password });
      if (error || !data.session || !data.user) fail("invalid_credentials", "Usuario o contraseña incorrectos");
      if (String(data.user.app_metadata?.role ?? "") !== "admin") {
        await sb().auth.signOut();
        fail("forbidden", "Esta cuenta no es administración remota");
      }
      authSession = {
        token: data.session.access_token,
        user: mapAuthUser(data.user),
        expires_at: Math.floor(Date.now() / 1000) + (data.session.expires_in ?? 3600),
      };
      return authSession as T;
    }
    case "auth_session": {
      if (!authSession) fail("session_expired", "Iniciá sesión para continuar");
      const { data } = await sb().auth.getSession();
      if (!data.session) fail("session_expired", "La sesión terminó. Volvé a ingresar");
      return authSession as T;
    }
    case "auth_logout": {
      await sb().auth.signOut();
      authSession = null;
      return undefined as T;
    }
    case "contract_info":
      return {
        contract_version: 1,
        app_version: "0.1.0",
        schema_migrations: ["supabase"],
      } satisfies ContractInfo as T;
    case "backup_status": {
      if (!authSession || authSession.user.role !== "admin") fail("session_expired", "Iniciá sesión para consultar los respaldos");
      // I08 inserts the manifest only after Storage confirms the encrypted object.
      const { data, error } = await sb().from("backups").select("backuped_at").order("backuped_at", { ascending: false }).limit(1);
      if (error) fail("storage", "No se pudo consultar el estado de respaldos en Supabase");
      return {
        last_local_at: null,
        last_remote_at: data?.[0]?.backuped_at ?? null,
        pending: 0,
        last_error: null,
        // Remote admin consults confirmed copies; «Respaldar ahora» is reception-only.
        ready: false,
      } satisfies BackupStatus as T;
    }
    case "list_rooms": {
      const { data, error } = await sb().from("rooms").select("*").order("number");
      if (error) fail("storage", error.message);
      return (data ?? []).map((r) => mapRoom(r as Record<string, unknown>)) as T;
    }
    case "list_rate_plans": {
      let q = sb().from("rate_plans").select("*").order("name");
      if (args.active_only) q = q.eq("active", true);
      const { data, error } = await q;
      if (error) fail("storage", error.message);
      return (data ?? []).map((r) => mapRate(r as Record<string, unknown>)) as T;
    }
    case "list_products": {
      let q = sb().from("products").select("*").order("sort_order");
      if (args.active_only !== false) q = q.eq("active", true);
      const { data, error } = await q;
      if (error) fail("storage", error.message);
      return (data ?? []).map((r) => mapProduct(r as Record<string, unknown>)) as T;
    }
    case "list_board": {
      const { data: rooms, error: e1 } = await sb().from("rooms").select("*").order("number");
      if (e1) fail("storage", e1.message);
      const { data: stays, error: e2 } = await sb()
        .from("stays")
        .select("*, room:rooms(local_id, number, status), guest:guests(name), rate_plan:rate_plans!rate_plan_uid(local_id, name, kind)")
        .eq("status", "open");
      if (e2) fail("storage", e2.message);
      const byRoomUid = new Map((stays ?? []).map((s) => [String((s as { room_uid: string }).room_uid), s]));
      const board: BoardRoom[] = (rooms ?? []).map((raw) => {
        const row = raw as Record<string, unknown>;
        const room = mapRoom(row);
        const stayRow = byRoomUid.get(String(row.uid)) as Record<string, unknown> | undefined;
        let stay: Stay | null = null;
        if (stayRow) {
          const guest = stayRow.guest as { name?: string } | null;
          const rate = stayRow.rate_plan as { local_id?: number; name?: string; kind?: string } | null;
          stay = {
            id: Number(stayRow.local_id ?? 0),
            room_id: room.id,
            room_number: room.number,
            guest_id: 0,
            guest_name: String(guest?.name ?? ""),
            guest_document: null,
            guest_phone: null,
            rate_plan_id: Number(rate?.local_id ?? 0),
            rate_plan_name: String(rate?.name ?? ""),
            rate_kind: (rate?.kind as Stay["rate_kind"]) ?? "hourly",
            reservation_id: null,
            check_in_at: String(stayRow.check_in_at ?? ""),
            expected_checkout_at: (stayRow.expected_checkout_at as string | null) ?? null,
            check_out_at: null,
            status: "open",
            converted_to_overnight: Boolean(stayRow.converted_to_overnight),
            overnight_rate_plan_id: null,
            notes: (stayRow.notes as string | null) ?? null,
          };
        }
        const display_status = stay
          ? "occupied"
          : room.status === "dirty"
            ? "dirty"
            : room.status === "blocked"
              ? "blocked"
              : "available";
        return {
          room,
          stay,
          reservation: null,
          display_status,
          estimated_total_cents: null,
          elapsed_minutes: stay
            ? Math.max(0, Math.floor((Date.now() - new Date(stay.check_in_at).getTime()) / 60000))
            : null,
        };
      });
      return board as T;
    }
    case "list_reservations": {
      const { data, error } = await sb()
        .from("reservations")
        .select("*, room:rooms(local_id, number), guest:guests(name, document, phone), rate_plan:rate_plans(local_id, name)")
        .order("expected_arrival_at");
      if (error) fail("storage", error.message);
      return (data ?? []).map((r) => {
        const row = r as Record<string, unknown>;
        const guest = row.guest as { name?: string; document?: string; phone?: string } | null;
        const room = row.room as { local_id?: number; number?: string } | null;
        const rate = row.rate_plan as { local_id?: number; name?: string } | null;
        return {
          id: Number(row.local_id ?? 0),
          guest_id: 0,
          guest_name: String(guest?.name ?? ""),
          guest_document: guest?.document ?? null,
          guest_phone: guest?.phone ?? null,
          room_id: Number(room?.local_id ?? 0),
          room_number: String(room?.number ?? ""),
          rate_plan_id: Number(rate?.local_id ?? 0),
          rate_plan_name: String(rate?.name ?? ""),
          expected_arrival_at: String(row.expected_arrival_at ?? ""),
          expected_nights: Number(row.expected_nights ?? 1),
          status: String(row.status ?? "booked"),
          notes: (row.notes as string | null) ?? null,
        } satisfies Reservation;
      }) as T;
    }
    case "list_history": {
      let q = sb()
        .from("stays")
        .select("*, room:rooms(local_id, number), guest:guests(name), rate_plan:rate_plans!rate_plan_uid(local_id, name, kind)")
        .eq("status", "closed")
        .order("check_out_at", { ascending: false })
        .limit(200);
      if (args.date) {
        const day = String(args.date);
        q = q.gte("check_out_at", `${day}T00:00:00`).lte("check_out_at", `${day}T23:59:59.999Z`);
      }
      const { data, error } = await q;
      if (error) fail("storage", error.message);
      return (data ?? []).map((r) => {
        const row = r as Record<string, unknown>;
        const guest = row.guest as { name?: string } | null;
        const room = row.room as { local_id?: number; number?: string } | null;
        const rate = row.rate_plan as { local_id?: number; name?: string; kind?: string } | null;
        const stay: Stay = {
          id: Number(row.local_id ?? 0),
          room_id: Number(room?.local_id ?? 0),
          room_number: String(room?.number ?? ""),
          guest_id: 0,
          guest_name: String(guest?.name ?? ""),
          guest_document: null,
          guest_phone: null,
          rate_plan_id: Number(rate?.local_id ?? 0),
          rate_plan_name: String(rate?.name ?? ""),
          rate_kind: (rate?.kind as Stay["rate_kind"]) ?? "hourly",
          reservation_id: null,
          check_in_at: String(row.check_in_at ?? ""),
          expected_checkout_at: null,
          check_out_at: (row.check_out_at as string | null) ?? null,
          status: "closed",
          converted_to_overnight: Boolean(row.converted_to_overnight),
          overnight_rate_plan_id: null,
          notes: null,
        };
        return { stay, total_cents: 0, payment_method: null } satisfies HistoryStay;
      }) as T;
    }
    case "analytics_summary":
      return await remoteAnalyticsSummary(args) as T;
    case "get_stay_detail": {
      const stayId = Number(args.stay_id);
      const { data: stayRow, error } = await sb()
        .from("stays")
        .select("*, room:rooms(local_id, number), guest:guests(name), rate_plan:rate_plans!rate_plan_uid(*)")
        .eq("local_id", stayId)
        .maybeSingle();
      if (error) fail("storage", error.message);
      if (!stayRow) fail("not_found", "Estadía no encontrada");
      const row = stayRow as Record<string, unknown>;
      const guest = row.guest as { name?: string } | null;
      const room = row.room as { local_id?: number; number?: string } | null;
      const rateRow = row.rate_plan as Record<string, unknown> | null;
      const stay: Stay = {
        id: stayId,
        room_id: Number(room?.local_id ?? 0),
        room_number: String(room?.number ?? ""),
        guest_id: 0,
        guest_name: String(guest?.name ?? ""),
        guest_document: null,
        guest_phone: null,
        rate_plan_id: Number(rateRow?.local_id ?? 0),
        rate_plan_name: String(rateRow?.name ?? ""),
        rate_kind: (rateRow?.kind as Stay["rate_kind"]) ?? "hourly",
        reservation_id: null,
        check_in_at: String(row.check_in_at ?? ""),
        expected_checkout_at: (row.expected_checkout_at as string | null) ?? null,
        check_out_at: (row.check_out_at as string | null) ?? null,
        status: String(row.status ?? "open"),
        converted_to_overnight: Boolean(row.converted_to_overnight),
        overnight_rate_plan_id: null,
        notes: null,
      };
      const { data: charges } = await sb().from("charges").select("*").eq("stay_uid", row.uid).is("deleted_at", null);
      const chargeList: Charge[] = (charges ?? []).map((c) => {
        const cr = c as Record<string, unknown>;
        return {
          id: Number(cr.local_id ?? 0),
          stay_id: stayId,
          kind: String(cr.kind ?? "other"),
          description: String(cr.description ?? ""),
          amount_cents: Number(cr.amount_cents ?? 0),
          created_at: String(cr.created_at ?? ""),
        };
      });
      const settings = await settingsFromKv();
      const rate = rateRow ? mapRate(rateRow) : null;
      const manualLines: LineItem[] = chargeList
        .filter((c) => c.kind === "surcharge" || c.kind === "discount")
        .map((c) => ({ kind: c.kind, description: c.description, amount_cents: c.amount_cents }));
      const bill: BillPreview = rate
        ? {
            ...previewBillLocal({
              stayId: stay.id,
              checkIn: new Date(stay.check_in_at),
              now: new Date(),
              rate,
              converted: stay.converted_to_overnight,
              manualLines,
              taxPercent: settings.tax_percent,
            }),
            duration_label: `Estimativo · ${previewBillLocal({
              stayId: stay.id,
              checkIn: new Date(stay.check_in_at),
              now: new Date(),
              rate,
              converted: stay.converted_to_overnight,
              manualLines,
              taxPercent: settings.tax_percent,
            }).duration_label}`,
          }
        : {
            stay_id: stay.id,
            lines: [],
            subtotal_cents: 0,
            tax_percent: settings.tax_percent,
            tax_cents: 0,
            total_cents: 0,
            applied_kind: "hourly",
            duration_label: "Estimativo",
            overnight_applied: false,
          };
      const payments: Payment[] = [];
      return [stay, bill, chargeList, payments] as T;
    }
    case "preview_bill": {
      const detail = await supabaseInvoke<[Stay, BillPreview, Charge[], Payment[]]>("get_stay_detail", {
        stay_id: args.stay_id,
      });
      return detail[1] as T;
    }
    case "get_settings":
      return (await settingsFromKv()) as T;
    case "save_room":
    case "save_rate_plan":
    case "save_product":
    case "set_product_active":
    case "save_settings":
    case "auth_create_user": {
      if (!authSession || authSession.user.role !== "admin") fail("forbidden", "Esta operación requiere administración");
      const entity = ({save_room:"rooms",save_rate_plan:"rate_plans",save_product:"products",set_product_active:"products",save_settings:"settings",auth_create_user:"app_users"} as Record<string,string>)[name];
      const original = (args.payload ?? {}) as Record<string,unknown>;
      const operation = String(args.operation_id ?? original.operation_id ?? crypto.randomUUID());
      let payload: Record<string,unknown>;
      if (name === "save_settings") {
        const settings = original as unknown as AppSettings;
        const values: Record<string,string> = {
          business_name:settings.business_name,address:settings.address,phone:settings.phone,
          tax_percent:String(settings.tax_percent),currency_symbol:settings.currency_symbol,
          receipt_footer:settings.receipt_footer,require_guest_name:String(settings.require_guest_name),
        };
        if (!settingsVersions) fail("conflict","Recargá los ajustes antes de guardar");
        payload = {values,versions:settings.catalog_versions ?? settingsVersions};
      } else if (name === "auth_create_user") {
        const {api} = await import("./api");
        const {hash} = await api.hashPassword(String(original.password ?? ""), operation);
        payload = {uid:operation,expected_version:0,username:String(original.username ?? "").trim().toLowerCase(),password_hash:hash,role:original.role,active:true};
      } else {
        const id = name === "set_product_active" ? args.product_id : original.id;
        let row: Record<string,unknown> | null = null;
        if (id != null) {
          const {data,error} = await sb().from(entity).select("*").eq("local_id",id).maybeSingle();
          if (error) fail("storage","Requiere conexión con administración");
          if (!data) fail("not_found","Ficha de catálogo no encontrada");
          row=data;
        }
        payload = name === "set_product_active" ? {...row,active:Boolean(args.active)} : {...original};
        payload.uid=row?.uid ?? operation;
        payload.local_id=row?.local_id ?? null;
        payload.expected_version=args.expected_version ?? original.expected_version ?? row?.version ?? 0;
        delete payload.id; delete payload.operation_id;
        if (entity==="rooms") { payload.active=row?.active ?? true; delete payload.status; }
        if (entity==="products") payload.sort_order ??= 0;
      }
      const {data,error} = await sb().rpc("catalog_write",{
        p_entity:entity,p_payload:payload,p_operation_id:operation,p_actor:null,
      });
      if (error) {
        if (error.code==="42501") fail("forbidden","Sin permiso para modificar el catálogo");
        if (error.message.includes("conflict:")) fail("conflict","La ficha cambió; recargá antes de guardar");
        fail("storage","Requiere conexión con administración. Revisá los datos y la migración I11.");
      }
      const row=(data as {row:Record<string,unknown>}).row;
      if (entity==="rooms") return mapRoom(row) as T;
      if (entity==="rate_plans") return mapRate(row) as T;
      if (entity==="products") return mapProduct(row) as T;
      if (entity==="app_users") return {id:Number(row.local_id),username:String(row.username),role:row.role} as T;
      return await settingsFromKv() as T;
    }
    default:
      fail("forbidden", `Comando no disponible en administración remota: ${name}`);
  }
}

export type OperationalListener = () => void;

export function subscribeOperational(onChange: OperationalListener): () => void {
  const c = sb();
  if (boardChannel) void c.removeChannel(boardChannel);
  boardChannel = c
    .channel("n07-operational")
    .on("postgres_changes", { event: "*", schema: "public", table: "stays" }, () => onChange())
    .on("postgres_changes", { event: "*", schema: "public", table: "rooms" }, () => onChange())
    .on("postgres_changes", { event: "*", schema: "public", table: "charges" }, () => onChange())
    .subscribe();
  return () => {
    if (boardChannel) {
      void c.removeChannel(boardChannel);
      boardChannel = null;
    }
  };
}
