---
name: nightdesk-conventions
description: >-
  Define folder layout, dual-mode Tauri/mock, language (English code / Spanish UI),
  money in cents, and how to verify Nightdesk changes. Use when adding files or
  features, choosing where code lives, working on api.ts, mock.ts, pages, or
  verifying the hotel-nightdesk repo.
---

# Convenciones Nightdesk

App de recepción **offline**. Código en inglés; UI y errores de dominio en español (es-AR).

## Dónde va cada cosa

| Qué | Dónde |
|-----|--------|
| Pantalla / ruta | `src/pages/*Page.tsx` |
| Tablero, check-in, checkout UI | `src/components/board/` |
| Primitivos (Button, Drawer, Field) | `src/components/ui/` |
| Layout y nav | `src/components/layout/AppShell.tsx` |
| Tipos TS | `src/lib/types.ts` (espejo de `models.rs`) |
| IPC | `src/lib/api.ts` → `api.*()` |
| Mock browser | `src/lib/mock.ts` |
| Preview mock | `src/lib/billing.ts` |
| Dinero / fechas / labels | `src/lib/format.ts` |
| Comando Tauri | `src-tauri/src/commands.rs` + registro en `lib.rs` |
| Queries / seed | `src-tauri/src/db.rs` |
| Cobro | `src-tauri/src/billing.rs` |
| ESC/POS | `src-tauri/src/printer.rs` |
| Schema | `src-tauri/migrations/` |

Rutas: `/`, `/reservas`, `/habitaciones`, `/historial`, `/ajustes`.

Alias `@/` → `src/`. Estado local (`useState`); no agregar store global.

## Idioma

- Identificadores y JSON: `check_in`, `amount_cents`, `display_status`, `rate_plan_id`.
- UI y `AppError::msg(...)`: español. Labels vía `statusLabel` / `rateKindLabel` / `paymentLabel` en `format.ts`.
- Recibos impresos: ASCII sin tildes (lo hace `printer.rs`).

## Dinero en UI

Enteros en centavos. Mostrar y parsear solo con `formatMoney` y `pesosToCents` (`src/lib/format.ts`). No persistir floats.

## Dual-mode

```
UI → api.*() → cmd() → invoke (Tauri) | mockInvoke (browser)
```

- Componentes **nunca** importan `@tauri-apps/api` ni llaman `invoke`.
- Detección: `"__TAURI_INTERNALS__" in window` (ya está en `api.ts`).
- `npm run dev` → Vite + `localStorage` (`nightdesk.mock.v1`).
- `npm run tauri dev` → SQLite en el data dir de la app.

Comando o tipo nuevo: seguir `nightdesk-add-command`. Cobro: `nightdesk-billing`. Schema/ocupación: `nightdesk-schema`. UI/CSS/componentes: `nightdesk-design`.

## Offline

No red, no sync, no servicios cloud. DB: `{app_data_dir}/nightdesk.db` (no en el repo).

## Verificar

- Cobro / horas extra / IVA: `cargo test` desde `src-tauri/`.
- UI que toca SQLite, PIN o impresora: `npm run tauri dev`. El mock **no** cubre impresora real.
- No hay tests de frontend; no inventar Vitest.
