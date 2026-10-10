//! One-time reset included only in the explicitly selected training-reset build.
use super::*;

// This identifies the maintenance event, not an app version. Never change it for
// a rebuild or a later release: doing so would erase the new production data.
const RESET_ID: &str = "post-training-2026-10";

/// Call only at app startup, before any sync/backup connection or UI is created.
/// Worker connections use db::open and must never initiate maintenance.
pub(crate) fn apply_once(conn: &mut Connection, db_path: &Path) -> AppResult<bool> {
    let applied: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM maintenance_resets WHERE reset_id = ?1)",
        [RESET_ID],
        |row| row.get(0),
    )?;
    if applied {
        return Ok(false);
    }

    let stem = db_path.file_stem().unwrap_or_default().to_string_lossy();
    let snapshot = db_path.with_file_name(format!("{stem}.pre-reset-{RESET_ID}.bak"));
    // Online Backup includes WAL contents. A failed backup prevents the reset.
    // Keep this recovery file outside backup_queue: never upload training again.
    conn.backup(DatabaseName::Main, &snapshot, None)?;

    let tx = conn.transaction()?;
    tx.execute_batch(include_str!("../../maintenance/reset_training.sql"))?;
    seed_if_empty(&tx)?;
    seed_jacuzzi_plans_if_missing(&tx)?;
    seed_products_if_empty(&tx)?;
    upsert_setting(&tx, "product_tracking_since", &now_rfc3339())?;
    tx.execute(
        "INSERT INTO maintenance_resets(reset_id, applied_at) VALUES (?1, ?2)",
        params![RESET_ID, now_rfc3339()],
    )?;
    tx.commit()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        dir: PathBuf,
        path: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("nightdesk-training-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Self {
                path: dir.join("nightdesk.db"),
                dir,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn populate(conn: &Connection) -> AppResult<()> {
        conn.execute_batch(
            "INSERT INTO users(id, username, password_hash, role, active, uid, version, updated_at)
             VALUES (7, 'admin', 'hash-admin', 'admin', 1, '11111111-1111-4111-8111-111111111111', 3, '2026-10-01'),
                    (9, 'turno', 'hash-turno', 'recepcion', 0, '22222222-2222-4222-8222-222222222222', 2, '2026-10-02');
             INSERT INTO guests(id, name, created_at) VALUES (1, 'Capacitación', '2026-10-01');
             INSERT INTO reservations(id, guest_id, room_id, rate_plan_id, expected_arrival_at, created_at)
             VALUES (1, 1, 1, 1, '2026-10-01', '2026-10-01');
             INSERT INTO stays(id, room_id, guest_id, rate_plan_id, reservation_id, check_in_at, status)
             VALUES (1, 1, 1, 1, 1, '2026-10-01', 'open');
             INSERT INTO charges(id, stay_id, kind, description, amount_cents, created_at)
             VALUES (1, 1, 'surcharge', 'Prueba', 5000, '2026-10-01');
             INSERT INTO payments(stay_id, method, amount_cents, created_at) VALUES (1, 'cash', 5000, '2026-10-01');
             INSERT INTO receipt_snapshots(stay_id, bytes, created_at) VALUES (1, X'0102', '2026-10-01');
             UPDATE rooms SET status = 'occupied', notes = 'Capacitación' WHERE id = 1;
             UPDATE rate_plans SET base_amount_cents = 1;
             UPDATE products SET price_cents = 1;
             INSERT INTO login_attempts VALUES ('admin', 3, 12345);
             INSERT INTO product_stock VALUES (1, 42, 5, '2026-10-01');
             INSERT INTO stock_movements(product_id, delta, quantity_after, reason, created_at)
             VALUES (1, 42, 42, 'restock', '2026-10-01');
             INSERT INTO catalog_audit VALUES ('old-audit', 'rooms', 'admin', '2026-10-01', '{}', '{}');
             INSERT INTO catalog_setting_versions VALUES ('business_name', 8);
             INSERT INTO sync_outbox(operation_id, root_operation_id, entity, entity_uid, op, payload, created_at)
             VALUES ('old-op', 'old-root', 'stay', 'old-stay', 'upsert', '{}', '2026-10-01');
             INSERT INTO sync_state VALUES ('bootstrap_done', '1'), ('pull_cursor:rooms', '2026-10-01');
             INSERT INTO backup_meta VALUES ('last_local_at', '2026-10-01');
             INSERT INTO backup_queue(backup_id, created_at, status, snapshot_path, size_bytes, checksum, schema_version, app_version, motel_id)
             VALUES ('training', '2026-10-01', 'pending_upload', 'old.db', 1, 'x', '021', '0.1.13', 'test');"
        )?;
        upsert_setting(conn, "business_name", "Capacitación")?;
        upsert_setting(conn, "device_mode", "reception")?;
        upsert_setting(conn, "printer_name", "Impresora de prueba")?;
        upsert_setting(conn, "price_rules", "reglas de prueba")?;
        upsert_setting(conn, "product_tracking_since", "2026-10-01T00:00:00Z")?;
        Ok(())
    }

    fn users(conn: &Connection) -> AppResult<Vec<String>> {
        let mut stmt = conn.prepare(
            "SELECT json_array(id, username, password_hash, role, active, uid, version, updated_at) FROM users ORDER BY id"
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn reset_erases_training_preserves_all_user_fields_and_seeds_defaults() -> AppResult<()> {
        let fixture = Fixture::new();
        let mut conn = open(&fixture.path)?;
        populate(&conn)?;
        let before_users = users(&conn)?;
        assert!(apply_once(&mut conn, &fixture.path)?);
        assert_eq!(users(&conn)?, before_users);
        for table in [
            "guests",
            "reservations",
            "stays",
            "charges",
            "payments",
            "receipt_snapshots",
            "login_attempts",
            "product_stock",
            "stock_movements",
            "catalog_audit",
            "catalog_setting_versions",
            "sync_outbox",
            "sync_state",
            "backup_queue",
            "backup_meta",
        ] {
            assert_eq!(count(&conn, table), 0, "{table} retained training data");
        }
        assert_eq!(count(&conn, "rooms"), 23);
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM rooms WHERE status = 'available' AND notes IS NULL",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            23
        );
        assert_eq!(
            conn.query_row(
                "SELECT base_amount_cents FROM rate_plans WHERE id = 1",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            45_000
        );
        assert_eq!(
            conn.query_row("SELECT price_cents FROM products WHERE id = 1", [], |r| r
                .get::<_, i64>(
                0
            ))?,
            5_000
        );
        assert_eq!(get_setting(&conn, "device_mode", "")?, "reception");
        assert_eq!(get_setting(&conn, "price_rules", "")?, "");
        assert_eq!(load_settings(&conn)?.printer_name, "");
        assert_ne!(
            get_setting(&conn, "product_tracking_since", "")?,
            "2026-10-01T00:00:00Z"
        );
        assert!(!get_setting(&conn, "product_tracking_since", "")?.is_empty());
        assert_eq!(
            conn.prepare("PRAGMA foreign_key_check")?
                .query([])?
                .next()?
                .is_none(),
            true
        );
        conn.execute("INSERT INTO users(username, password_hash, role) VALUES ('nuevo', 'hash', 'recepcion')", [])?;
        assert!(conn.last_insert_rowid() > 9);
        Ok(())
    }

    #[test]
    fn reset_runs_once_across_reopen_and_keeps_a_recovery_snapshot() -> AppResult<()> {
        let fixture = Fixture::new();
        let mut conn = open(&fixture.path)?;
        populate(&conn)?;
        assert!(apply_once(&mut conn, &fixture.path)?);
        conn.execute(
            "INSERT INTO guests(name, created_at) VALUES ('Cliente real', '2026-10-10')",
            [],
        )?;
        drop(conn);
        let mut conn = open(&fixture.path)?;
        assert!(!apply_once(&mut conn, &fixture.path)?);
        assert_eq!(
            conn.query_row("SELECT name FROM guests", [], |r| r.get::<_, String>(0))?,
            "Cliente real"
        );
        let backup = Connection::open(
            fixture
                .dir
                .join("nightdesk.pre-reset-post-training-2026-10.bak"),
        )?;
        assert_eq!(
            backup.query_row("SELECT name FROM guests", [], |r| r.get::<_, String>(0))?,
            "Capacitación"
        );
        assert_eq!(count(&backup, "users"), 2);
        Ok(())
    }

    #[test]
    fn reset_failure_rolls_back_training_data_and_can_be_retried() -> AppResult<()> {
        let fixture = Fixture::new();
        let mut conn = open(&fixture.path)?;
        populate(&conn)?;
        conn.execute_batch("CREATE TRIGGER fail_reset BEFORE DELETE ON rooms BEGIN SELECT RAISE(ABORT, 'test disk failure'); END;")?;
        assert!(apply_once(&mut conn, &fixture.path).is_err());
        assert_eq!(count(&conn, "stays"), 1);
        assert_eq!(count(&conn, "charges"), 1);
        assert_eq!(count(&conn, "sync_outbox"), 1);
        conn.execute_batch("DROP TRIGGER fail_reset")?;
        assert!(apply_once(&mut conn, &fixture.path)?);
        assert_eq!(count(&conn, "stays"), 0);
        Ok(())
    }

    #[test]
    fn reset_stops_before_deleting_when_snapshot_cannot_be_written() -> AppResult<()> {
        let fixture = Fixture::new();
        let mut conn = open(&fixture.path)?;
        populate(&conn)?;
        let impossible = fixture.dir.join("missing/nightdesk.db");
        assert!(apply_once(&mut conn, &impossible).is_err());
        assert_eq!(count(&conn, "stays"), 1);
        assert_eq!(count(&conn, "users"), 2);
        Ok(())
    }

    #[test]
    fn reset_keeps_remote_admin_mode_so_it_never_starts_reception_sync() -> AppResult<()> {
        let fixture = Fixture::new();
        let mut conn = open(&fixture.path)?;
        populate(&conn)?;
        upsert_setting(&conn, "device_mode", "remote")?;
        apply_once(&mut conn, &fixture.path)?;
        let mode = crate::service::device_mode_get(&conn)?;
        assert_eq!(mode.as_deref(), Some("remote"));
        assert!(!crate::sync::worker::should_start(mode.as_deref(), true));
        Ok(())
    }
}
