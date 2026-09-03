use crate::error::{AppError, AppResult};
use crate::models::{
    AppSettings, Charge, Guest, Payment, Product, RateKind, RatePlan, Reservation, Room, Stay,
};
use chrono::{DateTime, Local};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::Path;

const MIGRATION_001: &str = include_str!("../migrations/001_init.sql");
const MIGRATION_002: &str = include_str!("../migrations/002_products.sql");

pub fn open(db_path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    migrate(&conn)?;
    seed_if_empty(&conn)?;
    seed_products_if_empty(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(MIGRATION_001)?;
    apply_migration(conn, "001_init")?;
    conn.execute_batch(MIGRATION_002)?;
    apply_migration(conn, "002_products")?;
    Ok(())
}

fn apply_migration(conn: &Connection, id: &str) -> AppResult<()> {
    let applied: Option<String> = conn
        .query_row(
            "SELECT id FROM schema_migrations WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    if applied.is_none() {
        conn.execute(
            "INSERT INTO schema_migrations (id, applied_at) VALUES (?1, ?2)",
            params![id, now_rfc3339()],
        )?;
    }
    Ok(())
}

fn seed_if_empty(conn: &Connection) -> AppResult<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM rooms", [], |row| row.get(0))?;
    if count > 0 {
        return Ok(());
    }

    let now = now_rfc3339();
    for n in 1..=27 {
        let number = format!("{n:02}");
        let floor = ((n - 1) / 9) + 1;
        let room_type = if n % 9 == 0 { "Suite" } else { "Estándar" };
        conn.execute(
            "INSERT INTO rooms (number, room_type, floor, status, created_at) VALUES (?1, ?2, ?3, 'available', ?4)",
            params![number, room_type, floor, now],
        )?;
    }

    conn.execute(
        "INSERT INTO rate_plans (name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
        params!["3 horas", "hourly", 80_000i64, 20_000i64, 3i64, 10i64, 12i64],
    )?;
    conn.execute(
        "INSERT INTO rate_plans (name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
        params!["Noche", "night", 150_000i64, 25_000i64, 24i64, 15i64, 12i64],
    )?;
    conn.execute(
        "INSERT INTO rate_plans (name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
        params!["Pernocte", "overnight", 120_000i64, 20_000i64, 12i64, 15i64, 12i64],
    )?;

    let defaults = AppSettings::default();
    upsert_setting(conn, "business_name", &defaults.business_name)?;
    upsert_setting(conn, "address", &defaults.address)?;
    upsert_setting(conn, "phone", "")?;
    upsert_setting(conn, "tax_percent", "10")?;
    upsert_setting(conn, "currency_symbol", "Gs.")?;
    upsert_setting(conn, "theme", "dark")?;
    upsert_setting(conn, "receipt_footer", &defaults.receipt_footer)?;
    upsert_setting(conn, "printer_enabled", "false")?;
    upsert_setting(conn, "printer_path", "")?;
    upsert_setting(conn, "printer_name", "")?;
    upsert_setting(conn, "paper_width", "80")?;
    upsert_setting(conn, "auto_print_on_checkout", "true")?;
    upsert_setting(conn, "require_guest_name", "false")?;
    upsert_setting(conn, "pin_hash", "")?;
    Ok(())
}

