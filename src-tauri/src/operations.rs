//! Shared local/LAN operational boundary. Commit effects, result and audit together.
use crate::{
    auth, db,
    error::{AppError, AppResult},
    models::*,
    service,
    sync::outbox::{self, Entity, OutboxOp},
    AppState,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const MUTATIONS: &[&str] = &[
    "check_in",
    "check_out",
    "add_charge",
    "add_product_charge",
    "delete_charge",
    "set_room_status",
    "convert_to_overnight",
    "create_reservation",
    "set_reservation_status",
    "check_in_reservation",
    "update_product_stock",
    "save_price_rules",
];
pub fn decode<T: DeserializeOwned>(value: Value) -> AppResult<T> {
    serde_json::from_value(value)
        .map_err(|_| AppError::msg("Los datos de la solicitud no son válidos"))
}
pub fn encode<T: Serialize>(value: T) -> AppResult<Value> {
    serde_json::to_value(value).map_err(|_| AppError::storage("No se pudo preparar el resultado"))
}
pub fn field<T: DeserializeOwned>(args: &Value, key: &str) -> AppResult<T> {
    decode(args[key].clone())
}
pub fn digest(value: &Value) -> String {
    hex::encode(Sha256::digest(value.to_string().as_bytes()))
}
pub fn revision(conn: &Connection) -> AppResult<i64> {
    Ok(
        conn.query_row("SELECT revision FROM local_revision WHERE id=1", [], |r| {
            r.get(0)
        })?,
    )
}
pub fn entity_version(conn: &Connection, table: &str, id: i64) -> AppResult<i64> {
    if !matches!(table, "rooms" | "stays" | "reservations") {
        return Err(AppError::msg("Entidad inválida"));
    }
    conn.query_row(
        &format!("SELECT operational_version FROM {table} WHERE id=?1"),
        [id],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("La ficha ya no existe"))
}

pub fn quote(conn: &Connection, stay_id: i64) -> AppResult<AccountQuote> {
    let bill = service::preview_bill(conn, stay_id)?;
    let version = entity_version(conn, "stays", stay_id)?;
    let token = quote_token(conn, stay_id, &bill)?;
    Ok(AccountQuote {
        bill,
        version,
        token,
    })
}
pub fn quote_token(conn: &Connection, stay_id: i64, bill: &BillPreview) -> AppResult<String> {
    Ok(digest(
        &json!({ "version": entity_version(conn, "stays", stay_id)?, "stay_id": stay_id,
        "lines": bill.lines, "total": bill.total_cents, "kind": bill.applied_kind, "tax": bill.tax_percent }),
    ))
}

struct Transaction<'a> {
    conn: &'a mut Connection,
    committed: bool,
}
impl<'a> Transaction<'a> {
    fn begin(conn: &'a mut Connection) -> AppResult<Self> {
        conn.execute_batch("BEGIN IMMEDIATE")?;
        Ok(Self {
            conn,
            committed: false,
        })
    }
    fn commit(mut self) -> AppResult<()> {
        self.conn.execute_batch("COMMIT")?;
        self.committed = true;
        Ok(())
    }
}
impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.conn.execute_batch("ROLLBACK");
        }
    }
}

pub fn execute(
    state: &AppState,
    token: Option<&str>,
    station: &str,
    command: &str,
    args: Value,
) -> AppResult<Value> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| AppError::storage("Base no disponible"))?;
    {
        let config = state.device.lock().unwrap();
        if station != config.station_id {
            if !config.lan_enabled
                || args["_database_generation"].as_str()
                    != Some(config.database_generation.as_str())
            {
                return Err(AppError::new(
                    crate::error::ErrorCode::RecoveryRequired,
                    "La conexión o la base principal cambió. Revisá la operación",
                ));
            }
            let active: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM lan_stations WHERE id=?1 AND active=1)",
                [station],
                |r| r.get(0),
            )?;
            if !active {
                return Err(AppError::forbidden("El puesto ya no está autorizado"));
            }
        }
    }
    let mut sessions = state
        .auth
        .lock()
        .map_err(|_| AppError::storage("Sesiones no disponibles"))?;
    let user = auth::require_on(
        &conn,
        &mut sessions,
        token.ok_or_else(|| AppError::session_expired("Iniciá sesión"))?,
        false,
    )?;
    drop(sessions);
    execute_on(&mut conn, &user, station, command, args)
}

