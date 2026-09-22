import { createClient, type RealtimeChannel, type SupabaseClient } from "@supabase/supabase-js";
import { previewBill as previewBillLocal } from "./billing";
import { fail } from "./errors";
import type {
  AppSettings,
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
        .select("*, room:rooms(local_id, number, status), guest:guests(name), rate_plan:rate_plans(local_id, name, kind)")
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
        .select("*, room:rooms(local_id, number), guest:guests(name), rate_plan:rate_plans(local_id, name, kind)")
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
    case "get_stay_detail": {
      const stayId = Number(args.stay_id);
      const { data: stayRow, error } = await sb()
        .from("stays")
        .select("*, room:rooms(local_id, number), guest:guests(name), rate_plan:rate_plans(*)")
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
