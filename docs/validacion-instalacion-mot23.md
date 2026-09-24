# MOT-23 / MOT-65 — Validación de instalación y operación sin internet

Preparado el 23/09/2026. Responsable de la validación física: Naser. Este guion no acredita una prueba hasta que se complete la columna de resultado con evidencia. Hay una sola PC disponible; la instalación en administración y la restauración en otra PC quedan pendientes.

## Preparación sin tocar datos reales

1. Registrar versión, arquitectura y SHA-256 del instalador, versión de Windows, equipo, usuario de Windows, modo elegido y hora local. Conservar el paquete usado.
2. Usar una **cuenta de Windows de prueba o una VM** con directorio de datos de la aplicación vacío y separado del operativo. La app guarda `nightdesk.db` fuera del repositorio, en el directorio de datos de ese usuario. No renombrar, borrar, copiar en caliente ni sustituir la base de recepción en uso. Crear sólo huéspedes, habitaciones y cuentas ficticios.
3. Probar modo Recepción en la PC disponible. No alternar entre Recepción y Administración remota sobre el mismo directorio de datos para simular dos equipos. La segunda instalación requiere un entorno independiente.
4. Cortar internet sólo en el entorno de prueba: desactivar o desconectar todas sus interfaces y confirmar que no queda conexión activa. No interrumpir una recepción en servicio. La impresora USB puede permanecer conectada.
5. No guardar contraseñas, tokens, clave AES, anon key ni datos de huéspedes reales en capturas, logs, Jira o este documento. Para restauración, detener escrituras, conservar un snapshot consistente previo y seguir [la guía de recuperación](guia-respaldos-recuperacion.md). Nunca ensayar sobre producción.

## Inventario y evidencia por equipo

| Dato / evidencia | PC de recepción disponible | PC de administración pendiente |
|---|---|---|
| Identificador del equipo, Windows y arquitectura | Por registrar | No disponible |
| Instalador, versión y SHA-256 | Paquete actual: `Love Nestt Motel App_0.1.1_x64-setup.exe`, 6.857.991 bytes, SHA-256 `64370466C0A19994B1DF37310A0038518AB98125E02ED40A283AB07752213502`; sin firma. Instalación aún no ejecutada. El paquete 0.1.1 registra el inicio automático por usuario al iniciar sesión. | Mismo paquete previsto; equipo no disponible. |
| Cuenta aislada y directorio de datos vacío antes de abrir | Por verificar | Por verificar |
| Instalación limpia, modo al iniciar, creación de admin | No ejecutado | No ejecutado |
| Cierre/reapertura con datos conservados | No ejecutado | No ejecutado |
| Desinstalación/reinstalación con datos ficticios | No ejecutado | No ejecutado |
| Impresora, cola exacta, papel | Naser informó Epson TM-T(203dpi) Receipt6, 80 mm; instalación física no verificada | No corresponde: admin no imprime |
| Conectividad Supabase, sin exponer credenciales | Por verificar: Credential Manager y migración del JSON antiguo | Por verificar |
| Capturas o acta sin datos sensibles | Por adjuntar | Por adjuntar |

El modelo y ancho de impresora son datos informados por Naser. No prueban que el instalador haya imprimido ni cortado papel. Registrar el nombre exacto de la cola que muestre Windows durante la validación.

## Recorrido de aceptación

Identificar los datos ficticios con `PRUEBA-MOT65-...`. Para cada fila registrar hora, usuario, resultado (`Aprobado`, `Falló`, `No ejecutado`) y evidencia vinculada. Reabrir la app para comprobar persistencia: un botón que responde no basta.

