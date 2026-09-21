use super::client::SyncClient;
use super::outbox::{self, Entity, OutboxOp};
use super::pull;
use super::settings::BUSINESS_SETTING_KEYS;
use super::state;
use crate::db;
use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{json, Value};

pub const BOOTSTRAP_KEY: &str = "bootstrap_done";

pub fn run(conn: &mut Connection, client: &dyn SyncClient, device_id: &str) -> AppResult<bool> {
    if state::get(conn, BOOTSTRAP_KEY)?.is_some() {
        return Ok(false);
    }
    let catalog = dump_catalog(conn)?;
    let reply = client.bootstrap(device_id, &catalog)?;
    if reply["accepted"].as_bool() != Some(true) {
        overwrite_local(conn, &reply)?;
        let _ = pull::pull_all(conn, client)?;
    }
    enqueue_history(conn)?;
    state::set(conn, BOOTSTRAP_KEY, &db::now_rfc3339())?;
    Ok(true)
}

fn dump_catalog(conn: &Connection) -> AppResult<Value> {
    Ok(json!({
        "rooms": dump_rooms(conn)?,
        "rate_plans": dump_rate_plans(conn)?,
        "products": dump_products(conn)?,
        "settings": dump_settings(conn)?,
        "app_users": dump_users(conn)?,
    }))
}

