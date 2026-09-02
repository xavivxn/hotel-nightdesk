export type RateKind = "hourly" | "night" | "overnight";

export type Room = {
  id: number;
  number: string;
  room_type: string;
  floor: number;
  status: string;
  notes: string | null;
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

export type CheckInPayload = {
  room_id: number;
  guest_name: string;
  document?: string | null;
  phone?: string | null;
  rate_plan_id: number;
  expected_hours?: number | null;
  reservation_id?: number | null;
};

export type CheckOutPayload = {
  stay_id: number;
  method: string;
  amount_cents: number;
  print: boolean;
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
