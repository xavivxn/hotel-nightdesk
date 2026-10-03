-- Run after all migrations on an isolated Supabase test database as postgres.
-- Fixtures roll back; production data must never be used for this acceptance test.
BEGIN;
DO $$
DECLARE
  device_jwt text := '{"sub":"22222222-2222-2222-2222-222222222222","role":"authenticated","app_metadata":{"role":"device","device_id":"33333333-3333-3333-3333-333333333333"}}';
  admin_jwt text := '{"sub":"11111111-1111-1111-1111-111111111111","role":"authenticated","app_metadata":{"role":"admin"}}';
  payload jsonb := '{"uid":"99999999-1111-4111-8111-111111111111","operation_id":"99999999-2222-4222-8222-222222222222","actor_uid":"99999999-3333-4333-8333-333333333333","username":"Recepción histórica","station_id":"99999999-4444-4444-8444-444444444444","command":"check_out","entity_id":777777,"closed_total_cents":90000,"created_at":"2026-10-04T00:10:00-03:00"}';
  ops jsonb;
  result jsonb;
  n integer;
  denied boolean := false;
BEGIN
  ops := jsonb_build_array(jsonb_build_object('operation_id','99999999-5555-4555-8555-555555555555','entity','operational_audit','entity_uid',payload->>'uid','op','upsert','payload',payload));
  PERFORM set_config('request.jwt.claims',device_jwt,true);
  SET LOCAL ROLE authenticated;
  SELECT public.sync_apply_ops('33333333-3333-3333-3333-333333333333',ops) INTO result;
  IF (result->>'applied')::int <> 1 THEN RAISE EXCEPTION 'audit not applied: %', result; END IF;
  SELECT public.sync_apply_ops('33333333-3333-3333-3333-333333333333',ops) INTO result;
  IF (result->>'skipped')::int <> 1 THEN RAISE EXCEPTION 'audit replay not skipped'; END IF;
  SELECT count(*) INTO n FROM public.operational_audit WHERE uid=(payload->>'uid')::uuid;
  IF n<>0 THEN RAISE EXCEPTION 'device reads administrator audit'; END IF;
  RESET ROLE;
  PERFORM set_config('request.jwt.claims',admin_jwt,true);
  SET LOCAL ROLE authenticated;
  SELECT count(*) INTO n FROM public.operational_audit WHERE uid=(payload->>'uid')::uuid AND closed_total_cents=90000;
  IF n<>1 THEN RAISE EXCEPTION 'administrator cannot read exactly one audit'; END IF;
  BEGIN
    UPDATE public.operational_audit SET username='changed' WHERE uid=(payload->>'uid')::uuid;
  EXCEPTION WHEN insufficient_privilege THEN denied:=true;
  END;
  IF NOT denied THEN RAISE EXCEPTION 'administrator can modify audit'; END IF;
  denied:=false;
  BEGIN
    PERFORM public.sync_apply_ops('33333333-3333-3333-3333-333333333333',ops);
  EXCEPTION WHEN OTHERS THEN denied:=true;
  END;
  IF NOT denied THEN RAISE EXCEPTION 'administrator can use device sync'; END IF;
  RESET ROLE;
  RAISE NOTICE 'PASS: single-device audit replication, replay, immutable administrator read, role separation';
END $$;
ROLLBACK;
