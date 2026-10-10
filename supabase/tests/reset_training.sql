-- Isolated local database only. psql -v ON_ERROR_STOP=1 -f supabase/tests/reset_training.sql
BEGIN;

CREATE TEMP TABLE auth_before AS SELECT id FROM auth.users;
CREATE TEMP TABLE policies_before AS
  SELECT schemaname, tablename, policyname, cmd, qual, with_check
  FROM pg_policies WHERE schemaname IN ('public', 'storage');

INSERT INTO public.rooms(uid, number, status)
VALUES ('11111111-0000-4000-8000-000000000001', 'training-reset-test', 'occupied');
INSERT INTO public.rate_plans(uid, name, kind, base_amount_cents)
VALUES ('11111111-0000-4000-8000-000000000002', 'training-reset-test', 'hourly', 1);
INSERT INTO public.guests(uid, name)
VALUES ('11111111-0000-4000-8000-000000000003', 'training-reset-test');
INSERT INTO public.stays(uid, room_uid, rate_plan_uid, guest_uid, check_in_at)
VALUES ('11111111-0000-4000-8000-000000000004', '11111111-0000-4000-8000-000000000001',
        '11111111-0000-4000-8000-000000000002', '11111111-0000-4000-8000-000000000003', now());
INSERT INTO public.charges(stay_uid, kind, description, amount_cents)
VALUES ('11111111-0000-4000-8000-000000000004', 'surcharge', 'training-reset-test', 1);
INSERT INTO public.app_users(username, password_hash, role)
VALUES ('training-reset-test', 'local-hash', 'admin');
INSERT INTO public.business_settings(key, value) VALUES ('business_name', 'training-reset-test')
ON CONFLICT (key) DO UPDATE SET value = excluded.value;
INSERT INTO nightdesk.catalog_operations(operation_id, caller, fingerprint, result)
VALUES ('11111111-0000-4000-8000-000000000005', '11111111-0000-4000-8000-000000000006', 'old', '{}');

SET LOCAL nightdesk.confirm_training_reset = 'post-training-2026-10';
\ir ../maintenance/reset_training.sql

DO $test$
DECLARE
  tbl text;
  remaining bigint;
  result jsonb;
BEGIN
  FOREACH tbl IN ARRAY ARRAY[
    'public.rooms', 'public.rate_plans', 'public.products', 'public.business_settings',
    'public.app_users', 'public.guests', 'public.reservations', 'public.stays',
    'public.charges', 'public.payments', 'public.catalog_audit', 'public.catalog_deletes',
    'public.operational_audit', 'public.sync_applied_ops', 'public.sync_devices',
    'public.backups', 'nightdesk.catalog_operations'
  ] LOOP
    EXECUTE 'SELECT count(*) FROM ' || tbl INTO remaining;
    IF remaining <> 0 THEN RAISE EXCEPTION 'Training data retained in %', tbl; END IF;
  END LOOP;

  IF EXISTS ((SELECT id FROM auth.users EXCEPT SELECT id FROM auth_before)
             UNION ALL (SELECT id FROM auth_before EXCEPT SELECT id FROM auth.users)) THEN
    RAISE EXCEPTION 'Auth users changed';
  END IF;
  IF EXISTS (SELECT * FROM policies_before EXCEPT
             SELECT schemaname, tablename, policyname, cmd, qual, with_check FROM pg_policies) THEN
    RAISE EXCEPTION 'RLS policies changed';
  END IF;

  PERFORM set_config('request.jwt.claims',
    '{"sub":"11111111-0000-4000-8000-000000000006","app_metadata":{"role":"device","device_id":"11111111-0000-4000-8000-000000000006"}}', true);
  result := public.sync_bootstrap_catalog(
    '11111111-0000-4000-8000-000000000006',
    '[{"uid":"22222222-0000-4000-8000-000000000001","local_id":1,"number":"01","room_type":"Normal","status":"available"}]',
    '[]', '[]', '[]',
    '[{"uid":"22222222-0000-4000-8000-000000000002","local_id":7,"username":"admin-local","password_hash":"preserved-local-hash","role":"admin","version":3}]'
  );
  IF result->>'accepted' <> 'true' THEN RAISE EXCEPTION 'Clean bootstrap was rejected'; END IF;
  IF NOT EXISTS (SELECT 1 FROM public.app_users WHERE username = 'admin-local'
                 AND password_hash = 'preserved-local-hash' AND version = 3) THEN
    RAISE EXCEPTION 'Local users were not restored by bootstrap';
  END IF;
END;
$test$;

ROLLBACK;
