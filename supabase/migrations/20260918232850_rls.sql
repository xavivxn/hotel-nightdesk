-- N12 RLS, Auth helpers, rooms.status trigger, private backups bucket.
-- Authorization reads app_metadata.role only (never user_metadata).
-- JWT claims are not always fresh until the access token is refreshed.

CREATE SCHEMA IF NOT EXISTS nightdesk;
REVOKE ALL ON SCHEMA nightdesk FROM PUBLIC;
REVOKE ALL ON SCHEMA nightdesk FROM anon;
GRANT USAGE ON SCHEMA nightdesk TO authenticated, service_role;

-- ---------------------------------------------------------------------------
-- Role helpers (INVOKER; only read auth.jwt())
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION public.is_admin()
RETURNS boolean
LANGUAGE sql
STABLE
SET search_path = public
AS $$
  SELECT coalesce((auth.jwt() -> 'app_metadata' ->> 'role') = 'admin', false);
$$;

CREATE OR REPLACE FUNCTION public.is_device()
RETURNS boolean
LANGUAGE sql
STABLE
SET search_path = public
AS $$
  SELECT coalesce((auth.jwt() -> 'app_metadata' ->> 'role') = 'device', false);
$$;

CREATE OR REPLACE FUNCTION public.jwt_device_id()
RETURNS uuid
LANGUAGE sql
STABLE
SET search_path = public
AS $$
  SELECT NULLIF(auth.jwt() -> 'app_metadata' ->> 'device_id', '')::uuid;
$$;

REVOKE ALL ON FUNCTION public.is_admin() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.is_device() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.jwt_device_id() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.is_admin() TO authenticated, service_role;
GRANT EXECUTE ON FUNCTION public.is_device() TO authenticated, service_role;
GRANT EXECUTE ON FUNCTION public.jwt_device_id() TO authenticated, service_role;

-- Admin must not change operational room columns (reception owns status).
CREATE OR REPLACE FUNCTION public.protect_room_operational_columns()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = public
AS $$
BEGIN
  IF public.is_admin() AND NEW.status IS DISTINCT FROM OLD.status THEN
    RAISE EXCEPTION 'conflict: admin cannot change rooms.status'
      USING ERRCODE = 'P0001';
  END IF;
  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION public.protect_room_operational_columns() FROM PUBLIC;

CREATE TRIGGER rooms_protect_operational
  BEFORE UPDATE ON public.rooms
  FOR EACH ROW
  EXECUTE FUNCTION public.protect_room_operational_columns();

-- ---------------------------------------------------------------------------
-- Grants: no DELETE; anon has nothing; authenticated is fenced by RLS
-- ---------------------------------------------------------------------------

REVOKE ALL ON ALL TABLES IN SCHEMA public FROM anon, authenticated;
REVOKE ALL ON ALL SEQUENCES IN SCHEMA public FROM anon, authenticated;

GRANT SELECT ON
  public.rooms,
  public.rate_plans,
  public.products,
  public.app_users,
  public.business_settings,
  public.guests,
  public.reservations,
  public.stays,
  public.charges,
  public.payments,
  public.sync_applied_ops,
  public.sync_devices,
  public.backups
TO authenticated;

GRANT INSERT, UPDATE ON
  public.rooms,
  public.rate_plans,
  public.products,
  public.app_users,
  public.business_settings
TO authenticated;

GRANT INSERT ON public.backups TO authenticated;

-- ---------------------------------------------------------------------------
-- RLS
-- ---------------------------------------------------------------------------

ALTER TABLE public.rooms ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.rate_plans ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.products ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.app_users ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.business_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.guests ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.reservations ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.stays ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.charges ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.payments ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.sync_applied_ops ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.sync_devices ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.backups ENABLE ROW LEVEL SECURITY;

ALTER TABLE public.rooms FORCE ROW LEVEL SECURITY;
ALTER TABLE public.rate_plans FORCE ROW LEVEL SECURITY;
ALTER TABLE public.products FORCE ROW LEVEL SECURITY;
ALTER TABLE public.app_users FORCE ROW LEVEL SECURITY;
ALTER TABLE public.business_settings FORCE ROW LEVEL SECURITY;
ALTER TABLE public.guests FORCE ROW LEVEL SECURITY;
ALTER TABLE public.reservations FORCE ROW LEVEL SECURITY;
ALTER TABLE public.stays FORCE ROW LEVEL SECURITY;
ALTER TABLE public.charges FORCE ROW LEVEL SECURITY;
ALTER TABLE public.payments FORCE ROW LEVEL SECURITY;
ALTER TABLE public.sync_applied_ops FORCE ROW LEVEL SECURITY;
ALTER TABLE public.sync_devices FORCE ROW LEVEL SECURITY;
ALTER TABLE public.backups FORCE ROW LEVEL SECURITY;

-- Catalog SELECT: admin + device
CREATE POLICY rooms_select ON public.rooms
  FOR SELECT TO authenticated
  USING (public.is_admin() OR public.is_device());
