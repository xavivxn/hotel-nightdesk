# Reinicio al terminar la capacitación

Este corte elimina la operación y el catálogo de prueba en SQLite y en la réplica
Supabase. Conserva los usuarios locales completos: identificadores, contraseñas,
roles y estado activo/inactivo. Conserva también Supabase Auth, las credenciales
del equipo que están fuera de SQLite y el modo técnico Recepción/Administración
remota; sin este último, el arranque heredado convertiría la PC remota en recepción.

## Compilación especial

En Windows, con las mismas variables de firma del [procedimiento de actualización](actualizaciones.md):

```powershell
npm run build:installer:windows:reset-training
```

El comando hace el bump habitual y compila con la feature `reset-training`.
`npm run build:installer:windows` sigue generando actualizaciones normales que
conservan todos los datos. No cambiar versiones a mano.

La compilación especial ejecuta el reinicio al arrancar, antes de crear conexiones
de sincronización, respaldos o mostrar la interfaz. La migración `022_maintenance_resets`
solo crea la tabla de control: no borra datos por sí misma. El identificador fijo
`post-training-2026-10` se registra en la misma transacción que el borrado y la carga
inicial. Reabrir, reinstalar o recompilar esta campaña no vuelve a borrar los datos
creados después del reinicio. No cambiar el identificador para un rebuild.

Se eliminan huéspedes, reservas, estadías, cargos, pagos, tickets, stock y movimientos,
auditoría, habitaciones, tarifas, productos, ajustes, intentos de login, cola/cursores
de sincronización y cola/metadatos de respaldos. Se conservan `users`, las migraciones
y el registro de mantenimiento, además de `settings.device_mode`. Se cargan los valores iniciales de la app: 23 habitaciones
disponibles, tarifas y productos predeterminados. La fecha de cobertura de análisis
de productos comienza nuevamente. Hay que configurar nuevamente impresora y ajustes
del negocio; cada equipo conserva su modo anterior.

Antes de borrar se crea `nightdesk.pre-reset-post-training-2026-10.bak` junto a la base
mediante SQLite Online Backup, incluyendo lo que esté en WAL. Si la copia falla, no
se borra nada; si falla el borrado o la carga inicial, se revierte la transacción y
el arranque se detiene. La copia de recuperación no entra en la cola de upload.

## Corte coordinado

No distribuir esta compilación como actualización rutinaria durante la capacitación.
Preparar y firmar el instalador antes del corte. Para este reinicio, instalarlo
manualmente sin red: el botón de actualización exige conectar la app antigua y
podría enviar datos de prueba mientras se hace el corte.

1. Terminar la capacitación. Descargar/copiar el instalador a todas las PCs afectadas.
   Identificar la única recepción operativa: sus usuarios locales serán la fuente inicial
   de `app_users`. Asegurar que allí estén todas las cuentas que deben seguir disponibles.
2. Cerrar Nightdesk en todas las PCs y desconectarlas de la red. Incluir equipos de
   administración y cualquier otra copia de recepción que haya usado el mismo proyecto.
3. Instalar la compilación especial en cada PC **sin red** y abrirla una vez. En recepción,
   verificar login con los usuarios conservados y tablero limpio. Administración remota
   conserva sus usuarios y modo, pero su login requiere red y se comprueba en el paso 7.
   No registrar operación nueva. Cerrar la app nuevamente.
4. Con todas las apps cerradas, ejecutar como `postgres` el contenido de
   [reset_training.sql](../supabase/maintenance/reset_training.sql) en el SQL Editor de
   Supabase. Anteponer en la misma ejecución:

   ```sql
   SET nightdesk.confirm_training_reset = 'post-training-2026-10';
   ```

   Es una operación manual de datos, no una migración ni una RPC del cliente.
   El bloque es atómico, no usa `CASCADE` y consume la confirmación. No volver a
   ejecutarlo una vez iniciada la operación real: borraría también esos datos nuevos.
   No usar `supabase db reset`.
5. Vaciar los objetos de capacitación del bucket privado **`backups`** mediante el
   Dashboard o la API de Storage. El SQL solo vacía sus manifiestos; no borra archivos.
   Mantener los buckets y sus políticas; conservar **`updates`** y sus instaladores.
   No borrar filas directamente de `storage.objects`.
6. Reconectar y abrir **primero la recepción operativa actualizada** e iniciar sesión
   con un usuario conservado. Su primer bootstrap sube el catálogo
   limpio y sus usuarios. Esperar a que la sincronización termine sin errores.
7. Reconectar las instalaciones actualizadas de **Administración remota** y verificar
   su login. La arquitectura admite una única recepción como fuente operativa. Otras
   copias usadas como recepción durante las pruebas deben quedar cerradas/desconectadas
   de este proyecto, aunque también hayan sido limpiadas. No usar este corte para
   habilitar recepciones simultáneas: el bootstrap actual tampoco reconcilia las semillas
   de tarifas y productos de una segunda recepción.
8. Configurar impresora, ajustes y precios definitivos. Comprobar tablero sin estadías,
   historial y reservas vacíos, usuarios correctos y respaldo nuevo. Reabrir una segunda
   vez y comprobar que los nuevos ajustes se conservan.

Supabase Auth (`auth.users`, dispositivo y administración), esquema, RLS, RPC y
permisos permanecen intactos. `public.app_users` se vacía y se reconstruye desde
SQLite: si se conservaran sus filas remotas, el bootstrap actual rechazaría el
catálogo nuevo por considerar que Supabase ya está inicializado.

Una instalación antigua que se reconecte después del corte **puede volver a subir
datos**. Este procedimiento no implementa un bloqueo remoto de versiones: todas las
copias que puedan escribir deben actualizarse antes de reconectar. Para cortes sin
control de esas PCs haría falta un protocolo de generación de datos validado en el
servidor; no basta con este instalador.

## Copias fuera de la base

Este es un reinicio de la base activa, no un borrado forense. Los `.bak`, snapshots
locales y PDF ya exportados siguen en disco; no se vuelven a encolar porque la cola
antigua se vacía. Archivar o eliminar esas copias de capacitación durante el corte,
después de verificar el resultado, según se necesite conservar recuperación. No
restaurar un respaldo de capacitación sobre la operación real. No borrar las
credenciales ni las claves de cifrado del equipo.

## Verificación de desarrollo

```sh
cd src-tauri
cargo test
cargo test --features reset-training
```

Las pruebas usan bases temporales y comprueban preservación de usuarios, limpieza,
valores iniciales, ejecución única al reabrir, backup fallido y rollback ante fallo.
La prueba [reset_training.sql](../supabase/tests/reset_training.sql) se ejecuta con
`psql -v ON_ERROR_STOP=1 -f supabase/tests/reset_training.sql` en una base local aislada
con las migraciones aplicadas; termina con rollback. No usarla sobre el proyecto del cliente.

La compilación y firma del instalador Windows y el corte sobre las PCs reales son
pasos operativos adicionales a estas pruebas.
