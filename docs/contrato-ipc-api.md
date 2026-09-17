# Contrato IPC / API (versión 1)

Fecha: 17 de septiembre de 2026.
Estado: contrato v1 vigente en IPC Tauri. El transporte remoto es Supabase (N12/N07), no HTTP `/api/v1` ni VPN. Semántica de `operation_id` / `expected_version` documentada aquí; persistencia en I06/I11.

Este documento es la fuente de verdad de payloads, errores y semántica. El transporte cambia; las reglas de negocio no. Arquitectura: [arquitectura-offline-supabase.md](arquitectura-offline-supabase.md).

## Transporte

Tres caminos, mismos nombres de comando, mismos payloads snake_case, mismos códigos de error:

```
UI → api.ts → cmd()
               ├─ invoke          modo Recepción (Tauri + SQLite)
               ├─ supabaseInvoke  modo Administración remota (PostgREST / RPC)
               └─ mockInvoke      navegador (`npm run dev`)
                      ↓
         commands.rs (sesión, lock, impresión)     — solo invoke
                      ↓
         service.rs (Actor, authorize, reglas, SQL, TX)
                      ↓
         billing.rs / db.rs / sync outbox
```

`src/lib/supabase.ts` (N07) implementa `supabaseInvoke(name, args)`. Los componentes **nunca** importan `@tauri-apps/api` ni `@supabase/supabase-js`. No hay escritura remota directa sobre el archivo SQLite.

| Camino | Quién | Fuente de datos |
|---|---|---|
| `invoke` | PC recepción | SQLite local vía `service.rs` |
| `supabaseInvoke` | PC admin, modo remoto | PostgREST / RPC con sesión Auth `admin` |
| `mockInvoke` | Vite en navegador | `src/lib/mock.ts` |

Impresión, reimpresión, `save_daily_pdf` y `list_printers` son solo `invoke` (equipo de recepción). En `supabaseInvoke` devuelven `forbidden`. Mutaciones operativas (`check_in`, `check_out`, `add_charge`, `add_product_charge`, `delete_charge`, `convert_to_overnight`, `set_room_status`, reservas) también `forbidden` en remoto.

## Convenciones

- JSON snake_case. `null` ↔ `Option<T>`.
- Montos: enteros en guaraníes en campos `*_cents` (1 = 1 Gs).
- Fechas persistidas: RFC3339 UTC. La UI muestra hora local.
- Toda operación de negocio en recepción lleva `session_token` (excepto `auth_setup_required`, `auth_setup` y `auth_login`). En modo remoto la sesión es el JWT de Supabase Auth.
- Mutaciones aceptan `operation_id` (UUID del cliente) y `expected_version` (entero ≥ 0). Vacío o negativo → `validation`.
- **`operation_id`:** clave idempotente de extremo a extremo. `api.ts` la genera con `crypto.randomUUID()` en mutaciones de estadía/cargo/reserva. I06 la persiste en `sync_outbox`; N12 en `sync_applied_ops`. Repetir el mismo id no duplica el efecto. Hoy el IPC valida la forma y aún ignora el valor hasta I06.
- **`expected_version`:** control de concurrencia del **catálogo** (`rooms`, `rate_plans`, `products`, ajustes de negocio, `users`). Si no coincide con `version` → `conflict` y recarga. Hoy el IPC valida la forma y aún ignora el valor hasta I06/I11.
- `get_settings` / `save_settings` devuelven `pin_hash` vacío.
- Impresora: `print_error: string | null`. Un fallo de impresión no revierte el cobro.

## Versión

`CONTRACT_VERSION = 1`, expuesto por `contract_info`:

```json
{ "contract_version": 1, "app_version": "0.1.0", "schema_migrations": ["001_init", "002_products", "003_rooms_scope", "004_account_closure", "005_auth", "006_stay_integrity", "007_receipts", "008_ticket_header", "009_ticket_header_name", "010_jacuzzi_rooms", "011_jacuzzi_rooms_1_to_4", "012_love_nest_rates", "013_no_iva"] }
```

I06 añadirá `014_sync`. Incrementar `CONTRACT_VERSION` solo si cambia un payload (I11.3).

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

Autorización duplicada a propósito: `auth::require(..., admin)` en el adaptador Tauri y `service::authorize(actor, Operation)` en la capa reutilizable. En remoto, RLS y el rol Auth `admin` / `device` son la segunda barrera.

## Comandos → transporte → rol

Leyenda de `supabaseInvoke`: `lectura` = PostgREST/vista; `catálogo` = RPC `catalog_upsert_*` con `expected_version`; `auth` = Supabase Auth; `forbidden` = `ApiError{code:'forbidden'}`.

