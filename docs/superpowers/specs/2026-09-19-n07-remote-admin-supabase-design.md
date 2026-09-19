# N07 — Modo administración remota sobre Supabase

**Fecha:** 2026-09-19  
**Tickets:** MOT-19 (padre), MOT-54 (N07.1), MOT-55 (N07.2), MOT-56 (N07.3), edición remota de catálogo (N07.4 / MOT-85)  
**Estado:** diseño aprobado en conversación con Naser  
**Referencias:** `docs/arquitectura-offline-supabase.md`, `docs/contrato-ipc-api.md`, skills `nightdesk-conventions` / `nightdesk-add-command` / `nightdesk-design`

## 1. Objetivo

Un solo binario Tauri. En el primer arranque se elige el rol del equipo (**Recepción** o **Administración remota**). Recepción sigue offline-first sobre SQLite. El modo remoto consulta la réplica Supabase y edita catálogo/ajustes/usuarios; no opera estadías ni imprime.

## 2. Decisiones de alcance

| Decisión | Elección |
|---|---|
| Alcance de esta entrega | N07 completo: 54 + 55 + 56 + edición remota de catálogo |
| Enfoque de implementación | Transporte primero (`cmd` / `supabaseInvoke`), luego Auth/Realtime, luego catálogo, luego UI sync |
| Comandos `sync_*` (I07 pendiente) | **Stubs** en recepción; se enchufan al worker real después |
| Secretos | Nunca en el repo ni hardcodeados. URL/anon/device vía config + Windows Credential Manager |
| Docker / Supabase local | No requerido para implementar; mock en browser; proyecto hosted opcional para prueba real |
| Marca en pantalla de modo | Sin texto «Nightdesk»; solo «¿Cómo se usa esta PC?» |

## 3. Arranque y modo del equipo

### Flujo

1. App arranca → `device_mode_get`.
2. Si no hay modo: pantalla de primer arranque con dos opciones (Recepción / Administración remota) y Continuar → `device_mode_set`.
3. El modo es ajuste de dispositivo local (`device_mode`). **No se sincroniza.**
4. No se cambia en operación diaria. Reset solo con acción explícita / reinstalación (fuera del happy path de N07).

### Después del modo

- **Recepción:** flujo actual — `auth_setup_required` / setup admin local si hace falta → login **local** (SQLite + Argon2). Opera sin internet. Opcional más tarde: `sync_configure_device` (URL + anon + device email/password) para sync/backup.
- **Remoto:** si faltan URL + anon en keyring → pantalla de configuración que llama a **`remote_configure`** `{ project_url, anon_key }` → Credential Manager (sin `device_*`; el admin no es un device). Luego login **Supabase Auth**.

### UI

- Título: «¿Cómo se usa esta PC?»
- Subtítulo: elección queda en el equipo; no se sincroniza.
- Cards: Recepción (offline, SQLite, impresora, login local) / Administración remota (consulta y catálogo vía Supabase; requiere internet).
- Estilo Night Ops (`nightdesk-design`); sin marca «Nightdesk» en esta pantalla.

### Comandos

| Comando | Payload → resultado | Notas |
|---|---|---|
| `device_mode_get` | `{}` → `"reception" \| "remote" \| null` | `null` / ausente = primer arranque. Actualizar `docs/contrato-ipc-api.md` (hoy no admite `null`). |
| `device_mode_set` | `{ mode }` → `void` | Solo primer arranque / reset explícito |
| `remote_configure` | `{ project_url, anon_key }` → `void` | Solo modo remoto. Keyring; nunca en `settings` ni respuesta. Distinto de `sync_configure_device` (recepción + device Auth). |

## 4. Transporte

```
UI → api.*() → cmd() → invoke | supabaseInvoke | mockInvoke
```

### `cmd()` en `api.ts`

| Condición | Transporte |
|---|---|
| No Tauri (`npm run dev`) | `mockInvoke` (mock puede simular modo remoto) |
| Tauri + `reception` | `invoke` |
| Tauri + `remote` | `supabaseInvoke` **salvo** la allowlist local abajo |

