use crate::error::{AppError, AppResult};
use crate::models::{
    AppSettings, Charge, Guest, Payment, Product, RateKind, RatePlan, Reservation, Room, Stay,
};
use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Connection, DatabaseName, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MIGRATION_001: &str = include_str!("../migrations/001_init.sql");
const MIGRATION_002: &str = include_str!("../migrations/002_products.sql");
const MIGRATION_003: &str = include_str!("../migrations/003_rooms_scope.sql");
const MIGRATION_004: &str = include_str!("../migrations/004_account_closure.sql");
const MIGRATION_005: &str = include_str!("../migrations/005_auth.sql");
const MIGRATION_006: &str = include_str!("../migrations/006_stay_integrity.sql");

struct Migration {
    id: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        id: "001_init",
        sql: MIGRATION_001,
    },
    Migration {
        id: "002_products",
        sql: MIGRATION_002,
    },
    Migration {
        id: "003_rooms_scope",
        sql: MIGRATION_003,
    },
    Migration {
        id: "004_account_closure",
        sql: MIGRATION_004,
    },
    Migration {
        id: "005_auth",
        sql: MIGRATION_005,
    },
    Migration {
        id: "006_stay_integrity",
        sql: MIGRATION_006,
    },
    Migration {
        id: "007_receipts",
        sql: include_str!("../migrations/007_receipts.sql"),
    },
    Migration {
        id: "008_ticket_header",
        sql: include_str!("../migrations/008_ticket_header.sql"),
    },
    Migration {
        id: "009_ticket_header_name",
        sql: include_str!("../migrations/009_ticket_header_name.sql"),
    },
    Migration {
        id: "010_jacuzzi_rooms",
        sql: include_str!("../migrations/010_jacuzzi_rooms.sql"),
    },
    Migration {
        id: "011_jacuzzi_rooms_1_to_4",
        sql: include_str!("../migrations/011_jacuzzi_rooms_1_to_4.sql"),
    },
    Migration {
        id: "012_love_nest_rates",
        sql: include_str!("../migrations/012_love_nest_rates.sql"),
    },
    Migration {
        id: "013_no_iva",
        sql: include_str!("../migrations/013_no_iva.sql"),
    },
    Migration {
        id: "014_sync",
        sql: include_str!("../migrations/014_sync.sql"),
    },
    Migration {
        id: "015_catalog_audit",
        sql: include_str!("../migrations/015_catalog_audit.sql"),
    },
    Migration {
        id: "016_backup",
        sql: include_str!("../migrations/016_backup.sql"),
    },
];

pub fn open(db_path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    let backup = (!is_memory_path(db_path)).then_some(db_path);
    migrate(&mut conn, backup)?;
    seed_if_empty(&conn)?;
    seed_products_if_empty(&conn)?;
    Ok(conn)
}

fn is_memory_path(db_path: &Path) -> bool {
    db_path.as_os_str() == ":memory:"
}

fn migrate(conn: &mut Connection, db_path: Option<&Path>) -> AppResult<()> {
    run_pending(conn, db_path, MIGRATIONS)
}

fn run_pending(
    conn: &mut Connection,
    db_path: Option<&Path>,
    catalog: &[Migration],
) -> AppResult<()> {
    ensure_migrations_table(conn)?;
    let mut pending = Vec::new();
    for migration in catalog {
        if !migration_applied(conn, migration.id)? {
            pending.push(migration);
        }
    }
    if pending.is_empty() {
        return Ok(());
    }

    let backup_file = if let Some(path) = db_path.filter(|path| !is_memory_path(path)) {
        let bak = pre_migrate_backup_path(path, pending[0].id);
        conn.backup(DatabaseName::Main, &bak, None)?;
        Some(bak)
    } else {
        None
    };

    for migration in pending {
        if let Err(error) = apply_pending(conn, migration) {
            if let Some(bak) = &backup_file {
                let _ = conn.restore(DatabaseName::Main, bak, None::<fn(_)>);
                let _ = conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;");
            }
            return Err(error);
        }
    }
    Ok(())
}

fn pre_migrate_backup_path(db_path: &Path, migration_id: &str) -> PathBuf {
    let stem = db_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    db_path.with_file_name(format!("{stem}.pre-migrate-{migration_id}.bak"))
}

fn ensure_migrations_table(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            id TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;
    Ok(())
}

fn apply_pending(conn: &mut Connection, migration: &Migration) -> AppResult<()> {
    if migration.id == "006_stay_integrity" {
        reject_duplicate_open_stays(conn)?;
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(migration.sql)?;
    apply_migration(&tx, migration.id)?;
    tx.commit()?;
    Ok(())
}

fn reject_duplicate_open_stays(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT r.number, COUNT(*)
         FROM stays s
         JOIN rooms r ON r.id = s.room_id
         WHERE s.status = 'open'
         GROUP BY s.room_id
         HAVING COUNT(*) > 1
         ORDER BY r.number",
    )?;
    let rooms: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .filter_map(|row| row.ok())
        .collect();
    if rooms.is_empty() {
        return Ok(());
    }
    Err(AppError::msg(format!(
        "Hay más de una estadía abierta en: {}. Corregir antes de migrar.",
        rooms.join(", ")
    )))
}

