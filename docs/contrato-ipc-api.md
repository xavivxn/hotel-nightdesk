# Contrato IPC / API (versión 2)

Fecha: 20 de septiembre de 2026 (v2 / I11). Origen: 17/09/2026.
Estado: contrato v2 vigente en IPC Tauri + `supabaseInvoke`. El transporte remoto es Supabase (N12/N07/I11), no HTTP `/api/v1` ni VPN. Semántica de `operation_id` / `expected_version` cerrada en I06 (outbox) e I11 (catálogo write-through).

Este documento es la fuente de verdad de payloads, errores y semántica. El transporte cambia; las reglas de negocio no. Arquitectura: [arquitectura-offline-supabase.md](arquitectura-offline-supabase.md).

## Transporte

Tres caminos, mismos nombres de comando, mismos payloads snake_case, mismos códigos de error:

```
UI → api.ts → cmd()
               ├─ invoke          modo Recepción (Tauri + SQLite)
               ├─ supabaseInvoke  modo Administración remota (PostgREST / RPC)
               └─ mockInvoke      navegador (`npm run dev`)
                      ↓
         commands.rs (sesión, lock, impresión, write-through)     — solo invoke
                      ↓
         service.rs (Actor, authorize, reglas, SQL, TX)
                      ↓
         billing.rs / db.rs / sync::{outbox,catalog,remote,push,pull,worker}
```

`src/lib/supabase.ts` (N07) implementa `supabaseInvoke(name, args)`. Los componentes **nunca** importan `@tauri-apps/api` ni `@supabase/supabase-js`. No hay escritura remota directa sobre el archivo SQLite.

| Camino | Quién | Fuente de datos |
|---|---|---|
| `invoke` | PC recepción | SQLite local vía `service.rs`; catálogo con sync → write-through `catalog_write` |
| `supabaseInvoke` | PC admin, modo remoto | PostgREST / RPC `catalog_write` con sesión Auth `admin` |
| `mockInvoke` | Vite en navegador | `src/lib/mock.ts` |

Impresión, reimpresión, `save_daily_pdf` y `list_printers` son solo `invoke` (equipo de recepción). En `supabaseInvoke` devuelven `forbidden`. Mutaciones operativas (`check_in`, `check_out`, `add_charge`, `add_product_charge`, `delete_charge`, `convert_to_overnight`, `set_room_status`, reservas) también `forbidden` en remoto.

## Convenciones

- JSON snake_case. `null` ↔ `Option<T>`.
- Montos: enteros en guaraníes en campos `*_cents` (1 = 1 Gs).
- Fechas persistidas: RFC3339 UTC. La UI muestra hora local.
- Toda operación de negocio en recepción lleva `session_token` (excepto `auth_setup_required`, `auth_setup` y `auth_login`). En modo remoto la sesión es el JWT de Supabase Auth.
- Mutaciones aceptan `operation_id` (UUID del cliente) y `expected_version` (entero ≥ 0). Vacío o negativo → `validation`.
- **`operation_id`:** clave idempotente de extremo a extremo. `api.ts` la genera con `crypto.randomUUID()` en mutaciones de estadía/cargo/reserva **y** en escrituras de catálogo (`save_*`, `set_product_active`, `save_settings`, `auth_create_user`). En recepción operativa, I06 la persiste en `sync_outbox` (sub-ops = UUID v5 del root). En catálogo I11, la misma clave llega a Postgres `nightdesk.catalog_operations` / `public.catalog_audit` vía RPC `catalog_write`; repetir el mismo id con el mismo payload no duplica el efecto. Reutilizar el id con otro payload → `conflict`.
- **`expected_version`:** control de concurrencia del **catálogo** (`rooms`, `rate_plans`, `products`, ajustes de negocio, `users` / `app_users`). Si no coincide con `version` → `conflict` («La ficha cambió; recargá antes de guardar») y **no** se escribe SQLite local cuando hay sync. Con sync deshabilitado (instalación sin dispositivo), el IPC sigue validando en local.
- `get_settings` / `save_settings` devuelven `pin_hash` vacío. `get_settings` incluye `catalog_versions` (mapa clave → versión de cada ajuste de negocio) para el write-through.
- Impresora: `print_error: string | null`. Un fallo de impresión no revierte el cobro.

## Versión

`CONTRACT_VERSION = 2`, expuesto por `contract_info`:

```json
{ "contract_version": 2, "app_version": "0.1.0", "schema_migrations": ["001_init", "002_products", "003_rooms_scope", "004_account_closure", "005_auth", "006_stay_integrity", "007_receipts", "008_ticket_header", "009_ticket_header_name", "010_jacuzzi_rooms", "011_jacuzzi_rooms_1_to_4", "012_love_nest_rates", "013_no_iva", "014_sync", "015_catalog_audit"] }
```

