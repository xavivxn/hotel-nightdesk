use super::catalog;
use super::client::SyncClient;
use super::state;
use crate::error::{AppError, AppResult};
use rusqlite::Connection;

pub const PAGE_LIMIT: i64 = 500;
const TABLES: &[&str] = &[
    "rooms",
    "rate_plans",
    "products",
    "app_users",
    "business_settings",
    "catalog_deletes",
];

fn cursor_key(table: &str) -> String {
    format!("pull_cursor:{table}")
}

pub fn pull_table(conn: &mut Connection, client: &dyn SyncClient, table: &str) -> AppResult<usize> {
    let mut applied = 0usize;
    loop {
        let cursor = state::get(conn, &cursor_key(table))?;
        let rows = client.pull_rows(table, cursor.as_deref(), PAGE_LIMIT)?;
        if rows.is_empty() {
            break;
        }
        let page_max = rows
            .iter()
            .filter_map(|row| row["updated_at"].as_str())
            .max()
            .map(str::to_string);
        let tx = conn.transaction()?;
        for row in &rows {
            if apply_remote_row(&tx, table, row, false)? {
                applied += 1;
            }
        }
        if let Some(cursor) = page_max {
            state::set(&tx, &cursor_key(table), &cursor)?;
        }
        tx.commit()?;
        if rows.len() < PAGE_LIMIT as usize {
            break;
        }
    }
    Ok(applied)
}

fn skip_missing_catalog_table(table: &str, error: &AppError) -> bool {
    table == "catalog_deletes" && error.to_string().contains("schema cache")
}

