# Auditoría de persistencia de Nightdesk

Fecha: 10 de octubre de 2026. Código revisado: commit `ce03474`, aplicación `0.1.13`.

La persistencia operativa local tiene una base sólida: transacciones, claves foráneas, exclusión de estadías abiertas duplicadas y recibos congelados. Los principales problemas están en la recuperación de la réplica, los reintentos de respaldos y la compatibilidad de versiones. La SQLite de esta Mac conserva 142 operaciones rechazadas por referencias inexistentes en la réplica. Este dato no demuestra pérdida en la PC de recepción del motel.

## Alcance y evidencia

Se revisaron las 21 migraciones SQLite, las 12 migraciones Supabase del repositorio, los servicios operativos, el cliente remoto, outbox, bootstrap, pull, push, respaldos, restauración y actualizador.

La base de esta Mac se abrió mediante SQLite `mode=ro`, `query_only=ON` y una transacción de lectura. No se modificaron sus datos. Las reproducciones usaron bases temporales y el código actual. No se cambiaron reglas de negocio, migraciones ni versiones.

La conexión **ardentium** permitió completar la inspección del proyecto **nightdesk** el 10/10/2026, hasta las 13:00 de Asunción. Se consultaron migraciones, definiciones de funciones, políticas y privilegios efectivos, restricciones, conteos, UID y metadatos de Storage. No se ejecutaron escrituras SQL ni RPC de mutación. El manifiesto privado de actualización se leyó por HTTPS con la autenticación existente, sin registrar credenciales. La SQLite inspeccionada pertenece a esta Mac; no se obtuvo la base de la PC de recepción de Windows.