fn seed_products_if_empty(conn: &Connection) -> AppResult<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM products", [], |row| row.get(0))?;
    if count > 0 {
        return Ok(());
    }

    let products: &[(&str, &str, i64)] = &[
        ("Agua", "bebidas", 5_000),
        ("Coca", "bebidas", 8_000),
        ("Pulp", "bebidas", 8_000),
        ("Fanta", "bebidas", 8_000),
        ("Tónica", "bebidas", 7_000),
        ("Del valle", "bebidas", 10_000),
        ("Energy", "bebidas", 12_000),
        ("Power", "bebidas", 12_000),
        ("Bud 66", "bebidas", 12_000),
        ("Skol", "bebidas", 12_000),
        ("Smirnoff", "bebidas", 18_000),
        ("Beldent", "snacks", 5_000),
        ("Halls", "snacks", 5_000),
        ("Papa", "snacks", 10_000),
        ("Gullón", "snacks", 8_000),
        ("Turrón", "snacks", 8_000),
        ("Bonbon", "snacks", 7_000),
        ("Chocolate", "snacks", 10_000),
        ("Kent Conv.", "tabaco", 18_000),
        ("Lucky", "tabaco", 18_000),
        ("Encendedor", "tabaco", 8_000),
        ("Crema D.", "higiene", 10_000),
        ("Cepillo D.", "higiene", 8_000),
        ("Baño E.", "higiene", 12_000),
        ("Gel Pant.", "higiene", 15_000),
        ("Prestobarba", "higiene", 12_000),
        ("Prime", "adulto", 15_000),
        ("Control", "adulto", 15_000),
        ("Lubricante", "adulto", 25_000),
        ("Prot. 100", "adulto", 20_000),
        ("Prot. 150", "adulto", 30_000),
        ("Prot. 200", "adulto", 40_000),
        ("Capa P.", "adulto", 25_000),
        ("Agrandador", "adulto", 35_000),
        ("Anillo v.", "adulto", 45_000),
        ("Estimulador", "adulto", 50_000),
        ("Fantasía", "adulto", 60_000),
        ("Quinta", "licores", 45_000),
        ("Sta. Helena", "licores", 50_000),
        ("Monje", "licores", 55_000),
        ("Johnnie W.", "licores", 180_000),
    ];

    for (i, (name, category, price)) in products.iter().enumerate() {
        conn.execute(
            "INSERT INTO products (name, category, price_cents, active, sort_order) VALUES (?1, ?2, ?3, 1, ?4)",
            params![name, category, price, (i as i64) + 1],
        )?;
    }
    Ok(())
}

pub fn list_products(conn: &Connection, active_only: bool) -> AppResult<Vec<Product>> {
    let sql = if active_only {
        "SELECT id, name, category, price_cents, active, sort_order FROM products WHERE active = 1 ORDER BY sort_order, name"
    } else {
        "SELECT id, name, category, price_cents, active, sort_order FROM products ORDER BY sort_order, name"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(Product {
            id: row.get(0)?,
            name: row.get(1)?,
            category: row.get(2)?,
            price_cents: row.get(3)?,
            active: row.get::<_, i64>(4)? != 0,
            sort_order: row.get(5)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_product(conn: &Connection, id: i64) -> AppResult<Product> {
    conn.query_row(
        "SELECT id, name, category, price_cents, active, sort_order FROM products WHERE id = ?1",
        [id],
        |row| {
            Ok(Product {
                id: row.get(0)?,
                name: row.get(1)?,
                category: row.get(2)?,
                price_cents: row.get(3)?,
                active: row.get::<_, i64>(4)? != 0,
                sort_order: row.get(5)?,
            })
        },
    )
    .map_err(|_| AppError::msg("Producto no encontrado"))
}

pub fn now_rfc3339() -> String {
    Local::now().to_rfc3339()
}

pub fn parse_dt(value: &str) -> AppResult<DateTime<Local>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Local))
        .map_err(|e| AppError::msg(format!("Fecha inválida: {e}")))
}

pub fn upsert_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str, default: &str) -> AppResult<String> {
    let value: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?;
    Ok(value.unwrap_or_else(|| default.to_string()))
}

pub fn load_settings(conn: &Connection) -> AppResult<AppSettings> {
    let mut settings = AppSettings::default();
    settings.business_name = get_setting(conn, "business_name", &settings.business_name)?;
    settings.address = get_setting(conn, "address", &settings.address)?;
    settings.phone = get_setting(conn, "phone", "")?;
    settings.tax_percent = get_setting(conn, "tax_percent", "10")?
        .parse()
        .unwrap_or(10.0);
    settings.currency_symbol = get_setting(conn, "currency_symbol", "Gs.")?;
    settings.theme = get_setting(conn, "theme", "dark")?;
    settings.receipt_footer = get_setting(conn, "receipt_footer", &settings.receipt_footer)?;
    settings.printer_enabled = get_setting(conn, "printer_enabled", "false")? == "true";
    settings.printer_path = get_setting(conn, "printer_path", "")?;
    settings.printer_name = get_setting(conn, "printer_name", "")?;
    settings.paper_width = get_setting(conn, "paper_width", "80")?
        .parse()
        .unwrap_or(80);
    settings.auto_print_on_checkout =
        get_setting(conn, "auto_print_on_checkout", "true")? == "true";
    settings.require_guest_name =
        get_setting(conn, "require_guest_name", "false")? == "true";
    settings.pin_hash = get_setting(conn, "pin_hash", "")?;
    settings.has_pin = !settings.pin_hash.is_empty();
    Ok(settings)
}

