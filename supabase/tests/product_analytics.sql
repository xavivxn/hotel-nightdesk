-- Run as postgres after the product_analytics migration.
-- Fixtures use fresh UUIDs; the transaction always rolls back on success.
BEGIN;
SET LOCAL lock_timeout = '3s';
SET LOCAL statement_timeout = '20s';

DO $$
DECLARE
  device_id uuid := gen_random_uuid();
  room_id uuid := gen_random_uuid();
  rate_id uuid := gen_random_uuid();
  guest_id uuid := gen_random_uuid();
  stay_id uuid := gen_random_uuid();
  charge_id uuid := gen_random_uuid();
  product_id uuid := gen_random_uuid();
  tombstone_id uuid := gen_random_uuid();
  audit_id uuid := gen_random_uuid();
  sale_op_id uuid := gen_random_uuid();
  stay_payload jsonb;
  charge_payload jsonb;
  sale jsonb;
  result jsonb;
  charge_row public.charges%ROWTYPE;
  rejected boolean;
BEGIN
  INSERT INTO public.rooms(uid, number) VALUES (room_id, 'analytics-test-' || room_id::text);
  INSERT INTO public.rate_plans(uid, name, kind, base_amount_cents)
    VALUES (rate_id, 'Analytics test', 'hourly', 40000);
  INSERT INTO public.guests(uid, name) VALUES (guest_id, 'Analytics test');
  PERFORM set_config('request.jwt.claims', jsonb_build_object(
    'role', 'authenticated', 'app_metadata', jsonb_build_object('role', 'device', 'device_id', device_id))::text, true);

  stay_payload := jsonb_build_object('uid', stay_id, 'room_uid', room_id, 'guest_uid', guest_id,
    'rate_plan_uid', rate_id, 'check_in_at', '2026-09-02T00:00:00Z', 'status', 'open',
    'product_tracking_since', '2026-09-01T00:00:00Z');
  charge_payload := jsonb_build_object('uid', charge_id, 'stay_uid', stay_id, 'kind', 'surcharge',
    'description', 'Agua histórica', 'amount_cents', 5000, 'product_uid', product_id,
    'product_quantity', 1, 'created_at', '2026-09-02T01:00:00Z');
  sale := jsonb_build_object('operation_id', sale_op_id, 'entity', 'charge', 'op', 'upsert', 'payload', charge_payload);
  SET LOCAL ROLE authenticated;
  result := public.sync_apply_ops(device_id, jsonb_build_array(
    jsonb_build_object('operation_id', gen_random_uuid(), 'entity', 'stay', 'payload', stay_payload), sale));
  IF result <> '{"applied":2,"skipped":0}'::jsonb THEN RAISE EXCEPTION 'FAIL: sale batch'; END IF;
  result := public.sync_apply_ops(device_id, jsonb_build_array(sale));
  IF result <> '{"applied":0,"skipped":1}'::jsonb THEN RAISE EXCEPTION 'FAIL: replay'; END IF;
  RESET ROLE;
  SELECT * INTO STRICT charge_row FROM public.charges WHERE uid = charge_id;
  IF charge_row.product_uid <> product_id OR charge_row.product_quantity <> 1
     OR charge_row.amount_cents <> 5000 OR charge_row.created_at <> '2026-09-02T01:00:00Z'::timestamptz
     THEN RAISE EXCEPTION 'FAIL: product snapshot'; END IF;
  IF (SELECT product_tracking_since FROM public.stays WHERE uid = stay_id)
     IS DISTINCT FROM '2026-09-01T00:00:00Z'::timestamptz THEN RAISE EXCEPTION 'FAIL: coverage'; END IF;

  -- Older clients cannot erase already received product metadata or sale time.
  SET LOCAL ROLE authenticated;
  PERFORM public.sync_apply_ops(device_id, jsonb_build_array(
    jsonb_build_object('operation_id', gen_random_uuid(), 'entity', 'stay', 'payload', stay_payload - 'product_tracking_since'),
    jsonb_build_object('operation_id', gen_random_uuid(), 'entity', 'charge', 'payload',
      charge_payload - 'product_uid' - 'product_quantity' - 'created_at')));
  RESET ROLE;
  SELECT * INTO STRICT charge_row FROM public.charges WHERE uid = charge_id;
  IF charge_row.product_uid <> product_id OR charge_row.product_quantity <> 1
     OR charge_row.created_at <> '2026-09-02T01:00:00Z'::timestamptz
     THEN RAISE EXCEPTION 'FAIL: legacy payload erased product'; END IF;
  IF (SELECT product_tracking_since FROM public.stays WHERE uid = stay_id)
     IS DISTINCT FROM '2026-09-01T00:00:00Z'::timestamptz THEN RAISE EXCEPTION 'FAIL: legacy payload erased coverage'; END IF;

  SET LOCAL ROLE authenticated;
  PERFORM public.sync_apply_ops(device_id, jsonb_build_array(
    jsonb_build_object('operation_id', gen_random_uuid(), 'entity', 'charge', 'op', 'delete', 'payload',
      charge_payload || '{"deleted_at":"2026-09-02T02:00:00Z"}'::jsonb),
    jsonb_build_object('operation_id', gen_random_uuid(), 'entity', 'charge', 'op', 'delete', 'payload',
      charge_payload || jsonb_build_object('uid', tombstone_id, 'deleted_at', '2026-09-02T02:00:00Z'))));
  RESET ROLE;
  IF (SELECT count(*) FROM public.charges WHERE uid IN (charge_id, tombstone_id)
      AND deleted_at IS NOT NULL AND product_uid = product_id AND product_quantity = 1) <> 2
     THEN RAISE EXCEPTION 'FAIL: void metadata'; END IF;

  rejected := false;
  BEGIN
    SET LOCAL ROLE authenticated;
    PERFORM public.sync_apply_ops(device_id, jsonb_build_array(jsonb_build_object(
      'operation_id', gen_random_uuid(), 'entity', 'charge', 'payload',
      charge_payload || jsonb_build_object('uid', gen_random_uuid(), 'product_quantity', 0))));
    RESET ROLE;
  EXCEPTION WHEN check_violation THEN RESET ROLE; rejected := true;
  END;
  IF NOT rejected THEN RAISE EXCEPTION 'FAIL: invalid quantity accepted'; END IF;

  -- Keep the previously deployed append-only reception audit working.
  SET LOCAL ROLE authenticated;
  PERFORM public.sync_apply_ops(device_id, jsonb_build_array(jsonb_build_object(
    'operation_id', gen_random_uuid(), 'entity', 'operational_audit', 'op', 'upsert',
    'payload', jsonb_build_object('uid', audit_id, 'operation_id', gen_random_uuid(),
      'actor_uid', gen_random_uuid(), 'username', 'Analytics test', 'station_id', gen_random_uuid(),
      'command', 'check_out', 'entity_id', 1, 'closed_total_cents', 5000,
      'created_at', '2026-09-02T02:00:00Z'))));
  RESET ROLE;
  IF NOT EXISTS (SELECT 1 FROM public.operational_audit WHERE uid = audit_id)
     THEN RAISE EXCEPTION 'FAIL: reception audit lost'; END IF;
  rejected := false;
  BEGIN
    SET LOCAL ROLE authenticated;
    PERFORM public.sync_apply_ops(device_id, jsonb_build_array(jsonb_build_object(
      'operation_id', gen_random_uuid(), 'entity', 'operational_audit', 'op', 'delete',
      'payload', jsonb_build_object('uid', audit_id))));
    RESET ROLE;
  EXCEPTION WHEN raise_exception THEN
    RESET ROLE;
    IF SQLERRM NOT LIKE '%audit is append-only%' THEN RAISE; END IF;
    rejected := true;
  END;
  IF NOT rejected THEN RAISE EXCEPTION 'FAIL: audit delete accepted'; END IF;

  rejected := false;
  PERFORM set_config('request.jwt.claims', '{"role":"authenticated","app_metadata":{"role":"admin"}}', true);
  BEGIN
    SET LOCAL ROLE authenticated;
    PERFORM public.sync_apply_ops(device_id, '[]'::jsonb);
    RESET ROLE;
  EXCEPTION WHEN insufficient_privilege THEN
    RESET ROLE;
    IF SQLERRM NOT LIKE '%device role required%' THEN RAISE; END IF;
    rejected := true;
  END;
  IF NOT rejected THEN RAISE EXCEPTION 'FAIL: admin allowed operational sync'; END IF;
END;
$$;

ROLLBACK;
SELECT 'product_analytics: snapshots, replay, legacy clients, voids, validation, audit and authorization passed; fixtures rolled back' AS result;
