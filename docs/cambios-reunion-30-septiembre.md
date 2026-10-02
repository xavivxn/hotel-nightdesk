# Cambios de la reunión del 30/09/2026

Implementados el 02/10/2026. Estado: en el código local, **sin commit, sin push y sin instalador**. La app instalada (0.1.11) no los tiene hasta generar e instalar una versión nueva.

## Resumen

| Pedido de la reunión | Resultado |
|---|---|
| Una hora completa siempre se cobra como hora | Hecho. 2:30 = 2 h + 1 adicional de 30 min. |
| Habitaciones jacuzzi y normales con precios distintos | Hecho. Cada tarifa tiene categoría Normal / Con jacuzzi. |
| El admin cambia precio por hora, dormida y adicional 30 min de cada tipo | Hecho desde «Habitaciones y tarifas». |
| Historial con el usuario que hizo la operación | Hecho. Columnas «Check-in por» y «Cobró». |
| 3 PCs (2 recepción + 1 admin) con la misma base | Analizado: no soportado hoy con 2 recepciones. Ver más abajo. |
| Impresión doble fija | Hecho. 2 copias al cobrar; reimpresión desde Historial, 1 copia. |

Decisiones tomadas con Naser al implementar:

- Regla horaria: cada hora completa al precio de la hora; 30 min sueltos = 1 adicional. Tolerancia de 5 min.
- Precios iniciales de jacuzzi: copia de los normales (45.000 hora / 15.000 adicional / 120.000 dormida).
- Historial: se registra quién hizo el check-in y quién cobró.
- Impresión: 2 copias solo al cobrar.

## 1. Cobro por hora

Antes: 2:30 se cobraba como 1 hora + adicional + adicional.
Ahora: pasadas las horas incluidas y la tolerancia, cada 30 min iniciados es un bloque. Dos bloques forman una hora completa al precio de la hora (`base / horas incluidas`). Un bloque suelto es la línea «Adicional 30 min».

Ejemplos con la tarifa de 1 hora (45.000 / adicional 15.000, tolerancia 5 min):

| Tiempo | Antes | Ahora |
|---|---|---|
| 1:00 a 1:05 | 1 h = 45.000 | 1 h = 45.000 |
| 1:30 | 1 h + adicional = 60.000 | 1 h + adicional = 60.000 |
| 2:00 | 1 h + 2 adicionales = 75.000 | 2 h = 90.000 |
| 2:30 | 1 h + 3 adicionales = 90.000 | 2 h + adicional = 105.000 |
| 2:40 | 1 h + 4 adicionales = 105.000 | 3 h = 135.000 |
| 3:00 | 1 h + 4 adicionales = 105.000 | 3 h = 135.000 |

El ticket muestra la estadía como «1 hora (N h)» y, si corresponde, una línea «Adicional 30 min».

Archivos:

- `src-tauri/src/billing.rs`: `bill_hourly` reescrito. Es la fuente de verdad. Tests nuevos: 2:30, 5 min pasada la media hora, estadías largas (102 h). Tests existentes actualizados a la regla nueva.
- `src/lib/billing.ts`: mismo cálculo para la vista previa del navegador (`npm run dev`).

## 2. Tarifas jacuzzi y normales

- Cada tarifa tiene `room_category`: `normal` o `jacuzzi`.
- Una habitación es jacuzzi si su tipo contiene «jacuzzi» (sin distinguir mayúsculas). Mismo criterio en Rust (`models::room_category`) y TypeScript (`roomCategory` en `src/lib/format.ts`).
- Al instalar o actualizar se crean «1 hora Jacuzzi» y «Dormida Jacuzzi» copiando las normales. Usan identificadores fijos, iguales en recepción y en Supabase, para que la sincronización no las duplique:
  - `7a3c0f10-2026-4930-8000-000000000001` (hora)
  - `7a3c0f10-2026-4930-8000-000000000002` (dormida)
- El check-in, el tablero y las reservas solo ofrecen tarifas de la categoría de la habitación. El backend rechaza una tarifa de otra categoría con el mensaje «La tarifa «X» es para habitaciones con/sin jacuzzi…».
- El pase a dormida (manual o automático al cruzar la hora de corte) usa la dormida de la categoría de la habitación. Si no existe, usa cualquier dormida activa.

### Pantalla «Habitaciones y tarifas» (admin)

- Formulario de tarifa: campo nuevo «Habitaciones» (Normal / Con jacuzzi).
- Lista agrupada en «Habitaciones normal» y «Habitaciones con jacuzzi». Cada tarifa por hora muestra el precio y el adicional de 30 min.
- Para cambiar los precios: editar «1 hora» / «1 hora Jacuzzi» (monto base = precio de la hora; «Adicional 30 min») y «Dormida» / «Dormida Jacuzzi» (monto base = precio de dormida).

Archivos:

- `src-tauri/migrations/020_jacuzzi_rates_and_stay_users.sql`: columna `rate_plans.room_category`.
- `src-tauri/src/db.rs`: registro de la migración 020, `seed_jacuzzi_plans_if_missing` (idempotente por identificador), lectura de `room_category`, `find_plan_by_kind` por categoría.
- `src-tauri/src/models.rs`: `RatePlan.room_category`, `SaveRatePlanPayload.room_category`, `room_category()`.
- `src-tauri/src/service.rs`: `ensure_rate_fits_room` en check-in y reservas; `save_rate_plan` guarda y valida la categoría; vista previa y pase a dormida por categoría.
- `src-tauri/src/sync/catalog.rs` y `sync/bootstrap.rs`: `room_category` viaja en la sincronización del catálogo. Si el servidor todavía no la manda, se asume `normal`.
- `src/pages/RoomsPage.tsx`, `src/components/board/RoomDrawer.tsx`, `src/pages/ReservationsPage.tsx`: selector y filtros.
- `src/lib/types.ts`, `src/lib/supabase.ts`, `src/lib/mock.ts`: tipo `RoomCategory` y mapeos.

