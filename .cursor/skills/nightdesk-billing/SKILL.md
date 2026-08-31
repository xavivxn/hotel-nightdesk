---
name: nightdesk-billing
description: >-
  Hotel billing rules for Nightdesk: cents, rate kinds (hourly/night/overnight),
  IVA, live bill preview, checkout transaction, surcharges, and tickets. Use when
  changing billing.rs, billing.ts, check_out, preview_bill, convert_to_overnight,
  add_charge, rates, tax, recargos, pernocte, or ESC/POS receipts.
---

# Cobro Nightdesk

Montos: **enteros en centavos** (`i64` / `number`). Nunca floats en DB ni en IPC.

## Fuente de verdad

`src-tauri/src/billing.rs` es la autoridad en Tauri. `src/lib/billing.ts` solo alimenta el mock.

Si cambia la fórmula:

1. Actualizar `billing.rs`
2. Actualizar `billing.ts` para que el mock coincida
3. Extender o ajustar los tests al pie de `billing.rs`
4. Correr `cargo test` desde `src-tauri/`

## Preview

`preview_bill` / `build_preview` en `commands.rs` arma `BillingContext` y llama `billing::preview`.

- `now` = `check_out_at` si la stay está cerrada; si no, `Local::now()`.
- Líneas manuales: cargos `surcharge` / `discount` de DB.
- IVA: `tax_amount` redondea half-up; `total_cents = subtotal + tax`.

## Tarifas

| `RateKind` | UI |
|------------|-----|
| `hourly` | Por hora (base `included_hours` + extras) |
| `night` | Por noche hasta `night_cutoff_hour` |
| `overnight` | Pernocte (misma lógica que noche) |

- `grace_minutes` aplica a horas extra.
- Hourly que cruza `night_cutoff_hour` **reemplaza** el total hourly por el plan `overnight` (fallback: `night`). No sumar ambas.
- `convert_to_overnight` (`converted_to_overnight = true`) fuerza esa misma sustitución. Requiere un plan `overnight` o `night` activo.

## Cargos manuales

- Solo stay `open`.
- Negativo o `kind == "discount"` → `discount` (monto negativo en DB).
- Resto → `surcharge`.
- `delete_charge` solo borra `surcharge` / `discount`.

## Checkout (`check_out`)

Validar stay `open` y `amount_cents >= bill.total_cents` (el vuelto no se persiste). Luego transacción atómica:

1. `persist_computed_charges`: `DELETE` kinds `stay` / `extra_hour` / `tax`; `INSERT` líneas computadas + tax. **Nunca** borrar `surcharge` / `discount`.
2. Stay `status = 'closed'`, `check_out_at = now`
3. `INSERT` payment (`cash` | `card` | `transfer`)
4. Room `status = 'dirty'`

Todo o nada. Stay cerrada: no `add_charge`.

## Impresora (no revierte cobro)

Si `payload.print`: `build_receipt` + `print_bytes` **después** del `COMMIT`. Error → `CheckOutResult.print_error` (warning). El cobro ya cerró.

Tickets en `{app_data_dir}/tickets/` (`*.bin`, `last-ticket.bin`, `last-ticket.txt`). Reimpresión: `reprint_receipt`. Mock de browser no imprime de verdad.
