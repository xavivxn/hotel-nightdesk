-- Nightdesk: deja Supabase SIN DATOS para empezar de cero.
--
-- BORRA (no se puede deshacer):
--   habitaciones, tarifas, productos, ajustes del negocio, usuarios de la app,
--   huéspedes, reservas, estadías, consumos, pagos, auditorías, registros de
--   sincronización y la lista de respaldos.
--
-- NO BORRA:
--   - La estructura: tablas, funciones, RLS, buckets y migraciones.
--   - Los usuarios de Supabase Auth (admin remoto y `device`): la app sigue
--     pudiendo conectarse con las credenciales que ya tiene el instalador.
--   - Los archivos de Storage. Vaciá el bucket `backups` a mano desde el panel
--     (Storage → backups). No toques `updates` si querés seguir publicando
--     actualizaciones.
--
-- Uso:
--   1. Si hay algo que quieras conservar, exportalo antes.
--   2. Cerrá la app en las PCs de recepción (bandeja → salir).
--   3. Supabase → SQL Editor → pegá este archivo completo (sin seleccionar
--      una parte: así se ejecuta todo).
--   4. Más abajo, cambiá  confirmar text := 'NO';
--                   por   confirmar text := 'BORRAR TODO';
--      y tocá Run. Si queda 'NO', el script se detiene sin borrar nada.
--   5. El resultado final debe mostrar 0 filas en todas las tablas.

DO $$
DECLARE
  -- >>> CONFIRMACIÓN: reemplazá 'NO' por 'BORRAR TODO' <<<
  confirmar text := 'NO';

  tablas text[] := ARRAY[
    -- Operación
    'public.payments',
    'public.charges',
    'public.stays',
    'public.reservations',
    'public.guests',
    -- Auditoría y sincronización
    'public.operational_audit',
    'public.catalog_audit',
    'public.catalog_deletes',
    'nightdesk.catalog_operations',
    'public.sync_applied_ops',
    'public.sync_devices',
    'public.backups',
    -- Catálogo
    'public.rooms',
    'public.rate_plans',
    'public.products',
    'public.business_settings',
    'public.app_users'
  ];
  existentes text;
BEGIN
  IF confirmar IS DISTINCT FROM 'BORRAR TODO' THEN
    RAISE EXCEPTION 'No se borró nada: cambiá confirmar text := ''NO'' por ''BORRAR TODO'' y volvé a ejecutar.';
  END IF;

  -- Solo las tablas que existen (operational_audit puede no estar migrada todavía).
  SELECT string_agg(t, ', ') INTO existentes
  FROM unnest(tablas) AS t
  WHERE to_regclass(t) IS NOT NULL;

  -- Un único TRUNCATE: se vacían todas juntas o ninguna.
  EXECUTE 'TRUNCATE TABLE ' || existentes || ' RESTART IDENTITY';
END $$;

-- Verificación: todas las tablas deben mostrar 0.
SELECT t AS tabla,
       (xpath('/row/n/text()',
              query_to_xml(format('SELECT count(*) AS n FROM %s', t), false, true, '')))[1]::text::bigint AS filas
FROM unnest(ARRAY[
  'public.payments', 'public.charges', 'public.stays', 'public.reservations', 'public.guests',
  'public.operational_audit', 'public.catalog_audit', 'public.catalog_deletes',
  'nightdesk.catalog_operations', 'public.sync_applied_ops', 'public.sync_devices', 'public.backups',
  'public.rooms', 'public.rate_plans', 'public.products', 'public.business_settings', 'public.app_users'
]) AS t
WHERE to_regclass(t) IS NOT NULL;
