import type {
  SessionInfo, SessionUser, LoginPayload, CreateUserPayload,
  AppSettings,
  BillPreview,
  BoardRoom,
  Charge,
  CheckInPayload,
  CheckOutPayload,
  CheckOutResult,
  HistoryStay,
  Payment,
  Product,
  RatePlan,
  Reservation,
  Room,
  Stay,
} from "./types";

function isTauri() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
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

export async function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
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
    if (String(error).includes("SESSION_EXPIRED") && requestToken === sessionToken) {
      sessionToken = null;
      window.dispatchEvent(new Event("nightdesk-session-expired"));
    }
    throw error;
  }
}

let sessionToken: string | null = null;

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
  createUser: (payload: CreateUserPayload) => cmd<SessionUser>("auth_create_user", { payload }),
  listBoard: () => cmd<BoardRoom[]>("list_board"),
  listRooms: () => cmd<Room[]>("list_rooms"),
  saveRoom: (payload: unknown) => cmd<Room>("save_room", { payload }),
  setRoomStatus: (room_id: number, status: string) => cmd<Room>("set_room_status", { room_id, status }),
  listRatePlans: (active_only = false) => cmd<RatePlan[]>("list_rate_plans", { active_only }),
  saveRatePlan: (payload: unknown) => cmd<RatePlan>("save_rate_plan", { payload }),
  checkIn: (payload: CheckInPayload) => cmd<Stay>("check_in", { payload }),
  previewBill: (stay_id: number) => cmd<BillPreview>("preview_bill", { stay_id }),
  getStayDetail: (stay_id: number) =>
    cmd<[Stay, BillPreview, Charge[], Payment[]]>("get_stay_detail", { stay_id }),
  convertToOvernight: (stay_id: number) => cmd<Stay>("convert_to_overnight", { stay_id }),
  listProducts: (active_only = true) => cmd<Product[]>("list_products", { active_only }),
  saveProduct: (payload: unknown) => cmd<Product>("save_product", { payload }),
  setProductActive: (product_id: number, active: boolean) =>
    cmd<Product>("set_product_active", { product_id, active }),
  addCharge: (payload: unknown) => cmd<Charge>("add_charge", { payload }),
  addProductCharge: (payload: { stay_id: number; product_id: number }) =>
    cmd<Charge>("add_product_charge", { payload }),
  deleteCharge: (charge_id: number) => cmd<void>("delete_charge", { charge_id }),
  checkOut: (payload: CheckOutPayload) => cmd<CheckOutResult>("check_out", { payload }),
  listReservations: () => cmd<Reservation[]>("list_reservations"),
  createReservation: (payload: unknown) => cmd<Reservation>("create_reservation", { payload }),
  setReservationStatus: (reservation_id: number, status: string) =>
    cmd<Reservation>("set_reservation_status", { reservation_id, status }),
  checkInReservation: (reservation_id: number) => cmd<Stay>("check_in_reservation", { reservation_id }),
  listHistory: (date?: string) => cmd<HistoryStay[]>("list_history", { date }),
  getSettings: () => cmd<AppSettings>("get_settings"),
  saveSettings: (payload: AppSettings, new_pin?: string) =>
    cmd<AppSettings>("save_settings", new_pin === undefined ? { payload } : { payload, new_pin }),
  verifyPin: (pin: string) => cmd<boolean>("verify_pin", { pin }),
  pinRequired: () => cmd<boolean>("pin_required"),
  printTest: () => cmd<string | null>("print_test"),
  reprintReceipt: (stay_id: number) => cmd<string | null>("reprint_receipt", { stay_id }),
};