CREATE POLICY rate_plans_select ON public.rate_plans
  FOR SELECT TO authenticated
  USING (public.is_admin() OR public.is_device());
CREATE POLICY products_select ON public.products
  FOR SELECT TO authenticated
  USING (public.is_admin() OR public.is_device());
CREATE POLICY app_users_select ON public.app_users
  FOR SELECT TO authenticated
  USING (public.is_admin() OR public.is_device());
CREATE POLICY business_settings_select ON public.business_settings
  FOR SELECT TO authenticated
  USING (public.is_admin() OR public.is_device());

-- Catalog write via PostgREST: admin only (device uses catalog_upsert_* RPC)
CREATE POLICY rooms_admin_insert ON public.rooms
  FOR INSERT TO authenticated
  WITH CHECK (public.is_admin());
CREATE POLICY rooms_admin_update ON public.rooms
  FOR UPDATE TO authenticated
  USING (public.is_admin())
  WITH CHECK (public.is_admin());

CREATE POLICY rate_plans_admin_insert ON public.rate_plans
  FOR INSERT TO authenticated
  WITH CHECK (public.is_admin());
CREATE POLICY rate_plans_admin_update ON public.rate_plans
  FOR UPDATE TO authenticated
  USING (public.is_admin())
  WITH CHECK (public.is_admin());

CREATE POLICY products_admin_insert ON public.products
  FOR INSERT TO authenticated
  WITH CHECK (public.is_admin());
CREATE POLICY products_admin_update ON public.products
  FOR UPDATE TO authenticated
  USING (public.is_admin())
  WITH CHECK (public.is_admin());

CREATE POLICY app_users_admin_insert ON public.app_users
  FOR INSERT TO authenticated
  WITH CHECK (public.is_admin());
CREATE POLICY app_users_admin_update ON public.app_users
  FOR UPDATE TO authenticated
  USING (public.is_admin())
  WITH CHECK (public.is_admin());

CREATE POLICY business_settings_admin_insert ON public.business_settings
  FOR INSERT TO authenticated
  WITH CHECK (public.is_admin());
CREATE POLICY business_settings_admin_update ON public.business_settings
  FOR UPDATE TO authenticated
  USING (public.is_admin())
  WITH CHECK (public.is_admin());

-- Operation: admin SELECT only. No INSERT/UPDATE/DELETE policies (device uses RPC).
CREATE POLICY guests_admin_select ON public.guests
  FOR SELECT TO authenticated
  USING (public.is_admin());
CREATE POLICY reservations_admin_select ON public.reservations
  FOR SELECT TO authenticated
  USING (public.is_admin());
CREATE POLICY stays_admin_select ON public.stays
  FOR SELECT TO authenticated
  USING (public.is_admin());
CREATE POLICY charges_admin_select ON public.charges
  FOR SELECT TO authenticated
  USING (public.is_admin());
CREATE POLICY payments_admin_select ON public.payments
  FOR SELECT TO authenticated
  USING (public.is_admin());

CREATE POLICY sync_applied_ops_select ON public.sync_applied_ops
  FOR SELECT TO authenticated
  USING (public.is_admin() OR (public.is_device() AND device_id = public.jwt_device_id()));

CREATE POLICY sync_devices_select ON public.sync_devices
  FOR SELECT TO authenticated
  USING (public.is_admin() OR (public.is_device() AND device_id = public.jwt_device_id()));

CREATE POLICY backups_admin_select ON public.backups
  FOR SELECT TO authenticated
  USING (public.is_admin());
CREATE POLICY backups_device_insert ON public.backups
  FOR INSERT TO authenticated
  WITH CHECK (public.is_device());

-- No DELETE policies on any table (logical deletes only).

-- ---------------------------------------------------------------------------
-- Storage bucket backups (private). N12 does not upsert objects.
-- Delete is service_role / future I08 Edge Function only.
-- ---------------------------------------------------------------------------

INSERT INTO storage.buckets (id, name, public, file_size_limit)
VALUES ('backups', 'backups', false, 52428800)
ON CONFLICT (id) DO UPDATE
SET public = excluded.public,
    file_size_limit = excluded.file_size_limit;

DROP POLICY IF EXISTS backups_objects_device_insert ON storage.objects;
DROP POLICY IF EXISTS backups_objects_device_select ON storage.objects;
DROP POLICY IF EXISTS backups_objects_admin_select ON storage.objects;

CREATE POLICY backups_objects_device_insert
  ON storage.objects
  FOR INSERT
  TO authenticated
  WITH CHECK (bucket_id = 'backups' AND public.is_device());

CREATE POLICY backups_objects_device_select
  ON storage.objects
  FOR SELECT
  TO authenticated
  USING (
    bucket_id = 'backups'
    AND public.is_device()
    AND owner = auth.uid()
  );

CREATE POLICY backups_objects_admin_select
  ON storage.objects
  FOR SELECT
  TO authenticated
  USING (bucket_id = 'backups' AND public.is_admin());