v2 (I11.3): `catalog_versions` en ajustes; `set_product_active` acepta `operation_id` / `expected_version`; `hash_password` acepta `operation_id` opcional (salt determinística Argon2id para reintentos idempotentes); write-through documentado.

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
| `conflict` | Estado incompatible (habitación ocupada, estadía cerrada, duplicado, versión de catálogo) |
| `forbidden` | Rol insuficiente |
| `session_expired` | Sin sesión, vencida o usuario inactivo |
| `rate_limited` | Demasiados intentos de acceso |
| `invalid_credentials` | Usuario o contraseña incorrectos |
| `storage` | SQLite / IO / sin conexión a administración (write-through) |
| `printer` | Impresión (si se eleva a error; el checkout usa `print_error`) |

Sin red y con sync habilitado, las escrituras de catálogo devuelven `storage` con mensaje «Requiere conexión con administración…» y **no** mutan SQLite.

## Roles

`admin` y `recepcion`. Admin: habitaciones/tarifas, catálogo, cargos manuales y su baja, ajustes, usuarios, ticket de prueba. Recepción: tablero, check-in/out, reservas, historial, consumos de productos activos.

Autorización duplicada a propósito: `auth::require(..., admin)` en el adaptador Tauri y `service::authorize(actor, Operation)` en la capa reutilizable. En remoto, RLS y el rol Auth `admin` / `device` son la segunda barrera.

## Comandos → transporte → rol

Leyenda de `supabaseInvoke`: `lectura` = PostgREST/vista; `catálogo` = RPC `catalog_write` (idempotente; envuelve `catalog_upsert_*` + auditoría); `auth` = Supabase Auth; `forbidden` = `ApiError{code:'forbidden'}`.

| Comando | IPC recepción | supabaseInvoke (admin remoto) | Rol IPC |
|---|---|---|---|
| `auth_setup_required` | sí | no (setup solo en recepción) | público |
| `auth_setup` | sí | no | público (instalación vacía) |
| `auth_login` | sí (Argon2 local) | Auth email/password | público |
| `auth_session` | sí | sesión Auth en memoria | autenticado |
| `auth_logout` | sí | signOut Auth | autenticado |
| `auth_create_user` | admin; write-through si hay sync | `catalog_write` `app_users` + hash vía `hash_password` | admin |
| `contract_info` | sí | constante de app | autenticado |
| `list_board` | sí | lectura `stays`/`rooms` + Realtime | autenticado |
| `list_rooms` | sí | lectura `rooms` | autenticado |
| `save_room` | admin; write-through si hay sync | catálogo (sin `status`) | admin |
| `set_room_status` | sí | `forbidden` | autenticado |
| `list_rate_plans` | sí | lectura | autenticado |
| `save_rate_plan` | admin; write-through si hay sync | catálogo | admin |
| `check_in` | sí + outbox | `forbidden` | autenticado |
| `preview_bill` | sí | estimativo local | autenticado |
| `get_stay_detail` | sí | lectura | autenticado |
| `convert_to_overnight` | sí + outbox | `forbidden` | autenticado |
| `list_products` | sí | lectura | autenticado |
| `save_product` | admin; write-through si hay sync | catálogo | admin |
| `set_product_active` | admin; write-through si hay sync | catálogo | admin |
| `add_charge` / `add_product_charge` / `delete_charge` | sí + outbox | `forbidden` | admin (cargos manuales / baja) |
| `check_out` | sí + outbox + print | `forbidden` | autenticado |
| `list_reservations` / `create_reservation` / … | sí (+ outbox en mutación) | lectura / `forbidden` | autenticado |
| `list_history` / `daily_report` | sí | lectura | autenticado |
| `save_daily_pdf` / `print_test` / `list_printers` / `reprint_receipt` | sí | `forbidden` | admin / autenticado |
| `get_settings` | sí | lectura `business_settings` + `catalog_versions` | autenticado |
| `save_settings` | admin; write-through de claves de negocio | catálogo lista blanca | admin |
| `verify_pin` / `pin_required` | sí | no aplica / stub | autenticado |
| `device_mode_*` / `remote_*` / `hash_password` | siempre IPC local | — | ver tabla abajo |
| `sync_*` / `backup_*` | I07 worker / I08 motor local | lectura parcial (`backup_status`) | autenticado / admin |

## Comandos de dispositivo / sync / hash

Solo tienen sentido en recepción salvo donde se indica. Seguir `nightdesk-add-command`.

