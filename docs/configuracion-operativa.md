# MotelApp — Configuración operativa acordada

**Entregable:** [MOT-7 · N01](https://naserfer.atlassian.net/browse/MOT-7), con [MOT-27 · N01.1](https://naserfer.atlassian.net/browse/MOT-27) y [MOT-28 · N01.2](https://naserfer.atlassian.net/browse/MOT-28).  
**Versión:** 1.0 · 14 de septiembre de 2026.  
**Responsable de la documentación:** Naser Fernández.  
**Estado del documento:** configuración disponible consolidada; datos de instalación faltantes registrados en la sección 9.

Este documento registra el alcance confirmado por Naser y los valores concretos disponibles en el repositorio. Los valores de carga inicial del programa están identificados como tales: su existencia en el código no acredita que sean las tarifas, la numeración física o los equipos del motel. Completar esos datos durante la configuración no reabre el alcance.

## 1. Alcance confirmado

| Área | Configuración acordada |
|---|---|
| Habitaciones | 23 en total: 19 normales y 4 con jacuzzi. |
| Equipos y aplicaciones | Dos aplicaciones de escritorio Windows: recepción y administración. |
| Operación local | Habitaciones, reservas, estadías, tarifas, consumos, cuentas e historial sin depender de internet. |
| Usuarios | Cuentas individuales; roles `admin` y `recepcion`. |
| Administración remota | Consultar y modificar datos autorizados mediante API privada autenticada y VPN WireGuard. |
| Disponibilidad remota | Ambas PCs conectadas y recepción encendida. Durante un corte, recepción continúa y administración no puede guardar cambios remotos. |
| Persistencia | Una única base operativa SQLite, en disco local de recepción. No compartir su archivo ni crear dos bases editables independientes. |
| Tickets | Impresión y reimpresión de tickets internos únicamente en recepción. Naser realiza las pruebas físicas. |
| Informe | Resumen diario exportable a PDF local. |
| Respaldos | Copia diaria consistente y cifrada a servidor externo, con cola, reintentos y restauración comprobada en otro equipo. |
| Entrega | Instalación, configuración, capacitación y aceptación. Plan S1–S4, con reserva final para incidencias. |
| Exclusiones | Sin procesamiento de pagos, pasarelas ni facturación electrónica. Se llevan las cuentas. |

## 2. Habitaciones

El total y la distribución por tipo están confirmados. La siguiente numeración es la **carga inicial del código**, no un relevamiento de los carteles o pisos del establecimiento.

| Números en la carga inicial | Cantidad | Tipo | Piso generado por el código |
|---|---:|---|---:|
| 01, 02, 03, 04, 05, 06, 07, 08, 09 | 9 | Normal | 1 |
| 10, 11, 12, 13, 14, 15, 16, 17, 18 | 9 | Normal | 2 |
| 19 | 1 | Normal | 3 |
| 20, 21, 22, 23 | 4 | Jacuzzi | 3 |
| **Total** | **23** | **19 normales + 4 jacuzzi** | |

La pantalla «Habitaciones y tarifas» permite editar número, tipo, piso y notas. Antes de cargar datos reales se debe registrar la correspondencia con las habitaciones físicas. No se deduce del seed que el motel tenga tres pisos ni que los jacuzzi físicos sean 20–23.

**Bases existentes:** la migración `003_rooms_scope.sql` añade `active` y desactiva excedentes sin estadía abierta ni reserva en espera, empezando por los números más altos. Conserva las filas y sus relaciones históricas. No cambia los tipos existentes ni garantiza que siempre se retiren 24–27; si alguna está en uso, puede seleccionar otra habitación. Si no hay suficientes candidatas, pueden quedar más de 23 activas. No hay retiro automático posterior en SQLite de las que se conservaron en uso. La conciliación del inventario real y las garantías de respaldo previo/transacción corresponden a I01 antes de actualizar una instalación operativa.

El navegador utiliza una base de demostración separada en localStorage. La corrección del seed y del mock no constituye una actualización del equipo de recepción instalado.

## 3. Tarifas, moneda y reglas de estadía

**Moneda del código:** guaraníes enteros; `1 = 1 Gs.` aunque los campos se llamen `*_cents`. La autoridad de cálculo es `src-tauri/src/billing.rs`.

Estos valores proceden de `seed_if_empty` en `src-tauri/src/db.rs`. Se documentan para reproducir la configuración de desarrollo; los importes y reglas comerciales reales no figuran en la información recibida.

| Plan inicial | Importe base (Gs.) | Extra por hora (Gs.) | Horas incluidas en el campo | Tolerancia (min) | Hora de corte |
|---|---:|---:|---:|---:|---:|
| 3 horas (`hourly`) | 80.000 | 20.000 | 3 | 10 | 12 |
| Noche (`night`) | 150.000 | 25.000 | 24 | 15 | 12 |
| Pernocte (`overnight`) | 120.000 | 20.000 | 12 | 15 | 12 |

La hora de corte representa las 12:00 en el comportamiento actual basado en la hora local. No equivale a una regla comercial confirmada. El sistema maneja conversión a pernocte y extras; esas reglas deben cargarse de acuerdo con el tarifario real y validarse en N02/I01. No se encontró una tarifa inicial separada para jacuzzi ni una vinculación automática entre tipo de habitación y plan: hoy se selecciona el plan al ingresar.

| Regla operativa | Registro y alcance |
|---|---|
| Ocupación | Una estadía abierta por habitación; ingresar cambia a ocupada. La restricción de base y las transacciones completas pendientes se trabajan en I01. |
| Cierre y limpieza | Cerrar deja la habitación sucia; luego recepción la marca limpia/disponible. |
| Reservas | En espera, ingreso realizado, cancelada o no presentada; el tablero señala la reserva de llegada del día. |
| Consumos y ajustes | Cargos, recargos y descuentos sobre la cuenta; conservar trazabilidad de correcciones. |
| Histórico | Los importes definitivos y la reimpresión deben conservar lo aplicado al cerrar. Esa garantía sigue pendiente de fortalecer en I01/N05. |
| Datos de huésped | Nombre, documento y teléfono disponibles en el código. Exigir nombre está desactivado por defecto; obligatoriedad y conservación reales no están documentadas. |
| Impuesto | `tax_percent = 10` es un valor inicial del programa, no una política comercial validada para el motel. |

N02 debe adecuar los textos y acciones heredados de «cobro» al control de cuentas acordado. N01 documenta esa diferencia; no acredita el recorrido de recepción en producción.

## 4. Catálogo de consumos

Catálogo inicial disponible: **41 productos en seis categorías**. Sus precios son provisionales, tal como indica `src/lib/products.ts`, y están reflejados en el seed Rust. La lista completa figura en el anexo A. El nombre, presentación y precio final de cada producto se registran durante la configuración; no se consideran aceptados por aparecer en la demostración.

## 5. Usuarios y permisos

Están acordadas las cuentas individuales y los roles `admin`/`recepcion`. La arquitectura propone la siguiente matriz de implementación; no acredita permisos ya implementados ni usuarios existentes.

| Operación | Recepción | Admin |
|---|---|---|
| Consultar tablero/cuentas, ingresos, cierres y reservas | Sí | Sí, con las mismas reglas |
| Agregar consumos y marcar limpieza | Sí | Sí |
| Anular cargos, descuentos y correcciones de cuentas cerradas | No | Sí, con motivo y auditoría |
| Editar habitaciones, tarifas y catálogo | No | Sí |
| Administrar usuarios y respaldos/restauración | No | Sí |
| Imprimir o reimprimir | En recepción | Desde el equipo de recepción |

El detalle de permisos es la propuesta documentada para I04 y deberá quedar aplicado en el backend. El PIN opcional actual no implementa la identidad individual. Faltan nombres de usuarios operativos y su relación con cada rol. Naser e Ivan son desarrolladores; no se los convierte automáticamente en cuentas del motel. Las contraseñas y claves no se guardan en este documento ni en Jira.

## 6. PCs, impresora y despliegue

| Elemento | Dato disponible | Registro pendiente para instalación |
|---|---|---|
| PC recepción | Windows; app local, SQLite e impresora. Debe permanecer encendida para permitir acceso remoto. | Nombre del equipo, versión/arquitectura de Windows, RAM, disco libre, cuenta del servicio y permisos. |
| PC administración | Windows; consulta y escritura autorizada por API/VPN. | Nombre del equipo, Windows, ubicación de conexión y usuario operativo. |
| SQLite | Identificador actual `com.nightdesk.hotel`; archivo `nightdesk.db` en el directorio de datos de la aplicación. | Ruta efectiva de instalación y cuenta propietaria, sin carpeta compartida o sincronizada como base activa. |
| Impresora | Solo recepción. Código ESC/POS para 58/80 mm. Naser tiene la impresora para probar. | Marca, modelo, driver, conexión, nombre exacto de cola o puerto, ancho físico y disponibilidad para N03. |
| Red/VPN | WireGuard + API privada autenticada. | Router, proveedor, CGNAT/IPv6, endpoint, interfaces, firewall y confianza del certificado; prueba de I03/I07. |
| Servicio | Objetivo: iniciar con Windows y seguir disponible con la ventana cerrada. | Implementación y empaquetado de I07; integración del instalador en N09. |

**Ajustes actuales de demostración:** impresora desactivada; nombre/ruta vacíos; papel 80 mm; impresión al cierre activada como preferencia; tema oscuro; pie «Gracias por su visita». `Nightdesk Inn` y `Av. Principal 100` son valores de ejemplo, no datos del establecimiento. El ticket actual elimina tildes para producir ASCII; N03/N05 deben verificar legibilidad, márgenes y corte físicos.

**Secuencia para preparar la instalación:** registrar equipos y datos faltantes; conciliar habitaciones/tarifas/productos; preparar respaldo consistente previo de una base existente; validar migración sobre una copia; instalar recepción y verificar operación local; configurar usuarios y probar impresora; instalar administración/API/VPN y probar lectura/escritura y cortes; configurar el destino externo y restaurar en otro equipo; ejecutar checklist de N09/N10. Las etapas dependen de sus implementaciones en Jira y no se consideran ejecutadas por este documento.

## 7. Respaldo y recuperación

| Parámetro | Valor o estado documentado |
|---|---|
| Frecuencia | Diaria, acordada. |
| Consistencia y cifrado | Snapshot consistente, cifrado antes de subir, cola persistente y reintentos. |
| Destino | Servidor externo acordado; proveedor, dirección, protocolo concreto y capacidad todavía no constan. |
| Responsable operativo | Nombre y contacto de la persona que supervisa copias y restauraciones: no informados. Naser registra el dato; I03/I08 realizan la parte técnica. |
| Horario | 04:00 aparece como propuesta técnica previa; no confirmado para instalación. |
| Retención | 7 locales, 30 diarias y 12 mensuales remotas: propuesta previa pendiente de capacidad y definición operativa. |
| Custodia de clave | Persona, ubicación protegida y procedimiento de recuperación: pendientes de registrar. |
| Recuperación | Admin; detener escrituras, copia previa, validar integridad/compatibilidad, restaurar y verificar en otro equipo. |

El backup externo no es una segunda base operativa. No se copia únicamente el `.db` mientras WAL está activo. I03 debe medir tamaño y crecimiento de una copia representativa para dimensionar almacenamiento, espacio de cola y tiempo de subida; no se dispone de esa medición. La viabilidad de conexión directa de WireGuard tampoco está comprobada. No se ha contratado almacenamiento ni un intermediario de red.

## 8. Plan del mes y revisión de capacidad

Las semanas son relativas: **S1–S4**. No se conoce una fecha de inicio ni la disponibilidad semanal efectiva de Naser/Ivan. No se fijan vencimientos ni se interpreta la fecha del documento como inicio del proyecto.

Estimaciones iniciales de la preparación de Jira, sumando solamente tareas principales; las subtareas ya están incluidas. Son una referencia de planificación, no horas realizadas, presupuesto ni compromiso.

| Semana | Tareas Naser | Horas N | Tareas destinadas a Ivan, sin asignar | Horas I |
|---|---|---:|---|---:|
| S1 | N01, N02, N03 | 28–44 | I01, I02, I03 | 48–76 |
| S2 | N04, N05, N06 | 40–64 | I04, I05, I06 | 64–100 |
| S3 | N07, N08 | 32–52 | I07, I08 | 48–80 |
| S4 | N09, N10 | 24–36 | I09, I10 | 24–40 |
| **Total original** | | **124–196** | | **184–296** |

Con una referencia hipotética de 40 h/semana y 8 h/día, reservar dos días finales deja **144 h por persona** para trabajo planificado y 16 h para incidencias. Naser tiene una brecha posible de hasta 52 h frente a ese margen; el bloque I lo excede entre 40 y 152 h incluso antes de comprobar disponibilidad real. El total de tareas es 308–492 h frente a 288 h planificables en ese ejemplo. La mayor presión se concentra en backend/red/backups de S1–S3, con dependencias que impiden resolverla simplemente moviendo tarjetas.

**Resultado de revisión:** la entrega en un mes no está respaldada por capacidad confirmada. Propuesta de trabajo: contratos y pruebas de integración tempranos en I02, entregables pequeños, demostración semanal y coordinación de N07 con I06/I07; no anticipar una integración como terminada por existir un mock. Reestimar al cerrar S1 con avance real. Si persiste la brecha, Naser debe resolver capacidad adicional o calendario con las partes; este documento no modifica alcance, responsables ni fechas.

Se conserva una tarea principal en curso por persona, actualización diaria, congelamiento de funciones al iniciar S4 y los últimos dos días hábiles para incidencias/revalidación. Toda prueba física de impresión sigue con Naser. Las tareas I siguen sin asignar hasta que Naser las asigne. La garantía de seis meses posterior a entrega se registra aparte del trabajo de este mes.

## 9. Datos concretos faltantes y seguimiento

«No informado» significa que el dato no consta en los mensajes ni en los documentos/código revisados. No significa que deba renegociarse una función.

| ID | Dato que falta registrar | Seguimiento | Necesario antes de |
|---|---|---|---|
| D01 | Numeración física, tipos y ubicación/piso de las 23 habitaciones. | Naser; conciliación técnica I01/N02. | Carga real o migración operativa. |
| D02 | Tarifario real normal/jacuzzi, extras, tolerancias, corte, descuentos e impuesto configurado. | Naser / N02. | Operación con cuentas reales. |
| D03 | Nombres/presentaciones y precios finales de los 41 productos iniciales o catálogo real equivalente. | Naser / N02. | Carga de consumos reales. |
| D04 | Nombre del motel, datos del encabezado/pie de ticket y reglas de datos de huésped. | Naser / N03/N05. | Ticket/configuración de producción. |
| D05 | Usuarios operativos, rol de cada uno y responsable administrador. | Naser / N04; implementación I04. | Alta de usuarios de producción. |
| D06 | Inventario técnico de ambas PCs y cuenta/ruta local de datos. | Naser / N09; servicio I07. | Instalación y actualización. |
| D07 | Modelo/driver, cola o puerto, ancho de papel y disponibilidad de impresora física. | Naser / N03. | Prueba física e impresión real. |
| D08 | Proveedor/router, condición de CGNAT/IPv6, endpoint y resultado de conectividad entre ubicaciones. | I03/I07, sin asignar; Naser facilita datos. | Habilitar acceso remoto. |
| D09 | Dirección/proveedor/protocolo/capacidad del destino externo y responsable operativo nombrado. | Naser registra; I03/I08, sin asignar. | Subida real de respaldos. |
| D10 | Horario, retención, tamaño de copia, custodia de clave y responsable de probar restauración. | Naser registra; I03/I05/I08, sin asignar. | Respaldo/recuperación de producción. |
| D11 | Fecha de inicio y horas semanales efectivas de ambos desarrolladores. | Naser / planificación del mes. | Comprometer calendario. |

Las claves, contraseñas, certificados privados y credenciales de almacenamiento se entregan por un canal protegido durante la configuración. En Jira solo se registra que fueron configurados y quién los custodia.

## 10. Evidencia y aceptación documental

**Fuentes revisadas:** conversación y solicitud original de Naser; `AGENTS.md`; `docs/arquitectura-offline-vpn-backups.md`; `src-tauri/src/db.rs`, `models.rs`, `commands.rs`, `printer.rs`; `src-tauri/migrations/003_rooms_scope.sql`; `src-tauri/tauri.conf.json`; `src/lib/mock.ts`, `products.ts`; preparación local de Jira (`work/motelapp-import.csv`) y criterios vigentes de MOT-7/MOT-27/MOT-28. Base Git revisada: `8f446b76a252af73d79b5a916eab91a23e107d07`, con la corrección local de habitaciones sin commit.

| Criterio documental | Evidencia |
|---|---|
| Configuración y equipos acordados registrados (N01.1) | Secciones 1–6; inventario de 23, tres tarifas iniciales, 41 productos y distinción entre acuerdos y valores de demostración. |
| Despliegue, respaldo y capacidad registrados (N01.2) | Secciones 6–8; destino/responsable desconocidos explícitos, semanas sin fecha e identificación de sobrecarga. |
| Faltantes concretos y seguimiento | D01–D11 con responsable de relevamiento y momento en que cada dato es necesario. |
| Revisión del entregable | Contraste de cantidades, valores y estado de implementación con las fuentes; sin secretos ni datos de producción inventados. |

La corrección de habitaciones de la sesión anterior tiene evidencia de `cargo test` con 33 pruebas aprobadas, incluidas instalación nueva y migración heredada/reapertura, y `tsc --noEmit` aprobado. El empaquetado Vite no completó: falló por acceso al directorio de configuración en el entorno de ejecución. No hay evidencia aquí de instalador final, recorrido Tauri con base de producción, impresión física, VPN operativa o restauración remota. La documentación N01 no cierra I01, N03, I03 ni las demás tareas de implementación/prueba.

**Registro de decisión — 14/09/2026:** Naser confirmó que el alcance ya está acordado. Se documentan los datos disponibles sin volver a solicitar aceptación de funciones; los parámetros de instalación ausentes quedan expresamente identificados. Autor de la consolidación: Codex por encargo de Naser. No se registra aprobación de la clienta ni revisión externa que no haya ocurrido.

## Anexo A. Catálogo inicial provisional

Todos los importes siguientes están expresados en guaraníes enteros y proceden del código. No son un tarifario comercial confirmado.

| Producto | Categoría | Precio inicial (Gs.) |
|---|---|---:|
| Agua | bebidas | 5.000 |
| Coca | bebidas | 8.000 |
| Pulp | bebidas | 8.000 |
| Fanta | bebidas | 8.000 |
| Tónica | bebidas | 7.000 |
| Del valle | bebidas | 10.000 |
| Energy | bebidas | 12.000 |
| Power | bebidas | 12.000 |
| Bud 66 | bebidas | 12.000 |
| Skol | bebidas | 12.000 |
| Smirnoff | bebidas | 18.000 |
| Beldent | snacks | 5.000 |
| Halls | snacks | 5.000 |
| Papa | snacks | 10.000 |
| Gullón | snacks | 8.000 |
| Turrón | snacks | 8.000 |
| Bonbon | snacks | 7.000 |
| Chocolate | snacks | 10.000 |
| Kent Conv. | tabaco | 18.000 |
| Lucky | tabaco | 18.000 |
| Encendedor | tabaco | 8.000 |
| Crema D. | higiene | 10.000 |
| Cepillo D. | higiene | 8.000 |
| Baño E. | higiene | 12.000 |
| Gel Pant. | higiene | 15.000 |
| Prestobarba | higiene | 12.000 |
| Prime | adulto | 15.000 |
| Control | adulto | 15.000 |
| Lubricante | adulto | 25.000 |
| Prot. 100 | adulto | 20.000 |
| Prot. 150 | adulto | 30.000 |
| Prot. 200 | adulto | 40.000 |
| Capa P. | adulto | 25.000 |
| Agrandador | adulto | 35.000 |
| Anillo v. | adulto | 45.000 |
| Estimulador | adulto | 50.000 |
| Fantasía | adulto | 60.000 |
| Quinta | licores | 45.000 |
| Sta. Helena | licores | 50.000 |
| Monje | licores | 55.000 |
| Johnnie W. | licores | 180.000 |