pub fn hash_pin(pin: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"nightdesk.pin.v1:");
    hasher.update(pin.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn get_room(conn: &Connection, id: i64) -> AppResult<Room> {
    conn.query_row(
        "SELECT id, number, room_type, floor, status, notes FROM rooms WHERE id = ?1",
        [id],
        |row| {
            Ok(Room {
                id: row.get(0)?,
                number: row.get(1)?,
                room_type: row.get(2)?,
                floor: row.get(3)?,
                status: row.get(4)?,
                notes: row.get(5)?,
            })
        },
    )
    .map_err(|_| AppError::msg("Habitación no encontrada"))
}

pub fn get_rate_plan(conn: &Connection, id: i64) -> AppResult<RatePlan> {
    conn.query_row(
        "SELECT id, name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active
         FROM rate_plans WHERE id = ?1",
        [id],
        map_rate_plan,
    )
    .map_err(|_| AppError::msg("Tarifa no encontrada"))
}

fn map_rate_plan(row: &rusqlite::Row<'_>) -> rusqlite::Result<RatePlan> {
    let kind: String = row.get(2)?;
    let active: i64 = row.get(8)?;
    Ok(RatePlan {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: RateKind::parse(&kind),
        base_amount_cents: row.get(3)?,
        extra_hour_cents: row.get(4)?,
        included_hours: row.get(5)?,
        grace_minutes: row.get(6)?,
        night_cutoff_hour: row.get(7)?,
        active: active != 0,
    })
}

pub fn list_rate_plans(conn: &Connection, active_only: bool) -> AppResult<Vec<RatePlan>> {
    let sql = if active_only {
        "SELECT id, name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active
         FROM rate_plans WHERE active = 1 ORDER BY kind, name"
    } else {
        "SELECT id, name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active
         FROM rate_plans ORDER BY kind, name"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], map_rate_plan)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn find_plan_by_kind(conn: &Connection, kind: RateKind) -> AppResult<Option<RatePlan>> {
    conn.query_row(
        "SELECT id, name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active
         FROM rate_plans WHERE kind = ?1 AND active = 1 ORDER BY id LIMIT 1",
        [kind.as_str()],
        map_rate_plan,
    )
    .optional()
    .map_err(Into::into)
}

pub fn insert_guest(
    conn: &Connection,
    name: &str,
    document: Option<&str>,
    phone: Option<&str>,
) -> AppResult<Guest> {
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO guests (name, document, phone, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![name.trim(), document, phone, now],
    )?;
    Ok(Guest {
        id: conn.last_insert_rowid(),
        name: name.trim().to_string(),
        document: document.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        phone: phone.map(|s| s.to_string()).filter(|s| !s.is_empty()),
    })
}

pub fn get_stay(conn: &Connection, id: i64) -> AppResult<Stay> {
    conn.query_row(
        "SELECT s.id, s.room_id, r.number, s.guest_id, g.name, g.document, g.phone,
                s.rate_plan_id, rp.name, rp.kind, s.reservation_id, s.check_in_at,
                s.expected_checkout_at, s.check_out_at, s.status, s.converted_to_overnight,
                s.overnight_rate_plan_id, s.notes
         FROM stays s
         JOIN rooms r ON r.id = s.room_id
         JOIN guests g ON g.id = s.guest_id
         JOIN rate_plans rp ON rp.id = s.rate_plan_id
         WHERE s.id = ?1",
        [id],
        map_stay,
    )
    .map_err(|_| AppError::msg("Estadía no encontrada"))
}

fn map_stay(row: &rusqlite::Row<'_>) -> rusqlite::Result<Stay> {
    let kind: String = row.get(9)?;
    let converted: i64 = row.get(15)?;
    Ok(Stay {
        id: row.get(0)?,
        room_id: row.get(1)?,
        room_number: row.get(2)?,
        guest_id: row.get(3)?,
        guest_name: row.get(4)?,
        guest_document: row.get(5)?,
        guest_phone: row.get(6)?,
        rate_plan_id: row.get(7)?,
        rate_plan_name: row.get(8)?,
        rate_kind: RateKind::parse(&kind),
        reservation_id: row.get(10)?,
        check_in_at: row.get(11)?,
        expected_checkout_at: row.get(12)?,
        check_out_at: row.get(13)?,
        status: row.get(14)?,
        converted_to_overnight: converted != 0,
        overnight_rate_plan_id: row.get(16)?,
        notes: row.get(17)?,
    })
}

pub fn open_stay_for_room(conn: &Connection, room_id: i64) -> AppResult<Option<Stay>> {
    conn.query_row(
        "SELECT s.id, s.room_id, r.number, s.guest_id, g.name, g.document, g.phone,
                s.rate_plan_id, rp.name, rp.kind, s.reservation_id, s.check_in_at,
                s.expected_checkout_at, s.check_out_at, s.status, s.converted_to_overnight,
                s.overnight_rate_plan_id, s.notes
         FROM stays s
         JOIN rooms r ON r.id = s.room_id
         JOIN guests g ON g.id = s.guest_id
         JOIN rate_plans rp ON rp.id = s.rate_plan_id
         WHERE s.room_id = ?1 AND s.status = 'open'
         ORDER BY s.id DESC LIMIT 1",
        [room_id],
        map_stay,
    )
    .optional()
    .map_err(Into::into)
}

pub fn list_charges(conn: &Connection, stay_id: i64) -> AppResult<Vec<Charge>> {
    let mut stmt = conn.prepare(
        "SELECT id, stay_id, kind, description, amount_cents, created_at FROM charges WHERE stay_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map([stay_id], |row| {
        Ok(Charge {
            id: row.get(0)?,
            stay_id: row.get(1)?,
            kind: row.get(2)?,
            description: row.get(3)?,
            amount_cents: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_payments(conn: &Connection, stay_id: i64) -> AppResult<Vec<Payment>> {
    let mut stmt = conn.prepare(
        "SELECT id, stay_id, method, amount_cents, created_at FROM payments WHERE stay_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map([stay_id], |row| {
        Ok(Payment {
            id: row.get(0)?,
            stay_id: row.get(1)?,
            method: row.get(2)?,
            amount_cents: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn today_hold_for_room(conn: &Connection, room_id: i64) -> AppResult<Option<Reservation>> {
    conn.query_row(
        "SELECT res.id, res.guest_id, g.name, g.document, g.phone, res.room_id, r.number,
                res.rate_plan_id, rp.name, res.expected_arrival_at, res.expected_nights, res.status, res.notes
         FROM reservations res
         JOIN guests g ON g.id = res.guest_id
         JOIN rooms r ON r.id = res.room_id
         JOIN rate_plans rp ON rp.id = res.rate_plan_id
         WHERE res.room_id = ?1 AND res.status = 'hold'
           AND substr(res.expected_arrival_at, 1, 10) = substr(?2, 1, 10)
         ORDER BY res.id LIMIT 1",
        params![room_id, now_rfc3339()],
        map_reservation,
    )
    .optional()
    .map_err(Into::into)
}

pub fn get_reservation(conn: &Connection, id: i64) -> AppResult<Reservation> {
    conn.query_row(
        "SELECT res.id, res.guest_id, g.name, g.document, g.phone, res.room_id, r.number,
                res.rate_plan_id, rp.name, res.expected_arrival_at, res.expected_nights, res.status, res.notes
         FROM reservations res
         JOIN guests g ON g.id = res.guest_id
         JOIN rooms r ON r.id = res.room_id
         JOIN rate_plans rp ON rp.id = res.rate_plan_id
         WHERE res.id = ?1",
        [id],
        map_reservation,
    )
    .map_err(|_| AppError::msg("Reserva no encontrada"))
}

pub fn map_reservation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Reservation> {
    Ok(Reservation {
        id: row.get(0)?,
        guest_id: row.get(1)?,
        guest_name: row.get(2)?,
        guest_document: row.get(3)?,
        guest_phone: row.get(4)?,
        room_id: row.get(5)?,
        room_number: row.get(6)?,
        rate_plan_id: row.get(7)?,
        rate_plan_name: row.get(8)?,
        expected_arrival_at: row.get(9)?,
        expected_nights: row.get(10)?,
        status: row.get(11)?,
        notes: row.get(12)?,
    })
}

pub fn set_room_status(conn: &Connection, room_id: i64, status: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE rooms SET status = ?1 WHERE id = ?2",
        params![status, room_id],
    )?;
    Ok(())
}