## 3. Usuario en el historial

- Cada estadía guarda `checked_in_by` (quién hizo el check-in) y `checked_out_by` (quién cobró).
- El nombre lo pone el backend a partir de la sesión iniciada; la pantalla no puede enviarlo ni falsificarlo.
- Historial: columnas nuevas «Check-in por» y «Cobró». Las cuentas cerradas antes de este cambio muestran «—».
- El check-in de una reserva también registra el usuario.
- Los dos campos se sincronizan a Supabase, así que la administración remota también los ve.

Archivos:

- `src-tauri/migrations/020_jacuzzi_rates_and_stay_users.sql`: columnas `stays.checked_in_by` y `stays.checked_out_by`.
- `src-tauri/src/commands.rs`: `check_in`, `check_out` y `check_in_reservation` toman el usuario de la sesión.
- `src-tauri/src/service.rs`: lo guardan en el INSERT del check-in y en el cierre de la cuenta.
- `src-tauri/src/db.rs`: lectura y envío en la cola de sincronización.
- `src/pages/HistoryPage.tsx`: columnas nuevas.

## 4. Impresión doble

- Al cobrar con impresión activada se envían **2 copias** del mismo ticket. El ticket se archiva una sola vez.
- «Reimprimir» desde Historial imprime **1 copia**.
- Si la primera copia falla, no se intenta la segunda y se muestra el aviso. Un fallo de impresora nunca revierte el cobro.

Archivos:

- `src-tauri/src/printer.rs`: `print_copies` (archiva una vez y envía N copias). `print_bytes` sigue imprimiendo 1.
- `src-tauri/src/commands.rs`: constante `CHECKOUT_RECEIPT_COPIES = 2` usada en `check_out`.

## 5. Supabase

Archivo nuevo: `supabase/migrations/20261002120000_jacuzzi_rates_and_stay_users.sql`. **Hay que aplicarlo en Supabase antes de usar la versión nueva con sincronización.** Hace:

- Columna `rate_plans.room_category` (`normal` / `jacuzzi`).
- Columnas `stays.checked_in_by` y `stays.checked_out_by`.
- Crea las tarifas jacuzzi con los mismos identificadores fijos que recepción.
- Redefine `catalog_upsert_rate_plan`, `catalog_write`, `sync_bootstrap_catalog` y `sync_apply_ops` para incluir los campos nuevos.

Sin esta migración, editar una tarifa jacuzzi con sincronización activa da conflicto y los usuarios del historial no llegan a la administración remota.

## 6. Análisis: 3 PCs (2 recepción + 1 administración)

| Configuración | ¿Funciona hoy? |
|---|---|
| 1 recepción + 1 administración remota | Sí. La administración lee por Supabase y edita catálogo, tarifas y usuarios; recepción recibe esos cambios por sincronización. |
| 2 recepciones con la misma base | **No.** |

Por qué no funcionan 2 recepciones:

- Cada recepción tiene su propio SQLite y es la autoridad de lo operativo (ocupación, cuentas, reservas).
- Las operaciones suben a Supabase, pero **no bajan** a otra recepción. A recepción solo bajan catálogo, ajustes y usuarios.
- Resultado: una recepción no ve lo que ocupa la otra, se puede ocupar dos veces la misma habitación (se rompe «una estadía abierta por habitación») y los totales e historial quedan partidos.
- Hacer que ambas escriban directo en Supabase rompe la regla de que recepción funcione sin internet.

Opciones (pendiente de decisión de Naser):

1. **La recepción principal sirve a la segunda por la red local.** Sigue funcionando sin internet; la PC principal tiene que estar prendida. Es la opción más coherente con la arquitectura y la que más trabajo lleva.
2. **Supabase pasa a ser la base operativa.** Exige internet para operar.
3. **La segunda PC queda solo de consulta**, en modo administración remota.

## 7. Documentación actualizada

- `docs/configuracion-operativa.md` §3: tarifas iniciales reales, regla horaria, categorías, historial y tickets.
- `docs/contrato-ipc-api.md`: `room_category` en tarifas, `checked_in_by` / `checked_out_by` en estadías.
- `.cursor/skills/nightdesk-billing/SKILL.md`: regla horaria, categorías e impresión doble.

## 8. Verificación

- `cargo test` (en `src-tauri`): 149 tests aprobados, 0 fallidos. Incluye tests nuevos de categoría de tarifa, usuario de check-in/checkout y regla horaria.
- `npx tsc --noEmit`: sin errores.
- No probado todavía: recorrido en la app instalada, impresión física de las 2 copias y la migración aplicada en Supabase.

## 9. Pasos para entregarlo

1. Aplicar `supabase/migrations/20261002120000_jacuzzi_rates_and_stay_users.sql` en Supabase.
2. Commit y push.
3. Generar el instalador (`npm run build:installer:windows`, ver [actualizaciones.md](actualizaciones.md)) y commitear el bump de versión.
4. Instalar o publicar la actualización.
5. Con el admin, revisar los precios de «1 hora Jacuzzi» y «Dormida Jacuzzi».
6. Si hubiera reservas pendientes en habitaciones jacuzzi hechas con la tarifa normal, rehacerlas: el check-in las rechaza.
