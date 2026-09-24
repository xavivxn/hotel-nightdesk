//! I11 catalog protocol. Network is called without holding SQLite's mutex.
use crate::{db, error::{AppError, AppResult}, service::{Actor, authorize, Operation}};
use rusqlite::{Connection, OptionalExtension, params, types::Value as SqlValue};
use serde_json::{Value, json};
use std::path::Path;

pub trait RemoteClient {
    fn write(&self, request: &Value) -> AppResult<Value>;
}

pub fn columns(entity: &str) -> AppResult<&'static [&'static str]> {
    match entity {
        "rooms" => Ok(&["number","room_type","floor","notes","active"]),
        "rate_plans" => Ok(&["name","kind","base_amount_cents","extra_hour_cents","included_hours","grace_minutes","night_cutoff_hour","active"]),
        "products" => Ok(&["name","category","price_cents","active","sort_order"]),
        "app_users" => Ok(&["username","password_hash","role","active"]),
        _ => Err(AppError::msg("Entidad de catálogo inválida")),
    }
}
fn table(entity: &str) -> &str { if entity == "app_users" { "users" } else { entity } }

pub fn prepare(conn: &Connection, actor: &Actor, entity: &str, mut payload: Value, operation: Option<String>) -> AppResult<Value> {
    authorize(actor, Operation::SaveSettings)?;
    let op = operation.or_else(|| payload["operation_id"].as_str().map(str::to_owned))
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    uuid::Uuid::parse_str(&op).map_err(|_| AppError::msg("operation_id debe ser UUID"))?;
    if entity == "settings" {
        let mut values = serde_json::Map::new();
        let mut versions = serde_json::Map::new();
        let current = serde_json::to_value(db::load_settings(conn)?).map_err(|_| AppError::msg("Ajustes inválidos"))?;
        for key in super::settings::BUSINESS_SETTING_KEYS {
            if let Some(value) = payload.get(*key) {
                if current.get(*key) == Some(value) { continue; }
                values.insert(key.to_string(), json!(value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string())));
                let version: i64 = conn.query_row("SELECT version FROM catalog_setting_versions WHERE key=?1", [key], |r| r.get(0)).optional()?.unwrap_or(0);
                versions.insert(key.to_string(), json!(payload["catalog_versions"][*key].as_i64().unwrap_or(version)));
            }
        }
        payload = json!({"values":values,"versions":versions});
    } else {
        let fields = columns(entity)?;
        let id = payload["id"].as_i64();
        if entity == "rooms" && payload.get("active").is_none() {
            payload["active"]=json!(if let Some(id)=id { db::get_room(conn,id)?.active } else { true });
        }
        let mut data = serde_json::Map::new();
        for field in fields {
            if let Some(value) = payload.get(*field) { data.insert(field.to_string(), value.clone()); }
        }
        let (uid, version) = if let Some(id) = id {
            let sql = format!("SELECT uid,version FROM {} WHERE id=?1", table(entity));
            conn.query_row(&sql, [id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))
                .optional()?.ok_or_else(|| AppError::not_found("Ficha de catálogo no encontrada"))?
        } else { (uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, op.as_bytes()).to_string(),0) };
        let expected = payload["expected_version"].as_i64().unwrap_or(version);
        if expected < 0 { return Err(AppError::msg("expected_version inválida")); }
        data.insert("uid".into(), json!(uid));
        data.insert("expected_version".into(), json!(expected));
        data.insert("local_id".into(), json!(id));
        payload = Value::Object(data);
    }
    Ok(json!({"p_entity":entity,"p_payload":payload,"p_operation_id":op,"p_actor":actor.user_id.to_string()}))
}

