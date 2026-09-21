use super::client::SyncClient;
use crate::error::{AppResult, ErrorCode};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::time::Duration;

pub const BATCH_LIMIT: i64 = 200;

#[derive(Debug, Clone)]
pub struct OutboxRow {
    pub id: i64,
    pub operation_id: String,
    pub entity: String,
    pub entity_uid: String,
    pub op: String,
    pub payload: String,
    #[allow(dead_code)]
    pub attempts: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushStep {
    Empty,
    Sent(usize),
    Isolated,
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainResult {
    Done,
    Retry,
}

impl OutboxRow {
    fn as_json(&self) -> AppResult<Value> {
        let payload: Value = serde_json::from_str(&self.payload).unwrap_or(Value::Null);
        Ok(json!({
            "operation_id": self.operation_id,
            "entity": self.entity,
            "entity_uid": self.entity_uid,
            "op": self.op,
            "payload": payload,
        }))
    }
}

pub fn backoff(attempt: u32) -> Duration {
    let secs = [5_u64, 10, 20, 40, 60];
    let idx = attempt.saturating_sub(1).min(4) as usize;
    Duration::from_secs(secs[idx])
}

pub fn pending_count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM sync_outbox WHERE status='pending'",
        [],
        |row| row.get(0),
    )?)
}

