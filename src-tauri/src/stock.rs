//! Shop stock kept at reception. Local only (not synced to Supabase): stock is counted where the
//! fridge is. A product without a `product_stock` row is not tracked and sells without counting.
use crate::db::{self, now_rfc3339};
use crate::error::{AppError, AppResult};
use crate::models::{ProductStock, StockMovement, UpdateStockPayload};
use rusqlite::{params, Connection, OptionalExtension};

const MAX_QUANTITY: i64 = 100_000;

struct Movement<'a> {
    product_id: i64,
    delta: i64,
    quantity_after: i64,
    reason: &'a str,
    charge_id: Option<i64>,
    note: Option<&'a str>,
}

fn map_stock(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProductStock> {
    Ok(ProductStock {
        product_id: row.get(0)?,
        quantity: row.get(1)?,
        min_quantity: row.get(2)?,
        updated_at: row.get(3)?,
    })
}

pub fn list(conn: &Connection) -> AppResult<Vec<ProductStock>> {
    let mut stmt = conn.prepare(
        "SELECT product_id, quantity, min_quantity, updated_at FROM product_stock ORDER BY product_id",
    )?;
    let rows = stmt.query_map([], map_stock)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn get(conn: &Connection, product_id: i64) -> AppResult<Option<ProductStock>> {
    conn.query_row(
        "SELECT product_id, quantity, min_quantity, updated_at FROM product_stock WHERE product_id = ?1",
        [product_id],
        map_stock,
    )
    .optional()
    .map_err(Into::into)
}

fn upsert(conn: &Connection, product_id: i64, quantity: i64, min_quantity: i64, now: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO product_stock (product_id, quantity, min_quantity, updated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(product_id) DO UPDATE SET
           quantity = excluded.quantity, min_quantity = excluded.min_quantity, updated_at = excluded.updated_at",
        params![product_id, quantity, min_quantity, now],
    )?;
    Ok(())
}

fn record(conn: &Connection, movement: &Movement<'_>, username: &str, now: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO stock_movements (product_id, delta, quantity_after, reason, charge_id, username, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            movement.product_id,
            movement.delta,
            movement.quantity_after,
            movement.reason,
            movement.charge_id,
            Some(username).filter(|name| !name.is_empty()),
            movement.note,
            now
        ],
    )?;
    Ok(())
}

/// One unit sold from the room shop. Stock may go negative on purpose: refusing a sale at night
/// because a restock was not loaded costs more than the discrepancy, which stays visible in red.
pub fn record_sale(conn: &Connection, product_id: i64, charge_id: i64, username: &str) -> AppResult<()> {
    let Some(stock) = get(conn, product_id)? else {
        return Ok(());
    };
    let now = now_rfc3339();
    let after = stock.quantity - 1;
    conn.execute(
        "UPDATE product_stock SET quantity = ?1, updated_at = ?2 WHERE product_id = ?3",
        params![after, now, product_id],
    )?;
    record(
        conn,
        &Movement { product_id, delta: -1, quantity_after: after, reason: "sale", charge_id: Some(charge_id), note: None },
        username,
        &now,
    )
}

