# Contrato IPC / API (versión 1)

Fecha: 15 de septiembre de 2026.
Estado: contrato v1 vigente en IPC Tauri. La API HTTP `/api/v1` reutilizará la misma capa (`service.rs`) en I06 ([MOT-18](https://naserfer.atlassian.net/browse/MOT-18)).

Este documento es la fuente de verdad de payloads, errores y semántica. El transporte cambia; las reglas no.

## Transporte

Hoy: comandos Tauri invocados desde `src/lib/api.ts` (`cmd` → `invoke`). El navegador (`npm run dev`) usa `mock.ts` con la misma semántica.

Mañana (I06): HTTPS privado `/api/v1` detrás de VPN. Misma capa `service.rs`, mismos códigos de error, mismos payloads snake_case. No hay escritura remota directa sobre el archivo SQLite.

```
UI → api.ts → IPC Tauri | mock
                 ↓
         commands.rs (sesión, lock, impresión)
                 ↓
         service.rs (Actor, authorize, reglas, SQL, TX)
                 ↓
         billing.rs / db.rs
```

## Convenciones

- JSON snake_case. `null` ↔ `Option<T>`.
- Montos: enteros en guaraníes en campos `*_cents` (1 = 1 Gs).
- Fechas persistidas: RFC3339 UTC. La UI muestra hora local.
- Toda operación de negocio lleva `session_token` (excepto `auth_setup_required`, `auth_setup` y `auth_login`).
- Mutaciones aceptan `operation_id` (UUID del cliente) y `expected_version` (entero ≥ 0). En v1 se valida la forma y se ignoran. I06 persistirá idempotencia y versiones por entidad.
- `get_settings` / `save_settings` devuelven `pin_hash` vacío.
- Impresora: `print_error: string | null`. Un fallo de impresión no revierte el cobro.

## Versión

`CONTRACT_VERSION = 1`, expuesto por `contract_info`:

```json
{ "contract_version": 1, "app_version": "0.1.0", "schema_migrations": ["001_init", "002_products", "003_rooms_scope", "004_account_closure", "005_auth", "006_stay_integrity"] }
```

## Errores

Shape único:

```json
{ "code": "conflict", "message": "La habitación ya está ocupada" }
```

`code` en snake_case. `message` en español para mostrar en UI (`String(e)` / `ApiError.toString()`). Sesión vencida: `code === "session_expired"` (no buscar el texto).

| Código | Uso |
|---|---|
| `validation` | Datos inválidos o incompletos |
| `not_found` | Recurso inexistente |
| `conflict` | Estado incompatible (habitación ocupada, estadía cerrada, duplicado) |
| `forbidden` | Rol insuficiente |
| `session_expired` | Sin sesión, vencida o usuario inactivo |
| `rate_limited` | Demasiados intentos de acceso |
| `invalid_credentials` | Usuario o contraseña incorrectos |
| `storage` | SQLite / IO |
| `printer` | Impresión (si se eleva a error; el checkout usa `print_error`) |

## Roles

`admin` y `recepcion`. Admin: habitaciones/tarifas, catálogo, cargos manuales y su baja, ajustes, usuarios, ticket de prueba. Recepción: tablero, check-in/out, reservas, historial, consumos de productos activos.

Autorización duplicada a propósito: `auth::require(..., admin)` en el adaptador Tauri y `service::authorize(actor, Operation)` en la capa reutilizable.

## Comandos → ruta HTTP propuesta → rol

### Ampliación MOT-3 (16/09/2026)

| Comando | Transporte | Rol |
|---|---|---|
| `daily_report` | IPC; futuro `GET /reports/daily?date=YYYY-MM-DD` | autenticado |
| `save_daily_pdf` | Solo IPC local, sin ruta HTTP | autenticado |
| `list_printers` | Solo IPC local, sin ruta HTTP | admin |

`daily_report {date}` devuelve `DailyReport`: `date`, `generated_at`, `cutoff_at`, `timezone`, `occupied_rooms`, `closed_total_cents`, `adjustments_total_cents`, `accounts` y `adjustments`. Cada cuenta tiene `stay_id`, `room_number`, `check_in_at`, `check_out_at`, `closed_on_day`, `open_at_cutoff` y `total_cents` (null si no cerró ese día). Movimientos: `Charge[]` de consumos/recargos/descuentos del día. No sumar el subtotal de movimientos al total cerrado.

`save_daily_pdf {date, bytes}` guarda el documento generado por la interfaz local en `informes/` con nombre único y devuelve la ruta. Requiere fecha válida no futura, firma PDF, terminador y máximo 20 MB. No acepta rutas proporcionadas por el cliente. El PDF es un archivo exportado, no una fuente de datos operativos.

`list_printers {}` devuelve nombres de colas Windows. En mock devuelve lista vacía. El guardado de PDF en navegador usa descarga local. Impresión/reimpresión no tendrán ruta HTTP; están limitadas al equipo de recepción. `print_error` distingue envío fallido/desactivado; null significa aceptación por la cola, no confirmación de papel.

Migración `007_receipts`: bytes del ticket definitivo en `receipt_snapshots`, guardados atómicamente al cerrar. Reimpresión fiel sin cambiar la cuenta. Ver [uso y evidencia](tickets-informes.md).

Prefijo futuro: `/api/v1`.

| Comando | Método y ruta | Rol |
|---|---|---|
| `auth_setup_required` | `GET /auth/setup-required` | público |
| `auth_setup` | `POST /auth/setup` | público (solo instalación vacía) |
| `auth_login` | `POST /auth/login` | público |
| `auth_session` | `GET /auth/session` | autenticado |
| `auth_logout` | `POST /auth/logout` | autenticado |
| `auth_create_user` | `POST /users` | admin |
| `contract_info` | `GET /contract` | autenticado |
| `list_board` | `GET /board` | autenticado |
| `list_rooms` | `GET /rooms` | autenticado |
| `save_room` | `PUT /rooms` | admin |
| `set_room_status` | `POST /rooms/{id}/status` | autenticado |
| `list_rate_plans` | `GET /rate-plans` | autenticado |
| `save_rate_plan` | `PUT /rate-plans` | admin |
| `check_in` | `POST /stays` | autenticado |
| `preview_bill` | `GET /stays/{id}/bill` | autenticado |
| `get_stay_detail` | `GET /stays/{id}` | autenticado |
| `convert_to_overnight` | `POST /stays/{id}/overnight` | autenticado |
| `list_products` | `GET /products` | autenticado |
| `save_product` | `PUT /products` | admin |
| `set_product_active` | `POST /products/{id}/active` | admin |
| `add_charge` | `POST /stays/{id}/charges` | admin |
| `add_product_charge` | `POST /stays/{id}/product-charges` | autenticado |
| `delete_charge` | `DELETE /charges/{id}` | admin |
| `check_out` | `POST /stays/{id}/checkout` | autenticado |
| `list_reservations` | `GET /reservations` | autenticado |
| `create_reservation` | `POST /reservations` | autenticado |
| `set_reservation_status` | `POST /reservations/{id}/status` | autenticado |
| `check_in_reservation` | `POST /reservations/{id}/check-in` | autenticado |
| `list_history` | `GET /history` | autenticado |
| `get_settings` | `GET /settings` | autenticado |
| `save_settings` | `PUT /settings` | admin |
| `verify_pin` | `POST /pin/verify` | autenticado |
| `pin_required` | `GET /pin/required` | autenticado |
| `print_test` | `POST /print/test` | admin |
| `reprint_receipt` | `POST /stays/{id}/reprint` | autenticado |

Impresión (`print_test`, `reprint_receipt`, parte de `check_out`) permanece en el adaptador Tauri: la API remota no dispara la impresora de recepción.

## Payloads de mutación (v1)

Campos reservados en todos: `operation_id?: string`, `expected_version?: number`. Vacío o negativo → `validation`.

**check_in** → `Stay`. Errores: `validation`, `conflict` (ocupada / reserva de hoy), `not_found`.

```
{ room_id, guest_name, document?, phone?, rate_plan_id, expected_hours?, reservation_id?, operation_id?, expected_version? }
```

**check_out** → `{ stay, bill, print_error }`. Cierra en transacción; la impresión ocurre después. `conflict` si ya estaba cerrada.

```
{ stay_id, print, operation_id?, expected_version? }
```

**create_reservation** → `Reservation`. `conflict` si hay estadía abierta el mismo día local o reserva `hold` duplicada.

```
{ guest_name, document?, phone?, room_id, rate_plan_id, expected_arrival_at, expected_nights, notes?, operation_id?, expected_version? }
```

**add_charge** (admin) / **add_product_charge** → `Charge`. `conflict` sobre estadía cerrada.

**save_room** / **save_rate_plan** / **save_product** (admin): `id` ausente crea, presente actualiza.

Consultas (`list_board`, `list_rooms`, `preview_bill`, `get_stay_detail`, `list_history`, …) no llevan `operation_id`. Resultado = tipos de `models.rs` / `types.ts`.

## Qué queda para I06

- Persistencia de `operation_id` y respuesta idempotente.
- `expected_version` contra versión de entidad; conflicto si no coincide.
- Auditoría y HTTP `/api/v1`.
- No duplicar reglas fuera de `service.rs`.