fn sql_value(value: &Value) -> AppResult<SqlValue> {
    match value {
        Value::Null => Ok(SqlValue::Null),
        Value::Bool(v) => Ok(SqlValue::Integer(i64::from(*v))),
        Value::Number(v) => v.as_i64().map(SqlValue::Integer).ok_or_else(|| AppError::msg("Se esperaba un entero")),
        Value::String(v) => Ok(SqlValue::Text(v.clone())),
        _ => Err(AppError::msg("Valor de catálogo inválido")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyStatus {
    Applied(i64),
    Unchanged(i64),
}

impl ApplyStatus {
    pub fn id(self) -> i64 {
        match self {
            Self::Applied(id) | Self::Unchanged(id) => id,
        }
    }

    pub fn changed(self) -> bool {
        matches!(self, Self::Applied(_))
    }
}

/// Shared pull applicator: immutable operational fields never enter the whitelist.
pub fn apply(conn: &Connection, entity: &str, row: &Value) -> AppResult<i64> {
    Ok(apply_row(conn, entity, row, false)?.id())
}

/// Bootstrap overwrite: apply even if the local version is newer. Still never writes `rooms.status`.
#[allow(dead_code)]
pub fn apply_forced(conn: &Connection, entity: &str, row: &Value) -> AppResult<i64> {
    Ok(apply_row(conn, entity, row, true)?.id())
}

pub fn apply_status(conn: &Connection, entity: &str, row: &Value, force: bool) -> AppResult<ApplyStatus> {
    apply_row(conn, entity, row, force)
}

fn apply_row(conn: &Connection, entity: &str, row: &Value, force: bool) -> AppResult<ApplyStatus> {
    let fields = columns(entity)?;
    let uid = row["uid"].as_str().ok_or_else(|| AppError::msg("Respuesta sin uid"))?;
    uuid::Uuid::parse_str(uid).map_err(|_| AppError::msg("Respuesta con uid inválido"))?;
    let version = row["version"].as_i64().filter(|v| *v>0).ok_or_else(|| AppError::msg("Respuesta sin versión"))?;
    let table = table(entity);
    let current: Option<(i64,i64)> = conn.query_row(&format!("SELECT id,version FROM {table} WHERE uid=?1"), [uid], |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
    if let Some((id,old)) = current { if !force && old >= version { return Ok(ApplyStatus::Unchanged(id)); } }
    let mut names = fields.to_vec();
    names.extend(["uid","version","updated_at"]);
    let mut vals = Vec::new();
    for name in &names {
        vals.push(sql_value(row.get(*name).ok_or_else(|| AppError::msg(format!("Respuesta incompleta: {name}")))?)?);
    }
    if let Some((id,_)) = current {
        vals.push(SqlValue::Integer(id));
        let assignments = names.iter().map(|f| format!("{f}=?")).collect::<Vec<_>>().join(",");
        conn.execute(&format!("UPDATE {table} SET {assignments} WHERE id=?"), rusqlite::params_from_iter(vals))?;
        Ok(ApplyStatus::Applied(id))
    } else {
        if table == "rooms" {
            names.extend(["status","created_at"]);
            vals.push(SqlValue::Text("available".into()));
            vals.push(SqlValue::Text(db::now_rfc3339()));
        }
        let placeholders=vec!["?";names.len()].join(",");
        conn.execute(&format!("INSERT INTO {table} ({}) VALUES ({placeholders})",names.join(",")),rusqlite::params_from_iter(vals))?;
        Ok(ApplyStatus::Applied(conn.last_insert_rowid()))
    }
}

/// If the remote user uid is new but the username already exists, reuse the local row.
pub fn adopt_user_uid(conn: &Connection, row: &Value) -> AppResult<()> {
    let uid = row["uid"].as_str().ok_or_else(|| AppError::msg("Respuesta sin uid"))?;
    let username = row["username"].as_str().ok_or_else(|| AppError::msg("Respuesta sin usuario"))?;
    let by_uid: Option<i64> = conn
        .query_row("SELECT id FROM users WHERE uid=?1", [uid], |r| r.get(0))
        .optional()?;
    if by_uid.is_some() {
        return Ok(());
    }
    let by_name: Option<i64> = conn
        .query_row(
            "SELECT id FROM users WHERE username=?1 COLLATE NOCASE",
            [username],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = by_name {
        conn.execute("UPDATE users SET uid=?1 WHERE id=?2", params![uid, id])?;
    }
    Ok(())
}

pub fn apply_setting_row(conn: &Connection, row: &Value, force: bool) -> AppResult<bool> {
    let key = row["key"].as_str().unwrap_or("");
    if !super::settings::is_business_key(key) {
        return Err(AppError::forbidden("Clave de equipo rechazada"));
    }
    let version = row["version"].as_i64().filter(|v| *v > 0).ok_or_else(|| AppError::msg("Versión inválida"))?;
    let current: i64 = conn
        .query_row("SELECT version FROM catalog_setting_versions WHERE key=?1", [key], |r| r.get(0))
        .optional()?
        .unwrap_or(0);
    if !force && version <= current {
        return Ok(false);
    }
    db::upsert_setting(
        conn,
        key,
        row["value"].as_str().ok_or_else(|| AppError::msg("Ajuste inválido"))?,
    )?;
    conn.execute(
        "INSERT INTO catalog_setting_versions VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET version=excluded.version",
        params![key, version],
    )?;
    Ok(true)
}

pub fn apply_result(conn: &mut Connection, request: &Value, result: &Value) -> AppResult<i64> {
    let entity = request["p_entity"].as_str().ok_or_else(|| AppError::msg("Entidad inválida"))?;
    let tx = conn.transaction()?;
    let id = if entity == "settings" {
        for row in result["row"].as_array().ok_or_else(|| AppError::msg("Ajustes incompletos"))? {
            apply_setting_row(&tx, row, false)?;
        }
        0
    } else {
        if result["row"]["uid"] != request["p_payload"]["uid"] { return Err(AppError::storage("Administración devolvió otra ficha; no se aplicó el cambio")); }
        apply(&tx,entity,&result["row"])?
    };
    // Audit is generated by the server, with secrets removed there and again here.
    let audit=&result["audit"];
    let clean=|v:&Value| { let mut v=v.clone(); if let Some(o)=v.as_object_mut(){o.remove("password_hash");o.remove("password");o.remove("pin_hash");} v.to_string() };
    tx.execute("INSERT OR IGNORE INTO catalog_audit VALUES (?1,?2,?3,?4,?5,?6)",params![
        request["p_operation_id"].as_str(),entity,
        audit["actor"].as_str().unwrap_or("remote"),
        audit["created_at"].as_str().ok_or_else(||AppError::msg("Auditoría incompleta"))?,
        clean(&audit["before"]),clean(&audit["after"])
    ])?;
    tx.commit()?;
    Ok(id)
}

/// Returns None for a standalone, unsynchronized installation.
pub fn configured(path: &Path) -> AppResult<bool> { crate::credentials::device_configured(path) }

pub fn execute(client: &dyn RemoteClient, request: &Value) -> AppResult<Value> {
    client.write(request)
}

/// Standalone writes and their audit commit or roll back together.
pub fn local<T: serde::Serialize>(conn: &Connection, actor: &Actor, entity: &str, write: impl FnOnce(&Connection)->AppResult<T>) -> AppResult<T> {
    authorize(actor, Operation::SaveSettings)?;
    let tx=conn.unchecked_transaction()?;
    let before = if entity=="settings" {
        serde_json::to_value(db::load_settings(&tx)?).unwrap()
    } else {
        let mut fields=columns(entity)?.iter().copied().filter(|f| *f!="password_hash").collect::<Vec<_>>();
        fields.extend(["id","version"]);
        let entries=fields.iter().map(|f|format!("'{f}',{f}")).collect::<Vec<_>>().join(",");
        let mut stmt=tx.prepare(&format!("SELECT json_object({entries}) FROM {}",table(entity)))?;
        let rows=stmt.query_map([],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
        Value::Array(rows.iter().map(|s|serde_json::from_str(s).unwrap()).collect())
    };
    let value=write(&tx)?;
    let mut after=serde_json::to_value(&value).map_err(|_|AppError::storage("No se pudo registrar auditoría"))?;
    let mut before=if let Some(rows)=before.as_array() { rows.iter().find(|r|r["id"]==after["id"]).cloned().unwrap_or(Value::Null) } else {before};
    for v in [&mut before,&mut after] {
        if let Some(obj)=v.as_object_mut() { obj.remove("password_hash");obj.remove("password");obj.remove("pin_hash"); }
    }
    tx.execute("INSERT INTO catalog_audit VALUES (?1,?2,?3,?4,?5,?6)",params![uuid::Uuid::new_v4().to_string(),entity,actor.user_id.to_string(),db::now_rfc3339(),before.to_string(),after.to_string()])?;
    tx.commit()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::models::Role;
 fn actor(role:Role)->Actor { Actor{user_id:1,username:"admin".into(),role} }
 fn db()->Connection { crate::db::open(Path::new(":memory:")).unwrap() }
 struct Fake { fail:bool }
 impl RemoteClient for Fake {
  fn write(&self,r:&Value)->AppResult<Value> {
   if self.fail { return Err(AppError::storage("Requiere conexión con administración")); }
   let mut row=r["p_payload"].clone();
   row["version"]=json!(2); row["updated_at"]=json!("2026-09-20T12:00:00Z");
   Ok(json!({"row":row,"audit":{"actor":"test","created_at":"2026-09-20T12:00:00Z","before":null,"after":row}}))
  }
 }
 fn room()->Value { json!({"id":1,"number":"01","room_type":"Jacuzzi","floor":1,"notes":null,"active":false,"expected_version":1}) }
 #[test] fn offline_and_forbidden_never_mutate() {
  let conn=db();
  assert!(prepare(&conn,&actor(Role::Recepcion),"rooms",room(),None).is_err());
  let request=prepare(&conn,&actor(Role::Admin),"rooms",room(),None).unwrap();
  assert!(execute(&Fake{fail:true},&request).is_err());
  assert!(crate::db::get_room(&conn,1).unwrap().active);
  assert_eq!(conn.query_row("SELECT count(*) FROM catalog_audit",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test] fn pull_preserves_occupancy_and_replay_has_one_audit() {
  let mut conn=db();
  conn.execute("UPDATE rooms SET status='occupied' WHERE id=1",[]).unwrap();
  let r=prepare(&conn,&actor(Role::Admin),"rooms",room(),None).unwrap();
  let result=execute(&Fake{fail:false},&r).unwrap();
  assert_eq!(apply_result(&mut conn,&r,&result).unwrap(),1);
  apply_result(&mut conn,&r,&result).unwrap();
  let room=crate::db::get_room(&conn,1).unwrap();
  assert!(!room.active); assert_eq!(room.status,"occupied"); assert_eq!(room.version,2);
  assert_eq!(conn.query_row("SELECT count(*) FROM catalog_audit",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
 #[test] fn invalid_audit_rolls_back_catalog() {
  let mut conn=db();
  let r=prepare(&conn,&actor(Role::Admin),"rooms",room(),None).unwrap();
  let mut result=execute(&Fake{fail:false},&r).unwrap(); result["audit"]=Value::Null;
  assert!(apply_result(&mut conn,&r,&result).is_err());
  assert!(crate::db::get_room(&conn,1).unwrap().active);
 }
 #[test] fn older_pull_cannot_overwrite_newer_version() {
  let mut conn=db();
  let r=prepare(&conn,&actor(Role::Admin),"rooms",room(),None).unwrap();
  let mut result=execute(&Fake{fail:false},&r).unwrap();
  conn.execute("UPDATE rooms SET version=5 WHERE id=1",[]).unwrap();
  result["row"]["number"]=json!("WRONG");
  apply_result(&mut conn,&r,&result).unwrap();
  assert_eq!(crate::db::get_room(&conn,1).unwrap().number,"01");
 }
 #[test] fn settings_exclude_device_and_secret_fields() {
  let conn=db();
  let r=prepare(&conn,&actor(Role::Admin),"settings",json!({"theme":"dark","printer_name":"SECRET","pin_hash":"SECRET","business_name":"Nuevo"}),None).unwrap();
  assert!(!r.to_string().contains("SECRET"));
  assert_eq!(r["p_payload"]["values"]["business_name"],"Nuevo");
 }
 #[test] fn hash_password_is_argon2id_and_verifies() {
  use argon2::{Argon2,PasswordVerifier,password_hash::PasswordHash};
  let hash=crate::service::hash_password("Prueba123").unwrap().hash;
  assert!(hash.starts_with("$argon2id$"));
  Argon2::default().verify_password(b"Prueba123",&PasswordHash::new(&hash).unwrap()).unwrap();
 }
 #[test] fn local_audit_failure_rolls_back_write() {
  let conn=db();
  conn.execute_batch("CREATE TRIGGER reject_audit BEFORE INSERT ON catalog_audit BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
  let result=local(&conn,&actor(Role::Admin),"rooms",|tx| {
   tx.execute("UPDATE rooms SET notes='changed' WHERE id=1",[])?;
   crate::db::get_room(tx,1)
  });
  assert!(result.is_err());
  assert_ne!(crate::db::get_room(&conn,1).unwrap().notes.as_deref(),Some("changed"));
 }
 #[test] fn conflicting_remote_leaves_local_unchanged() {
  struct Conflict;
  impl RemoteClient for Conflict { fn write(&self,_:&Value)->AppResult<Value> { Err(AppError::conflict("La ficha cambió")) } }
  let conn=db();
  let r=prepare(&conn,&actor(Role::Admin),"rooms",room(),None).unwrap();
  assert_eq!(execute(&Conflict,&r).unwrap_err().code(),crate::error::ErrorCode::Conflict);
  assert_eq!(crate::db::get_room(&conn,1).unwrap().version,1);
 }
 #[test] fn operation_salt_makes_user_retry_identical() {
  let op=uuid::Uuid::new_v4().to_string();
  let first=crate::service::hash_password_with_operation("Prueba123",Some(&op)).unwrap().hash;
  let second=crate::service::hash_password_with_operation("Prueba123",Some(&op)).unwrap().hash;
  assert_eq!(first,second);
 }
}