fn dump_rooms(conn: &Connection) -> AppResult<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, uid, number, room_type, floor, status, notes, active, version FROM rooms ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "local_id": row.get::<_, i64>(0)?,
            "uid": row.get::<_, String>(1)?,
            "number": row.get::<_, String>(2)?,
            "room_type": row.get::<_, String>(3)?,
            "floor": row.get::<_, i64>(4)?,
            "status": row.get::<_, String>(5)?,
            "notes": row.get::<_, Option<String>>(6)?,
            "active": row.get::<_, i64>(7)? != 0,
            "version": row.get::<_, i64>(8)?,
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn dump_rate_plans(conn: &Connection) -> AppResult<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, uid, name, kind, base_amount_cents, extra_hour_cents, included_hours, grace_minutes, night_cutoff_hour, active, version
         FROM rate_plans ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "local_id": row.get::<_, i64>(0)?,
            "uid": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?,
            "kind": row.get::<_, String>(3)?,
            "base_amount_cents": row.get::<_, i64>(4)?,
            "extra_hour_cents": row.get::<_, i64>(5)?,
            "included_hours": row.get::<_, i64>(6)?,
            "grace_minutes": row.get::<_, i64>(7)?,
            "night_cutoff_hour": row.get::<_, i64>(8)?,
            "active": row.get::<_, i64>(9)? != 0,
            "version": row.get::<_, i64>(10)?,
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn dump_products(conn: &Connection) -> AppResult<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, uid, name, category, price_cents, active, sort_order, version FROM products ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "local_id": row.get::<_, i64>(0)?,
            "uid": row.get::<_, String>(1)?,
            "name": row.get::<_, String>(2)?,
            "category": row.get::<_, String>(3)?,
            "price_cents": row.get::<_, i64>(4)?,
            "active": row.get::<_, i64>(5)? != 0,
            "sort_order": row.get::<_, i64>(6)?,
            "version": row.get::<_, i64>(7)?,
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn dump_users(conn: &Connection) -> AppResult<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, uid, username, password_hash, role, active, version FROM users ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "local_id": row.get::<_, i64>(0)?,
            "uid": row.get::<_, String>(1)?,
            "username": row.get::<_, String>(2)?,
            "password_hash": row.get::<_, String>(3)?,
            "role": row.get::<_, String>(4)?,
            "active": row.get::<_, i64>(5)? != 0,
            "version": row.get::<_, i64>(6)?,
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn dump_settings(conn: &Connection) -> AppResult<Vec<Value>> {
    let mut out = Vec::new();
    for key in BUSINESS_SETTING_KEYS {
        let value = db::get_setting(conn, key, "")?;
        if value.is_empty() {
            continue;
        }
        let version: i64 = conn
            .query_row(
                "SELECT version FROM catalog_setting_versions WHERE key=?1",
                [key],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(1);
        out.push(json!({"key": key, "value": value, "version": version.max(1)}));
    }
    Ok(out)
}

fn overwrite_local(conn: &mut Connection, reply: &Value) -> AppResult<()> {
    let tx = conn.transaction()?;
    apply_array(&tx, "rooms", &reply["rooms"], true)?;
    apply_array(&tx, "rate_plans", &reply["rate_plans"], true)?;
    apply_array(&tx, "products", &reply["products"], true)?;
    apply_array(&tx, "app_users", &reply["app_users"], true)?;
    if let Some(rows) = reply["settings"].as_array() {
        for row in rows {
            super::catalog::apply_setting_row(&tx, row, true)?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn apply_array(conn: &Connection, table: &str, value: &Value, force: bool) -> AppResult<()> {
    let Some(rows) = value.as_array() else {
        return Ok(());
    };
    for row in rows {
        pull::apply_remote_row(conn, table, row, force)?;
    }
    Ok(())
}

fn enqueue_history(conn: &mut Connection) -> AppResult<()> {
    enqueue_table(conn, Entity::Guest, "SELECT id FROM guests ORDER BY id", db::payload_for_guest)?;
    enqueue_table(conn, Entity::Room, "SELECT id FROM rooms ORDER BY id", db::payload_for_room)?;
    enqueue_table(
        conn,
        Entity::Reservation,
        "SELECT id FROM reservations ORDER BY id",
        db::payload_for_reservation,
    )?;
    enqueue_table(conn, Entity::Stay, "SELECT id FROM stays ORDER BY id", db::payload_for_stay)?;
    enqueue_table(conn, Entity::Charge, "SELECT id FROM charges ORDER BY id", db::payload_for_charge)?;
    Ok(())
}

fn enqueue_table(
    conn: &mut Connection,
    entity: Entity,
    sql: &str,
    payload: fn(&Connection, i64) -> AppResult<Value>,
) -> AppResult<()> {
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for chunk in ids.chunks(500) {
        let tx = conn.transaction()?;
        for id in chunk {
            let payload = payload(&tx, *id)?;
            let uid = payload["uid"]
                .as_str()
                .ok_or_else(|| AppError::msg("Fila sin uid"))?
                .to_string();
            let root = outbox::bootstrap_operation_id(entity.as_str(), &uid);
            outbox::enqueue_if_absent(&tx, &root, &[OutboxOp::upsert(entity, uid, payload)])?;
        }
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::client::FakeClient;
    use std::path::Path;

    fn db() -> Connection {
        crate::db::open(Path::new(":memory:")).unwrap()
    }

    fn remote_room(uid: &str, number: &str, version: i64) -> Value {
        json!({
            "uid": uid,
            "number": number,
            "room_type": "Estándar",
            "floor": 1,
            "notes": null,
            "active": true,
            "status": "dirty",
            "version": version,
            "updated_at": "2026-09-21T12:00:00Z",
        })
    }

    #[test]
    fn empty_remote_sends_local_catalog() {
        let mut conn = db();
        let client = FakeClient::new();
        assert!(run(&mut conn, &client, "33333333-3333-3333-3333-333333333333").unwrap());
        let sent = client.bootstrap_calls.lock().unwrap()[0].clone();
        assert!(sent["rooms"].as_array().unwrap().len() >= 23);
        assert!(sent["rate_plans"].as_array().unwrap().len() >= 2);
        assert!(state::get(&conn, BOOTSTRAP_KEY).unwrap().is_some());
        assert!(!run(&mut conn, &client, "33333333-3333-3333-3333-333333333333").unwrap());
        assert_eq!(client.bootstrap_calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn remote_catalog_overwrites_even_if_local_version_is_newer() {
        let mut conn = db();
        conn.execute("UPDATE rooms SET version=9, status='occupied' WHERE id=1", []).unwrap();
        let uid: String = conn.query_row("SELECT uid FROM rooms WHERE id=1", [], |r| r.get(0)).unwrap();
        let client = FakeClient::new();
        *client.bootstrap_reply.lock().unwrap() = json!({
            "accepted": false,
            "rooms": [remote_room(&uid, "99", 2)],
            "rate_plans": [],
            "products": [],
            "settings": [],
            "app_users": [],
        });
        run(&mut conn, &client, "dev").unwrap();
        let room = crate::db::get_room(&conn, 1).unwrap();
        assert_eq!(room.number, "99");
        assert_eq!(room.version, 2);
        assert_eq!(room.status, "occupied");
    }

    #[test]
    fn history_is_enqueued_in_fk_order_and_replay_does_not_duplicate() {
        let mut conn = db();
        let now = db::now_rfc3339();
        conn.execute("INSERT INTO guests (name, created_at) VALUES ('Ana', ?1)", [&now]).unwrap();
        conn.execute(
            "INSERT INTO stays (room_id, guest_id, rate_plan_id, check_in_at, status) VALUES (1, 1, 1, ?1, 'open')",
            [&now],
        )
        .unwrap();
        let client = FakeClient::new();
        run(&mut conn, &client, "dev").unwrap();
        let entities: Vec<String> = conn
            .prepare("SELECT entity FROM sync_outbox ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let guest = entities.iter().position(|e| e == "guest").unwrap();
        let room = entities.iter().position(|e| e == "room").unwrap();
        let stay = entities.iter().position(|e| e == "stay").unwrap();
        assert!(guest < room);
        assert!(room < stay);
        let first = entities.len();
        conn.execute("DELETE FROM sync_state WHERE key='bootstrap_done'", []).unwrap();
        run(&mut conn, &client, "dev").unwrap();
        let second: i64 = conn.query_row("SELECT count(*) FROM sync_outbox", [], |r| r.get(0)).unwrap();
        assert_eq!(second as usize, first);
    }
}
