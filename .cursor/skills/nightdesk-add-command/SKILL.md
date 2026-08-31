---
name: nightdesk-add-command
description: >-
  Add or change a Tauri IPC command in Nightdesk across models, commands.rs,
  lib.rs, api.ts, types.ts, and mock.ts. Use when adding a command, payload,
  invoke, mockInvoke case, or a UI screen that calls the backend.
---

# Agregar comando Tauri

Los cinco (o seis) puntos van **juntos**. Mismo nombre snake_case en Rust, `api.ts` y `mock.ts`.

## Checklist

1. **Tipos** — struct/enum + payload en `src-tauri/src/models.rs` y espejo en `src/lib/types.ts` (snake_case JSON, `null` ↔ `Option<T>`).
2. **Handler** — `#[tauri::command]` en `src-tauri/src/commands.rs`. Dominio: `AppError::msg("…")` en español. `AppResult<T>`.
3. **Registro** — `tauri::generate_handler![..., commands::nombre]` en `src-tauri/src/lib.rs`.
4. **SQLite** — queries en `src-tauri/src/db.rs` si toca la DB. Schema nuevo: skill `nightdesk-schema`.
5. **Frontend** — método en `src/lib/api.ts` vía `cmd(...)`. Payload anidado `{ payload }` cuando el comando Rust recibe `payload: …Payload`.
6. **Mock** — `case "nombre":` en `src/lib/mock.ts` (`mockInvoke`). Misma semántica que Rust.

La UI solo usa `import { api } from "@/lib/api"`. Nunca `invoke` en componentes.

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

El frontend muestra `String(e)`. No inventar toasts ni error boundaries. Impresora: `string | null` (`print_error`), no excepción que deshaga el cobro.

## Cobro u ocupación

Si el comando mueve dinero o estados de habitación/stay/reserva, aplicar también `nightdesk-billing` o `nightdesk-schema`.
