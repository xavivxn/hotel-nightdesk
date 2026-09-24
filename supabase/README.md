# Nightdesk — esquema Supabase (N12)

Réplica de lectura, autoridad del catálogo y destino de respaldos. SQLite en recepción sigue siendo la fuente operativa. Contrato: [docs/arquitectura-offline-supabase.md](../docs/arquitectura-offline-supabase.md) y [docs/contrato-ipc-api.md](../docs/contrato-ipc-api.md).

No commitear URL, anon key, `service_role` ni contraseñas. En git/Jira solo «configurado».

## Requisitos locales

- Docker Desktop en marcha
- Node.js 20+ (el CLI se invoca con `npx supabase`)

## Arranque y reset

```bash
npx supabase start
npx supabase db reset --yes
```

`db reset` aplica las migraciones de `supabase/migrations/` y el seed local `supabase/seed.sql`.

Studio local: http://127.0.0.1:54323  
API local: http://127.0.0.1:54321

Las claves de `npx supabase status` son del stack local. No las subas al repo.

## Pruebas SQL (roles admin y device)

`supabase db query --local -f` no acepta varios statements. Correr el script con `psql` del contenedor:

```bash
docker exec -i supabase_db_hotel-nightdesk \
  psql -U postgres -d postgres -v ON_ERROR_STOP=1 -f - \
  < supabase/tests/n12_rls_rpc.sql
```

Cubre: admin no updatea `stays` ni `rooms.status`; device no escribe catálogo por PostgREST; el mismo `operation_id` no duplica; `expected_version` viejo falla con SQLSTATE `P0001` y mensaje `conflict:`.

Advisors:

```bash
npx supabase db advisors --local --type security
```

## Migraciones

Nombres generados por `supabase migration new` (timestamp). Lógicamente:

| Archivo | Contenido |
|---|---|
| `*_schema.sql` | Tablas espejo (`uid` PK, `local_id` IPC), triggers `set_updated_at` / `bump_version`, `sync_*`, `backups` |
| `*_rls.sql` | `is_admin()` / `is_device()` por `app_metadata.role`, políticas, trigger de `rooms.status`, bucket `backups` |
| `*_rpc.sql` | RPC `SECURITY DEFINER` en schema privado `nightdesk`, wrappers `INVOKER` en `public`, Realtime |

El schema `nightdesk` no está en `config.toml` → `[api].schemas`. PostgREST llama a los wrappers de `public` (`sync_apply_ops`, `sync_bootstrap_catalog`, `catalog_upsert_*`).

JWT: la autorización lee **`app_metadata.role`**, nunca `user_metadata`. El access token no siempre está fresco hasta el refresh.

## Auth de prueba

### Local (seed)

Tras `db reset` quedan dos usuarios Auth de desarrollo (no usar en hosted):

| Email | `app_metadata` | Contraseña |
|---|---|---|
| `admin@nightdesk.local` | `role=admin` | `123456` (solo local) |
| `device@nightdesk.local` | `role=device`, `device_id` fijo | `123456` (solo local) |

### Hosted

Crear en Authentication → Users (contraseñas solo por canal seguro). Después, en SQL editor, **sin** tocar `user_metadata`:

```sql
-- admin
UPDATE auth.users
SET raw_app_meta_data = raw_app_meta_data || '{"role":"admin"}'::jsonb
WHERE email = '<admin-email>';

-- device de recepción
UPDATE auth.users
SET raw_app_meta_data = raw_app_meta_data || jsonb_build_object(
  'role', 'device',
  'device_id', '<device-uuid>'
)
WHERE email = '<device-email>';
```

El usuario debe refrescar sesión para que el JWT lleve el claim nuevo.

## Linkear y aplicar a producción

Proyecto hosted **nightdesk** creado (región `sa-east-1`). La sesión MCP no veía `nightdesk-org`; quedó en la org Lomiteria. No reutilizar otros proyectos.

```bash
npx supabase login
npx supabase link --project-ref <project-ref>
npx supabase db push
```

Si el hosted ya tiene las migraciones N12, `db push` no debe reaplicarlas. URL y anon key van a I07 / `sync_configure_device` por canal seguro. Rotar la anon key desde el dashboard si se filtra; no commitearla.

Usuarios Auth hosted: `lovenestt@gmail.com` (`role=admin`) y `device@nightdesk.app` (`role=device`). Contraseñas y `device_id` solo por canal seguro; rotarlas al entregar.

Advisors: `npx supabase db advisors --linked --type security` o MCP `get_advisors`.

## Bucket `backups`

Privado. Device: `INSERT` de objetos (+ `SELECT` del propio para confirmar). Admin: `SELECT`. Delete solo `service_role` / Edge Function de I08. N12 no hace upsert de objetos.

## Qué no entra en N12

Worker Rust, `src/lib/supabase.ts`, UI remota, Edge Function de retención (I08), migración SQLite `014_sync` (I06).