| Verificación | Resultado |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml` | 157 aprobadas, 0 fallidas, 2 ignoradas por necesitar impresora física |
| `npm run build` | TypeScript y Vite finalizaron correctamente |
| SQLite `PRAGMA integrity_check` | `ok` |
| SQLite `PRAGMA foreign_key_check` | 0 violaciones |
| Habitaciones con más de una estadía abierta | 0 |
| Desacuerdos entre `rooms.status=occupied` y estadía abierta | 0 |
| Filas sin UID en las entidades inspeccionadas | 0 |
| Outbox | 94 enviadas, 125 pendientes, 142 rechazadas |
| Rechazos de estadías | 48 operaciones: `stays_rate_plan_uid_fkey` |
| Rechazos de cargos | 94 operaciones: `charges_stay_uid_fkey` |
| Cola de respaldos local | 3 fallidos por conectividad, 2 pendientes, 0 registrados como subidos |
| Cuentas cerradas sin ambos marcadores de cierre | 2; no implica por sí solo pérdida de líneas, pueden ser anteriores al marcador |

Los 142 rechazos son operaciones, no 142 cuentas distintas. Los fallos de respaldos observados son de conectividad; no prueban que los defectos de reintento descritos abajo ya hayan ocurrido. El estado local tampoco permite concluir que Storage carezca de respaldos.

## Hallazgos prioritarios

P1: corregir antes de confiar en la recuperación o en la información remota afectada. P2: defecto concreto de alcance condicionado o menor urgencia.

### A1 P1 Las dependencias faltantes dejan operaciones rechazadas permanentemente

**Evidencia observada y código.** Los 48 rechazos de estadías apuntan a tarifas inexistentes en Supabase; los 94 de cargos apuntan a estadías inexistentes. Esos UID de tarifa sí existen en SQLite. El cliente clasifica el error PostgreSQL de clave foránea como `Validation`; el push lo mueve a `rejected`, mientras la selección posterior solo lee `pending`. Corregir la dependencia no recupera las operaciones. Un drenaje que termina aislando rechazos puede limpiar `last_error`, por lo que “sin pendientes” no garantiza réplica completa.

Existe un origen estructural compatible con lo observado: si el catálogo remoto ya existe, el bootstrap agrega las tarifas remotas por UID, pero mantiene las tarifas locales con UID distinto y sus referencias históricas. Solo habitaciones y usuarios adoptan identidad por una clave natural. Las estadías siguen enviando el UID de la tarifa local. La comparación remota confirmó que el UID de tarifa de esos rechazos no existe en Supabase: solo dos de los cinco UID de tarifas de la Mac coinciden, correspondientes a las tarifas Jacuzzi de identidad fija. Tampoco coinciden los 34 UID de estadías, los 94 de cargos ni las 94 operaciones localmente marcadas como enviadas. Esto confirma conjuntos divergentes, pero no demuestra el origen histórico exacto: puede haber otra instalación de recepción o una reinicialización previa del remoto. No se debe importar automáticamente el histórico de desarrollo de la Mac sobre la operación del motel.

Referencias: [clasificación HTTP](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/client.rs:459>), [selección pendiente](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/push.rs:73>), [rechazo permanente](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/push.rs:118>), [catálogo en bootstrap](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/bootstrap.rs:139>), [adopción de UID](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/pull.rs:79>).

**Corrección propuesta:** reconciliar las referencias de tarifas preservando historia, distinguir dependencias recuperables de payloads inválidos, permitir reparación y reencolado controlado, y mostrar por separado pendientes y rechazados. No borrar la outbox ni reemplazar UID históricos de forma indiscriminada.

### A2 P1 Una subida aceptada puede quedar atascada al reintentarse

**Defecto de flujo; no reproducido contra Storage real.** Cada intento primero sube el objeto con `x-upsert=false` y después inserta el manifiesto. Si Storage guarda el archivo pero se pierde la respuesta, o falla la inserción del manifiesto, el reintento vuelve a subir a la misma ruta. El objeto ya existente provoca error y nunca se alcanza la confirmación del manifiesto. Supabase documenta ese rechazo para una ruta existente sin upsert en [Standard Uploads](https://supabase.com/docs/guides/storage/uploads/standard-uploads).

La cola selecciona siempre los tres más antiguos. Tres errores permanentes impiden intentar las copias posteriores mientras sigan en la cola. La retención local no constituye una reparación de este protocolo.

Referencias: [selección de respaldos](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/backup.rs:411>), [subida y manifiesto](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/backup.rs:463>), [upload sin upsert](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/client.rs:174>).

**Corrección propuesta:** separar las fases persistidas de subida y confirmación; verificar tamaño/checksum cuando el objeto ya existe; hacer idempotente el registro del manifiesto y evitar que errores permanentes monopolicen la cola. No basta con sobrescribir objetos sin verificar su identidad.

### A3 P1 Restaurar acepta esquemas incompatibles antes de reemplazar la base activa

**Defecto comprobado por código y reproducción aislada.** La restauración obtiene la última migración del archivo temporal y la compara con la última migración de ese mismo archivo. La condición acepta siempre esa entrada. Después reemplaza la base activa; recién entonces el comando intenta abrirla y migrarla con el binario actual.

El runner de migraciones tampoco rechaza IDs desconocidos: si todas las migraciones conocidas ya figuran aplicadas, termina sin comprobar compatibilidad. En una reproducción, una base con un ID futuro y una columna requerida ausente pasó `integrity_check` y la comprobación de versión, pero una consulta actual falló. La prueba demuestra el guard ausente, no que toda migración desconocida sea incompatible.

La instalación local contiene `014_audit` y `021_local_reception`, ausentes del catálogo actual. Es una divergencia real de historial que debe reconciliarse; no es evidencia de corrupción. No deben borrarse esos registros para hacer coincidir artificialmente las listas.

Referencias: [validación contra sí misma](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/backup.rs:663>), [comprobador de esquema](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/backup.rs:694>), [apertura posterior al reemplazo](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/commands.rs:695>), [runner](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/db.rs:153>).

**Corrección propuesta:** verificar compatibilidad contra el catálogo soportado por el binario, migrar y validar una copia temporal antes de sustituir la activa, comprobar claves foráneas y requisitos de dominio, y restaurar automáticamente la copia previa si falla el paso final. Rechazar o tratar explícitamente esquemas futuros/divergentes.

### A4 P1 Restaurar no garantiza la reconciliación de la réplica

**Defecto de control y de identidad de operaciones.** La restauración borra `sync_outbox` y `sync_state`, pero solo despierta un push. Ese camino no ejecuta bootstrap, por lo que no reencola el histórico restaurado. Los temporizadores y eventos normales de reconexión tampoco repiten un bootstrap inicial fallido. Se requiere reiniciar o invocar manualmente el ciclo completo.

Incluso ejecutando bootstrap, sus UUID se derivan siempre de entidad y UID. El servidor ignora operaciones previamente aplicadas; una fila ya enviada en otro bootstrap no vuelve a reconciliarse aunque el respaldo restaurado contenga un estado diferente.

Referencias: [limpieza de sync](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/backup.rs:688>), [despertar push](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/commands.rs:701>), [ciclos del worker](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/worker.rs:115>), [identidad del bootstrap](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/bootstrap.rs:197>), [UUID determinista](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/outbox.rs:134>), [deduplicación remota](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/supabase/migrations/20261009152753_product_analytics.sql:50>).

**Corrección propuesta:** persistir el estado de recuperación y reintentar bootstrap desde los ciclos automáticos; definir una reconciliación identificada por restauración, con política explícita para datos remotos posteriores al respaldo. Cambiar UUID sin resolver ese conflicto temporal sería insuficiente.

### A5 P1 Recrear un usuario puede conservar su contraseña y rol anteriores

**Reproducción mínima en SQLite y trazado del código.** Mientras recepción está desconectada, administración elimina un usuario y crea otro con el mismo nombre. El pull procesa usuarios antes de las bajas. Primero cambia el UID de la fila local, conservando la versión anterior; después omite el resto de la actualización si esa versión es mayor o igual que la versión 1 del nuevo usuario. La baja del UID anterior ya no encuentra esa fila.

Con un usuario anterior `admin/version=3` y otro nuevo `recepcion/version=1`, el resultado conserva rol, hash y versión anteriores bajo el UID nuevo. Las versiones de entidades distintas no son comparables.

Referencias: [orden del pull](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/pull.rs:8>), [adopción de usuario](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/catalog.rs:165>), [comparación de versión](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/catalog.rs:115>).

**Corrección propuesta:** tratar el cambio de UID como cambio de identidad y aplicar sus campos de forma atómica; reconciliar las bajas y revocar las sesiones locales afectadas.

### A6 P1 El historial remoto ignora los importes cerrados persistidos

**Defecto directo del adaptador remoto.** La lista devuelve `total_cents: 0` para todas las cuentas cerradas. Al abrir una, calcula desde check-in hasta la hora actual, con tarifa e impuesto actuales, y descarta los cargos computados del cierre. El total puede variar después del checkout aunque Supabase tenga todos los datos correctos.

Referencias: [importe cero](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src/lib/supabase.ts:767>), [reconstrucción del detalle](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src/lib/supabase.ts:822>), [cálculo con hora actual](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src/lib/supabase.ts:825>).

**Corrección propuesta:** leer y validar los cargos y marcadores de cierre persistidos. Reservar las estimaciones para estadías abiertas. Este defecto afecta la lectura remota, no modifica el cobro ya guardado en SQLite.

### A7 P2 Restaurar en otra PC conserva rutas de respaldos de la PC anterior

**Reproducción de cola y restauración en SQLite temporal.** El snapshot contiene `backup_queue` con rutas absolutas. Tras restaurarlo solo se limpian tablas de sync. Los archivos referenciados pueden no existir en el equipo nuevo; sus entradas pasan a `failed` y vuelven a ocupar los primeros tres lugares de la cola. En la reproducción, cuatro entradas antiguas hicieron que una nueva copia válida nunca fuese seleccionada en los ciclos probados.

Además, la lista descarta el manifiesto remoto cuando encuentra un ID local coincidente, incluso si falta el archivo local.

Referencias: [archivo local ausente](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/backup.rs:432>), [restauración de toda la BD](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/backup.rs:678>), [limpieza limitada](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/backup.rs:688>), [deduplicación de la lista](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/backup.rs:226>).

**Corrección propuesta:** reconciliar el estado de respaldos al restaurar, distinguir archivos disponibles de referencias históricas y conservar la opción de recuperación remota cuando falte la copia local.

### A8 P2 La paginación del catálogo puede bloquear el worker

**Reproducción del paginador con respuestas simuladas.** El cursor guarda solo la fecha máxima y consulta `>=` con límite 500. Con 500 filas de la misma fecha, cada página repite las mismas filas. Aunque la consulta ordena también por UID, el cursor no incluye ese desempate.

Para `catalog_deletes` el cliente usa `deleted_at`, pero el paginador busca `updated_at`; nunca avanza el cursor. Con 500 bajas se bloquea incluso si tienen fechas diferentes. El bucle puede impedir que el worker atienda nuevos pushes.

Referencias: [bucle y cursor](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/pull.rs:21>), [columna temporal y consulta](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/client.rs:288>).

**Corrección propuesta:** cursor compuesto fecha + UID/clave, columna temporal correspondiente a cada tabla y comprobación explícita de avance. Verificar páginas con fechas empatadas y más de 500 bajas.

### A9 P2 Un fallo en el seed deja una instalación parcial

**Reproducción con módulos Rust reales.** El seed se omite si ya existe cualquier habitación, pero sus inserciones no están en una transacción única. Inyectando un fallo en la habitación 02, la primera apertura falla; al retirar el fallo, la siguiente apertura devuelve éxito con una habitación y cero tarifas. El estado parcial queda persistido.

Referencia: [guard e inserciones del seed](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/db.rs:268>).

**Corrección propuesta:** inicialización transaccional y un mecanismo explícito para detectar y recuperar seeds incompletos sin duplicar datos de instalaciones existentes.

### A10 P2 Desactivar una habitación ocupada oculta su cuenta abierta

**Reproducción con módulos Rust reales.** El pull puede aplicar `active=false` conservando `occupied`, pero el tablero local filtra solo `active=1`. En la prueba, el tablero pasó de una cuenta abierta a cero; la estadía seguía persistida como abierta. El esquema/RPC remoto permite ese cambio, aunque la UI remota actual no expone directamente la desactivación de habitaciones.

Referencias: [filtro del tablero](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/service.rs:346>), [preservación de ocupación durante pull](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src-tauri/src/sync/pull.rs:101>), [actualización remota de habitación](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/supabase/migrations/20260918232853_rpc.sql:509>).

**Corrección propuesta:** incluir habitaciones con estadía abierta aunque estén inactivas, conforme a la arquitectura acordada.

### A11 P2 La retención mensual conserva todas las copias de cada mes

**Reproducción de la función con cliente simulado.** El bucle agrega al conjunto todas las filas de cada uno de los meses seleccionados. No se limita a una copia por mes. Una muestra de 90 copias diarias repartidas en cuatro meses produjo cero eliminaciones y 90 conservadas.

Referencia: [selección mensual](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/supabase/functions/backup-retention/index.ts:35>).

**Corrección propuesta:** conservar solo un representante por mes, además de las diarias. Verificar paginación y resultados de borrado. La comprobación remota confirmó que el proyecto no tiene Edge Functions desplegadas ni extensión/esquema pg_cron. Por tanto, la retención descrita no está operativa mediante el mecanismo implementado. No se inspeccionaron posibles programadores externos ajenos al proyecto.

### A12 P1 El historial de migraciones remoto no coincide con el repositorio

**Comprobación del servidor.** Supabase registra 19 migraciones y el repositorio contiene 12; solo coinciden dos identificadores: `20261003141857` y `20261009152753`. Hay 17 versiones remotas sin archivo del mismo ID y 10 archivos locales sin entrada del mismo ID. Parte de la diferencia proviene de SQL dividido en varias migraciones y pasos intermedios de catálogo; no significa que falten siete funcionalidades.

Ejemplos: `schema` aparece como `20260919000550` en servidor y `20260918232847` en repo; Jacuzzi como `20261003141809` frente a `20261002120000`. Los cuerpos de las 25 funciones de catálogo/sync/roles inspeccionadas coinciden con las definiciones finales del repositorio al ignorar comentarios y formato. El problema confirmado es la trazabilidad y reconciliación del historial, no una divergencia demostrada de esas funciones.

**Corrección propuesta:** recuperar y versionar el historial efectivo, documentar la correspondencia y ensayar el siguiente despliegue en una base separada. No reaplicar migraciones iniciales ni usar reparación del historial a ciegas: los IDs actuales no permiten tratar un `db push` como una actualización incremental ya reconciliada.

### A13 P2 Siete archivos de respaldo carecen de manifiesto remoto

**Comprobación de Storage y Postgres.** El bucket privado contiene nueve objetos, pero la tabla `backups` solo tiene dos manifiestos. Ambos manifiestos tienen objeto y nonce; los siete restantes, creados entre el 24/09 por la noche y el 02/10 en hora de Asunción, no están asociados a una fila de `backups`. El listado/restaurador de la app obtiene los respaldos remotos de esa tabla, por lo que no ofrece esos archivos huérfanos.

La última copia con manifiesto corresponde al **08/10/2026 a las 09:57 de Asunción**, app 0.1.13 y esquema `020_jacuzzi_rates_and_stay_users`: supera 48 horas al momento de la revisión y precede a analítica 021. El tamaño del manifiesto corresponde a SQLite sin comprimir; su diferencia con el objeto cifrado y comprimido es esperable. No se descifraron estas copias ni se certificó su restauración.

**Corrección propuesta:** reconciliar cada objeto con su manifiesto original verificable y comprobar el motor de respaldos de la recepción. No inventar nonce/checksum ni borrar objetos huérfanos antes de determinar si son recuperables. La causa de los huérfanos no se atribuye automáticamente al fallo de reintentos A2: una limpieza anterior de tablas también puede producirlos.

## Versionado

| Capa | Estado verificado | Implicación |
|---|---|---|
| Aplicación | Repo `0.1.13` coherente en los cinco archivos; manifiesto publicado `0.1.14` | El instalador 0.1.14 existe, URL del mismo proyecto y campo de firma presente; falta vincular el artefacto publicado a su commit de origen |
| Identidad de instalación | Identificador estable; downgrade NSIS deshabilitado | Protege actualización normal, pero no valida esquemas al restaurar |
| Contrato IPC | Rust declara versión 2 | Remoto declara versión 1 y app `0.1.0` constantes en [contract_info](</Users/ivanortiz/Documents/Ivandev's Projects/hotel-nightdesk/src/lib/supabase.ts:596>); diagnóstico de versión incorrecto |
| SQLite | 21 migraciones conocidas; instalación con 23 registros | Las dos adicionales requieren reconciliación documentada; falta guard de compatibilidad y checksum del contenido |
| Supabase | 19 migraciones desplegadas frente a 12 archivos; solo 2 IDs coinciden | Funciones principales verificadas equivalentes; historial por reconciliar antes del siguiente despliegue, A12 |
| Filas de catálogo | `version` y `expected_version` | Mantener control de concurrencia; no comparar versiones después de cambiar UID |
| Payload operativo | Migración de analítica conserva campos nuevos ante payloads antiguos mediante `COALESCE` | Hay compatibilidad parcial deliberada; no equivale a una negociación completa de capacidades |
| Respaldo | Incluye versiones de aplicación/esquema y checksum | La validación de compatibilidad actualmente no usa esos datos contra el binario destino |

Antes de publicar una actualización, aplicar primero las migraciones Supabase compatibles, verificar clientes antiguos con el nuevo servidor y comprobar que una BD de una versión previa abre y conserva sus cuentas en la versión nueva. Una reversión del código debe mantener soporte del esquema ya instalado; publicar un número SemVer mayor no hace compatible un esquema por sí solo.

La documentación general todavía menciona SQLite 014, contrato v1 o aplicación 0.1.7 en distintos lugares. El contrato principal sí documenta v2 y analítica 021. Conviene unificar una matriz de versión de app, contrato, migraciones y capacidades del servidor para evitar diagnósticos contradictorios.

## Cobertura positiva y observaciones

- SQLite habilita WAL y claves foráneas; las migraciones pendientes son transaccionales y tienen respaldo previo mediante Online Backup API.
- Las operaciones principales de recepción encolan sus cambios en la misma transacción. Checkout y snapshot de recibo conservan atomicidad; fallar la impresión no deshace el cierre.
- El índice parcial evita dos estadías abiertas para una habitación.
- El pull confirma filas y cursor conjuntamente, y su lista blanca excluye `rooms.status`.
- Los respaldos usan snapshot consistente, SHA-256 y AES-GCM. Las pruebas existentes incluyen roundtrip y rechazo con clave incorrecta.
- Las migraciones Supabase revisadas usan roles en `app_metadata`, RLS, wrappers y controles en las RPC; el catálogo revoca escrituras directas después de introducir la RPC auditada. Se confirmó RLS en las 16 tablas públicas, los triggers de versión/ocupación y restricciones activas. `anon` y `authenticated` no tienen escritura directa sobre estadías ni habitaciones; las RPC privadas comprueban los roles correspondientes.

Observaciones secundarias:

- Las reservas comparan el prefijo textual de RFC3339 con el día local. Una llegada UTC a las 01:00 del día siguiente corresponde a las 22:00 del día anterior en Asunción y no se detecta como reserva de ese día. La UI actual conserva el offset local, por lo que el caso requiere datos UTC de IPC o históricos; no se presenta como fallo cotidiano de esa UI.
- La comprobación local de analítica vuelve a sumar el mismo detalle de cargos en ambos lados de su comparación; no contrasta adecuadamente los marcadores del cierre. Es una carencia de detección de corrupción, no evidencia de corrupción actual.
- `integrity_check` no comprueba por sí solo las claves foráneas; por eso se ejecutó también `foreign_key_check`, conforme a la [documentación SQLite](https://sqlite.org/pragma.html#pragma_integrity_check).
- La existencia de pagos históricos no implica que el checkout actual registre pagos: el comportamiento actual está documentado y probado. El bootstrap actual tampoco incluye la tabla payments; debe contemplarse si se exige recuperar esos registros históricos en remoto.
- No se probaron impresora, instalador Windows, cortes físicos de disco ni una restauración completa por IPC en otra PC. Docker no estaba disponible para ejecutar de nuevo las pruebas SQL del stack Supabase local.

## Orden recomendado de corrección

1. Distinguir la instalación operativa de la base de desarrollo de la Mac y definir recuperación por UID sin mezclar historiales; reconciliar las migraciones efectivas antes de otro despliegue.
2. Corregir identidad/versiones de usuarios y lectura de cuentas cerradas.
3. Corregir protocolo de respaldo, validación de esquemas y reconciliación después de restaurar; probar una restauración en un entorno separado con un snapshot representativo.
4. Corregir paginación, seed, visibilidad de habitaciones y retención.
5. Añadir verificaciones de compatibilidad de versiones, reconciliar las migraciones locales fuera del catálogo y registrar el commit de cada instalador publicado.

## Resultados de la inspección remota

| Comprobación del proyecto nightdesk | Resultado |
|---|---|
| Estado del proyecto | Activo, PostgreSQL 17 |
| Habitaciones y tarifas | 23 habitaciones, 4 tarifas |
| Estadías | 14: 8 abiertas y 6 cerradas |
| Cargos, reservas y pagos | 6 cargos, 0 reservas, 0 pagos |
| Invariantes remotas comprobadas | 0 duplicados abiertos, 0 desacuerdos de ocupación, 0 referencias rotas de estadías a habitación/huésped/tarifa |
| Cierres | Los 6 cierres tienen marcadores y coinciden con suma/cantidad de cargos persistidos |
| Último push registrado | 06/10/2026 a las 20:52 de Asunción; indica última recepción de datos, no prueba por sí solo una caída del servicio |
| Coincidencia con la Mac | 0/34 estadías, 0/94 cargos y 0/94 operaciones marcadas enviadas; la tarifa involucrada en los rechazos también falta |
| Definiciones SQL | 25 cuerpos de funciones contrastados, equivalentes ignorando formato y comentarios |
| Esquema reciente | Categorías Jacuzzi, autores de check-in/out y campos de analítica de productos presentes; restricciones validadas |
| Respaldos | 2 manifiestos con objeto y nonce; 7 objetos sin manifiesto |
| Retención | Sin Edge Functions desplegadas, sin pg_cron |
| Actualización publicada | 0.1.14, publicada el 10/10 a las 12:25; instalador existente, campo de firma presente y URL del mismo proyecto |

El estado del servidor es internamente consistente en las comprobaciones ejecutadas, pero no acredita que contenga toda la operación actual de la recepción. No se equiparó una marca `sent` de la Mac con confirmación actual en el servidor. Tampoco se ejecutó ni instaló el EXE publicado, ni se verificó criptográficamente su firma: comprobar su presencia en el manifiesto no equivale a validarla.

### Seguridad y rendimiento

- Todas las tablas públicas inspeccionadas tienen RLS. La herramienta de listado avisó que `nightdesk.catalog_operations` carece de RLS y la describió como expuesta; la comprobación efectiva de privilegios mostró que **anon y authenticated no tienen SELECT, INSERT ni UPDATE sobre ella**, y anon tampoco tiene USAGE del esquema. No se confirmó la exposición afirmada por ese aviso genérico. Añadir RLS allí sería defensa adicional, con pruebas de las RPC. [Referencia RLS](https://supabase.com/docs/guides/database/postgres/row-level-security).
- El asesor de seguridad solo reportó protección de contraseñas filtradas desactivada en Supabase Auth. Revisar disponibilidad/configuración; este aviso no demuestra filtración de contraseñas. [Remediación](https://supabase.com/docs/guides/auth/password-security#password-strength-and-leaked-password-protection).
- Hay privilegios heredados excesivos de anon sobre `catalog_audit` y `catalog_deletes`, incluidos TRUNCATE/REFERENCES/TRIGGER. Las políticas actuales impiden acceso a filas sin rol autorizado, y no se identificó una RPC que exponga TRUNCATE, pero conviene revocar privilegios innecesarios y controlar los privilegios por defecto. No se explotaron ni modificaron esos permisos.
- Los wrappers públicos de sync permiten EXECUTE a anon, pero las funciones privadas y sus guardas bloquean esa llamada sin autorización. Conviene alinear el ACL de los wrappers con la intención del contrato.
- El asesor de rendimiento reportó seis claves foráneas sin índice de cobertura en reservas/estadías, una superposición de políticas SELECT para respaldos y once índices sin uso registrado. Son señales de revisión, no evidencia de pérdida de datos; no se recomienda eliminar índices solo por esas estadísticas en una réplica pequeña. [Índices de claves foráneas](https://supabase.com/docs/guides/database/database-linter?lint=0001_unindexed_foreign_keys), [políticas superpuestas](https://supabase.com/docs/guides/database/database-linter?lint=0006_multiple_permissive_policies).

## Cierre y límites

La auditoría de código, SQLite de esta Mac, esquema Supabase desplegado y versionado queda completada con **13 hallazgos priorizados**. No se corrigieron datos ni se aplicaron migraciones. Las pruebas de restauración real en otra PC, validación del instalador Windows y cotejo con la SQLite operativa del motel son pasos de remediación/verificación posteriores; no se presentaron como realizados. Las consultas remotas y las comprobaciones de permisos no reemplazan esas pruebas de extremo a extremo.
