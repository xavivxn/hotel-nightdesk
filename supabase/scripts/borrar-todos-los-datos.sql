-- Nightdesk: deja Supabase SIN DATOS para empezar de cero.
-- Auditado el 09/10/2026: 17 tablas de la app, incluida operational_audit.
-- La migración product_analytics no crea tablas adicionales: product_uid y
-- product_quantity se eliminan al vaciar charges; product_tracking_since,
-- al vaciar stays. No se reconstruyen ni conservan ventas históricas.
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
--      IMPORTANTE: este SQL no limpia SQLite ni la outbox de esas PCs.
--      Antes de reconectarlas, prepará una base local limpia para Día D;
--      una base vieja puede volver a subir los datos que acabás de borrar.
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
  no_incluidas text;
  tabla text;
  filas bigint;
BEGIN
  IF confirmar IS DISTINCT FROM 'BORRAR TODO' THEN
    RAISE EXCEPTION 'No se borró nada: cambiá confirmar text := ''NO'' por ''BORRAR TODO'' y volvé a ejecutar.';
  END IF;

  -- Una tabla nueva de la app exige revisar el listado antes de borrar.
  -- Se excluyen tablas que pertenecen a extensiones de PostgreSQL.
  SELECT string_agg(format('%I.%I', n.nspname, c.relname), ', ' ORDER BY n.nspname, c.relname)
  INTO no_incluidas
  FROM pg_class c
  JOIN pg_namespace n ON n.oid = c.relnamespace
  WHERE n.nspname IN ('public', 'nightdesk')
    AND c.relkind IN ('r', 'p')
    AND NOT c.relispartition
    AND format('%I.%I', n.nspname, c.relname) <> ALL(tablas)
    AND NOT EXISTS (
      SELECT 1 FROM pg_depend d
      WHERE d.classid = 'pg_class'::regclass AND d.objid = c.oid
        AND d.refclassid = 'pg_extension'::regclass AND d.deptype = 'e'
    );
  IF no_incluidas IS NOT NULL THEN
    RAISE EXCEPTION 'No se borró nada: hay tablas nuevas sin revisar: %', no_incluidas;
  END IF;

  -- Solo las tablas que existen (operational_audit puede no estar migrada todavía).
  SELECT string_agg(t, ', ' ORDER BY t) INTO existentes
  FROM unnest(tablas) AS t
  WHERE to_regclass(t) IS NOT NULL;

  IF existentes IS NULL THEN
    RAISE EXCEPTION 'No se borró nada: no se encontraron las tablas de Nightdesk.';
  END IF;

  -- Un único TRUNCATE: se vacían todas juntas o ninguna.
  -- Sin CASCADE: no alcanza tablas ajenas al listado por una clave foránea.
  PERFORM set_config('lock_timeout', '5s', true);
  EXECUTE 'TRUNCATE TABLE ' || existentes || ' RESTART IDENTITY';

  -- Comprobar dentro de la misma transacción: si alguna no queda vacía,
  -- la excepción revierte también el TRUNCATE.
  FOR tabla IN SELECT t FROM unnest(tablas) AS t WHERE to_regclass(t) IS NOT NULL
  LOOP
    EXECUTE format('SELECT count(*) FROM %s', tabla) INTO filas;
    IF filas <> 0 THEN
      RAISE EXCEPTION 'Borrado revertido: % conserva % filas.', tabla, filas;
    END IF;
  END LOOP;
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