fn migration_applied(conn: &Connection, id: &str) -> AppResult<bool> {
    let applied: Option<String> = conn
        .query_row(
            "SELECT id FROM schema_migrations WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(applied.is_some())
}

fn apply_migration(conn: &Connection, id: &str) -> AppResult<()> {
    if !migration_applied(conn, id)? {
        conn.execute(
            "INSERT INTO schema_migrations (id, applied_at) VALUES (?1, ?2)",
            params![id, now_rfc3339()],
        )?;
    }
    Ok(())
}

pub fn list_applied_migrations(conn: &Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM schema_migrations ORDER BY id")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.filter_map(|row| row.ok()).collect())
}

fn seed_if_empty(conn: &Connection) -> AppResult<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM rooms", [], |row| row.get(0))?;
    if count > 0 {
        return Ok(());
    }

    let now = now_rfc3339();
    for n in 1..=23 {
        let number = format!("{n:02}");
        let floor = ((n - 1) / 9) + 1;
        let room_type = if n <= 4 { "Jacuzzi" } else { "Normal" };
        conn.execute(
            "INSERT INTO rooms (number, room_type, floor, status, created_at) VALUES (?1, ?2, ?3, 'available', ?4)",
            params![number, room_type, floor, now],
        )?;
    }

    conn.execute(
        "INSERT INTO rate_plans (name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
        params!["1 hora", "hourly", 45_000i64, 15_000i64, 1i64, 5i64, 10i64],
    )?;
    conn.execute(
        "INSERT INTO rate_plans (name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
        params!["Dormida", "overnight", 120_000i64, 15_000i64, 12i64, 5i64, 10i64],
    )?;

    let defaults = AppSettings::default();
    upsert_setting(conn, "business_name", &defaults.business_name)?;
    upsert_setting(conn, "address", &defaults.address)?;
    upsert_setting(conn, "phone", "")?;
    upsert_setting(conn, "tax_percent", "0")?;
    upsert_setting(conn, "currency_symbol", "Gs.")?;
    upsert_setting(conn, "theme", "light")?;
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

fn map_product(row: &rusqlite::Row<'_>) -> rusqlite::Result<Product> {
    Ok(Product {
        id: row.get(0)?,
        name: row.get(1)?,
        category: row.get(2)?,
        price_cents: row.get(3)?,
        active: row.get::<_, i64>(4)? != 0,
        sort_order: row.get(5)?,
        version: row.get(6)?,
    })
}

pub fn list_products(conn: &Connection, active_only: bool) -> AppResult<Vec<Product>> {
    let sql = if active_only {
        "SELECT id, name, category, price_cents, active, sort_order, version FROM products WHERE active = 1 ORDER BY sort_order, name"
    } else {
        "SELECT id, name, category, price_cents, active, sort_order, version FROM products ORDER BY sort_order, name"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], map_product)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get_product(conn: &Connection, id: i64) -> AppResult<Product> {
    conn.query_row(
        "SELECT id, name, category, price_cents, active, sort_order, version FROM products WHERE id = ?1",
        [id],
        map_product,
    )
    .map_err(|_| AppError::msg("Producto no encontrado"))
}

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub fn parse_dt(value: &str) -> AppResult<DateTime<Local>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Local))
        .map_err(|e| AppError::msg(format!("Fecha inválida: {e}")))
}

pub fn local_today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub fn local_calendar_date(value: &str) -> String {
    parse_dt(value)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| value.chars().take(10).collect())
}

pub struct ClosedSnapshot {
    pub applied_kind: Option<String>,
    pub tax_percent: Option<f64>,
    pub duration_label: Option<String>,
}

pub fn closed_snapshot(conn: &Connection, stay_id: i64) -> AppResult<ClosedSnapshot> {
    conn.query_row(
        "SELECT closed_applied_kind, closed_tax_percent, closed_duration_label FROM stays WHERE id = ?1",
        [stay_id],
        |row| {
            Ok(ClosedSnapshot {
                applied_kind: row.get(0)?,
                tax_percent: row.get(1)?,
                duration_label: row.get(2)?,
            })
        },
    )
    .map_err(|_| AppError::msg("Estadía no encontrada"))
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
    settings.tax_percent = get_setting(conn, "tax_percent", "0")?
        .parse()
        .unwrap_or(10.0);
    settings.currency_symbol = get_setting(conn, "currency_symbol", "Gs.")?;
    settings.theme = get_setting(conn, "theme", "light")?;
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
    let mut stmt=conn.prepare("SELECT key,version FROM catalog_setting_versions")?;
    settings.catalog_versions=stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))?.collect::<Result<_,_>>()?;
    Ok(settings)
}