pub fn execute_on(
    conn: &mut Connection,
    user: &SessionUser,
    station: &str,
    command: &str,
    mut args: Value,
) -> AppResult<Value> {
    if !MUTATIONS.contains(&command) {
        return Err(AppError::forbidden("Operación no permitida"));
    }
    let id = args["operation_id"]
        .as_str()
        .or_else(|| args["payload"]["operation_id"].as_str())
        .ok_or_else(|| AppError::msg("Falta el identificador de operación"))?
        .to_string();
    uuid::Uuid::parse_str(&id).map_err(|_| AppError::msg("Identificador de operación inválido"))?;
    let actor_uid = db::uid_of(conn, "users", user.id)?;
    if let Some(expected) = args["_actor_uid"].as_str() {
        if expected != actor_uid {
            return Err(AppError::new(
                crate::error::ErrorCode::RecoveryRequired,
                "La solicitud pertenece a otra identidad de usuario",
            ));
        }
    }
    let hash = digest(&json!({"command":command,"args":args}));
    let tx = Transaction::begin(conn)?;
    let current: Option<(String, String)> = tx
        .conn
        .query_row(
            "SELECT username,role FROM users WHERE id=?1 AND uid=?2 AND active=1",
            params![user.id, actor_uid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let role = if user.role.is_admin() {
        "admin"
    } else {
        "recepcion"
    };
    if current
        .as_ref()
        .map(|(name, r)| (name.as_str(), r.as_str()))
        != Some((user.username.as_str(), role))
    {
        return Err(AppError::session_expired(
            "El usuario cambió. Volvé a ingresar",
        ));
    }
    if let Some((owner, original_station, original_hash, result)) = tx.conn.query_row(
        "SELECT actor_uid,station_id,request_hash,result_json FROM operation_results WHERE operation_id=?1", [&id],
        |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?))).optional()? {
        if owner != actor_uid || station != original_station || hash != original_hash {
            return Err(AppError::conflict("Ese identificador corresponde a otra solicitud"));
        }
        let value = serde_json::from_str(&result).map_err(|_| AppError::storage("No se pudo recuperar la operación"))?;
        tx.commit()?;
        return Ok(value);
    }
    let actor = service::Actor::from(user);
    if matches!(command, "update_product_stock" | "save_price_rules") && !user.role.is_admin() {
        return Err(AppError::forbidden(
            "Esta operación requiere administración",
        ));
    }
    let body = if args["payload"].is_object() {
        &args["payload"]
    } else {
        &args
    };
    let expected = body["expected_version"].as_i64();
    if expected.is_some_and(|v| v < 0) {
        return Err(AppError::msg("La versión esperada no puede ser negativa"));
    }
    let target = match command {
        "check_in" | "set_room_status" => Some(("rooms", field::<i64>(body, "room_id")?)),
        "set_reservation_status" | "check_in_reservation" => {
            Some(("reservations", field::<i64>(body, "reservation_id")?))
        }
        "check_out" | "convert_to_overnight" => Some(("stays", field::<i64>(body, "stay_id")?)),
        _ => None,
    };
    if let (Some((table, entity)), Some(expected)) = (target, expected) {
        if entity_version(tx.conn, table, entity)? != expected {
            return Err(AppError::conflict(
                "La ficha cambió en otro puesto. Recargá antes de continuar",
            ));
        }
    }
    let before: i64 =
        tx.conn
            .query_row("SELECT COALESCE(MAX(id),0) FROM sync_outbox", [], |r| {
                r.get(0)
            })?;
    if let Some(payload) = args.get_mut("payload").and_then(Value::as_object_mut) {
        payload.insert("operation_id".into(), json!(id));
    }
    let result = match command {
        "check_in" => {
            let mut p: CheckInPayload = field(&args, "payload")?;
            p.username = Some(user.username.clone());
            encode(service::check_in_on(tx.conn, p)?)?
        }
        "check_out" => {
            let mut p: CheckOutPayload = field(&args, "payload")?;
            p.username = Some(user.username.clone());
            p.print = false;
            let confirmation = p.quote_token.as_deref().ok_or_else(|| {
                AppError::conflict("Revisá la cuenta antes de confirmar el cierre")
            })?;
            let (stay, bill) = service::check_out_confirmed(tx.conn, &p, Some(confirmation))?;
            encode(CheckOutResult {
                stay,
                bill,
                print_error: None,
            })?
        }
        "add_charge" => encode(service::add_charge(
            tx.conn,
            &actor,
            field(&args, "payload")?,
        )?)?,
        "add_product_charge" => encode(service::add_product_charge_by(
            tx.conn,
            field(&args, "payload")?,
            &user.username,
        )?)?,
        "delete_charge" => {
            service::delete_charge(tx.conn, &actor, field(&args, "charge_id")?)?;
            Value::Null
        }
        "set_room_status" => encode(service::set_room_status(
            tx.conn,
            field(&args, "room_id")?,
            field(&args, "status")?,
        )?)?,
        "convert_to_overnight" => encode(service::convert_to_overnight(
            tx.conn,
            field(&args, "stay_id")?,
        )?)?,
        "create_reservation" => encode(service::create_reservation_on(
            tx.conn,
            field(&args, "payload")?,
        )?)?,
        "set_reservation_status" => encode(service::set_reservation_status(
            tx.conn,
            field(&args, "reservation_id")?,
            field(&args, "status")?,
        )?)?,
        "check_in_reservation" => encode(service::check_in_reservation(
            tx.conn,
            field(&args, "reservation_id")?,
            Some(user.username.clone()),
        )?)?,
        "update_product_stock" => encode(crate::stock::update(
            tx.conn,
            &user.username,
            field(&args, "payload")?,
        )?)?,
        "save_price_rules" => encode(crate::pricing::save(tx.conn, field(&args, "rules")?)?)?,
        _ => unreachable!(),
    };
    let now = db::now_rfc3339();
    let audit_id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, format!("audit:{id}").as_bytes())
        .to_string();
    let entity_id = result["stay"]["id"]
        .as_i64()
        .or_else(|| result["id"].as_i64())
        .or_else(|| args["stay_id"].as_i64())
        .or_else(|| args["charge_id"].as_i64())
        .or_else(|| args["payload"]["product_id"].as_i64());
    let total = if command == "check_out" {
        result["bill"]["total_cents"].as_i64()
    } else {
        None
    };
    tx.conn.execute(
        "INSERT INTO operational_audit VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            audit_id,
            id,
            actor_uid,
            user.username,
            station,
            command,
            entity_id,
            total,
            now
        ],
    )?;
    let audit = json!({"uid":audit_id,"operation_id":id,"actor_uid":actor_uid,"username":user.username,
        "station_id":station,"command":command,"entity_id":entity_id,"closed_total_cents":total,"created_at":now});
    outbox::enqueue(
        tx.conn,
        &audit_id,
        &[OutboxOp::upsert(Entity::Audit, audit_id.clone(), audit)],
    )?;
    tx.conn.execute(
        "UPDATE sync_outbox SET root_operation_id=?1 WHERE id>?2",
        params![id, before],
    )?;
    tx.conn.execute(
        "INSERT INTO operation_results VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            id,
            actor_uid,
            station,
            command,
            hash,
            result.to_string(),
            now
        ],
    )?;
    tx.commit()?;
    Ok(result)
}

