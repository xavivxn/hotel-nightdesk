use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Recepcion,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Recepcion => "recepcion",
        }
    }

    pub fn parse(value: &str) -> Self {
        if value == "admin" {
            Self::Admin
        } else {
            Self::Recepcion
        }
    }

    pub fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUser {
    pub id: i64,
    pub username: String,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedUser {
    pub id: i64,
    pub username: String,
    pub role: Role,
    pub active: bool,
    pub version: i64,
}

#[derive(Clone, Serialize)]
pub struct SessionInfo {
    pub token: String,
    pub user: SessionUser,
    pub expires_at: i64,
    /// Supabase JWT when remote mode used the embedded admin. Never shown in UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supabase_access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supabase_refresh_token: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct CreateUserPayload {
    pub username: String,
    pub password: String,
    pub role: String,
}

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

fn version_one() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: i64,
    pub number: String,
    pub room_type: String,
    pub floor: i64,
    pub status: String,
    pub notes: Option<String>,
    pub active: bool,
    #[serde(default = "version_one")]
    pub version: i64,
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
    #[serde(default = "version_one")]
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub price_cents: i64,
    pub active: bool,
    pub sort_order: i64,
    #[serde(default = "version_one")]
    pub version: i64,
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
    #[serde(default)]
    pub catalog_versions: std::collections::BTreeMap<String, i64>,
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
            catalog_versions: Default::default(),
            business_name: "MotelApp".into(),
            address: "Av. Principal 100".into(),
            phone: "".into(),
            tax_percent: 0.0,
            currency_symbol: "Gs.".into(),
            theme: "light".into(),
            receipt_footer: "Gracias por su visita".into(),
            printer_enabled: false,
            printer_path: "".into(),
            printer_name: "".into(),
            paper_width: 80,
            auto_print_on_checkout: true,
            require_guest_name: false,
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
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CheckOutPayload {
    pub stay_id: i64,
    pub print: bool,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveRoomPayload {
    pub id: Option<i64>,
    pub number: String,
    pub room_type: String,
    pub floor: i64,
    pub notes: Option<String>,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
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
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub expected_version: Option<i64>,
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
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AddChargePayload {
    pub stay_id: i64,
    pub kind: String,
    pub description: String,
    pub amount_cents: i64,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AddProductChargePayload {
    pub stay_id: i64,
    pub product_id: i64,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveProductPayload {
    pub id: Option<i64>,
    pub name: String,
    pub category: String,
    pub price_cents: i64,
    pub active: bool,
    pub sort_order: Option<i64>,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub expected_version: Option<i64>,
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

#[derive(Debug, Serialize)]
pub struct DailyAccount {
    pub stay_id: i64,
    pub room_number: String,
    pub check_in_at: String,
    pub check_out_at: Option<String>,
    pub closed_on_day: bool,
    pub open_at_cutoff: bool,
    pub total_cents: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct DailyReport {
    pub date: String,
    pub generated_at: String,
    pub cutoff_at: String,
    pub timezone: String,
    pub occupied_rooms: usize,
    pub closed_total_cents: i64,
    pub adjustments_total_cents: i64,
    pub accounts: Vec<DailyAccount>,
    pub adjustments: Vec<Charge>,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsStatusCount {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsDay {
    pub date: String,
    pub revenue_cents: i64,
    pub closed_accounts: i64,
    pub check_ins: i64,
    pub reservation_arrivals: i64,
    pub reservation_cancellations: i64,
    pub no_shows: i64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsHour {
    pub hour: i64,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsTypeTotal {
    pub room_type: String,
    pub revenue_cents: i64,
    pub closed_accounts: i64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsRoomTotal {
    pub room_number: String,
    pub room_type: String,
    pub revenue_cents: i64,
    pub closed_accounts: i64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsExtra {
    pub description: String,
    pub count: i64,
    pub revenue_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsHourRevenue {
    pub hour: i64,
    pub count: i64,
    pub revenue_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsSummary {
    pub from: String,
    pub to: String,
    pub generated_at: String,
    pub total_revenue_cents: i64,
    pub closed_accounts: i64,
    pub average_ticket_cents: i64,
    pub lodging_cents: i64,
    pub extras_cents: i64,
    pub discount_cents: i64,
    pub tax_cents: i64,
    pub average_stay_minutes: Option<i64>,
    pub reservation_arrivals: i64,
    pub reservation_cancellations: i64,
    pub no_shows: i64,
    pub current_rooms: Vec<AnalyticsStatusCount>,
    pub daily: Vec<AnalyticsDay>,
    pub check_in_hours: Vec<AnalyticsHour>,
    pub by_room_type: Vec<AnalyticsTypeTotal>,
    pub by_room: Vec<AnalyticsRoomTotal>,
    pub top_extras: Vec<AnalyticsExtra>,
    /// Closed accounts and their revenue by local checkout hour (0..24), for the daily summary.
    pub closed_hours: Vec<AnalyticsHourRevenue>,
    /// Shop or manual charges deleted from open accounts in the period (by deletion date).
    pub voided_count: i64,
    pub voided_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInfo {
    pub contract_version: u32,
    pub app_version: String,
    pub schema_migrations: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeviceModeSetPayload {
    pub mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteConfigurePayload {
    pub project_url: String,
    pub anon_key: String,
}

#[derive(Debug, Deserialize)]
pub struct SyncConfigureDevicePayload {
    pub project_url: String,
    pub anon_key: String,
    pub device_email: String,
    pub device_password: String,
}

#[derive(Debug, Deserialize)]
pub struct HashPasswordPayload {
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct HashPasswordResult {
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub connected: bool,
    pub pending_outbox: i64,
    pub last_push_at: Option<String>,
    pub last_pull_at: Option<String>,
    pub last_error: Option<String>,
    pub configured: bool,
    #[serde(default)]
    pub realtime_connected: bool,
    #[serde(default)]
    pub embedded: bool,
}

#[derive(Debug, Serialize)]
pub struct BackupStatus {
    pub last_local_at: Option<String>,
    pub last_remote_at: Option<String>,
    pub pending: i64,
    pub last_error: Option<String>,
    pub ready: bool,
}

#[derive(Debug, Serialize)]
pub struct BackupRunResult {
    pub backup_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupRestorePayload {
    pub backup_id: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupImportKeyPayload {
    pub key_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupListItem {
    pub backup_id: String,
    pub created_at: String,
    pub status: String,
    pub size_bytes: i64,
    pub checksum: String,
    pub schema_version: String,
    pub uploaded_at: Option<String>,
    pub remote_path: Option<String>,
    #[serde(default = "default_backup_source")]
    pub source: String,
}

fn default_backup_source() -> String {
    "local".into()
}

/// Newer build published in the `updates` Storage bucket.
#[derive(Debug, Clone, Serialize)]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub version: String,
    pub notes: Option<String>,
}

/// Streamed to the UI while an update downloads and installs.
#[derive(Debug, Clone, Serialize)]
pub struct AppUpdateProgress {
    /// `downloading` | `installing`
    pub stage: &'static str,
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Stock kept at reception for one product. Products without a row are not tracked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductStock {
    pub product_id: i64,
    pub quantity: i64,
    pub min_quantity: i64,
    pub updated_at: String,
}

/// One line of the stock trail: sale, void, restock or physical count.
#[derive(Debug, Clone, Serialize)]
pub struct StockMovement {
    pub id: i64,
    pub product_id: i64,
    pub delta: i64,
    pub quantity_after: i64,
    pub reason: String,
    pub username: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateStockPayload {
    pub product_id: i64,
    /// `add` (units received), `set` (physical count) or `untrack` (stop counting).
    pub mode: String,
    #[serde(default)]
    pub quantity: i64,
    #[serde(default)]
    pub min_quantity: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Promotion or special price for a rate plan, chosen by the check-in weekday (or date) and hour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceRule {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub rate_plan_id: i64,
    /// ISO weekdays: 1 = lunes ... 7 = domingo. Empty when the rule uses `dates`.
    #[serde(default)]
    pub days: Vec<u8>,
    /// `YYYY-MM-DD`. On those dates a date rule wins over weekday rules.
    #[serde(default)]
    pub dates: Vec<String>,
    /// Check-in hour window [from_hour, to_hour), 0..=24.
    pub from_hour: u8,
    pub to_hour: u8,
    pub base_amount_cents: i64,
    /// `None` keeps the plan's additional 30 min price.
    #[serde(default)]
    pub extra_hour_cents: Option<i64>,
    #[serde(default = "default_true")]
    pub active: bool,
}

/// Price a rate plan would get if the check-in happened now.
#[derive(Debug, Clone, Serialize)]
pub struct EffectivePrice {
    pub rate_plan_id: i64,
    pub base_amount_cents: i64,
    pub extra_hour_cents: i64,
    pub rule_name: Option<String>,
}
