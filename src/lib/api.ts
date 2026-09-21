import type {
  DailyReport,
  SessionInfo, SessionUser, LoginPayload, CreateUserPayload,
  AddChargePayload,
  AddProductChargePayload,
  AppSettings,
  BillPreview,
  BoardRoom,
  Charge,
  CheckInPayload,
  CheckOutPayload,
  CheckOutResult,
  ContractInfo,
  CreateReservationPayload,
  DeviceMode,
  HistoryStay,
  HashPasswordResult,
  Payment,
  Product,
  RatePlan,
  RemoteConfigurePayload,
  Reservation,
  Room,
  SaveProductPayload,
  SaveRatePlanPayload,
  SaveRoomPayload,
  Stay,
  SyncConfigureDevicePayload,
  SyncStatus,
  BackupStatus,
} from "./types";
import { toApiError } from "./errors";

function isTauri() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

if (typeof window !== "undefined" && isTauri()) {
  void import("@tauri-apps/api/event").then(({ listen }) => {
    void listen("sync:catalog-updated", () => {
      window.dispatchEvent(new Event("sync:catalog-updated"));
    });
  });
}

/** Tauri 2 expects camelCase for top-level command args; nested payloads stay snake_case. */
function toTauriArgs(args?: Record<string, unknown>) {
  if (!args) return undefined;
  const out: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(args)) {
    const camel = key.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());
    out[camel] = value;
  }
  return out;
}

function withOperationId<T extends { operation_id?: string | null }>(payload: T): T {
  if (payload.operation_id) return payload;
  return { ...payload, operation_id: crypto.randomUUID() };
}

/** Always local IPC / mock — never supabaseInvoke. */
const LOCAL_ALWAYS = new Set([
  "device_mode_get",
  "device_mode_set",
  "remote_configure",
  "remote_configured",
  "remote_get_config",
  "hash_password",
  "sync_status",
  "sync_pull_now",
  "sync_configure_device",
  "backup_status",
]);

let sessionToken: string | null = null;
let cachedMode: DeviceMode | null | undefined;

export async function refreshDeviceMode(): Promise<DeviceMode | null> {
  cachedMode = undefined;
  return getDeviceMode();
}

export async function getDeviceMode(): Promise<DeviceMode | null> {
  if (cachedMode !== undefined) return cachedMode;
  const mode = await cmdRaw<DeviceMode | null>("device_mode_get", {});
  cachedMode = mode;
  return mode;
}

export function peekDeviceMode(): DeviceMode | null | undefined {
  return cachedMode;
}

async function cmdRaw<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  const requestToken = sessionToken;
  const requestArgs = { ...args, session_token: requestToken };
  try {
    if (isTauri()) {
      const { invoke } = await import("@tauri-apps/api/core");
      return await invoke<T>(name, toTauriArgs(requestArgs));
    }
    const { mockInvoke } = await import("./mock");
    return await mockInvoke<T>(name, requestArgs);
  } catch (error) {
    const apiError = toApiError(error);
    if (apiError.code === "session_expired" && requestToken === sessionToken) {
      sessionToken = null;
      window.dispatchEvent(new Event("nightdesk-session-expired"));
    }
    throw apiError;
  }
}

export async function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  const mode = LOCAL_ALWAYS.has(name) ? null : await getDeviceMode();
  if (isTauri() && mode === "remote" && !LOCAL_ALWAYS.has(name)) {
    const { supabaseInvoke } = await import("./supabase");
    try {
      return await supabaseInvoke<T>(name, { ...args, session_token: sessionToken });
    } catch (error) {
      const apiError = toApiError(error);
      if (apiError.code === "session_expired") {
        sessionToken = null;
        window.dispatchEvent(new Event("nightdesk-session-expired"));
      }
      throw apiError;
    }
  }
  return cmdRaw<T>(name, args);
}

