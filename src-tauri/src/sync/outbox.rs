use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

const OUTBOX_NS: Uuid = uuid::uuid!("a3c1e8f0-2b4d-4f6a-9c8e-1d0f2a3b4c5d");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entity {
    Guest,
    Reservation,
    Stay,
    Charge,
    #[allow(dead_code)]
    Payment,
    Room,
}

impl Entity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Guest => "guest",
            Self::Reservation => "reservation",
            Self::Stay => "stay",
            Self::Charge => "charge",
            Self::Payment => "payment",
            Self::Room => "room",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Upsert,
    Delete,
}

impl OpKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutboxOp {
    pub entity: Entity,
    pub entity_uid: String,
    pub op: OpKind,
    pub payload: serde_json::Value,
}

impl OutboxOp {
    pub fn upsert(entity: Entity, entity_uid: String, payload: serde_json::Value) -> Self {
        Self {
            entity,
            entity_uid,
            op: OpKind::Upsert,
            payload,
        }
    }

    pub fn delete(entity: Entity, entity_uid: String, payload: serde_json::Value) -> Self {
        Self {
            entity,
            entity_uid,
            op: OpKind::Delete,
            payload,
        }
    }
}

pub fn resolve_operation_id(payload_op: &Option<String>) -> String {
    match payload_op
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(raw) => match Uuid::parse_str(raw) {
            Ok(uid) => uid.to_string(),
            Err(_) => Uuid::new_v5(&OUTBOX_NS, raw.as_bytes()).to_string(),
        },
        None => Uuid::new_v4().to_string(),
    }
}

fn derived_operation_id(root: &str, entity: &str, entity_uid: &str) -> String {
    let name = format!("{root}:{entity}:{entity_uid}");
    Uuid::new_v5(&OUTBOX_NS, name.as_bytes()).to_string()
}

pub fn enqueue(conn: &Connection, root_operation_id: &str, ops: &[OutboxOp]) -> AppResult<()> {
    let root = resolve_operation_id(&Some(root_operation_id.to_string()));
    let already: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sync_outbox WHERE root_operation_id = ?1 LIMIT 1",
            [&root],
            |row| row.get(0),
        )
        .optional()?;
    if already.is_some() {
        return Err(AppError::conflict("La operación ya fue registrada"));
    }
    if ops.is_empty() {
        return Ok(());
    }

    let now = crate::db::now_rfc3339();
    for (index, op) in ops.iter().enumerate() {
        let operation_id = if index == 0 {
            root.clone()
        } else {
            derived_operation_id(&root, op.entity.as_str(), &op.entity_uid)
        };
        conn.execute(
            "INSERT INTO sync_outbox (
                operation_id, root_operation_id, entity, entity_uid, op, payload, created_at, attempts, status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 'pending')",
            params![
                operation_id,
                root,
                op.entity.as_str(),
                op.entity_uid,
                op.op.as_str(),
                op.payload.to_string(),
                now
            ],
        )?;
    }
    Ok(())
}