/// A deleted shop charge puts its unit back, only once.
pub fn record_void(conn: &Connection, charge_id: i64, username: &str) -> AppResult<()> {
    let product_id: Option<i64> = conn
        .query_row(
            "SELECT product_id FROM stock_movements
             WHERE charge_id = ?1 AND reason = 'sale'
               AND NOT EXISTS (SELECT 1 FROM stock_movements WHERE charge_id = ?1 AND reason = 'void')",
            [charge_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(product_id) = product_id else {
        return Ok(());
    };
    let Some(stock) = get(conn, product_id)? else {
        return Ok(());
    };
    let now = now_rfc3339();
    let after = stock.quantity + 1;
    conn.execute(
        "UPDATE product_stock SET quantity = ?1, updated_at = ?2 WHERE product_id = ?3",
        params![after, now, product_id],
    )?;
    record(
        conn,
        &Movement { product_id, delta: 1, quantity_after: after, reason: "void", charge_id: Some(charge_id), note: None },
        username,
        &now,
    )
}

/// Restock (`add`), physical count (`set`) or stop counting (`untrack`). Admin only.
pub fn update(conn: &mut Connection, username: &str, payload: UpdateStockPayload) -> AppResult<Option<ProductStock>> {
    db::get_product(conn, payload.product_id)?;
    if let Some(min) = payload.min_quantity {
        if !(0..=MAX_QUANTITY).contains(&min) {
            return Err(AppError::msg("El aviso de stock bajo va de 0 a 100.000 unidades"));
        }
    }
    let note = payload.note.as_deref().map(str::trim).filter(|note| !note.is_empty());
    if note.is_some_and(|note| note.chars().count() > 200) {
        return Err(AppError::msg("La nota puede tener hasta 200 caracteres"));
    }
    let tx = conn.transaction()?;
    let current = get(&tx, payload.product_id)?;
    let before = current.as_ref().map_or(0, |stock| stock.quantity);
    let min_quantity = payload
        .min_quantity
        .or(current.as_ref().map(|stock| stock.min_quantity))
        .unwrap_or(0);
    let now = now_rfc3339();
    let (after, delta, reason) = match payload.mode.as_str() {
        "untrack" => {
            tx.execute("DELETE FROM product_stock WHERE product_id = ?1", [payload.product_id])?;
            tx.commit()?;
            return Ok(None);
        }
        "add" => {
            if !(1..=MAX_QUANTITY).contains(&payload.quantity) {
                return Err(AppError::msg("Ingresá cuántas unidades llegaron (1 a 100.000)"));
            }
            (before + payload.quantity, payload.quantity, "restock")
        }
        "set" => {
            if !(0..=MAX_QUANTITY).contains(&payload.quantity) {
                return Err(AppError::msg("Ingresá el conteo físico (0 a 100.000)"));
            }
            (payload.quantity, payload.quantity - before, "count")
        }
        _ => return Err(AppError::msg("Movimiento de stock inválido")),
    };
    upsert(&tx, payload.product_id, after, min_quantity, &now)?;
    record(
        &tx,
        &Movement { product_id: payload.product_id, delta, quantity_after: after, reason, charge_id: None, note },
        username,
        &now,
    )?;
    let saved = get(&tx, payload.product_id)?;
    tx.commit()?;
    Ok(saved)
}

pub fn movements(conn: &Connection, product_id: i64, limit: i64) -> AppResult<Vec<StockMovement>> {
    let mut stmt = conn.prepare(
        "SELECT id, product_id, delta, quantity_after, reason, username, note, created_at
         FROM stock_movements WHERE product_id = ?1 ORDER BY id DESC LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![product_id, limit.clamp(1, 200)], |row| {
            Ok(StockMovement {
                id: row.get(0)?,
                product_id: row.get(1)?,
                delta: row.get(2)?,
                quantity_after: row.get(3)?,
                reason: row.get(4)?,
                username: row.get(5)?,
                note: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn fixture() -> (Connection, i64) {
        let conn = db::open(Path::new(":memory:")).unwrap();
        let id = conn
            .query_row("SELECT id FROM products ORDER BY id LIMIT 1", [], |row| row.get(0))
            .unwrap();
        (conn, id)
    }

    fn payload(product_id: i64, mode: &str, quantity: i64) -> UpdateStockPayload {
        UpdateStockPayload { product_id, mode: mode.into(), quantity, min_quantity: None, note: None }
    }

    #[test]
    fn untracked_products_sell_without_counting() {
        let (conn, id) = fixture();
        record_sale(&conn, id, 1, "recepcion").unwrap();
        assert!(get(&conn, id).unwrap().is_none());
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM stock_movements", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn restock_sale_void_and_count_leave_a_trail() {
        let (mut conn, id) = fixture();
        update(&mut conn, "admin", UpdateStockPayload { min_quantity: Some(2), ..payload(id, "add", 5) }).unwrap();
        record_sale(&conn, id, 10, "recepcion").unwrap();
        record_sale(&conn, id, 11, "recepcion").unwrap();
        assert_eq!(get(&conn, id).unwrap().unwrap().quantity, 3);
        record_void(&conn, 10, "admin").unwrap();
        record_void(&conn, 10, "admin").unwrap();
        assert_eq!(get(&conn, id).unwrap().unwrap().quantity, 4, "a void returns the unit once");
        update(&mut conn, "admin", UpdateStockPayload { note: Some(" conteo ".into()), ..payload(id, "set", 1) }).unwrap();
        let stock = get(&conn, id).unwrap().unwrap();
        assert_eq!((stock.quantity, stock.min_quantity), (1, 2));
        let trail = movements(&conn, id, 10).unwrap();
        let reasons: Vec<&str> = trail.iter().map(|m| m.reason.as_str()).collect();
        assert_eq!(reasons, ["count", "void", "sale", "sale", "restock"]);
        assert_eq!((trail[0].delta, trail[0].note.as_deref()), (-3, Some("conteo")));
        assert_eq!(trail[2].username.as_deref(), Some("recepcion"));
    }

    #[test]
    fn stock_goes_negative_and_bad_input_is_rejected() {
        let (mut conn, id) = fixture();
        update(&mut conn, "admin", payload(id, "set", 0)).unwrap();
        record_sale(&conn, id, 1, "").unwrap();
        assert_eq!(get(&conn, id).unwrap().unwrap().quantity, -1);
        assert!(update(&mut conn, "admin", payload(id, "add", 0)).is_err());
        assert!(update(&mut conn, "admin", payload(id, "set", -2)).is_err());
        assert!(update(&mut conn, "admin", payload(id, "otro", 1)).is_err());
        assert!(update(&mut conn, "admin", payload(999_999, "add", 1)).is_err());
        assert!(update(&mut conn, "admin", payload(id, "untrack", 0)).unwrap().is_none());
        assert!(get(&conn, id).unwrap().is_none());
    }
}
