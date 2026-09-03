import type {
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
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(name, toTauriArgs(args));
  }
  const { mockInvoke } = await import("./mock");
  return mockInvoke<T>(name, args ?? {});
}

export const api = {
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
