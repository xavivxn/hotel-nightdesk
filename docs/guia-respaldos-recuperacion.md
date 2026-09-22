# Respaldos y recuperación

La pantalla **Ajustes → Respaldos y recuperación** está disponible para administradores. En recepción muestra la fecha de la última copia local y de la última copia remota confirmada, la cola pendiente y el último error. En administración remota solo muestra la copia confirmada en Supabase; desde allí no es posible conocer la cola local ni restaurar la base.

Una copia remota cuenta únicamente cuando el objeto cifrado está confirmado en Storage y su manifiesto figura en `public.backups`. La pantalla avisa si no existe una copia remota confirmada o si la última tiene más de 24 horas. «Pendientes de subir» es la cola de **respaldos**, distinta de la cola de sincronización operativa. Una fecha vacía no significa que se perdió una copia: significa que el sistema todavía no puede confirmar una.

## Estado de implementación

El motor I05/I08 está operativo en el equipo de recepción: Online Backup, verificación de integridad, gzip + AES-GCM, cola local (`backup_queue`), planificador diario (~04:00), `backup_run_now` / `backup_status` (`ready: true` cuando hay clave), upload a Storage e INSERT en `public.backups`, y `backup_restore` desde una copia local de la cola.

«Respaldar ahora» y «Restaurar copia» se habilitan cuando `backup_status.ready` es verdadero. La restauración valida manifiesto e integridad, reemplaza la base, limpia `sync_outbox` / `sync_state` y revoca sesiones.

**Pendiente fuera de este código:** desplegar la Edge Function `backup-retention` + cron (política 30 diarios / 12 mensuales); prueba E2E de restauración en otro equipo con Storage real; custodio de la clave de cifrado (D10) fuera del repositorio.

## Si necesitás recuperar datos

1. Detené la operación de recepción. No hagas ingresos, cierres ni ajustes mientras se recupera.
2. Identificá una copia en la lista local (o un artefacto cifrado conocido) y al custodio de la clave. No copies la clave a este repositorio ni a la base.
3. En Ajustes → Respaldos, usá **Restaurar copia**, elegí la fila y confirmá. El motor comprueba checksum e integridad antes del swap.
4. Volvé a iniciar sesión. Validá habitaciones, cuentas, historial y tickets.
5. La sincronización se reinicia sin reenviar la outbox vieja (bootstrap de nuevo cuando haya red).

No reemplaces manualmente el archivo SQLite mientras la aplicación está abierta. La copia previa a una migración no sustituye al respaldo diario cifrado.
