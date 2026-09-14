# Recepción y control de cuentas

MOT-8 (N02) deja el cierre de una estadía como una operación de cuenta interna para la recepción local.

## Recorrido operativo

1. Recepción abre una estadía desde el tablero o toma una reserva vigente.
2. Mientras la estadía está abierta, la cuenta se recalcula en SQLite con tarifa, horas extra, consumos, recargos, descuentos e impuesto configurado.
3. `Cerrar cuenta` guarda las líneas calculadas, cierra la estadía en una transacción y marca la habitación como `dirty` para limpieza.
4. El total cerrado se reconstruye desde los cargos guardados. Cambiar una tarifa o el impuesto después del cierre no modifica el historial.
5. El ticket es opcional. Si la impresora falla, la cuenta permanece cerrada y el ticket queda archivado localmente para reintento.

El cierre no solicita medio de pago ni importe recibido y no crea nuevos registros en `payments`. La tabla existente se conserva para datos históricos de versiones anteriores.

## Reglas de consistencia

- No se puede cerrar dos veces la misma estadía.
- No se pueden agregar ni quitar cargos una vez cerrada la cuenta.
- Una habitación sucia requiere limpieza antes de un nuevo check-in.
- Una reserva activa no puede duplicarse para la misma habitación y fecha.
- Una reserva usada debe coincidir con su habitación y tarifa.
- El modo demo conserva el mismo contrato y las mismas reglas mediante `localStorage`.

## Evidencia técnica

- Migración `004_account_closure` e índice para cargos por estadía.
- Backend Rust: `check_out`, historial y reimpresión usan el total persistido al cierre.
- Interfaz React: `RoomDrawer` muestra `Cerrar cuenta` y el historial muestra cuentas cerradas.
- Pruebas Rust: cierre persiste el total y no crea pagos; segundo cierre rechazado sin reescribir cargos.
- Verificación: `cargo test --manifest-path src-tauri/Cargo.toml` (35 pruebas) y `tsc --noEmit`.

En Windows, la compilación se ejecuta con `--configLoader runner` para evitar la lectura del directorio padre de OneDrive por el cargador esbuild predeterminado; con ese cargador el build de producción completa correctamente.
