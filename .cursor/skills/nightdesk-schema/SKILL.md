---
name: nightdesk-schema
description: >-
  SQLite schema, migrations, room occupancy, stay and reservation status for
  Nightdesk. Use when editing migrations, 001_init.sql, db.rs, rooms, stays,
  reservations, check_in, set_room_status, occupancy, or display_status.
---

# Schema y ocupación

SQLite local (`nightdesk.db` en el data dir). `PRAGMA foreign_keys=ON` y `journal_mode=WAL` en `db::open`. No commitear la DB.

## Migraciones

Hoy: `src-tauri/migrations/001_init.sql`, cargada con `include_str!` en `db.rs` y registrada en `schema_migrations`.

Migración nueva (no reescribir `001_init` en installs existentes):

1. Archivo `src-tauri/migrations/002_descripcion.sql`
2. Constante `include_str!` en `db.rs`
3. Ejecutar el SQL **solo** si ese `id` no está en `schema_migrations`, luego `INSERT` el id
4. Seed: `seed_if_empty` corre solo si `COUNT(rooms) = 0`. No duplicar seed.

Tablas: `rooms`, `rate_plans`, `guests`, `reservations`, `stays`, `charges`, `payments`, `settings`.

## Habitación

**Persistido** (`rooms.status`): `available` | `dirty` | `blocked` | `occupied`.

**Tablero** (`display_status` en `list_board`):

| Condición | `display_status` |
|-----------|------------------|
| Stay `open` | `occupied` |
| `rooms.status = blocked` | `blocked` |
| `rooms.status = dirty` | `dirty` |
| Reserva `hold` con llegada hoy | `reserved` |
| Resto | `available` |

`set_room_status` solo acepta `available` | `dirty` | `blocked`. Falla si hay stay `open`.

## Stay

- Una estadía `open` por habitación (`open_stay_for_room`). Check-in rechaza ocupada o `blocked`.
- Check-in → `rooms.status = occupied`.
- Checkout → `rooms.status = dirty` (nunca `available` directo).
- Status stay: `open` | `closed`. Cerrada: inmutable para cargos.

Walk-in: si hay `today_hold_for_room`, exige el `reservation_id` de esa reserva.

## Reserva

Status: `hold` (alta) → `checked_in` (al check-in) | `cancelled` | `no_show`.

`set_reservation_status` solo desde `hold` hacia `cancelled` o `no_show`. Check-in de reserva: `check_in_reservation` → `check_in` con ese `reservation_id`.

## Tarifas en DB

`kind`: `hourly` | `night` | `overnight`. Montos `*_cents`. `night_cutoff_hour` 0–23. `active` 0/1.

Cargos: `stay` | `extra_hour` | `tax` (computados al cerrar) y `surcharge` | `discount` (manuales). Pagos: `cash` | `card` | `transfer`.