**Siempre `invoke` (o mock), incluso en modo remoto** — nunca pasan por Supabase:

| Comando | Motivo |
|---|---|
| `device_mode_get` / `device_mode_set` | Ajuste de dispositivo local; puede ejecutarse antes de tener modo/cliente |
| `remote_configure` | Escribe URL/anon en Credential Manager de esa PC |
| `hash_password` | Argon2id en Rust local del PC admin |

Componentes **nunca** importan `@tauri-apps/api` ni `@supabase/supabase-js`.

### `src/lib/supabase.ts` (nuevo)

- Dependencia `@supabase/supabase-js` en `package.json`.
- Cliente creado con URL + anon del keyring/config (no del repo).
- `persistSession: false` (sesión Auth en memoria, alineado con `docs/acceso-sesiones.md`).
- `supabaseInvoke(name, args)`: mismos nombres snake_case del contrato v1; errores como `ApiError { code, message }` en español de dominio.

### Mapa remoto (resumen)

| Grupo | Comportamiento |
|---|---|
| Lecturas operativas / catálogo | PostgREST / vistas / RPC → tipos de `types.ts` |
| `preview_bill` | Estimativo con `billing.ts`; UI etiqueta «estimativo» |
| Mutaciones operativas + impresión / PDF / PIN | `forbidden` |
| Escrituras de catálogo / ajustes de negocio / usuarios | RPC `catalog_upsert_*` (o equivalente N12) con `expected_version` |
| Auth | Solo en remoto: Auth email/password; recepción no usa Auth |

Detalle comando a comando: tabla existente en `docs/contrato-ipc-api.md` (actualizar si se añaden stubs).

### Mock

- Flag / setting mock de `device_mode`.
- `mockInvoke` reproduce lecturas, `forbidden` en operativo remoto, y estados de sync stub.
- Sin secretos reales en `localStorage`.

## 5. Login y sesión

| Modo | Login | Sesión |
|---|---|---|
| Recepción | Local (`users` SQLite, Argon2id) — **sin Supabase** | Token de sesión local en memoria (como hoy) |
| Remoto | Supabase Auth email/password | Sesión Auth en memoria; logout hace `signOut` y limpia vistas |

- Evento `nightdesk-session-expired` (o equivalente Auth) limpia UI.
- En remoto, `auth_setup*` no aplica (setup solo recepción).
- Rol remoto esperado: `admin` en `app_metadata.role` (N12).

## 6. Tablero en vivo (solo remoto)

- Tablero, historial, reservas, detalle de cuenta: solo lectura.
- Realtime: suscripciones solo dentro de `src/lib/supabase.ts` (p. ej. `subscribeBoard(onChange)` / `unsubscribeBoard()`). La UI llama wrappers vía `api.ts` o un módulo fino reexportado; **no** abre canales desde páginas/componentes.
- Realtime sobre `stays`, `rooms`, `charges` (sin polling como fuente primaria).
- Indicador de conexión + última actualización.
- Sin red: aviso visible; guardar deshabilitado.
- `conflict` por `expected_version`: mensaje + recarga; no sobrescribir.
- Estado del equipo de recepción desde `sync_devices` (conectado / última sync / pendientes). Si I07 aún no escribe filas reales, UI muestra vacío o “sin datos” sin inventar conectividad.

## 7. Edición remota de catálogo

- Pantallas existentes (Habitaciones, Catálogo, Ajustes de negocio, Usuarios) reutilizadas.
- Remoto **no** puede cambiar `rooms.status` ni mutar estadías/cargos/reservas (RLS + `forbidden` en UI).
- Writes con `expected_version`; conflicto → recarga.
- Creación de usuarios desde remoto: `hash_password` (comando Rust local Argon2id, mismo formato que `auth.rs`); hash enviado a Supabase, nunca password en claro por PostgREST.
- En recepción con sync futuro: write-through (I11); fuera del núcleo de N07 salvo stubs necesarios para no romper IPC.

