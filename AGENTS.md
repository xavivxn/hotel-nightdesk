# Nightdesk

Recepción hotel/motel 100% offline: Tauri 2 + React + SQLite local. Sin red, sin sync, sin nube.

## Mapa

```
src/pages/                 pantallas (Board, Reservas, Habitaciones, Historial, Ajustes)
src/components/board/      tablero y drawers de check-in/checkout
src/components/ui/         Button, Drawer, Field
src/lib/api.ts             único puente IPC (nunca invoke desde UI)
src/lib/mock.ts            fallback browser (`npm run dev`)
src/lib/types.ts           espejo de models.rs
src/lib/billing.ts         preview mock; la autoridad es billing.rs
src-tauri/src/commands.rs  24 comandos Tauri
src-tauri/src/billing.rs   cobro (fuente de verdad + tests)
src-tauri/src/db.rs        SQLite, seed, queries
src-tauri/migrations/      schema (hoy 001_init.sql)
```

## Skills

Leer el skill que corresponda **antes** de tocar esa área:

| Skill | Cuándo |
|-------|--------|
| `nightdesk-conventions` | archivos nuevos, features, idioma, verificar |
| `nightdesk-design` | UI, CSS, componentes, tablero, light/dark, Night Ops |
| `nightdesk-billing` | tarifas, IVA, preview, checkout, recargos, tickets |
| `nightdesk-add-command` | comando Tauri, payload, llamada desde UI |
| `nightdesk-schema` | migraciones, habitaciones, reservas, ocupación |

No hay tests de frontend.

## Invariantes

- Montos: enteros en **guaraníes** (campos `*_cents` heredan el nombre, 1 = 1 Gs). `billing.rs` manda; `billing.ts` y sus tests se actualizan juntos.
- Una estadía `open` por habitación. Check-in → `occupied`. Checkout → `dirty` (nunca `available`).
- Checkout atómico. Fallo de impresora **no** revierte el cobro.
- Dual-mode: cada comando nuevo vive en `models.rs` + `types.ts` + `commands.rs` + `lib.rs` + `api.ts` + `mock.ts`.