| ID | Equipo / conexión | Pasos | Resultado esperado | Resultado / evidencia |
|---|---|---|---|---|
| R01 | Recepción, offline | Instalar en cuenta aislada, elegir Recepción, crear admin local, cerrar y reabrir, iniciar sesión. | App y login local funcionan sin Supabase; no se reutiliza otra base. | No ejecutado / — |
| R02 | Recepción, offline | Crear una reserva ficticia para habitación libre; buscarla en Reservas y tablero. | Persiste al reabrir; no permite duplicar habitación/franja. | No ejecutado / — |
| R03 | Recepción, offline | Ingresar la reserva; intentar un segundo ingreso para esa habitación. | Una estadía abierta; habitación ocupada; segundo ingreso rechazado sin duplicar. | No ejecutado / — |
| R04 | Recepción, offline | Añadir dos consumos activos; revisar descripción, precio y total en Gs. en la cuenta. | Cargos e importes enteros persisten sin red; productos inactivos no se pueden consumir. | No ejecutado / — |
| R05 | Recepción, offline | Cerrar la cuenta con impresión activada; reabrir Historial. | Cierre único y atómico; total histórico persistido; habitación queda sucia. Fallo de impresora no revierte el cierre. | No ejecutado / — |
| R06 | Recepción, offline | Intentar reingresar antes de limpiar; marcar habitación limpia y reintentar. | Ingreso rechazado mientras está sucia; admitido tras limpieza. | No ejecutado / — |
| R07 | Recepción, offline | Buscar R05 en Historial y comparar total/ticket congelado. Intentar editar catálogo con sync habilitado. | El cierre no cambia con precios nuevos. Edición de catálogo offline muestra error claro y no modifica el local; no bloquea el resto de recepción. | No ejecutado / — |
| R08 | Recepción, offline | Exportar PDF diario del día de R05 desde Historial y abrir archivo local. | PDF legible con cuenta e importes correctos, sin internet. No se presenta como factura ni pago. | No ejecutado / — |
| R09 | Recepción, offline | Imprimir prueba, ticket de R05 y reimpresión en la Epson de 80 mm. | Papel legible, márgenes, acentos y corte correctos; reimpresión no cambia ni duplica cierre. Foto con datos ficticios. | No ejecutado / — |
| R10 | Recepción, offline | En otra cuenta ficticia, desconectar impresora antes de cerrar; reconectar y reimprimir. | Cierre persiste con error visible, sin falso éxito; reimpresión posterior correcta. | No ejecutado / — |
| C01 | Recepción, usuario Windows interactivo | Configurar credenciales de dispositivo de prueba; comprobar una entrada genérica de la app en Windows Credential Manager, ausencia de `device_supabase.json`, cerrar y reabrir. En una instalación aislada anterior con el JSON legado, verificar la migración y eliminación del archivo. | El dispositivo vuelve a conectarse sin contraseña en texto plano. Si se elimina el directorio de datos de prueba, una credencial huérfana no configura automáticamente una instalación nueva. | No ejecutado / — |
| S01 | Recepción, corte y reconexión | Con dispositivo de prueba configurado, ejecutar varias R02–R05 offline; anotar cola, reconectar y cotejar réplica. | La cola crece sin bloquear operación, drena en orden al volver, sin reservas/estadías/cargos duplicados. | No ejecutado / — |
| S02 | Dos equipos, online | Admin remoto consulta tablero/historial, cambia producto de prueba; recepción recibe cambio. Intentar cerrar/imprimir desde admin. | Réplica y catálogo convergen; admin no puede operar estadías ni impresora. | Pendiente de segunda PC / — |
| B01 | Dos equipos | Crear backup cifrado en recepción, confirmar Storage y restaurar en data dir limpio de segundo equipo. | Integridad verificada; habitaciones, R05, historial y ticket disponibles; outbox vieja no reenviada. | Pendiente de segunda PC / — |

### Actualización y reinstalación

En la cuenta de prueba, anotar ID y total de R05. Cerrar la app, aplicar un paquete nuevo y reabrir. Verificar migración, conservación de habitaciones, cuentas, historial y tickets, y versión esperada. Probar desinstalación/reinstalación sólo con datos ficticios y registrar qué hizo realmente el instalador con el directorio de datos. Registrar avisos UAC/SmartScreen o WebView2. **No ejecutado.**

## Evidencia técnica al preparar el guion

| Comando | Resultado el 23/09/2026 | Qué no acredita |
|---|---|---|
| `node scripts/test-daily-pdf.mjs` | Pasó: PDF multipágina, importes, acentos, escapes, offsets, streams y día vacío. | Exportación desde el instalador. |
| `node scripts/test-auth.mjs` | Pasó tras corregir el fixture de permisos y estabilizar la fecha de corte: setup, login, permisos, operación, expiración, logout y bloqueo de intentos. | Verifica el mock, no la instalación ni el ejecutable Tauri. |
| `cargo test --offline --locked --manifest-path src-tauri/Cargo.toml` | Pasó: 121 pruebas Rust, 0 fallidas, 3 ignoradas. Dos requieren impresora física; la prueba de Credential Manager necesita un inicio de sesión interactivo de Windows, ausente en esta sesión aislada. Incluye conservación de modo, impresora, cuenta cerrada y ticket al migrar SQLite, y rechazo de URLs HTTP remotas. | Instalación, red real, acceso real a Credential Manager e impresión física. |
| `tauri build --bundles nsis --ci --no-sign` | Pasó: NSIS x64 v0.1.0 generado en `src-tauri/target/release/bundle/nsis/`, hash y tamaño registrados arriba. El script usa caché local de herramientas en `target/.tauri`. | Ejecución del instalador, actualización de una instalación existente, firma de código y prueba entre dos PCs. |

## Bloqueantes e incidencias

Crear bug vinculado a [MOT-65](https://naserfer.atlassian.net/browse/MOT-65) con versión y SHA-256 del instalador, equipo/Windows/rol, estado de red e impresora, pasos numerados, **esperado**, **actual**, severidad e impacto, hora y evidencia sin secretos ni datos personales. Para cierre, indicar el ID ficticio y si el total persistió; para red, tamaño de cola antes/después y duplicados. Aplicar etiqueta `bloqueada` con motivo sólo si realmente impide continuar. No marcar MOT-65 como hecho antes de completar las pruebas físicas de recepción y los criterios que necesitan el segundo equipo.
