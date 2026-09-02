use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateKind {
    Hourly,
    Night,
    Overnight,
}

impl RateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hourly => "hourly",
            Self::Night => "night",
            Self::Overnight => "overnight",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "night" => Self::Night,
            "overnight" => Self::Overnight,
            _ => Self::Hourly,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: i64,
    pub number: String,
    pub room_type: String,
    pub floor: i64,
    pub status: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatePlan {
    pub id: i64,
    pub name: String,
    pub kind: RateKind,
    pub base_amount_cents: i64,
    pub extra_hour_cents: i64,
    pub included_hours: i64,
    pub grace_minutes: i64,
    pub night_cutoff_hour: i64,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guest {
    pub id: i64,
    pub name: String,
    pub document: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reservation {
    pub id: i64,
    pub guest_id: i64,
    pub guest_name: String,
    pub guest_document: Option<String>,
    pub guest_phone: Option<String>,
    pub room_id: i64,
    pub room_number: String,
    pub rate_plan_id: i64,
    pub rate_plan_name: String,
    pub expected_arrival_at: String,
    pub expected_nights: i64,
    pub status: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Charge {
    pub id: i64,
    pub stay_id: i64,
    pub kind: String,
    pub description: String,
    pub amount_cents: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    pub id: i64,
    pub stay_id: i64,
    pub method: String,
    pub amount_cents: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stay {
    pub id: i64,
    pub room_id: i64,
    pub room_number: String,
    pub guest_id: i64,
    pub guest_name: String,
    pub guest_document: Option<String>,
    pub guest_phone: Option<String>,
    pub rate_plan_id: i64,
    pub rate_plan_name: String,
    pub rate_kind: RateKind,
    pub reservation_id: Option<i64>,
    pub check_in_at: String,
    pub expected_checkout_at: Option<String>,
    pub check_out_at: Option<String>,
    pub status: String,
    pub converted_to_overnight: bool,
    pub overnight_rate_plan_id: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardRoom {
    pub room: Room,
    pub display_status: String,
    pub stay: Option<Stay>,
    pub reservation: Option<Reservation>,
    pub estimated_total_cents: Option<i64>,
    pub elapsed_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineItem {
    pub kind: String,
    pub description: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillPreview {
    pub stay_id: i64,
    pub lines: Vec<LineItem>,
    pub subtotal_cents: i64,
    pub tax_percent: f64,
    pub tax_cents: i64,
    pub total_cents: i64,
    pub applied_kind: RateKind,
    pub duration_label: String,
    pub overnight_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub business_name: String,
    pub address: String,
    pub phone: String,
    pub tax_percent: f64,
    pub currency_symbol: String,
    pub theme: String,
    pub receipt_footer: String,
    pub printer_enabled: bool,
    pub printer_path: String,
    pub printer_name: String,
    pub paper_width: i64,
    pub auto_print_on_checkout: bool,
    pub require_guest_name: bool,
    pub pin_hash: String,
    pub has_pin: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            business_name: "Nightdesk Inn".into(),
            address: "Av. Principal 100".into(),
            phone: "".into(),
            tax_percent: 10.0,
            currency_symbol: "Gs.".into(),
            theme: "dark".into(),
            receipt_footer: "Gracias por su visita".into(),
            printer_enabled: false,
            printer_path: "".into(),
            printer_name: "".into(),
            paper_width: 80,
            auto_print_on_checkout: true,
            require_guest_name: true,
            pin_hash: "".into(),
            has_pin: false,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CheckInPayload {
    pub room_id: i64,
    pub guest_name: String,
    pub document: Option<String>,
    pub phone: Option<String>,
    pub rate_plan_id: i64,
    pub expected_hours: Option<i64>,
    pub reservation_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CheckOutPayload {
    pub stay_id: i64,
    pub method: String,
    pub amount_cents: i64,
    pub print: bool,
}

#[derive(Debug, Deserialize)]
pub struct SaveRoomPayload {
    pub id: Option<i64>,
    pub number: String,
    pub room_type: String,
    pub floor: i64,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaveRatePlanPayload {
    pub id: Option<i64>,
    pub name: String,
    pub kind: RateKind,
    pub base_amount_cents: i64,
    pub extra_hour_cents: i64,
    pub included_hours: i64,
    pub grace_minutes: i64,
    pub night_cutoff_hour: i64,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateReservationPayload {
    pub guest_name: String,
    pub document: Option<String>,
    pub phone: Option<String>,
    pub room_id: i64,
    pub rate_plan_id: i64,
    pub expected_arrival_at: String,
    pub expected_nights: i64,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddChargePayload {
    pub stay_id: i64,
    pub kind: String,
    pub description: String,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct CheckOutResult {
    pub stay: Stay,
    pub bill: BillPreview,
    pub print_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HistoryStay {
    pub stay: Stay,
    pub total_cents: i64,
    pub payment_method: Option<String>,
}