| Comando | IPC recepción | supabaseInvoke (admin remoto) | Rol IPC |
|---|---|---|---|
| `auth_setup_required` | sí | no (setup solo en recepción) | público |
| `auth_setup` | sí | no | público (instalación vacía) |
| `auth_login` | sí (Argon2 local) | Auth email/password | público |
| `auth_session` | sí | sesión Auth en memoria | autenticado |
| `auth_logout` | sí | signOut Auth | autenticado |
| `auth_create_user` | sí | `app_users` + hash vía `hash_password` | admin |
| `contract_info` | sí | constante de app | autenticado |
| `list_board` | sí | lectura `stays`/`rooms` + Realtime | autenticado |
| `list_rooms` | sí | lectura `rooms` | autenticado |
| `save_room` | admin; write-through si hay sync | catálogo (sin `status`) | admin |
| `set_room_status` | sí | forbidden | autenticado |
| `list_rate_plans` | sí | lectura `rate_plans` | autenticado |
| `save_rate_plan` | admin; write-through si hay sync | catálogo | admin |
| `check_in` | sí + outbox | forbidden | autenticado |
| `preview_bill` | sí (`billing.rs`) | estimativo `billing.ts` | autenticado |
| `get_stay_detail` | sí | lectura | autenticado |
| `convert_to_overnight` | sí + outbox | forbidden | autenticado |
| `list_products` | sí | lectura `products` | autenticado |
| `save_product` | admin; write-through si hay sync | catálogo | admin |
| `set_product_active` | admin; write-through si hay sync | catálogo | admin |
| `add_charge` | admin + outbox | forbidden | admin |
| `add_product_charge` | sí + outbox | forbidden | autenticado |
| `delete_charge` | admin; soft `deleted_at` + outbox | forbidden | admin |
| `check_out` | sí + outbox; impresión después | forbidden | autenticado |
| `list_reservations` | sí | lectura | autenticado |
| `create_reservation` | sí + outbox | forbidden | autenticado |
| `set_reservation_status` | sí + outbox | forbidden | autenticado |
| `check_in_reservation` | sí + outbox | forbidden | autenticado |
| `list_history` | sí | lectura | autenticado |
| `daily_report` | sí | lectura (sin bytes de ticket) | autenticado |
| `save_daily_pdf` | sí, solo local | forbidden | autenticado |
| `get_settings` | sí | lectura claves de negocio | autenticado |
| `save_settings` | admin; write-through de claves de negocio | catálogo lista blanca | admin |
| `verify_pin` | sí | forbidden | autenticado |
| `pin_required` | sí | forbidden | autenticado |
| `print_test` | admin | forbidden | admin |
| `list_printers` | admin | forbidden | admin |
| `reprint_receipt` | sí | forbidden | autenticado |

`daily_report {date}` devuelve `DailyReport`: `date`, `generated_at`, `cutoff_at`, `timezone`, `occupied_rooms`, `closed_total_cents`, `adjustments_total_cents`, `accounts` y `adjustments`. Cada cuenta tiene `stay_id`, `room_number`, `check_in_at`, `check_out_at`, `closed_on_day`, `open_at_cutoff` y `total_cents` (null si no cerró ese día). Movimientos: `Charge[]` de consumos/recargos/descuentos del día. No sumar el subtotal de movimientos al total cerrado.

`save_daily_pdf {date, bytes}` guarda el documento en `informes/` con nombre único y devuelve la ruta. Fecha válida no futura, firma PDF, máximo 20 MB. El PDF no es fuente operativa.

`list_printers {}` devuelve nombres de colas Windows. En mock, lista vacía. `print_error` distingue envío fallido; `null` es aceptación por la cola, no papel.

Migración `007_receipts`: bytes del ticket en `receipt_snapshots`. No se sincronizan. Ver [tickets-informes.md](tickets-informes.md).

### Comandos de sync, modo y respaldo (I06/I07/I08/N07)

Solo tienen sentido en recepción salvo donde se indica. Seguir `nightdesk-add-command`.

| Comando | Payload → resultado | Notas |
|---|---|---|
| `device_mode_get` | `{}` → `"reception" \| "remote"` | Ajuste de dispositivo. |
| `device_mode_set` | `{ mode }` → `void` | Primer arranque. No se sincroniza. |
| `sync_configure_device` | `{ project_url, anon_key, device_email, device_password }` → `void` | Credenciales a Credential Manager. Nunca en `settings` ni en la respuesta. |
| `sync_status` | `{}` → `{ connected, pending_outbox, last_push_at, last_pull_at, last_error }` | Indicador de `AppShell`. |
| `sync_pull_now` | `{}` → `void` | Pull incremental; emite `sync:catalog-updated`. |
| `hash_password` | `{ password }` → `{ hash }` | Argon2id, mismo formato que `auth.rs`. Solo para modo remoto al crear usuarios. |
| `backup_run_now` | `{}` → `{ backup_id }` | Admin local. Encola snapshot. |
| `backup_status` | `{}` → `{ last_local_at, last_remote_at, pending, last_error }` | Recepción y, en remoto, lectura de tabla `backups`. |

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

**save_room** / **save_rate_plan** / **save_product** (admin): `id` ausente crea, presente actualiza. Con sync habilitado, write-through a Supabase; `expected_version` debe coincidir o `conflict`. Sin conexión: no escribir local y devolver error claro.

Consultas (`list_board`, `list_rooms`, `preview_bill`, `get_stay_detail`, `list_history`, …) no llevan `operation_id`. Resultado = tipos de `models.rs` / `types.ts`.

## Qué queda por implementar

- **I06:** persistir `operation_id` en `sync_outbox`; `expected_version` local en catálogo; `014_sync`.
- **N12:** esquema Postgres, RLS, `sync_apply_ops`, `catalog_upsert_*`, Realtime, Auth, bucket `backups`.
- **I07:** worker push/pull/Realtime y comandos `sync_*`.
- **I11 / N07:** write-through, `hash_password`, `supabaseInvoke`, modo del equipo.
- **I08:** `backup_run_now` / `backup_status`.
- No duplicar reglas fuera de `service.rs`. No reintroducir `/api/v1` ni WireGuard.