pub fn pull_all(conn: &mut Connection, client: &dyn SyncClient) -> AppResult<bool> {
    let mut changed = false;
    for table in TABLES {
        match pull_table(conn, client, table) {
            Ok(n) if n > 0 => changed = true,
            Ok(_) => {}
            Err(error) if skip_missing_catalog_table(table, &error) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(changed)
}

pub fn apply_remote_row(conn: &rusqlite::Connection, table: &str, row: &serde_json::Value, force: bool) -> AppResult<bool> {
    if table == "catalog_deletes" {
        let uid = row["uid"].as_str().unwrap_or("");
        if row["entity"].as_str() != Some("app_users") || uid.is_empty() {
            return Ok(false);
        }
        return catalog::delete_local_user(conn, uid);
    }
    if table == "business_settings" {
        return catalog::apply_setting_row(conn, row, force);
    }
    if table == "app_users" {
        catalog::adopt_user_uid(conn, row)?;
    }
    Ok(catalog::apply_status(conn, table, row, force)?.changed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::client::FakeClient;
    use rusqlite::Connection;
    use serde_json::json;
    use std::path::Path;

    fn db() -> Connection {
        crate::db::open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn cursor_uses_server_updated_at_and_preserves_occupancy() {
        let mut conn = db();
        conn.execute("UPDATE rooms SET status='occupied' WHERE id=1", []).unwrap();
        let uid: String = conn.query_row("SELECT uid FROM rooms WHERE id=1", [], |r| r.get(0)).unwrap();
        let client = FakeClient::new();
        client.pull.lock().unwrap().insert(
            "rooms".into(),
            vec![json!({
                "uid": uid,
                "number": "01",
                "room_type": "Jacuzzi",
                "floor": 1,
                "notes": null,
                "active": false,
                "version": 4,
                "updated_at": "2026-09-21T15:00:00Z",
            })],
        );
        assert!(pull_all(&mut conn, &client).unwrap());
        let room = crate::db::get_room(&conn, 1).unwrap();
        assert!(!room.active);
        assert_eq!(room.status, "occupied");
        assert_eq!(room.version, 4);
        assert_eq!(
            state::get(&conn, "pull_cursor:rooms").unwrap().as_deref(),
            Some("2026-09-21T15:00:00Z")
        );
    }

    #[test]
    fn new_row_gets_local_id() {
        let mut conn = db();
        let client = FakeClient::new();
        let uid = uuid::Uuid::new_v4().to_string();
        client.pull.lock().unwrap().insert(
            "products".into(),
            vec![json!({
                "uid": uid,
                "name": "Agua",
                "category": "bebidas",
                "price_cents": 5000,
                "active": true,
                "sort_order": 1,
                "version": 1,
                "updated_at": "2026-09-21T16:00:00Z",
            })],
        );
        pull_all(&mut conn, &client).unwrap();
        let id: i64 = conn
            .query_row("SELECT id FROM products WHERE uid=?1", [&uid], |r| r.get(0))
            .unwrap();
        assert!(id > 0);
    }

    #[test]
    fn device_setting_is_rejected_and_cursor_stays() {
        let mut conn = db();
        let client = FakeClient::new();
        client.pull.lock().unwrap().insert(
            "business_settings".into(),
            vec![json!({"key":"theme","value":"dark","version":2,"updated_at":"2026-09-21T17:00:00Z"})],
        );
        assert!(pull_table(&mut conn, &client, "business_settings").is_err());
        assert!(state::get(&conn, "pull_cursor:business_settings").unwrap().is_none());
    }

    #[test]
    fn failed_page_rolls_back_cursor() {
        let mut conn = db();
        let client = FakeClient::new();
        let uid = uuid::Uuid::new_v4().to_string();
        client.pull.lock().unwrap().insert(
            "rate_plans".into(),
            vec![json!({
                "uid": uid,
                "kind": "hourly",
                "base_amount_cents": 1000,
                "extra_hour_cents": 0,
                "included_hours": 1,
                "grace_minutes": 5,
                "night_cutoff_hour": 10,
                "active": true,
                "version": 1,
                "updated_at": "2026-09-21T19:00:00Z"
            })],
        );
        assert!(pull_table(&mut conn, &client, "rate_plans").is_err());
        assert!(state::get(&conn, "pull_cursor:rate_plans").unwrap().is_none());
        let count: i64 = conn
            .query_row("SELECT count(*) FROM rate_plans WHERE uid=?1", [&uid], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn app_users_same_username_adopts_uid() {
        let mut conn = db();
        conn.execute(
            "INSERT INTO users (username, password_hash, role, active) VALUES ('admin','hash','admin',1)",
            [],
        )
        .unwrap();
        let local_id: i64 = conn.last_insert_rowid();
        let remote_uid = uuid::Uuid::new_v4().to_string();
        let client = FakeClient::new();
        client.pull.lock().unwrap().insert(
            "app_users".into(),
            vec![json!({
                "uid": remote_uid,
                "username": "Admin",
                "password_hash": "hash",
                "role": "admin",
                "active": true,
                "version": 2,
                "updated_at": "2026-09-21T18:00:00Z",
            })],
        );
        pull_all(&mut conn, &client).unwrap();
        let (id, uid): (i64, String) = conn
            .query_row("SELECT id, uid FROM users WHERE username='admin'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(id, local_id);
        assert_eq!(uid, remote_uid);
    }

    #[test]
    fn catalog_delete_tombstone_removes_local_user() {
        let mut conn = db();
        conn.execute(
            "INSERT INTO users (username, password_hash, role, active) VALUES ('recepcion','hash','recepcion',1)",
            [],
        )
        .unwrap();
        let extra_uid: String = conn
            .query_row("SELECT uid FROM users WHERE username='recepcion'", [], |r| r.get(0))
            .unwrap();
        let client = FakeClient::new();
        client.pull.lock().unwrap().insert(
            "catalog_deletes".into(),
            vec![json!({
                "entity": "app_users",
                "uid": extra_uid,
                "deleted_at": "2026-09-24T18:00:00Z",
            })],
        );
        assert!(pull_all(&mut conn, &client).unwrap());
        let remaining: i64 = conn
            .query_row("SELECT count(*) FROM users WHERE uid=?1", [&extra_uid], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
    }
}