pub fn pending_attempts(conn: &Connection) -> AppResult<Option<u32>> {
    let value: Option<i64> = conn
        .query_row(
            "SELECT attempts FROM sync_outbox WHERE status='pending' ORDER BY id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.map(|attempts| attempts.max(0) as u32))
}

pub fn next_batch(conn: &Connection) -> AppResult<Vec<OutboxRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, operation_id, entity, entity_uid, op, payload, attempts
         FROM sync_outbox WHERE status='pending' ORDER BY id LIMIT ?1",
    )?;
    let rows = stmt.query_map([BATCH_LIMIT], |row| {
        Ok(OutboxRow {
            id: row.get(0)?,
            operation_id: row.get(1)?,
            entity: row.get(2)?,
            entity_uid: row.get(3)?,
            op: row.get(4)?,
            payload: row.get(5)?,
            attempts: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn push_once(conn: &Connection, client: &dyn SyncClient, device_id: &str) -> AppResult<PushStep> {
    let batch = next_batch(conn)?;
    if batch.is_empty() {
        return Ok(PushStep::Empty);
    }
    let ops: AppResult<Vec<Value>> = batch.iter().map(OutboxRow::as_json).collect();
    let ops = ops?;
    match client.apply_ops(device_id, &ops) {
        Ok(_) => {
            mark_sent(conn, &batch)?;
            Ok(PushStep::Sent(batch.len()))
        }
        Err(error) if error.code() == ErrorCode::Validation && batch.len() > 1 => {
            isolate(conn, client, device_id, &batch)
        }
        Err(error) if error.code() == ErrorCode::Validation => {
            mark_rejected(conn, &batch[0], &error.to_string())?;
            Ok(PushStep::Isolated)
        }
        Err(error) => {
            bump_attempts(conn, &batch, &error.to_string())?;
            Ok(PushStep::Retry)
        }
    }
}

fn isolate(
    conn: &Connection,
    client: &dyn SyncClient,
    device_id: &str,
    batch: &[OutboxRow],
) -> AppResult<PushStep> {
    for (index, row) in batch.iter().enumerate() {
        let op = row.as_json()?;
        match client.apply_ops(device_id, &[op]) {
            Ok(_) => mark_sent(conn, std::slice::from_ref(row))?,
            Err(error) if error.code() == ErrorCode::Validation => {
                mark_rejected(conn, row, &error.to_string())?;
            }
            Err(error) => {
                bump_attempts(conn, &batch[index..], &error.to_string())?;
                return Ok(PushStep::Retry);
            }
        }
    }
    Ok(PushStep::Isolated)
}

pub fn drain(conn: &Connection, client: &dyn SyncClient, device_id: &str) -> AppResult<DrainResult> {
    loop {
        match push_once(conn, client, device_id)? {
            PushStep::Empty => return Ok(DrainResult::Done),
            PushStep::Retry => return Ok(DrainResult::Retry),
            PushStep::Sent(_) | PushStep::Isolated => {}
        }
    }
}

fn mark_sent(conn: &Connection, rows: &[OutboxRow]) -> AppResult<()> {
    for row in rows {
        conn.execute(
            "UPDATE sync_outbox SET status='sent', last_error=NULL WHERE id=?1 AND status='pending'",
            [row.id],
        )?;
    }
    Ok(())
}

fn mark_rejected(conn: &Connection, row: &OutboxRow, error: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE sync_outbox SET status='rejected', last_error=?1, attempts=attempts+1 WHERE id=?2",
        params![error, row.id],
    )?;
    Ok(())
}

fn bump_attempts(conn: &Connection, rows: &[OutboxRow], error: &str) -> AppResult<()> {
    for row in rows {
        conn.execute(
            "UPDATE sync_outbox SET attempts=attempts+1, last_error=?1 WHERE id=?2 AND status='pending'",
            params![error, row.id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::client::FakeClient;
    use rusqlite::params;
    use std::path::Path;

    fn db() -> Connection {
        crate::db::open(Path::new(":memory:")).unwrap()
    }

    fn seed_outbox(conn: &Connection, n: usize) -> Vec<String> {
        let mut ids = Vec::new();
        for _ in 0..n {
            let op = uuid::Uuid::new_v4().to_string();
            let uid = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO sync_outbox (operation_id, root_operation_id, entity, entity_uid, op, payload, created_at, attempts, status)
                 VALUES (?1,?1,'guest',?2,'upsert',?3,?4,0,'pending')",
                params![op, uid, json!({"uid": uid, "name": "A"}).to_string(), crate::db::now_rfc3339()],
            )
            .unwrap();
            ids.push(op);
        }
        ids
    }

    fn status_count(conn: &Connection, status: &str) -> i64 {
        conn.query_row(
            "SELECT count(*) FROM sync_outbox WHERE status=?1",
            [status],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn backoff_caps_at_sixty() {
        assert_eq!(backoff(1), Duration::from_secs(5));
        assert_eq!(backoff(2), Duration::from_secs(10));
        assert_eq!(backoff(3), Duration::from_secs(20));
        assert_eq!(backoff(4), Duration::from_secs(40));
        assert_eq!(backoff(5), Duration::from_secs(60));
        assert_eq!(backoff(6), Duration::from_secs(60));
    }

    #[test]
    fn two_hundred_fifty_ops_go_in_two_fifo_batches() {
        let conn = db();
        seed_outbox(&conn, 250);
        let client = FakeClient::new();
        assert_eq!(drain(&conn, &client, "dev").unwrap(), DrainResult::Done);
        let calls = client.apply_calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].len(), 200);
        assert_eq!(calls[1].len(), 50);
        assert_eq!(status_count(&conn, "sent"), 250);
        assert_eq!(status_count(&conn, "pending"), 0);
    }

    #[test]
    fn sent_only_after_confirmation_and_offline_keeps_pending() {
        let conn = db();
        seed_outbox(&conn, 3);
        let client = FakeClient::new();
        client.set_offline(true);
        assert_eq!(push_once(&conn, &client, "dev").unwrap(), PushStep::Retry);
        assert_eq!(status_count(&conn, "pending"), 3);
        assert_eq!(status_count(&conn, "sent"), 0);
        let attempts: i64 = conn
            .query_row("SELECT attempts FROM sync_outbox ORDER BY id LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(attempts, 1);
        let err: String = conn
            .query_row("SELECT last_error FROM sync_outbox ORDER BY id LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert!(err.contains("conexión"));
        client.set_offline(false);
        assert_eq!(drain(&conn, &client, "dev").unwrap(), DrainResult::Done);
        assert_eq!(status_count(&conn, "sent"), 3);
    }

    #[test]
    fn validation_isolates_bad_op() {
        let conn = db();
        let ids = seed_outbox(&conn, 3);
        let client = FakeClient::new();
        *client.reject_op.lock().unwrap() = Some(ids[1].clone());
        assert_eq!(drain(&conn, &client, "dev").unwrap(), DrainResult::Done);
        let statuses: Vec<String> = conn
            .prepare("SELECT status FROM sync_outbox ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(statuses, ["sent", "rejected", "sent"]);
    }

    #[test]
    fn six_hours_offline_then_one_drain() {
        use crate::models::{AddChargePayload, CheckInPayload, CheckOutPayload, Role};
        use crate::service::{self, Actor};
        let mut conn = db();
        let admin = Actor {
            user_id: 1,
            username: "admin".into(),
            role: Role::Admin,
        };
        let mut closed_ids = Vec::new();
        for room_id in 1..=10 {
            let stay = service::check_in_on(
                &mut conn,
                CheckInPayload {
                    room_id,
                    guest_name: format!("Huésped {room_id}"),
                    document: None,
                    phone: None,
                    rate_plan_id: 1,
                    expected_hours: Some(1),
                    reservation_id: None,
                    operation_id: None,
                    expected_version: None,
                },
            )
            .unwrap();
            service::add_charge(
                &mut conn,
                &admin,
                AddChargePayload {
                    stay_id: stay.id,
                    kind: "surcharge".into(),
                    description: "Extra".into(),
                    amount_cents: 1_000,
                    operation_id: None,
                    expected_version: None,
                },
            )
            .unwrap();
            let (closed, _) = service::check_out(
                &mut conn,
                &CheckOutPayload {
                    stay_id: stay.id,
                    print: false,
                    operation_id: None,
                    expected_version: None,
                },
            )
            .unwrap();
            assert_eq!(closed.status, "closed");
            closed_ids.push(closed.id);
        }
        let pending = pending_count(&conn).unwrap();
        assert!(pending >= 50, "pending={pending}");
        let client = FakeClient::new();
        client.set_offline(true);
        assert_eq!(drain(&conn, &client, "dev").unwrap(), DrainResult::Retry);
        assert_eq!(pending_count(&conn).unwrap(), pending);
        client.set_offline(false);
        assert_eq!(drain(&conn, &client, "dev").unwrap(), DrainResult::Done);
        assert_eq!(pending_count(&conn).unwrap(), 0);
        for id in closed_ids {
            let stay = crate::db::get_stay(&conn, id).unwrap();
            assert_eq!(stay.status, "closed");
        }
    }

    #[test]
    fn replay_is_skipped_and_does_not_change_local() {
        let conn = db();
        seed_outbox(&conn, 2);
        let client = FakeClient::new();
        drain(&conn, &client, "dev").unwrap();
        conn.execute("UPDATE sync_outbox SET status='pending'", []).unwrap();
        drain(&conn, &client, "dev").unwrap();
        let last = client.apply_calls.lock().unwrap().last().cloned().unwrap();
        // Fake reports skipped for known ids; rows go back to sent.
        assert_eq!(status_count(&conn, "sent"), 2);
        assert_eq!(last.len(), 2);
    }
}
