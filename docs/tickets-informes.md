# MOT-3 — Tickets e informes

Fecha: 16 de septiembre de 2026.  
Rama: `feature/cambios`.  
Equipo de prueba: Windows, cola `EPSON_TM_T20III_Receipt`, dispositivo `EPSON TM-T(203dpi) Receipt6`, papel de 80 mm.

Este documento cubre el entregable de tickets e informes: N03 (validar impresora física), N05 (impresión y reimpresión) y N06 (PDF diario local). La impresión ocurre solo en la PC de recepción. Un fallo de impresora no revierte ni duplica el cierre.

## Qué quedó hecho

### N05.1 — Ticket desde la cuenta definitiva

- El cierre y el snapshot del ticket se guardan en la misma transacción (`receipt_snapshots`, migración `007_receipts`). Si no se puede guardar el snapshot, el cierre se revierte.
- La impresión ocurre después del commit. El error va en `print_error`. La cuenta queda cerrada y no se registra un pago.
- El ticket incluye consumos, recargos y descuentos con los importes del cierre. No dice «Cuenta #N — TICKET INTERNO».
- El encabezado del ticket es el nombre del establecimiento. En esta instalación pasó de «Nightdesk Inn» a **MotelApp** (migraciones `008` y `009`).
- Reimprimir usa los bytes congelados. Cambiar tarifa, habitación o nombre no altera el ticket ni la cuenta. Una cuenta anterior a la migración 007 congela su primera reconstrucción; no se recuperan encabezados que nunca se guardaron.
- Comandos de impresión y cola: solo IPC local. No hay ruta HTTP. La app de administración no dispara la impresora.

### N06 — PDF diario local

- En Historial, **Exportar PDF diario** arma el resumen y lo guarda en `informes/` con nombre único. El navegador descarga una demostración. No usa internet ni un controlador PDF.
- El informe separa cuentas cerradas, cuentas abiertas al corte y consumos/descuentos del día. Esos subtotales no se suman entre sí.
- Las cuentas abiertas de un día histórico no se valorizan con tarifas actuales. Fechas inválidas y futuras se rechazan.
- El PDF conserva acentos. El ticket térmico usa otra tabla, documentada abajo.

### N03.1 — Cola de Windows

- La app lista colas con `EnumPrinters` y envía ESC/POS en RAW por el spooler, sin shell y sin archivo temporal compartido.
- En Ajustes: buscar impresoras, elegir la cola Epson, ancho 80 mm, activar envío y dejar vacía la ruta de puerto. `/dev/usb/lp0` es un ejemplo de Linux y, si queda guardado, impide usar la cola.
- Una cola que acepta el trabajo no prueba que salió papel. Una cola inexistente devuelve error y no se informa éxito.

### N03.2 y N05.2 — Muestras físicas

Se enviaron muestras a `EPSON_TM_T20III_Receipt`. La cola volvió a `Normal` con `JobCount` 0.

Decisiones tomadas en papel, no solo en código:

| Prueba | Resultado | Decisión |
|---|---|---|
| Windows-1252 (`ESC t 16`) | No se vieron acentos | No se usa |
| PC850 (`ESC t 2`), bytes de áéíóúñ compatibles con la página 0 | Es la tabla que entiende esta TM-T20 | Queda así |
| Título 4× y cuerpo 2× | Demasiado grande | Descartado |
| Cuerpo a doble alto | El salto respecto del tamaño normal es demasiado | Descartado |
| Letra normal en negrita | Más chica de lo pedido | Se agrandó |
| Dibujo un poco más alto, mismo ancho de 12 puntos | Salió chico: el alto pedido no era el alto visible | Descartado |
| Letra dibujada más grande que la Font A nativa y menor que el doble | Muestra enviada el 16/09/2026 | Tamaño en uso |

El cuerpo se dibuja a 18×40 puntos y el título a 24×52. La Epson solo ofrece multiplicadores enteros (1× o 2×); por eso el tamaño intermedio se manda como imagen, no como `GS !`. En 80 mm caben 32 columnas a ese ancho.

Falta, y no se da por cerrado por la aceptación del spooler:

- Foto o nota de Naser de que el papel se lee, con márgenes, corte y acentos.
- Ensayo con la impresora desconectada o sin papel: el cierre debe quedar y el aviso no puede ser un éxito falso.
- Reimpresión de una cuenta ya cerrada antes de quitar «TICKET INTERNO»: ese ticket congelado todavía trae esa línea. Un cierre nuevo no.

## Uso en recepción

1. Conectar la TM-T20 por USB a la PC de recepción.
2. En Ajustes, marcar **Enviar tickets a la impresora**, ancho **80 mm**, cola `EPSON_TM_T20III_Receipt`, ruta de puerto vacía, y guardar.
3. **Imprimir prueba** guarda antes de enviar. Si el guardado falla, no imprime con datos viejos.
4. Al cerrar, si la impresora falla, reimprimir desde la confirmación o desde Historial. No volver a cerrar la cuenta.
5. El informe diario se exporta desde Historial. No es un resumen de dinero cobrado ni una factura.

## Verificación

- Rust: snapshot inmutable, cierre duplicado, rollback si falla el snapshot, impresora desactivada, cola inexistente, PDF/informe de medianoche, día vacío y fechas inválidas.
- `node scripts/test-daily-pdf.mjs`: PDF multipágina, acentos, importes y offsets.
- Muestra física, excluida de la suite normal:

```text
$env:RECEIPT_TEST_PRINTER = "EPSON_TM_T20III_Receipt"
cargo test --offline --manifest-path src-tauri/Cargo.toml physical_printer_sample -- --ignored --nocapture
```

## Transporte

`print_test`, `reprint_receipt`, `list_printers` y `save_daily_pdf` quedan en IPC de recepción. La app administradora, cuando exista, consume el informe por la API privada y no controla la impresora ni el archivo SQLite.

Referencias: [enviar datos RAW al spooler](https://learn.microsoft.com/en-us/windows/win32/printdocs/sending-data-directly-to-a-printer), [EnumPrinters nivel 4](https://learn.microsoft.com/en-us/windows/win32/printdocs/enumprinters).
