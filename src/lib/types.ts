export type RateKind = "hourly" | "night" | "overnight";
export type SessionUser = { id: number; username: string; role: "admin" | "recepcion" };
export type SessionInfo = { token: string; user: SessionUser; expires_at: number };
export type LoginPayload = { username: string; password: string };
export type CreateUserPayload = LoginPayload & { role: SessionUser["role"] };
export type ManagedUser = SessionUser & { active: boolean; version: number };

export type Room = {
  id: number;
  number: string;
  room_type: string;
  floor: number;
  status: string;
  notes: string | null;
  active: boolean;
  version: number;
};

export type RatePlan = {
  id: number;
  name: string;
  kind: RateKind;
  base_amount_cents: number;
  extra_hour_cents: number;
  included_hours: number;
  grace_minutes: number;
  night_cutoff_hour: number;
  active: boolean;
  version: number;
};

export type Product = {
  id: number;
  name: string;
  category: string;
  price_cents: number;
  active: boolean;
  sort_order: number;
  version: number;
};

export type Stay = {
  id: number;
  room_id: number;
  room_number: string;
  guest_id: number;
  guest_name: string;
  guest_document: string | null;
  guest_phone: string | null;
  rate_plan_id: number;
  rate_plan_name: string;
  rate_kind: RateKind;
  reservation_id: number | null;
  check_in_at: string;
  expected_checkout_at: string | null;
  check_out_at: string | null;
  status: string;
  converted_to_overnight: boolean;
  overnight_rate_plan_id: number | null;
  notes: string | null;
};

export type Reservation = {
  id: number;
  guest_id: number;
  guest_name: string;
  guest_document: string | null;
  guest_phone: string | null;
  room_id: number;
  room_number: string;
  rate_plan_id: number;
  rate_plan_name: string;
  expected_arrival_at: string;
  expected_nights: number;
  status: string;
  notes: string | null;
};

export type Charge = {
  id: number;
  stay_id: number;
  kind: string;
  description: string;
  amount_cents: number;
  created_at: string;
  deleted_at?: string | null;
};

export type Payment = {
  id: number;
  stay_id: number;
  method: string;
  amount_cents: number;
  created_at: string;
};

export type BoardRoom = {
  room: Room;
  display_status: string;
  stay: Stay | null;
  reservation: Reservation | null;
  estimated_total_cents: number | null;
  elapsed_minutes: number | null;
};

export type LineItem = {
  kind: string;
  description: string;
  amount_cents: number;
};

export type BillPreview = {
  stay_id: number;
  lines: LineItem[];
  subtotal_cents: number;
  tax_percent: number;
  tax_cents: number;
  total_cents: number;
  applied_kind: RateKind;
  duration_label: string;
  overnight_applied: boolean;
};

export type AppSettings = {
  catalog_versions?: Record<string, number>;
  business_name: string;
  address: string;
  phone: string;
  tax_percent: number;
  currency_symbol: string;
  theme: string;
  receipt_footer: string;
  printer_enabled: boolean;
  printer_path: string;
  printer_name: string;
  paper_width: number;
  auto_print_on_checkout: boolean;
  require_guest_name: boolean;
  pin_hash: string;
  has_pin: boolean;
};

export type Guest = {
  id: number;
  name: string;
  document: string | null;
  phone: string | null;
};

export type MutationMeta = {
  operation_id?: string | null;
  expected_version?: number | null;
};

export type SaveRoomPayload = MutationMeta & {
  id?: number | null;
  number: string;
  room_type: string;
  floor: number;
  notes?: string | null;
};

export type SaveRatePlanPayload = MutationMeta & {
  id?: number | null;
  name: string;
  kind: RateKind;
  base_amount_cents: number;
  extra_hour_cents: number;
  included_hours: number;
  grace_minutes: number;
  night_cutoff_hour: number;
  active: boolean;
};

export type SaveProductPayload = MutationMeta & {
  id?: number | null;
  name: string;
  category: string;
  price_cents: number;
  active: boolean;
  sort_order?: number | null;
};