pub fn result(
    conn: &Connection,
    user: &SessionUser,
    station: &str,
    id: &str,
) -> AppResult<Option<Value>> {
    let uid = db::uid_of(conn, "users", user.id)?;
    let raw: Option<String> = conn.query_row("SELECT result_json FROM operation_results WHERE operation_id=?1 AND actor_uid=?2 AND station_id=?3",params![id,uid,station],|r|r.get(0)).optional()?;
    raw.map(|s| serde_json::from_str(&s).map_err(|_| AppError::storage("Resultado inválido")))
        .transpose()
}

pub fn activity(
    conn: &Connection,
    user: &SessionUser,
    from: &str,
    to: &str,
    user_uid: Option<&str>,
) -> AppResult<Value> {
    let start = chrono::DateTime::parse_from_rfc3339(from)
        .map_err(|_| AppError::msg("Inicio inválido"))?
        .with_timezone(&chrono::Utc);
    let end = chrono::DateTime::parse_from_rfc3339(to)
        .map_err(|_| AppError::msg("Fin inválido"))?
        .with_timezone(&chrono::Utc);
    if end <= start || end - start > chrono::Duration::days(366) {
        return Err(AppError::msg("Elegí un rango de hasta 366 días"));
    }
    let own = db::uid_of(conn, "users", user.id)?;
    let selected = if user.role.is_admin() {
        user_uid
    } else {
        Some(own.as_str())
    };
    let mut stmt=conn.prepare("SELECT uid,actor_uid,username,station_id,command,entity_id,closed_total_cents,created_at FROM operational_audit WHERE julianday(created_at)>=julianday(?1) AND julianday(created_at)<julianday(?2) AND (?3 IS NULL OR actor_uid=?3) ORDER BY created_at,uid")?;
    let mut rows=stmt.query_map(params![start.to_rfc3339(),end.to_rfc3339(),selected],|r|Ok(json!({"uid":r.get::<_,String>(0)?,"actor_uid":r.get::<_,String>(1)?,"username":r.get::<_,String>(2)?,"station_id":r.get::<_,String>(3)?,"command":r.get::<_,String>(4)?,"entity_id":r.get::<_,Option<i64>>(5)?,"closed_total_cents":r.get::<_,Option<i64>>(6)?,"created_at":r.get::<_,String>(7)?})))?.collect::<Result<Vec<_>,_>>()?;
    if user.role.is_admin() && selected.is_none() {
        let mut legacy=conn.prepare("SELECT uid,id,check_out_at,closed_total_cents FROM stays WHERE status='closed' AND julianday(check_out_at)>=julianday(?1) AND julianday(check_out_at)<julianday(?2) AND NOT EXISTS(SELECT 1 FROM operational_audit a WHERE a.command='check_out' AND a.entity_id=stays.id)")?;
        let old=legacy.query_map(params![start.to_rfc3339(),end.to_rfc3339()],|r|Ok(json!({"uid":format!("legacy:{}",r.get::<_,String>(0)?),"actor_uid":"legacy","username":"Sin identidad verificable (histórico)","station_id":"","command":"check_out","entity_id":r.get::<_,i64>(1)?,"closed_total_cents":r.get::<_,Option<i64>>(3)?,"created_at":r.get::<_,String>(2)?})))?.collect::<Result<Vec<_>,_>>()?;
        rows.extend(old);
        rows.sort_by_key(|r| {
            chrono::DateTime::parse_from_rfc3339(r["created_at"].as_str().unwrap_or("")).ok()
        });
    }
    let total = rows
        .iter()
        .try_fold(0i64, |sum, r| {
            sum.checked_add(r["closed_total_cents"].as_i64().unwrap_or(0))
        })
        .ok_or_else(|| AppError::storage("Total fuera de rango"))?;
    let count = rows.iter().filter(|r| r["command"] == "check_out").count();
    Ok(
        json!({"from":start.to_rfc3339(),"to":end.to_rfc3339(),"timezone":"America/Asuncion","operations":rows,"closed_accounts":count,"closed_total_cents":total}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    fn setup(path: &std::path::Path) -> Connection {
        let conn = db::open(path).unwrap();
        conn.execute("INSERT INTO users(username,password_hash,role) VALUES ('ana','unused','admin'),('beto','unused','recepcion')",[]).unwrap();
        conn
    }
    fn ana() -> SessionUser {
        SessionUser {
            id: 1,
            username: "ana".into(),
            role: Role::Admin,
        }
    }
    fn beto() -> SessionUser {
        SessionUser {
            id: 2,
            username: "beto".into(),
            role: Role::Recepcion,
        }
    }
    fn entry(room: i64) -> Value {
        json!({"operation_id":uuid::Uuid::new_v4().to_string(),"payload":{"room_id":room,"guest_name":"Prueba LAN","rate_plan_id":1,"expected_hours":1}})
    }
    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }
    fn close(conn: &Connection, stay: i64) -> Value {
        let q = quote(conn, stay).unwrap();
        json!({"operation_id":uuid::Uuid::new_v4().to_string(),"payload":{"stay_id":stay,"print":false,"expected_version":q.version,"quote_token":q.token}})
    }
    #[test]
    fn replay_survives_reopen_and_binds_actor_station_and_content() {
        let dir = std::env::temp_dir().join(format!("nightdesk-replay-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");
        let mut conn = setup(&path);
        let args = entry(5);
        let first = execute_on(&mut conn, &ana(), "A", "check_in", args.clone()).unwrap();
        let outbox = count(&conn, "sync_outbox");
        drop(conn);
        let mut conn = db::open(&path).unwrap();
        assert_eq!(
            first,
            execute_on(&mut conn, &ana(), "A", "check_in", args.clone()).unwrap()
        );
        assert_eq!(count(&conn, "stays"), 1);
        assert_eq!(count(&conn, "operational_audit"), 1);
        assert_eq!(count(&conn, "sync_outbox"), outbox);
        assert_eq!(
            execute_on(&mut conn, &beto(), "A", "check_in", args.clone())
                .unwrap_err()
                .code(),
            ErrorCode::Conflict
        );
        assert_eq!(
            execute_on(&mut conn, &ana(), "B", "check_in", args.clone())
                .unwrap_err()
                .code(),
            ErrorCode::Conflict
        );
        let mut changed = args;
        changed["payload"]["guest_name"] = json!("Otro");
        assert_eq!(
            execute_on(&mut conn, &ana(), "A", "check_in", changed)
                .unwrap_err()
                .code(),
            ErrorCode::Conflict
        );
        drop(conn);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn concurrent_checkins_have_exactly_one_winner() {
        let dir = std::env::temp_dir().join(format!("nightdesk-race-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");
        drop(setup(&path));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = [ana(), beto()]
            .into_iter()
            .map(|user| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut conn = db::open(&path).unwrap();
                    barrier.wait();
                    execute_on(&mut conn, &user, &user.username, "check_in", entry(5)).is_ok()
                })
            })
            .collect();
        assert_eq!(
            threads
                .into_iter()
                .map(|t| t.join().unwrap() as u32)
                .sum::<u32>(),
            1
        );
        let conn = db::open(&path).unwrap();
        assert_eq!(count(&conn, "stays"), 1);
        assert_eq!(count(&conn, "guests"), 1);
        assert_eq!(count(&conn, "operation_results"), 1);
        drop(conn);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn receipt_result_audit_and_outbox_roll_back_together() {
        let mut conn = setup(std::path::Path::new(":memory:"));
        let args = entry(5);
        conn.execute_batch("CREATE TRIGGER injected BEFORE INSERT ON operation_results BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(execute_on(&mut conn, &ana(), "A", "check_in", args.clone()).is_err());
        for table in [
            "stays",
            "guests",
            "operational_audit",
            "sync_outbox",
            "operation_results",
        ] {
            assert_eq!(count(&conn, table), 0, "{table}");
        }
        assert_eq!(db::get_room(&conn, 5).unwrap().status, "available");
        conn.execute_batch("DROP TRIGGER injected;").unwrap();
        let stay = execute_on(&mut conn, &ana(), "A", "check_in", args).unwrap()["id"]
            .as_i64()
            .unwrap();
        let close = close(&conn, stay);
        let before = count(&conn, "sync_outbox");
        conn.execute_batch("CREATE TRIGGER injected BEFORE INSERT ON operational_audit WHEN NEW.command='check_out' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(execute_on(&mut conn, &beto(), "B", "check_out", close).is_err());
        assert_eq!(db::get_stay(&conn, stay).unwrap().status, "open");
        assert_eq!(count(&conn, "receipt_snapshots"), 0);
        assert_eq!(count(&conn, "sync_outbox"), before);
    }
    #[test]
    fn stale_quote_rejected_then_single_close_and_frozen_replay() {
        let mut conn = setup(std::path::Path::new(":memory:"));
        let stay = execute_on(&mut conn, &ana(), "A", "check_in", entry(5)).unwrap()["id"]
            .as_i64()
            .unwrap();
        let old = close(&conn, stay);
        let charge = json!({"operation_id":uuid::Uuid::new_v4().to_string(),"payload":{"stay_id":stay,"kind":"surcharge","description":"Agua","amount_cents":5000}});
        execute_on(&mut conn, &ana(), "A", "add_charge", charge).unwrap();
        assert_eq!(
            execute_on(&mut conn, &beto(), "B", "check_out", old)
                .unwrap_err()
                .code(),
            ErrorCode::Conflict
        );
        let current = close(&conn, stay);
        let result = execute_on(&mut conn, &beto(), "B", "check_out", current.clone()).unwrap();
        assert_eq!(
            result,
            execute_on(&mut conn, &beto(), "B", "check_out", current.clone()).unwrap()
        );
        let mut another = current;
        another["operation_id"] = json!(uuid::Uuid::new_v4().to_string());
        assert!(execute_on(&mut conn, &ana(), "A", "check_out", another).is_err());
        assert_eq!(count(&conn, "receipt_snapshots"), 1);
        assert_eq!(count(&conn, "payments"), 0);
        assert_eq!(db::get_room(&conn, 5).unwrap().status, "dirty");
        assert_eq!(
            conn.query_row::<String, _, _>(
                "SELECT username FROM operational_audit WHERE command='check_out'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
            "beto"
        );
        let before = crate::printing::document(&conn, stay).unwrap();
        conn.execute(
            "UPDATE settings SET value='Cambiado' WHERE key='business_name'",
            [],
        )
        .unwrap();
        assert_eq!(before, crate::printing::document(&conn, stay).unwrap());
    }
    #[test]
    fn lost_sale_response_does_not_reduce_stock_or_duplicate_charges_twice() {
        let mut conn = setup(std::path::Path::new(":memory:"));
        let stay = execute_on(&mut conn, &ana(), "A", "check_in", entry(5)).unwrap()["id"]
            .as_i64()
            .unwrap();
        let product: i64 = conn
            .query_row("SELECT id FROM products WHERE active=1 LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        execute_on(&mut conn,&ana(),"A","update_product_stock",json!({"operation_id":uuid::Uuid::new_v4().to_string(),"payload":{"product_id":product,"mode":"set","quantity":10}})).unwrap();
        let request = json!({"operation_id":uuid::Uuid::new_v4().to_string(),"payload":{"stay_id":stay,"product_id":product}});
        let first = execute_on(
            &mut conn,
            &beto(),
            "B",
            "add_product_charge",
            request.clone(),
        )
        .unwrap();
        let outbox = count(&conn, "sync_outbox");
        assert_eq!(
            first,
            execute_on(&mut conn, &beto(), "B", "add_product_charge", request).unwrap()
        );
        assert_eq!(count(&conn, "charges"), 1);
        assert_eq!(count(&conn, "sync_outbox"), outbox);
        assert_eq!(
            conn.query_row::<i64, _, _>(
                "SELECT quantity FROM product_stock WHERE product_id=?1",
                [product],
                |r| r.get(0)
            )
            .unwrap(),
            9
        );
        assert_eq!(
            conn.query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM stock_movements WHERE reason='sale'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn quote_checks_amount_even_without_an_account_revision_change() {
        let mut conn = setup(std::path::Path::new(":memory:"));
        let stay = execute_on(&mut conn, &ana(), "A", "check_in", entry(5)).unwrap()["id"]
            .as_i64()
            .unwrap();
        let old = close(&conn, stay);
        conn.execute(
            "UPDATE stays SET check_in_at=?1 WHERE id=?2",
            params![
                (chrono::Utc::now() - chrono::Duration::hours(8)).to_rfc3339(),
                stay
            ],
        )
        .unwrap();
        assert_eq!(
            execute_on(&mut conn, &beto(), "B", "check_out", old)
                .unwrap_err()
                .code(),
            ErrorCode::Conflict
        );
        assert_eq!(db::get_stay(&conn, stay).unwrap().status, "open");
    }
    #[test]
    fn stale_room_and_reservation_state_cannot_overwrite_newer_state() {
        let mut conn = setup(std::path::Path::new(":memory:"));
        let version = entity_version(&conn, "rooms", 5).unwrap();
        execute_on(&mut conn,&ana(),"A","set_room_status",json!({"operation_id":uuid::Uuid::new_v4().to_string(),"room_id":5,"status":"blocked","expected_version":version})).unwrap();
        let err=execute_on(&mut conn,&beto(),"B","set_room_status",json!({"operation_id":uuid::Uuid::new_v4().to_string(),"room_id":5,"status":"available","expected_version":version})).unwrap_err();
        assert_eq!(err.code(), ErrorCode::Conflict);
    }
    #[test]
    fn activity_filters_own_identity_and_survives_user_deletion_across_midnight() {
        let mut conn = setup(std::path::Path::new(":memory:"));
        let stay = execute_on(&mut conn, &ana(), "A", "check_in", entry(5)).unwrap()["id"]
            .as_i64()
            .unwrap();
        let close = close(&conn, stay);
        execute_on(&mut conn, &beto(), "B", "check_out", close).unwrap();
        conn.execute("UPDATE operational_audit SET created_at=CASE command WHEN 'check_in' THEN '2026-10-03T23:50:00-03:00' ELSE '2026-10-04T00:10:00-03:00' END",[]).unwrap();
        let own = activity(
            &conn,
            &beto(),
            "2026-10-03T23:00:00-03:00",
            "2026-10-04T01:00:00-03:00",
            None,
        )
        .unwrap();
        assert_eq!(own["operations"].as_array().unwrap().len(), 1);
        assert_eq!(own["closed_accounts"], 1);
        conn.execute("DELETE FROM users WHERE id=2", []).unwrap();
        let all = activity(
            &conn,
            &ana(),
            "2026-10-03T23:00:00-03:00",
            "2026-10-04T01:00:00-03:00",
            None,
        )
        .unwrap();
        assert_eq!(all["operations"].as_array().unwrap().len(), 2);
        assert_eq!(all["operations"][1]["username"], "beto");
    }
    #[test]
    fn revoked_or_demoted_user_cannot_commit_and_own_report_cannot_impersonate() {
        let mut conn = setup(std::path::Path::new(":memory:"));
        conn.execute("UPDATE users SET active=0 WHERE id=2", [])
            .unwrap();
        assert_eq!(
            execute_on(&mut conn, &beto(), "B", "check_in", entry(5))
                .unwrap_err()
                .code(),
            ErrorCode::SessionExpired
        );
        assert_eq!(count(&conn, "stays"), 0);
    }
}
