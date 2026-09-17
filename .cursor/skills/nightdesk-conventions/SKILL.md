---
name: nightdesk-conventions
description: >-
  Define folder layout, dual-mode Tauri/mock, language (English code / Spanish UI),
  money in guaraníes (fields still named *_cents), and how to verify Nightdesk changes.   Use when adding files or
  features, choosing where code lives, working on api.ts, mock.ts, supabase.ts, pages, or
  verifying the hotel-nightdesk repo.
---

# Convenciones Nightdesk

App de recepción **offline-first**. Código en inglés; UI y errores de dominio en español (es-AR).

## Dónde va cada cosa

| Qué | Dónde |
|-----|--------|
| Pantalla / ruta | `src/pages/*Page.tsx` |
| Tablero, check-in, checkout UI | `src/components/board/` |
| Primitivos (Button, Drawer, Field) | `src/components/ui/` |
| Layout y nav | `src/components/layout/AppShell.tsx` |
| Tipos TS | `src/lib/types.ts` (espejo de `models.rs`) |
| IPC | `src/lib/api.ts` → `api.*()` |
| Transporte remoto | `src/lib/supabase.ts` (`supabaseInvoke`; N07) |
| Errores | `src/lib/errors.ts` (`ApiError`) |
| Mock browser | `src/lib/mock.ts` |
| Preview mock | `src/lib/billing.ts` (solo mock; autoridad: `billing.rs`) |
| Dinero / fechas / labels | `src/lib/format.ts` |
| Comando Tauri | adaptador en `commands.rs` + registro en `lib.rs` |
| Negocio | `src-tauri/src/service.rs` |
| Queries / seed | `src-tauri/src/db.rs` |
| Cobro | `src-tauri/src/billing.rs` |
| ESC/POS | `src-tauri/src/printer.rs` |
| Schema SQLite | `src-tauri/migrations/` |
| Sync recepción | `src-tauri/src/sync/` (I06/I07) |
| Respaldo | `src-tauri/src/backup.rs` (I05/I08) |
| Esquema remoto | `supabase/` (N12) |

Rutas: `/`, `/reservas`, `/habitaciones`, `/historial`, `/ajustes`.

Alias `@/` → `src/`. Estado local (`useState`); no agregar store global.

## Idioma

- Identificadores y JSON: `check_in`, `amount_cents`, `display_status`, `rate_plan_id`.
- UI y `AppError::msg(...)`: español. Labels vía `statusLabel` / `rateKindLabel` / `paymentLabel` en `format.ts`.
- Recibos impresos: PC850 (`ESC t 2`), con áéíóúñ en bytes que también lee la página 0 de la TM-T20. Controles, incluido ESC, salen como `?`.

## Dinero en UI

Enteros en **guaraníes** (1 = 1 Gs). Los campos IPC/DB se llaman `*_cents` por herencia. Mostrar y parsear solo con `formatMoney` y `parseGuaranies` (`src/lib/format.ts`). Formato: `80.000 Gs.` (símbolo al final, sin decimales). No persistir floats.

## Dual-mode

```
UI → api.*() → cmd() → invoke (Tauri/recepción) | supabaseInvoke (admin remoto) | mockInvoke (browser)
```

- Componentes **nunca** importan `@tauri-apps/api` ni `@supabase/supabase-js` ni llaman `invoke`.
- Detección Tauri: `"__TAURI_INTERNALS__" in window` (ya está en `api.ts`). El modo del equipo (`reception` / `remote`) decide entre `invoke` y `supabaseInvoke`.
- `npm run dev` → Vite + `localStorage` (`nightdesk.mock.v2`).
- `npm run tauri dev` → SQLite en el data dir de la app.

Comando o tipo nuevo: seguir `nightdesk-add-command`. Cobro: `nightdesk-billing`. Schema/ocupación: `nightdesk-schema`. UI/CSS/componentes: `nightdesk-design`. Arquitectura remota: `docs/arquitectura-offline-supabase.md`.

## Offline

Recepción opera al 100 % sobre `{app_data_dir}/nightdesk.db` (no en el repo) **sin internet**. La cola `sync_outbox` acumula y se drena al reconectar. Con sync habilitado, editar catálogo/ajustes de negocio requiere conexión (write-through a Supabase; sin red no se escribe local). Impresora, PIN y PDF no salen del equipo de recepción. Credenciales del dispositivo: Windows Credential Manager, nunca en `settings`.

No reintroducir WireGuard, API HTTP `/api/v1`, servicio Windows ni escritura remota sobre el archivo SQLite.

## Verificar

- Cobro / horas extra / IVA: `cargo test` desde `src-tauri/`.
- UI que toca SQLite, PIN o impresora: `npm run tauri dev`. El mock **no** cubre impresora real.
- No hay tests de frontend; no inventar Vitest.