## 8. Sync UI en recepción (stubs hasta I07)

### AppShell

Indicador Night Ops (light/dark): conectado / sincronizando / sin conexión con N pendientes / error. Fuente: `sync_status` stub.

### Ajustes → «Sincronización»

- Última push/pull, cola pendiente, último error.
- «Sincronizar ahora» → `sync_pull_now` stub.
- «Configurar dispositivo» → `sync_configure_device` stub: acepta `project_url`, `anon_key`, `device_email`, `device_password`; persiste en Credential Manager cuando el backend lo soporte; **nunca** muestra de vuelta secretos ni los guarda en `settings` visibles / SQLite de negocio.

### Evento

Pantallas de tablero/catálogo/ajustes escuchan `sync:catalog-updated` y refrescan. Hasta I07 el evento puede no emitirse en producción; mock puede simularlo.

### Stubs — contrato

| Comando | Comportamiento stub | Luego (I07) |
|---|---|---|
| `sync_status` | Valores por defecto / “no configurado” / cola 0 | Worker real |
| `sync_pull_now` | no-op o error claro si no hay dispositivo | Pull incremental + evento |
| `sync_configure_device` | Guarda en keyring si hay crate; si no, error controlado documentado | Mismo, usado por worker |

## 9. Credenciales (acuerdo operativo)

- Instalador **sin** secretos.
- Recepción: una vez, `sync_configure_device` (URL + anon + device) → Credential Manager; sync/backup los leen en runtime.
- Admin remoto: `remote_configure` (solo URL + anon) en esa PC; login Auth; sesión en memoria. **No** reutilizar el payload de `sync_configure_device` (incluye device email/password).
- En git/Jira solo «configurado». Cero `.env` embebido en el binario.

## 10. Fuera de alcance (no implementar en N07)

- Worker push/pull/Realtime Rust completo (I07).
- Write-through completo de catálogo desde recepción (I11), salvo lo mínimo para no romper builds.
- Backups cifrados / N08 UI de recuperación.
- Instalador N09 / capacitación N10.
- WireGuard, API HTTP `/api/v1`, servicio Windows.

## 11. Criterios de aceptación (agregados)

1. Primer arranque elige modo; recepción opera offline con login local.
2. Remoto: Auth Supabase; sin secretos en `localStorage` ni en el repo.
3. `cmd()` enruta `invoke | supabaseInvoke | mockInvoke`; UI sin imports directos de Tauri/Supabase.
4. Lecturas remotas tipadas; operativo e impresión → `forbidden`.
5. Admin cambia tarifa (remoto) → con sync real (I07) recepción la refleja &lt; 3 s; en N07 con stub, mock demuestra el refresco por evento.
6. Dos ediciones concurrentes de catálogo → segundo recibe `conflict` y recarga.
7. Indicador sync + sección Ajustes presentes; stubs no bloquean operación de recepción.
8. Estilo Night Ops; pantalla de modo sin marca «Nightdesk».

## 12. Verificación

- `npm run build` (tsc).
- Mock browser: modo remoto simulado + `forbidden` en operativo.
- `npm run tauri dev`: modo recepción intacto; modo remoto si hay URL/anon de prueba (no commiteados).
- `cargo test` sin regresiones en billing/auth existentes.
- Sin tests de frontend nuevos (convención del repo).

## 13. Orden de implementación sugerido

1. `device_mode_get/set` + pantalla de arranque + mock.
2. `remote_configure` + keyring stub; `@supabase/supabase-js` + `supabase.ts` + `cmd()` por modo (con allowlist local) + casos lectura/`forbidden`.
3. Login Auth remoto.
4. Realtime tablero/historial/reservas (wrappers en `supabase.ts`) + preview estimativo.
5. Escrituras catálogo + `hash_password` + conflictos.
6. Stubs `sync_*` + indicador AppShell + sección Ajustes + listener de evento.
7. Actualizar `docs/contrato-ipc-api.md` (incl. `null` en `device_mode_get`, `remote_configure`) / evidencia en Jira (MOT-54…56).
