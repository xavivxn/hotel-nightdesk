import type {
  AccountQuote, LanStatus, LanHost, PendingOperation, OperatorActivity,
  DailyReport,
  AnalyticsSummary,
  SessionInfo, SessionUser, LoginPayload, CreateUserPayload, ManagedUser,
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
  BackupRunResult,
  BackupListItem,
  BackupSource,
  AppUpdateInfo,
  AppUpdateProgress,
  EffectivePrice,
  PriceRule,
  ProductStock,
  StockMovement,
  UpdateStockPayload,
} from "./types";
import { ApiError, toApiError } from "./errors";

function isTauri() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

if (typeof window !== "undefined" && isTauri()) {
  void import("@tauri-apps/api/event").then(({ listen }) => {
    void listen("reception:changed", () => { window.dispatchEvent(new Event("reception:changed")); });
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
  "remote_embedded_auth",
  "hash_password",
  "sync_status",
  "sync_pull_now",
  "sync_configure_device",
  "app_update_check",
  "app_update_install",
  "lan_control",
]);

const OPERATIONAL = new Set([
  "check_in", "check_out", "add_charge", "add_product_charge", "delete_charge", "set_room_status",
  "convert_to_overnight", "create_reservation", "set_reservation_status", "check_in_reservation",
  "update_product_stock", "save_price_rules",
]);
const DIRECT_DEVICE = new Set(["auth_setup", "lan_control", "backup_run_now", "backup_list", "backup_import_key", "backup_restore"]);

/** Local SQLite auth when the binary embeds the remote Supabase admin. */
const AUTH_WHEN_EMBEDDED = new Set([
  "auth_setup_required",
  "auth_setup",
  "auth_login",
  "auth_session",
  "auth_logout",
]);

let sessionToken: string | null = null;
let cachedMode: DeviceMode | null | undefined;
let cachedEmbeddedRemote: boolean | undefined;

export async function refreshDeviceMode(): Promise<DeviceMode | null> {
  cachedMode = undefined;
  cachedEmbeddedRemote = undefined;
  return getDeviceMode();
}

export async function remoteEmbeddedAuth(): Promise<boolean> {
  if (cachedEmbeddedRemote !== undefined) return cachedEmbeddedRemote;
  cachedEmbeddedRemote = await cmdRaw<boolean>("remote_embedded_auth", {});
  return cachedEmbeddedRemote;
}

function publicSession(session: SessionInfo): SessionInfo {
  return {
    token: session.token,
    user: session.user,
    expires_at: session.expires_at,
  };
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
      const reception = cachedMode === "reception" || cachedMode === "reception_client";
      if (reception && !DIRECT_DEVICE.has(name) && (!LOCAL_ALWAYS.has(name) || (cachedMode === "reception_client" && name === "sync_status"))) {
        const payload = args?.payload as { operation_id?: string } | undefined;
        const operationArgs = OPERATIONAL.has(name)
          ? { ...args, operation_id: args?.operation_id ?? payload?.operation_id ?? crypto.randomUUID() }
          : { ...args };
        const value = await invoke<T>("reception_invoke", { command: name, args: operationArgs, sessionToken: requestToken });
        if (OPERATIONAL.has(name)) window.dispatchEvent(new Event("reception:changed"));
        return value;
      }
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

/**
 * Stock and promotions live in the reception SQLite only. The remote admin PC gets the fallback
 * for reads and a clear message for writes instead of touching its own, non-operational database.
 */
async function receptionCmd<T>(name: string, args?: Record<string, unknown>, remoteFallback?: T): Promise<T> {
  if (isTauri() && (await getDeviceMode()) === "remote") {
    if (remoteFallback !== undefined) return remoteFallback;
    throw new ApiError("validation", "Esto se configura en la PC de recepción.");
  }
  return cmdRaw<T>(name, args);
}

export async function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (LOCAL_ALWAYS.has(name)) return cmdRaw<T>(name, args);
  const mode = await getDeviceMode();
  if (isTauri() && mode === "remote") {
    if (AUTH_WHEN_EMBEDDED.has(name) && (await remoteEmbeddedAuth())) {
      return cmdRaw<T>(name, args);
    }
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
    try {
      if (session.supabase_access_token && session.supabase_refresh_token) {
        const { adoptEmbeddedSession } = await import("./supabase");
        await adoptEmbeddedSession(
          session.supabase_access_token,
          session.supabase_refresh_token,
          publicSession(session),
        );
      }
      return publicSession(session);
    } catch (error) {
      sessionToken = null;
      try {
        await cmd<void>("auth_logout");
      } catch {
        /* local session already unusable */
      }
      throw error;
    }
  },
  session: () => cmd<SessionInfo>("auth_session").then(publicSession),
  logout: async () => {
    try {
      await cmd<void>("auth_logout");
    } finally {
      sessionToken = null;
      if (isTauri()) {
        const { clearSupabaseAuth } = await import("./supabase");
        await clearSupabaseAuth();
      }
    }
  },
  clearSession: () => {
    sessionToken = null;
    if (isTauri()) {
      void import("./supabase").then(({ clearSupabaseAuth }) => clearSupabaseAuth());
    }
  },
  createUser: (payload: CreateUserPayload, operation_id = crypto.randomUUID()) => cmd<SessionUser>("auth_create_user", { payload, operation_id }),
  listUsers: () => cmd<ManagedUser[]>("list_users"),
  setUserActive: (user_id: number, active: boolean, expected_version?: number, operation_id = crypto.randomUUID()) =>
    cmd<ManagedUser>("set_user_active", { user_id, active, expected_version, operation_id }),
  deleteUser: (user_id: number, expected_version?: number, operation_id = crypto.randomUUID()) =>
    cmd<void>("delete_user", { user_id, expected_version, operation_id }),
  contractInfo: () => cmd<ContractInfo>("contract_info"),
  listBoard: () => cmd<BoardRoom[]>("list_board"),
  listRooms: () => cmd<Room[]>("list_rooms"),
  saveRoom: (payload: SaveRoomPayload) => cmd<Room>("save_room", { payload: withOperationId(payload) }),
  setRoomStatus: (room_id: number, status: string, expected_version?: number) => cmd<Room>("set_room_status", { room_id, status, expected_version }),
  listRatePlans: (active_only = false) => cmd<RatePlan[]>("list_rate_plans", { active_only }),
  saveRatePlan: (payload: SaveRatePlanPayload) => cmd<RatePlan>("save_rate_plan", { payload: withOperationId(payload) }),
  checkIn: (payload: CheckInPayload) => cmd<Stay>("check_in", { payload: withOperationId(payload) }),
  previewBill: (stay_id: number) => cmd<BillPreview>("preview_bill", { stay_id }),
  getStayDetail: (stay_id: number) =>
    cmd<[Stay, BillPreview, Charge[], Payment[]]>("get_stay_detail", { stay_id }),
  convertToOvernight: (stay_id: number, expected_version?: number) => cmd<Stay>("convert_to_overnight", { stay_id, expected_version }),
  listProducts: (active_only = true) => cmd<Product[]>("list_products", { active_only }),
  saveProduct: (payload: SaveProductPayload) => cmd<Product>("save_product", { payload: withOperationId(payload) }),
  setProductActive: (product_id: number, active: boolean, expected_version?: number, operation_id = crypto.randomUUID()) =>
    cmd<Product>("set_product_active", { product_id, active, expected_version, operation_id }),
  addCharge: (payload: AddChargePayload) => cmd<Charge>("add_charge", { payload: withOperationId(payload) }),
  addProductCharge: (payload: AddProductChargePayload) =>
    cmd<Charge>("add_product_charge", { payload: withOperationId(payload) }),
  deleteCharge: (charge_id: number) => cmd<void>("delete_charge", { charge_id }),
  checkOut: async (payload: CheckOutPayload): Promise<CheckOutResult> => {
    const operation = withOperationId({ ...payload, print: false });
    const result = await cmd<CheckOutResult>("check_out", { payload: operation });
    if (payload.print && isTauri() && (await getDeviceMode()) !== "remote") {
      try { result.print_error = await cmd<string | null>("receipt_print", { stay_id: result.stay.id, copies: 2, job_id: operation.operation_id }); }
      catch (error) { result.print_error = String(error); }
    }
    return result;
  },
  listReservations: () => cmd<Reservation[]>("list_reservations"),
  createReservation: (payload: CreateReservationPayload) =>
    cmd<Reservation>("create_reservation", { payload: withOperationId(payload) }),
  setReservationStatus: (reservation_id: number, status: string, expected_version?: number) =>
    cmd<Reservation>("set_reservation_status", { reservation_id, status, expected_version }),
  checkInReservation: (reservation_id: number, expected_version?: number) => cmd<Stay>("check_in_reservation", { reservation_id, expected_version }),
  listHistory: (date?: string) => cmd<HistoryStay[]>("list_history", { date }),
  dailyReport: (date: string) => cmd<DailyReport>("daily_report", { date }),
  analyticsSummary: (from: string, to: string, roomType?: string) =>
    cmd<AnalyticsSummary>("analytics_summary", { from, to, room_type: roomType ?? null }),
  exportAnalyticsPdf: async (from: string, to: string, roomType?: string) => {
    const report = await cmd<AnalyticsSummary>("analytics_summary", { from, to, room_type: roomType ?? null });
    const { buildAnalyticsPdf } = await import("./analytics-pdf");
    const bytes = buildAnalyticsPdf(report, roomType);
    if (isTauri() && await getDeviceMode() === "reception") {
      return cmd<string>("save_analytics_pdf", { from, to, bytes: Array.from(bytes) });
    }
    const url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "application/pdf" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = `analisis-${from}-a-${to}.pdf`;
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 60000);
    return "Carpeta de descargas";
  },
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
  printTest: () => cmd<string | null>("print_test", { job_id: crypto.randomUUID() }),
  listPrinters: () => cmd<string[]>("list_printers"),
  reprintReceipt: async (stay_id: number, target?: "local" | "principal") => {
    if (!isTauri() || (await getDeviceMode()) === "remote") return cmd<string | null>("reprint_receipt", { stay_id });
    return cmd<string | null>("receipt_print", { stay_id, target, copies: 1, job_id: crypto.randomUUID() });
  },
  accountQuote: (stay_id: number) => cmd<AccountQuote>("account_quote", { stay_id }),
  lanStatus: () => cmd<LanStatus>("lan_control", { action: "status", args: {} }),
  lanControl: <T = unknown>(action: string, args: Record<string, unknown> = {}) => cmd<T>("lan_control", { action, args }),
  discoverReception: () => cmd<LanHost[]>("lan_control", { action: "discover", args: {} }),
  pendingOperations: () => cmd<PendingOperation[]>("lan_control", { action: "pending", args: {} }),
  retryOperation: (operation_id: string) => cmd<PendingOperation[]>("lan_control", { action: "retry", args: { operation_id } }),
  receptionRevision: () => cmd<{ epoch: string; revision: number }>("lan_revision"),
  printerConfig: () => cmd<{ settings: AppSettings; target: "local" | "principal" }>("printer_config_get"),
  savePrinterConfig: (settings: AppSettings, target: "local" | "principal") => cmd<void>("printer_config_save", { settings, target }),
  operatorActivity: (from: string, to: string, user_uid?: string) => cmd<OperatorActivity>("operator_activity", { from, to, user_uid }),

  deviceModeGet: () => refreshDeviceMode(),
  deviceModeSet: async (mode: DeviceMode) => {
    await cmd<void>("device_mode_set", { payload: { mode } });
    cachedMode = mode;
    cachedEmbeddedRemote = undefined;
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
  remoteEmbeddedAuth: () => remoteEmbeddedAuth(),
  hashPassword: (password: string, operation_id?: string) => cmd<HashPasswordResult>("hash_password", { payload: { password }, operation_id }),
  syncStatus: () => cmd<SyncStatus>("sync_status"),
  syncPullNow: () => cmd<void>("sync_pull_now"),
  syncConfigureDevice: (payload: SyncConfigureDevicePayload) =>
    cmd<void>("sync_configure_device", { payload }),
  backupStatus: () => cmd<BackupStatus>("backup_status"),
  backupRunNow: () => cmd<BackupRunResult>("backup_run_now"),
  backupList: () => cmd<BackupListItem[]>("backup_list"),
  backupImportKey: (key_hex: string) => cmd<void>("backup_import_key", { payload: { key_hex } }),
  backupRestore: (backup_id: string, source?: BackupSource) =>
    cmd<void>("backup_restore", { payload: { backup_id, source } }),
  listProductStock: () => receptionCmd<ProductStock[]>("list_product_stock", undefined, []),
  updateProductStock: (payload: UpdateStockPayload) =>
    receptionCmd<ProductStock | null>("update_product_stock", { payload }),
  listStockMovements: (product_id: number) =>
    receptionCmd<StockMovement[]>("list_stock_movements", { product_id }, []),
  listPriceRules: () => receptionCmd<PriceRule[]>("list_price_rules", undefined, []),
  savePriceRules: (rules: PriceRule[]) => receptionCmd<PriceRule[]>("save_price_rules", { rules }),
  currentPrices: () => receptionCmd<EffectivePrice[]>("current_prices", undefined, []),
  /** Versión nueva publicada o `null`. Siempre local: el binario se actualiza en esta PC. */
  updateCheck: () => cmd<AppUpdateInfo | null>("app_update_check"),
  /** Descarga, verifica e instala. En Windows la app se cierra y el instalador la vuelve a abrir. */
  updateInstall: async (onProgress: (progress: AppUpdateProgress) => void) => {
    if (!isTauri()) return cmd<void>("app_update_install");
    const { Channel } = await import("@tauri-apps/api/core");
    const channel = new Channel<AppUpdateProgress>();
    channel.onmessage = onProgress;
    return cmd<void>("app_update_install", { on_progress: channel });
  },
  subscribeOperational: async (onChange: () => void) => {
    const mode = await getDeviceMode();
    if (mode !== "remote") {
      window.addEventListener("reception:changed", onChange);
      return () => window.removeEventListener("reception:changed", onChange);
    }
    const { subscribeOperational } = await import("./supabase");
    return subscribeOperational(onChange);
  },
};