| Comando | Payload → resultado | Notas |
|---|---|---|
| `device_mode_get` | `{}` → `"reception" \| "remote" \| null` | Ajuste de dispositivo. `null` = primer arranque. Siempre IPC local. |
| `device_mode_set` | `{ mode }` → `void` | Primer arranque. No se sincroniza. |
| `remote_configure` | `{ project_url, anon_key }` → `void` | Solo PC admin. Credenciales en app data; nunca en `settings`. |
| `remote_configured` / `remote_get_config` | `{}` → `bool` / payload | Lectura local de URL/anon. |
| `sync_configure_device` | `{ project_url, anon_key, device_email, device_password }` → `void` | Credenciales a app data (nunca en `settings` ni en la respuesta). Habilita write-through I11 y, en modo recepción, arranca el worker I07. |
| `sync_status` | `{}` → `{ connected, pending_outbox, last_push_at, last_pull_at, last_error, configured, realtime_connected }` | Indicador de `AppShell`. `pending_outbox` se cuenta en SQLite; el resto sale del snapshot del worker. |
| `sync_pull_now` | `{}` → `void` | Drena la outbox y hace pull. Espera hasta 20 s; sin worker o sin red → `storage`. Si el catálogo cambió, el worker emite `sync:catalog-updated` (Tauri → `window`). |
| `hash_password` | `{ password }` + `operation_id?` → `{ hash }` | Argon2id, mismo formato que `auth.rs`. Siempre IPC local (también en modo remoto). Con `operation_id` UUID el salt es determinístico (reintento idempotente de alta de usuario). |
| `backup_run_now` | `{}` → `{ backup_id }` | Admin, solo recepción. Snapshot Online Backup + cifrado; encola upload. Responde tras snapshot local verificado (no tras upload). El manifiesto remoto incluye `nonce_hex`. |
| `backup_status` | `{}` → `{ last_local_at, last_remote_at, pending, last_error, ready }` | Recepción: cola local + último remoto confirmado; `ready: true` cuando el motor tiene clave. Admin remoto: lee la última fila de `public.backups`; `last_local_at`/`pending` no observables; `ready: false`. |
| `backup_list` | `{}` → `BackupListItem[]` | Admin, solo recepción. Cola local + manifiestos remotos (`source: local \| remote`). Remoto → `forbidden`. |
| `backup_import_key` | `{ key_hex }` → `void` | Admin, solo recepción. Importa la clave AES-256 del custodio (64 hex). No se devuelve. Remoto → `forbidden`. |
| `backup_restore` | `{ backup_id, source? }` → `void` | Admin, solo recepción. `source` default `local`. `remote`: descarga Storage, descifra, integrity/checksum, swap, limpia sync, revoca sesiones y despierta bootstrap. Remoto → `forbidden`. |

## Payloads de mutación (v2)

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

**save_room** / **save_rate_plan** / **save_product** (admin): `id` ausente crea, presente actualiza. `Room`, `RatePlan` y `Product` exponen `version`. Si `expected_version` no coincide con `version`, `conflict`. Con sync habilitado (`sync_configure_device` hecho): write-through a Supabase vía `catalog_write` y aplicación local de la fila + `catalog_audit`; sin conexión → `storage` y SQLite intacto. Sin sync: escritura local + auditoría local (`015_catalog_audit`).

**set_product_active** (admin):

```
{ product_id, active, expected_version?, operation_id? }
```

**delete_charge** (admin): baja lógica (`charges.deleted_at`). `list_charges` / preview / detalle omiten filas borradas. La outbox registra `op=delete`.

**auth_create_user** (admin): `{ payload: { username, password, role }, operation_id? }`. Con sync: hash local (`hash_password` + mismo `operation_id`) y `catalog_write` `app_users` (el hash viaja; la contraseña en claro no).

Mutaciones operativas escriben `sync_outbox` en la misma TX. El `operation_id` de `api.ts` es `root_operation_id`; si una operación genera varias filas (checkout: stay + cargos + room), los sub-ops usan un UUID v5 derivado. Repetir el mismo `operation_id` → `conflict` y rollback (no hay segunda estadía ni segunda fila). El adaptador IPC despierta al worker (`wake_push`) después de cada mutación operativa.

Tras restaurar un snapshot (I05): borrar `sync_outbox`, `sync_state` (`bootstrap_done` y `pull_cursor:*`) y volver a bootstrap para no reenviar operaciones antiguas.

Consultas (`list_board`, `list_rooms`, `preview_bill`, `get_stay_detail`, `list_history`, …) no llevan `operation_id`. Resultado = tipos de `models.rs` / `types.ts`.

## Qué queda por implementar

- **I06:** hecho (`014_sync`, outbox en TX, lista blanca, `expected_version` local).
- **N12:** hecho (esquema Postgres, RLS, `sync_apply_ops`, `catalog_upsert_*`, Realtime, Auth, bucket `backups`).
- **N07:** hecho (modo remoto, `supabaseInvoke`, Auth, catálogo remoto).
- **I11:** hecho (write-through recepción, `catalog_write` + auditoría, `hash_password` con salt por `operation_id`, contrato v2).
- **I07:** hecho (worker push/pull/Realtime, bootstrap, `sync_status` / `sync_pull_now` / `sync_configure_device`).
- **I08 / I05:** hecho (`backup_run_now` / `backup_status` / `backup_list` / `backup_import_key` / `backup_restore` local y `source=remote`, cola, cifrado, upload Storage con `nonce_hex`, retención local 7). Pendiente ops: deploy/cron Edge `backup-retention`.
- No duplicar reglas fuera de `service.rs`. No reintroducir `/api/v1` ni WireGuard.
