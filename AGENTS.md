# Nightdesk

Recepción hotel/motel con operación local sin internet: Tauri 2 + React + SQLite local.

## Arquitectura vigente (17/09 + ampliación LAN 03/10/2026)

Leer `docs/arquitectura-offline-supabase.md` y `docs/contrato-ipc-api.md` antes de implementar usuarios, permisos, sincronización, administración remota o respaldos. `docs/arquitectura-offline-vpn-backups.md` está sustituido (WireGuard + API privada descartados).

Un binario, tres modos: Recepción principal (`reception`), Recepción adicional (`reception_client`) y Administración remota (`remote`). Leer también `docs/recepciones-lan.md`. Se autoriza el servidor HTTPS/WebSocket LAN integrado; la prohibición anterior sigue aplicando a VPN, servicio Windows, archivo compartido y acceso WAN. SQLite en recepción es la única fuente operativa. Supabase es réplica de lectura para el admin, autoridad del catálogo y destino de respaldos diarios cifrados. Sin escritura remota directa sobre el archivo SQLite ni segunda base editable. Catálogo y ajustes del negocio: Supabase gana. Ocupación, cuentas y reservas: recepción gana. La operación local no puede depender de internet.

## Mapa

```
src/pages/                    pantallas (Board, Reservas, Habitaciones, Historial, Ajustes)
src/components/board/         tablero y drawers de check-in/checkout
src/components/ui/            Button, Drawer, Field
src/lib/api.ts                único puente (nunca invoke ni supabase-js desde UI)
src/lib/supabase.ts           supabaseInvoke (N07; mismo contrato v3)
src/lib/mock.ts               fallback browser (`npm run dev`)
src/lib/types.ts              espejo de models.rs
src/lib/billing.ts            preview mock; la autoridad es billing.rs
src/lib/errors.ts             ApiError { code, message }
src-tauri/src/service.rs      reglas de negocio + SQL + TX (sin Tauri)
src-tauri/src/commands.rs     adaptadores de catálogo, equipo y mantenimiento
src-tauri/src/backend.rs      allowlist común IPC/LAN
src-tauri/src/operations.rs   TX, resultado idempotente y auditoría
src-tauri/src/device.rs       configuración y diario por puesto
src-tauri/src/lan/            HTTPS, WS, mDNS, vinculación, cliente y adaptador IPC
src-tauri/src/printing.rs     comprobantes congelados y trabajos por copia
src-tauri/src/billing.rs      cobro (fuente de verdad + tests)
src-tauri/src/db.rs           SQLite, seed, queries
src-tauri/src/sync/           outbox, client, push, pull, bootstrap, realtime, worker (I06/I07)
src-tauri/src/backup.rs       snapshot cifrado a Storage (I05/I08)
src-tauri/src/updater.rs      actualizaciones (bucket privado `updates`; docs/actualizaciones.md)
src-tauri/migrations/         schema SQLite (001_init … 021_local_reception)
supabase/                     esquema Postgres, RLS, RPC (N12)
docs/contrato-ipc-api.md      contrato IPC v3 + mapeo supabaseInvoke
docs/arquitectura-offline-supabase.md
```

## Skills

Leer el skill que corresponda **antes** de tocar esa área:

| Skill | Cuándo |
|-------|--------|
| `nightdesk-conventions` | archivos nuevos, features, idioma, verificar |
| `nightdesk-design` | UI, CSS, componentes, tablero, light/dark, Night Ops |
| `nightdesk-billing` | tarifas, IVA, preview, checkout, recargos, tickets |
| `nightdesk-add-command` | comando Tauri, payload, llamada desde UI, caso supabaseInvoke |
| `nightdesk-schema` | migraciones, habitaciones, reservas, ocupación |
| `nightdesk-updates` | instalador Windows, firma, `latest.json`, bump de versión |

No hay tests de frontend.

## Invariantes

- Montos: enteros en **guaraníes** (campos `*_cents` heredan el nombre, 1 = 1 Gs). `billing.rs` manda; `billing.ts` y sus tests se actualizan juntos.
- Una estadía `open` por habitación. Check-in → `occupied`. Checkout → `dirty` (nunca `available`).
- Checkout atómico. Fallo de impresora **no** revierte el cobro.
- Transporte local: comandos operativos pasan por `reception_invoke` → `backend.rs` → `operations.rs` → `service.rs`. No registrar adaptadores alternativos que omitan idempotencia, autorización o confirmación de cierre. La adicional nunca abre una base operativa ni arranca sync/respaldo.
- Registro durable: efecto + resultado + auditoría + outbox en la misma TX; imprimir después de liberar SQLite.
- Compatibilidad: cada comando nuevo vive en `models.rs` + `types.ts` + `service.rs` + `commands.rs` + `lib.rs` + `api.ts` + `mock.ts`. Con sync: mutación operativa → `enqueue` en la misma TX; comando nuevo → caso en `supabaseInvoke`. `billing.ts` solo alimenta el mock.