pub fn hash_pin(pin: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"nightdesk.pin.v1:");
    hasher.update(pin.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn map_room(row: &rusqlite::Row<'_>) -> rusqlite::Result<Room> {
    Ok(Room {
        id: row.get(0)?,
        number: row.get(1)?,
        room_type: row.get(2)?,
        floor: row.get(3)?,
        status: row.get(4)?,
        notes: row.get(5)?,
        active: row.get::<_, i64>(6)? != 0,
        version: row.get(7)?,
    })
}

pub fn get_room(conn: &Connection, id: i64) -> AppResult<Room> {
    conn.query_row(
        "SELECT id, number, room_type, floor, status, notes, active, version FROM rooms WHERE id = ?1",
        [id],
        map_room,
    )
    .map_err(|_| AppError::msg("Habitación no encontrada"))
}

pub fn get_rate_plan(conn: &Connection, id: i64) -> AppResult<RatePlan> {
    conn.query_row(
        "SELECT id, name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active, version
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
        version: row.get(9)?,
    })
}

pub fn list_rate_plans(conn: &Connection, active_only: bool) -> AppResult<Vec<RatePlan>> {
    let sql = if active_only {
        "SELECT id, name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active, version
         FROM rate_plans WHERE active = 1 ORDER BY kind, name"
    } else {
        "SELECT id, name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active, version
         FROM rate_plans ORDER BY kind, name"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], map_rate_plan)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn find_plan_by_kind(conn: &Connection, kind: RateKind) -> AppResult<Option<RatePlan>> {
    conn.query_row(
        "SELECT id, name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active, version
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
        "SELECT id, stay_id, kind, description, amount_cents, created_at FROM charges WHERE stay_id = ?1 AND deleted_at IS NULL ORDER BY id",
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
           AND substr(res.expected_arrival_at, 1, 10) = ?2
         ORDER BY res.id LIMIT 1",
        params![room_id, local_today()],
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

const UID_TABLES: &[&str] = &[
    "rooms",
    "rate_plans",
    "products",
    "guests",
    "reservations",
    "stays",
    "charges",
    "payments",
    "users",
];

pub fn uid_of(conn: &Connection, table: &str, id: i64) -> AppResult<String> {
    if !UID_TABLES.contains(&table) {
        return Err(AppError::msg("Tabla inválida"));
    }
    match conn.query_row(
        &format!("SELECT uid FROM {table} WHERE id = ?1"),
        [id],
        |row| row.get::<_, Option<String>>(0),
    ) {
        Ok(Some(uid)) if !uid.is_empty() => Ok(uid),
        Ok(_) => Err(AppError::not_found("Fila sin uid")),
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(AppError::not_found("Fila sin uid")),
        Err(error) => Err(error.into()),
    }
}

pub fn payload_for_guest(conn: &Connection, id: i64) -> AppResult<serde_json::Value> {
    conn.query_row(
        "SELECT id, uid, name, document, phone FROM guests WHERE id = ?1",
        [id],
        |row| {
            Ok(serde_json::json!({
                "local_id": row.get::<_, i64>(0)?,
                "uid": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "document": row.get::<_, Option<String>>(3)?,
                "phone": row.get::<_, Option<String>>(4)?,
            }))
        },
    )
    .map_err(|_| AppError::not_found("Huésped no encontrado"))
}

pub fn payload_for_room(conn: &Connection, id: i64) -> AppResult<serde_json::Value> {
    conn.query_row(
        "SELECT id, uid, number, room_type, floor, status, notes, active FROM rooms WHERE id = ?1",
        [id],
        |row| {
            Ok(serde_json::json!({
                "local_id": row.get::<_, i64>(0)?,
                "uid": row.get::<_, String>(1)?,
                "number": row.get::<_, String>(2)?,
                "room_type": row.get::<_, String>(3)?,
                "floor": row.get::<_, i64>(4)?,
                "status": row.get::<_, String>(5)?,
                "notes": row.get::<_, Option<String>>(6)?,
                "active": row.get::<_, i64>(7)? != 0,
            }))
        },
    )
    .map_err(|_| AppError::not_found("Habitación no encontrada"))
}

pub fn payload_for_reservation(conn: &Connection, id: i64) -> AppResult<serde_json::Value> {
    conn.query_row(
        "SELECT res.id, res.uid, g.uid, r.uid, rp.uid, res.expected_arrival_at, res.expected_nights, res.status, res.notes
         FROM reservations res
         JOIN guests g ON g.id = res.guest_id
         JOIN rooms r ON r.id = res.room_id
         JOIN rate_plans rp ON rp.id = res.rate_plan_id
         WHERE res.id = ?1",
        [id],
        |row| {
            Ok(serde_json::json!({
                "local_id": row.get::<_, i64>(0)?,
                "uid": row.get::<_, String>(1)?,
                "guest_uid": row.get::<_, String>(2)?,
                "room_uid": row.get::<_, String>(3)?,
                "rate_plan_uid": row.get::<_, String>(4)?,
                "expected_arrival_at": row.get::<_, String>(5)?,
                "expected_nights": row.get::<_, i64>(6)?,
                "status": row.get::<_, String>(7)?,
                "notes": row.get::<_, Option<String>>(8)?,
            }))
        },
    )
    .map_err(|_| AppError::not_found("Reserva no encontrada"))
}

pub fn payload_for_stay(conn: &Connection, id: i64) -> AppResult<serde_json::Value> {
    conn.query_row(
        "SELECT s.id, s.uid, r.uid, g.uid, rp.uid, res.uid, s.check_in_at, s.expected_checkout_at,
                s.check_out_at, s.status, s.converted_to_overnight, orp.uid, s.notes,
                s.closed_applied_kind, s.closed_tax_percent, s.closed_duration_label
         FROM stays s
         JOIN rooms r ON r.id = s.room_id
         JOIN guests g ON g.id = s.guest_id
         JOIN rate_plans rp ON rp.id = s.rate_plan_id
         LEFT JOIN reservations res ON res.id = s.reservation_id
         LEFT JOIN rate_plans orp ON orp.id = s.overnight_rate_plan_id
         WHERE s.id = ?1",
        [id],
        |row| {
            Ok(serde_json::json!({
                "local_id": row.get::<_, i64>(0)?,
                "uid": row.get::<_, String>(1)?,
                "room_uid": row.get::<_, String>(2)?,
                "guest_uid": row.get::<_, String>(3)?,
                "rate_plan_uid": row.get::<_, String>(4)?,
                "reservation_uid": row.get::<_, Option<String>>(5)?,
                "check_in_at": row.get::<_, String>(6)?,
                "expected_checkout_at": row.get::<_, Option<String>>(7)?,
                "check_out_at": row.get::<_, Option<String>>(8)?,
                "status": row.get::<_, String>(9)?,
                "converted_to_overnight": row.get::<_, i64>(10)? != 0,
                "overnight_rate_plan_uid": row.get::<_, Option<String>>(11)?,
                "notes": row.get::<_, Option<String>>(12)?,
                "closed_applied_kind": row.get::<_, Option<String>>(13)?,
                "closed_tax_percent": row.get::<_, Option<f64>>(14)?,
                "closed_duration_label": row.get::<_, Option<String>>(15)?,
            }))
        },
    )
    .map_err(|_| AppError::not_found("Estadía no encontrada"))
}

pub fn payload_for_charge(conn: &Connection, id: i64) -> AppResult<serde_json::Value> {
    conn.query_row(
        "SELECT c.id, c.uid, s.uid, c.kind, c.description, c.amount_cents, c.deleted_at
         FROM charges c
         JOIN stays s ON s.id = c.stay_id
         WHERE c.id = ?1",
        [id],
        |row| {
            Ok(serde_json::json!({
                "local_id": row.get::<_, i64>(0)?,
                "uid": row.get::<_, String>(1)?,
                "stay_uid": row.get::<_, String>(2)?,
                "kind": row.get::<_, String>(3)?,
                "description": row.get::<_, String>(4)?,
                "amount_cents": row.get::<_, i64>(5)?,
                "deleted_at": row.get::<_, Option<String>>(6)?,
            }))
        },
    )
    .map_err(|_| AppError::not_found("Cargo no encontrado"))
}

#[allow(dead_code)]
pub fn payload_for_payment(conn: &Connection, id: i64) -> AppResult<serde_json::Value> {
    conn.query_row(
        "SELECT p.id, p.uid, s.uid, p.method, p.amount_cents
         FROM payments p
         JOIN stays s ON s.id = p.stay_id
         WHERE p.id = ?1",
        [id],
        |row| {
            Ok(serde_json::json!({
                "local_id": row.get::<_, i64>(0)?,
                "uid": row.get::<_, String>(1)?,
                "stay_uid": row.get::<_, String>(2)?,
                "method": row.get::<_, String>(3)?,
                "amount_cents": row.get::<_, i64>(4)?,
            }))
        },
    )
    .map_err(|_| AppError::not_found("Pago no encontrado"))
}

#[cfg(test)]
mod room_scope_tests {
    use super::*;

    #[test]
    fn fresh_database_seeds_confirmed_room_scope() -> AppResult<()> {
        let mut conn = Connection::open_in_memory()?;
        migrate(&mut conn, None)?;
        seed_if_empty(&conn)?;
        migrate(&mut conn, None)?;

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM rooms", [], |row| row.get(0))?;
        let active: i64 = conn.query_row("SELECT COUNT(*) FROM rooms WHERE active = 1", [], |row| row.get(0))?;
        let normal: i64 = conn.query_row(
            "SELECT COUNT(*) FROM rooms WHERE active = 1 AND room_type = 'Normal'",
            [],
            |row| row.get(0),
        )?;
        let jacuzzi: i64 = conn.query_row(
            "SELECT COUNT(*) FROM rooms WHERE active = 1 AND room_type = 'Jacuzzi'",
            [],
            |row| row.get(0),
        )?;

        assert_eq!(total, 23);
        assert_eq!(active, 23);
        assert_eq!(normal, 19);
        assert_eq!(jacuzzi, 4);

        let jacuzzi_numbers: Vec<String> = conn
            .prepare("SELECT number FROM rooms WHERE room_type = 'Jacuzzi' ORDER BY CAST(number AS INTEGER)")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(jacuzzi_numbers, ["01", "02", "03", "04"]);
        Ok(())
    }

    #[test]
    fn jacuzzi_migration_reclassifies_legacy_suite_labels() -> AppResult<()> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(MIGRATION_001)?;
        apply_migration(&conn, "001_init")?;
        for n in 1..=23 {
            let room_type = if n == 9 || n == 18 { "Suite" } else { "Normal" };
            conn.execute(
                "INSERT INTO rooms (number, room_type, floor, status, created_at) VALUES (?1, ?2, 1, 'available', ?3)",
                params![format!("{n:02}"), room_type, now_rfc3339()],
            )?;
        }
        conn.execute_batch(include_str!("../migrations/010_jacuzzi_rooms.sql"))?;
        conn.execute_batch(include_str!("../migrations/011_jacuzzi_rooms_1_to_4.sql"))?;
        let jacuzzi_numbers: Vec<String> = conn
            .prepare("SELECT number FROM rooms WHERE room_type = 'Jacuzzi' ORDER BY CAST(number AS INTEGER)")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let suite: i64 = conn.query_row(
            "SELECT COUNT(*) FROM rooms WHERE room_type = 'Suite'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(jacuzzi_numbers, ["01", "02", "03", "04"]);
        assert_eq!(suite, 0);
        Ok(())
    }

    #[test]
    fn legacy_scope_retires_only_safe_rooms_and_keeps_history() -> AppResult<()> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(MIGRATION_001)?;
        apply_migration(&conn, "001_init")?;
        conn.execute_batch(MIGRATION_002)?;
        apply_migration(&conn, "002_products")?;

        for number in 1..=27 {
            conn.execute(
                "INSERT INTO rooms (number, room_type, floor, status, created_at) VALUES (?1, 'Normal', 1, 'available', ?2)",
                params![format!("{number:02}"), now_rfc3339()],
            )?;
        }
        conn.execute(
            "INSERT INTO rate_plans (name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active)
             VALUES ('3 horas', 'hourly', 80000, 20000, 3, 10, 12, 1)",
            [],
        )?;
        conn.execute(
            "INSERT INTO guests (name, created_at) VALUES ('Huésped histórico', ?1)",
            [now_rfc3339()],
        )?;
        conn.execute(
            "INSERT INTO stays (room_id, guest_id, rate_plan_id, check_in_at, status)
             VALUES (27, 1, 1, ?1, 'open')",
            [now_rfc3339()],
        )?;

        conn.execute_batch(MIGRATION_003)?;
        apply_migration(&conn, "003_rooms_scope")?;

        let active: i64 = conn.query_row("SELECT COUNT(*) FROM rooms WHERE active = 1", [], |row| row.get(0))?;
        let room_27_active: i64 = conn.query_row("SELECT active FROM rooms WHERE number = '27'", [], |row| row.get(0))?;
        let room_26_active: i64 = conn.query_row("SELECT active FROM rooms WHERE number = '26'", [], |row| row.get(0))?;
        let history_count: i64 = conn.query_row("SELECT COUNT(*) FROM stays WHERE room_id = 27", [], |row| row.get(0))?;

        assert_eq!(active, 23);
        assert_eq!(room_27_active, 1);
        assert_eq!(room_26_active, 0);
        assert_eq!(history_count, 1);
        Ok(())
    }
}

#[cfg(test)]
mod product_catalog_tests {
    use super::*;

    fn catalog_db() -> AppResult<Connection> {
        let mut conn = Connection::open_in_memory()?;
        migrate(&mut conn, None)?;
        seed_if_empty(&conn)?;
        seed_products_if_empty(&conn)?;
        Ok(conn)
    }

    #[test]
    fn product_price_must_be_positive_and_integer() -> AppResult<()> {
        let conn = catalog_db()?;
        let error = save_product(&conn, None, "Agua test", "bebidas", 0, true, None)
            .expect_err("zero price must be rejected");
        assert!(error.to_string().contains("entero mayor que 0"));
        let product = save_product(&conn, None, "Agua test", "bebidas", 15_000, true, None)?;
        assert_eq!(product.price_cents, 15_000);
        Ok(())
    }

    #[test]
    fn deactivation_is_logical_and_reactivation_keeps_the_row() -> AppResult<()> {
        let conn = catalog_db()?;
        let product = save_product(&conn, None, "Producto temporal", "snacks", 9_000, true, None)?;
        set_product_active(&conn, product.id, false)?;
        assert!(!list_products(&conn, true)?.iter().any(|item| item.id == product.id));
        assert!(!get_product(&conn, product.id)?.active);
        let restored = set_product_active(&conn, product.id, true)?;
        assert!(restored.active);
        assert!(list_products(&conn, true)?.iter().any(|item| item.id == product.id));
        Ok(())
    }
}

pub fn save_product(
    conn: &Connection,
    id: Option<i64>,
    name: &str,
    category: &str,
    price_cents: i64,
    active: bool,
    sort_order: Option<i64>,
) -> AppResult<Product> {
    let name = name.trim();
    let category = category.trim();
    if name.is_empty() {
        return Err(AppError::msg("El nombre del producto es obligatorio"));
    }
    if category.is_empty() {
        return Err(AppError::msg("La categoría del producto es obligatoria"));
    }
    if !matches!(category, "bebidas" | "snacks" | "tabaco" | "higiene" | "adulto" | "licores") {
        return Err(AppError::msg("La categoría del producto no es válida"));
    }
    if price_cents <= 0 {
        return Err(AppError::msg("El precio debe ser un número entero mayor que 0 Gs."));
    }
    if let Some(id) = id {
        let current = get_product(conn, id)?;
        conn.execute(
            "UPDATE products SET name = ?1, category = ?2, price_cents = ?3, active = ?4, sort_order = ?5, version = version + 1, updated_at = ?6 WHERE id = ?7",
            params![name, category, price_cents, if active { 1 } else { 0 }, sort_order.unwrap_or(current.sort_order), now_rfc3339(), id],
        )?;
        return get_product(conn, id);
    }
    let next_order = sort_order.unwrap_or_else(|| {
        conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM products", [], |row| row.get(0))
            .unwrap_or(1)
    });
    conn.execute(
        "INSERT INTO products (name, category, price_cents, active, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![name, category, price_cents, if active { 1 } else { 0 }, next_order],
    )?;
    get_product(conn, conn.last_insert_rowid())
}

pub fn set_product_active(conn: &Connection, id: i64, active: bool) -> AppResult<Product> {
    get_product(conn, id)?;
    conn.execute(
        "UPDATE products SET active = ?1, version = version + 1, updated_at = ?2 WHERE id = ?3",
        params![if active { 1 } else { 0 }, now_rfc3339(), id],
    )?;
    get_product(conn, id)
}

pub fn is_unique_violation(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(e, Some(message)) => {
            e.code == rusqlite::ErrorCode::ConstraintViolation
                || message.to_uppercase().contains("UNIQUE")
        }
        rusqlite::Error::SqliteFailure(e, None) => {
            e.code == rusqlite::ErrorCode::ConstraintViolation
        }
        _ => false,
    }
}

#[cfg(test)]
mod migration_runner_tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nightdesk-mig-{nanos}"));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn now_rfc3339_is_utc_and_parses_to_matching_local() -> AppResult<()> {
        let stored = now_rfc3339();
        assert!(
            stored.ends_with('Z') || stored.contains("+00:00"),
            "expected UTC timestamp, got {stored}"
        );
        let parsed = parse_dt(&stored)?;
        let utc = DateTime::parse_from_rfc3339(&stored).expect("rfc3339");
        assert_eq!(parsed.timestamp(), utc.timestamp());
        assert_eq!(
            parsed.format("%Y-%m-%d %H:%M").to_string(),
            utc.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()
        );
        Ok(())
    }

    #[test]
    fn second_migrate_does_not_reapply_sql() -> AppResult<()> {
        let dir = temp_db_dir();
        let db_path = dir.join("nightdesk.db");
        let mut conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrate(&mut conn, Some(&db_path))?;
        conn.execute_batch("DROP INDEX IF EXISTS idx_payments_stay;")?;
        migrate(&mut conn, Some(&db_path))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_payments_stay'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(count, 0);
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn migrate_writes_backup_before_pending() -> AppResult<()> {
        let dir = temp_db_dir();
        let db_path = dir.join("nightdesk.db");
        let mut conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrate(&mut conn, Some(&db_path))?;
        let bak = pre_migrate_backup_path(&db_path, "001_init");
        assert!(bak.exists(), "missing backup at {}", bak.display());
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn upgrade_preserves_local_mode_printer_and_closed_account() -> AppResult<()> {
        let dir = temp_db_dir();
        let db_path = dir.join("nightdesk.db");
        let mut conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrate_up_to(&mut conn, "015_catalog_audit")?;
        seed_if_empty(&conn)?;
        upsert_setting(&conn, "device_mode", "reception")?;
        upsert_setting(&conn, "printer_name", "Epson test 80 mm")?;
        upsert_setting(&conn, "paper_width", "80")?;
        let now = now_rfc3339();
        conn.execute("INSERT INTO guests (name, created_at) VALUES ('Prueba instalación', ?1)", [&now])?;
        conn.execute(
            "INSERT INTO stays (room_id, guest_id, rate_plan_id, check_in_at, check_out_at, status)
             VALUES (1, 1, 1, ?1, ?1, 'closed')",
            [&now],
        )?;
        conn.execute(
            "INSERT INTO charges (stay_id, kind, description, amount_cents, created_at)
             VALUES (1, 'product', 'Agua', 5000, ?1)",
            [&now],
        )?;
        conn.execute(
            "INSERT INTO receipt_snapshots (stay_id, bytes, created_at) VALUES (1, ?1, ?2)",
            params![b"ticket congelado", now],
        )?;

        migrate(&mut conn, Some(&db_path))?;
        let backup_path = pre_migrate_backup_path(&db_path, "016_backup");
        assert!(backup_path.exists());
        drop(conn);

        let mut reopened = Connection::open(&db_path)?;
        reopened.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrate(&mut reopened, Some(&db_path))?;
        assert_eq!(get_setting(&reopened, "device_mode", "")?, "reception");
        let settings = load_settings(&reopened)?;
        assert_eq!(settings.printer_name, "Epson test 80 mm");
        assert_eq!(settings.paper_width, 80);
        assert_eq!(reopened.query_row("SELECT COUNT(*) FROM rooms", [], |r| r.get::<_, i64>(0))?, 23);
        assert_eq!(reopened.query_row("SELECT amount_cents FROM charges WHERE stay_id = 1", [], |r| r.get::<_, i64>(0))?, 5000);
        assert_eq!(reopened.query_row("SELECT status FROM stays WHERE id = 1", [], |r| r.get::<_, String>(0))?, "closed");
        assert_eq!(reopened.query_row("SELECT bytes FROM receipt_snapshots WHERE stay_id = 1", [], |r| r.get::<_, Vec<u8>>(0))?, b"ticket congelado");
        drop(reopened);
        let backup = Connection::open(&backup_path)?;
        assert_eq!(get_setting(&backup, "device_mode", "")?, "reception");
        assert_eq!(backup.query_row("SELECT COUNT(*) FROM receipt_snapshots", [], |r| r.get::<_, i64>(0))?, 1);
        drop(backup);
        fs::remove_dir_all(&dir)?;
        Ok(())
    }

    #[test]
    fn failed_migration_rolls_back_and_restores_backup() -> AppResult<()> {
        let dir = temp_db_dir();
        let db_path = dir.join("nightdesk.db");
        let mut conn = Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrate(&mut conn, Some(&db_path))?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('probe', 'ok')",
            [],
        )?;
        let failing = [Migration {
            id: "999_fail",
            sql: "CREATE TABLE intact (id INTEGER); CREATE TABLE intact (id INTEGER);",
        }];
        let error = run_pending(&mut conn, Some(&db_path), &failing).expect_err("must fail");
        assert!(!error.to_string().is_empty());
        let intact: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'intact'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(intact, 0);
        let probe: String = conn.query_row(
            "SELECT value FROM settings WHERE key = 'probe'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(probe, "ok");
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn unique_partial_index_rejects_second_open_stay() -> AppResult<()> {
        let mut conn = Connection::open_in_memory()?;
        migrate(&mut conn, None)?;
        seed_if_empty(&conn)?;
        conn.execute(
            "INSERT INTO guests (name, created_at) VALUES ('Uno', ?1)",
            [now_rfc3339()],
        )?;
        conn.execute(
            "INSERT INTO stays (room_id, guest_id, rate_plan_id, check_in_at, status) VALUES (1, 1, 1, ?1, 'open')",
            [now_rfc3339()],
        )?;
        let error = conn
            .execute(
                "INSERT INTO stays (room_id, guest_id, rate_plan_id, check_in_at, status) VALUES (1, 1, 1, ?1, 'open')",
                [now_rfc3339()],
            )
            .expect_err("duplicate open stay");
        assert!(is_unique_violation(&error));
        Ok(())
    }

    #[test]
    fn migrate_rejects_duplicate_open_stays_without_silently_closing() -> AppResult<()> {
        let mut conn = Connection::open_in_memory()?;
        conn.execute_batch(MIGRATION_001)?;
        apply_migration(&conn, "001_init")?;
        conn.execute(
            "INSERT INTO rooms (number, room_type, floor, status, created_at) VALUES ('01', 'Normal', 1, 'available', ?1)",
            [now_rfc3339()],
        )?;
        conn.execute(
            "INSERT INTO rate_plans (name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active)
             VALUES ('3 horas', 'hourly', 80000, 20000, 3, 10, 12, 1)",
            [],
        )?;
        conn.execute(
            "INSERT INTO guests (name, created_at) VALUES ('Dup', ?1)",
            [now_rfc3339()],
        )?;
        conn.execute(
            "INSERT INTO stays (room_id, guest_id, rate_plan_id, check_in_at, status) VALUES (1, 1, 1, ?1, 'open')",
            [now_rfc3339()],
        )?;
        conn.execute(
            "INSERT INTO stays (room_id, guest_id, rate_plan_id, check_in_at, status) VALUES (1, 1, 1, ?1, 'open')",
            [now_rfc3339()],
        )?;
        let error = apply_pending(
            &mut conn,
            &Migration {
                id: "006_stay_integrity",
                sql: MIGRATION_006,
            },
        )
        .expect_err("duplicates must block 006");
        assert!(error.to_string().contains("estadía abierta"));
        let open: i64 = conn.query_row(
            "SELECT COUNT(*) FROM stays WHERE status = 'open'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(open, 2);
        Ok(())
    }

    fn migrate_up_to(conn: &mut Connection, last_id: &str) -> AppResult<()> {
        let mut catalog = Vec::new();
        for migration in MIGRATIONS {
            catalog.push(Migration {
                id: migration.id,
                sql: migration.sql,
            });
            if migration.id == last_id {
                break;
            }
        }
        run_pending(conn, None, &catalog)
    }

    fn assert_uuid(value: &str) {
        uuid::Uuid::parse_str(value).unwrap_or_else(|_| panic!("invalid uid {value}"));
    }

    #[test]
    fn sync_migration_backfills_uid_and_trigger_fills_inserts() -> AppResult<()> {
        let mut conn = Connection::open_in_memory()?;
        migrate_up_to(&mut conn, "013_no_iva")?;
        seed_if_empty(&conn)?;
        seed_products_if_empty(&conn)?;
        conn.execute(
            "INSERT INTO guests (name, created_at) VALUES ('Pre-sync', ?1)",
            [now_rfc3339()],
        )?;
        conn.execute(
            "INSERT INTO stays (room_id, guest_id, rate_plan_id, check_in_at, status) VALUES (1, 1, 1, ?1, 'open')",
            [now_rfc3339()],
        )?;
        conn.execute(
            "INSERT INTO charges (stay_id, kind, description, amount_cents, created_at) VALUES (1, 'surcharge', 'Pre', 1000, ?1)",
            [now_rfc3339()],
        )?;
        conn.execute(
            "INSERT INTO payments (stay_id, method, amount_cents, created_at) VALUES (1, 'cash', 1000, ?1)",
            [now_rfc3339()],
        )?;
        conn.execute(
            "INSERT INTO users (username, password_hash, role) VALUES ('admin', 'x', 'admin')",
            [],
        )?;

        let missing_uid: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('rooms') WHERE name = 'uid'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(missing_uid, 0);

        migrate(&mut conn, None)?;
        seed_if_empty(&conn)?;

        for table in [
            "rooms",
            "rate_plans",
            "products",
            "guests",
            "stays",
            "charges",
            "payments",
            "users",
        ] {
            let mut stmt = conn.prepare(&format!(
                "SELECT uid FROM {table} WHERE uid IS NULL OR uid = ''"
            ))?;
            let empty: i64 = stmt.query_map([], |_| Ok(()))?.count() as i64;
            assert_eq!(empty, 0, "{table} has empty uid");
            let mut ids = conn.prepare(&format!("SELECT uid FROM {table}"))?;
            let mut seen = std::collections::HashSet::new();
            for uid in ids.query_map([], |row| row.get::<_, String>(0))? {
                let uid = uid?;
                assert_uuid(&uid);
                assert!(seen.insert(uid), "duplicate uid in {table}");
            }
        }

        conn.execute(
            "INSERT INTO rooms (number, room_type, floor, status, created_at) VALUES ('88', 'Normal', 1, 'available', ?1)",
            [now_rfc3339()],
        )?;
        let uid = uid_of(&conn, "rooms", conn.last_insert_rowid())?;
        assert_uuid(&uid);

        let version: i64 = conn.query_row("SELECT version FROM rooms WHERE id = 1", [], |row| row.get(0))?;
        assert_eq!(version, 1);
        let outbox: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'sync_outbox'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(outbox, 1);
        Ok(())
    }
}