export type CreateReservationPayload = MutationMeta & {
  guest_name: string;
  document?: string | null;
  phone?: string | null;
  room_id: number;
  rate_plan_id: number;
  expected_arrival_at: string;
  expected_nights: number;
  notes?: string | null;
};

export type AddChargePayload = MutationMeta & {
  stay_id: number;
  kind: string;
  description: string;
  amount_cents: number;
};

export type AddProductChargePayload = MutationMeta & {
  stay_id: number;
  product_id: number;
};

export type CheckInPayload = MutationMeta & {
  room_id: number;
  guest_name: string;
  document?: string | null;
  phone?: string | null;
  rate_plan_id: number;
  expected_hours?: number | null;
  reservation_id?: number | null;
};

export type CheckOutPayload = MutationMeta & {
  stay_id: number;
  print: boolean;
};

export type ContractInfo = {
  contract_version: number;
  app_version: string;
  schema_migrations: string[];
};

export type CheckOutResult = {
  stay: Stay;
  bill: BillPreview;
  print_error: string | null;
};

export type HistoryStay = {
  stay: Stay;
  total_cents: number;
  payment_method: string | null;
};

export type DailyAccount = {
  stay_id: number; room_number: string; check_in_at: string; check_out_at: string | null;
  closed_on_day: boolean; open_at_cutoff: boolean; total_cents: number | null;
};
export type DailyReport = {
  date: string; generated_at: string; cutoff_at: string; timezone: string;
  occupied_rooms: number; closed_total_cents: number; adjustments_total_cents: number;
  accounts: DailyAccount[]; adjustments: Charge[];
};

/** Historical amounts come from the closed account, never from today's catalog prices. */
export type AnalyticsSummary = {
  from: string;
  to: string;
  generated_at: string;
  total_revenue_cents: number;
  closed_accounts: number;
  average_ticket_cents: number;
  lodging_cents: number;
  extras_cents: number;
  discount_cents: number;
  tax_cents: number;
  average_stay_minutes: number | null;
  reservation_arrivals: number;
  reservation_cancellations: number;
  no_shows: number;
  check_in_hours: { hour: number; count: number }[];
  /** Current snapshot, independent of the selected date range. */
  current_rooms: { status: string; count: number }[];
  daily: { date: string; revenue_cents: number; closed_accounts: number; check_ins: number; reservation_arrivals: number; reservation_cancellations: number; no_shows: number }[];
  by_room_type: { room_type: string; revenue_cents: number; closed_accounts: number }[];
  by_room: { room_number: string; room_type: string; revenue_cents: number; closed_accounts: number }[];
  /** Additional-charge descriptions; a charge is not necessarily a catalog product. */
  top_extras: { description: string; count: number; revenue_cents: number }[];
};

export type DeviceMode = "reception" | "remote";

export type RemoteConfigurePayload = {
  project_url: string;
  anon_key: string;
};

export type SyncConfigureDevicePayload = RemoteConfigurePayload & {
  device_email: string;
  device_password: string;
};

export type SyncStatus = {
  connected: boolean;
  pending_outbox: number;
  last_push_at: string | null;
  last_pull_at: string | null;
  last_error: string | null;
  configured: boolean;
  realtime_connected: boolean;
  embedded?: boolean;
};

export type BackupStatus = {
  last_local_at: string | null;
  last_remote_at: string | null;
  pending: number;
  last_error: string | null;
  /** False until the local backup engine can actually create a snapshot. */
  ready?: boolean;
};

export type BackupRunResult = { backup_id: string };

export type BackupSource = "local" | "remote";

export type BackupImportKeyPayload = { key_hex: string };

export type BackupListItem = {
  backup_id: string;
  created_at: string;
  status: string;
  size_bytes: number;
  checksum: string;
  schema_version: string;
  uploaded_at: string | null;
  remote_path: string | null;
  source?: BackupSource;
};

export type HashPasswordResult = { hash: string };