export const api = {
  setupRequired: () => cmd<boolean>("auth_setup_required"),
  setupAdmin: (payload: LoginPayload, legacy_pin?: string) => cmd<SessionUser>("auth_setup", { payload, legacy_pin }),
  login: async (payload: LoginPayload) => {
    const session = await cmd<SessionInfo>("auth_login", { payload });
    sessionToken = session.token;
    return session;
  },
  session: () => cmd<SessionInfo>("auth_session"),
  logout: async () => { try { await cmd<void>("auth_logout"); } finally { sessionToken = null; } },
  clearSession: () => { sessionToken = null; },
  createUser: (payload: CreateUserPayload, operation_id = crypto.randomUUID()) => cmd<SessionUser>("auth_create_user", { payload, operation_id }),
  contractInfo: () => cmd<ContractInfo>("contract_info"),
  listBoard: () => cmd<BoardRoom[]>("list_board"),
  listRooms: () => cmd<Room[]>("list_rooms"),
  saveRoom: (payload: SaveRoomPayload) => cmd<Room>("save_room", { payload: withOperationId(payload) }),
  setRoomStatus: (room_id: number, status: string) => cmd<Room>("set_room_status", { room_id, status }),
  listRatePlans: (active_only = false) => cmd<RatePlan[]>("list_rate_plans", { active_only }),
  saveRatePlan: (payload: SaveRatePlanPayload) => cmd<RatePlan>("save_rate_plan", { payload: withOperationId(payload) }),
  checkIn: (payload: CheckInPayload) => cmd<Stay>("check_in", { payload: withOperationId(payload) }),
  previewBill: (stay_id: number) => cmd<BillPreview>("preview_bill", { stay_id }),
  getStayDetail: (stay_id: number) =>
    cmd<[Stay, BillPreview, Charge[], Payment[]]>("get_stay_detail", { stay_id }),
  convertToOvernight: (stay_id: number) => cmd<Stay>("convert_to_overnight", { stay_id }),
  listProducts: (active_only = true) => cmd<Product[]>("list_products", { active_only }),
  saveProduct: (payload: SaveProductPayload) => cmd<Product>("save_product", { payload: withOperationId(payload) }),
  setProductActive: (product_id: number, active: boolean, expected_version?: number, operation_id = crypto.randomUUID()) =>
    cmd<Product>("set_product_active", { product_id, active, expected_version, operation_id }),
  addCharge: (payload: AddChargePayload) => cmd<Charge>("add_charge", { payload: withOperationId(payload) }),
  addProductCharge: (payload: AddProductChargePayload) =>
    cmd<Charge>("add_product_charge", { payload: withOperationId(payload) }),
  deleteCharge: (charge_id: number) => cmd<void>("delete_charge", { charge_id }),
  checkOut: (payload: CheckOutPayload) => cmd<CheckOutResult>("check_out", { payload: withOperationId(payload) }),
  listReservations: () => cmd<Reservation[]>("list_reservations"),
  createReservation: (payload: CreateReservationPayload) =>
    cmd<Reservation>("create_reservation", { payload: withOperationId(payload) }),
  setReservationStatus: (reservation_id: number, status: string) =>
    cmd<Reservation>("set_reservation_status", { reservation_id, status }),
  checkInReservation: (reservation_id: number) => cmd<Stay>("check_in_reservation", { reservation_id }),
  listHistory: (date?: string) => cmd<HistoryStay[]>("list_history", { date }),
  dailyReport: (date: string) => cmd<DailyReport>("daily_report", { date }),
  exportDailyPdf: async (date: string) => {
    const report = await cmd<DailyReport>("daily_report", { date });
    const { buildDailyPdf } = await import("./daily-pdf");
    const bytes = buildDailyPdf(report);
    if (isTauri()) return cmd<string>("save_daily_pdf", { date, bytes: Array.from(bytes) });
    const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "application/pdf" }));
    const link = document.createElement("a");
    link.href = url; link.download = `resumen-${date}.pdf`; link.click();
    setTimeout(() => URL.revokeObjectURL(url), 60000);
    return "Carpeta de descargas del navegador (datos de demostración)";
  },
  getSettings: () => cmd<AppSettings>("get_settings"),
  saveSettings: (payload: AppSettings, new_pin?: string, operation_id = crypto.randomUUID()) =>
    cmd<AppSettings>("save_settings", { payload, new_pin, operation_id }),
  verifyPin: (pin: string) => cmd<boolean>("verify_pin", { pin }),
  pinRequired: () => cmd<boolean>("pin_required"),
  printTest: () => cmd<string | null>("print_test"),
  listPrinters: () => cmd<string[]>("list_printers"),
  reprintReceipt: (stay_id: number) => cmd<string | null>("reprint_receipt", { stay_id }),

  deviceModeGet: () => refreshDeviceMode(),
  deviceModeSet: async (mode: DeviceMode) => {
    await cmd<void>("device_mode_set", { payload: { mode } });
    cachedMode = mode;
  },
  remoteConfigure: async (payload: RemoteConfigurePayload) => {
    await cmd<void>("remote_configure", { payload });
    if (isTauri()) {
      const { initSupabase } = await import("./supabase");
      initSupabase(payload.project_url, payload.anon_key);
    }
  },
  remoteConfigured: () => cmd<boolean>("remote_configured"),
  remoteGetConfig: () => cmd<RemoteConfigurePayload | null>("remote_get_config"),
  hashPassword: (password: string, operation_id?: string) => cmd<HashPasswordResult>("hash_password", { payload: { password }, operation_id }),
  syncStatus: () => cmd<SyncStatus>("sync_status"),
  syncPullNow: () => cmd<void>("sync_pull_now"),
  syncConfigureDevice: (payload: SyncConfigureDevicePayload) =>
    cmd<void>("sync_configure_device", { payload }),
  backupStatus: () => cmd<BackupStatus>("backup_status"),
  subscribeOperational: async (onChange: () => void) => {
    const mode = await getDeviceMode();
    if (mode !== "remote") return () => {};
    const { subscribeOperational } = await import("./supabase");
    return subscribeOperational(onChange);
  },
};
