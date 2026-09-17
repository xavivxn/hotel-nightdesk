---
name: nightdesk-add-command
description: >-
  Add or change a Tauri IPC command in Nightdesk across models, service.rs,
  commands.rs, lib.rs, api.ts, types.ts, mock.ts, and supabaseInvoke. Use when adding a command,
  payload, invoke, mockInvoke case, supabaseInvoke case, or a UI screen that calls the backend.
---

# Agregar comando Tauri

Los puntos van **juntos**. Mismo nombre snake_case en Rust, `api.ts` y `mock.ts`. Contrato: `docs/contrato-ipc-api.md`. Arquitectura remota: `docs/arquitectura-offline-supabase.md`.

## Checklist

1. **Tipos** — struct/enum + payload en `src-tauri/src/models.rs` y espejo en `src/lib/types.ts` (snake_case JSON, `null` ↔ `Option<T>`). Mutaciones: `operation_id` y `expected_version` opcionales.
2. **Servicio** — reglas, SQL y transacción en `src-tauri/src/service.rs` (`&Connection`/`&mut Connection`, `&Actor`, payload). Sin `tauri::`, `State` ni `AppHandle`. Autorización de rol: `service::authorize(actor, Operation)`. Errores: `AppError::msg` / `conflict` / `not_found` / `forbidden` en español. **Mutación operativa** (check-in, checkout, cargos, reservas, `set_room_status`, huésped): `sync::outbox::enqueue` **en la misma TX** (I06). Catálogo con sync habilitado: write-through a Supabase, no outbox local divergente.
3. **Adaptador** — `#[tauri::command]` en `src-tauri/src/commands.rs`. Primero `crate::auth::require(&state, session_token.as_deref(), admin)?;`, después `conn(&state)`, después `service::…`. Impresión solo aquí.
4. **Registro** — `tauri::generate_handler![..., commands::nombre]` en `src-tauri/src/lib.rs`.
5. **SQLite** — queries en `src-tauri/src/db.rs` si toca la DB. Schema nuevo: skill `nightdesk-schema`.
6. **Frontend** — método en `src/lib/api.ts` vía `cmd(...)`. Payload anidado `{ payload }` cuando el comando Rust recibe `payload: …Payload`. Mutaciones de estadía/cargo/reserva generan `operation_id` con `crypto.randomUUID()`.
7. **Mock** — `case "nombre":` en `src/lib/mock.ts`. Misma semántica y mismos códigos (`fail(code, message)`).
8. **supabaseInvoke** — `case` en `src/lib/supabase.ts` (N07): lectura operativa → PostgREST; escritura de catálogo → RPC `catalog_upsert_*`; impresión y mutación operativa → `forbidden`. Si el comando no aplica en remoto, documentarlo en la tabla del contrato.
9. **Contrato** — actualizar `docs/contrato-ipc-api.md`: tabla comando → IPC / supabaseInvoke / rol, y el payload.

La UI solo usa `import { api } from "@/lib/api"`. Nunca `invoke` ni cliente Supabase en componentes.

## Payloads y naming

```typescript
// api.ts
checkIn: (payload: CheckInPayload) => cmd<Stay>("check_in", { payload }),
previewBill: (stay_id: number) => cmd<BillPreview>("preview_bill", { stay_id }),
```

Args sueltos (`stay_id`, `room_id`, `status`) van planos. Structs de escritura van en `{ payload }`.

## PIN y settings

- `get_settings` / respuesta de `save_settings`: `pin_hash` vacío hacia el frontend.
- Hash en DB (`db::hash_pin`); nunca loguear ni devolver el PIN en claro.

## Errores

Serialización `{ code, message }`. El frontend envuelve en `ApiError` (`src/lib/errors.ts`); `toString()` es el mensaje en español. Sesión: `error.code === "session_expired"`. Impresora: `string | null` (`print_error`), no excepción que deshaga el cobro.

## Cobro u ocupación

Si el comando mueve dinero o estados de habitación/stay/reserva, aplicar también `nightdesk-billing` o `nightdesk-schema`.
