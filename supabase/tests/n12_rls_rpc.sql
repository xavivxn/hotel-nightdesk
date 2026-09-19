-- N12 acceptance: RLS, rooms.status trigger, idempotent apply_ops, catalog conflict.
-- Run after `supabase db reset` as postgres:
--   docker exec -i supabase_db_hotel-nightdesk psql -U postgres -d postgres -v ON_ERROR_STOP=1 -f - < supabase/tests/n12_rls_rpc.sql
-- JWT claims are injected; they are not always fresh in production until refresh.

DO $$
DECLARE
  admin_jwt text := $jwt${
    "sub":"11111111-1111-1111-1111-111111111111",
    "role":"authenticated",
    "app_metadata":{"role":"admin"}
  }$jwt$;
  device_jwt text := $jwt${
    "sub":"22222222-2222-2222-2222-222222222222",
    "role":"authenticated",
    "app_metadata":{"role":"device","device_id":"33333333-3333-3333-3333-333333333333"}
  }$jwt$;
  device_id uuid := '33333333-3333-3333-3333-333333333333';
  room_uid uuid := 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa';
  rate_uid uuid := 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb';
  guest_uid uuid := 'cccccccc-cccc-cccc-cccc-cccccccccccc';
  stay_uid uuid := 'dddddddd-dddd-dddd-dddd-dddddddddddd';
  op_id uuid := 'eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee';
  n int;
  notes_val text;
  status_val text;
  apply_result jsonb;
  conflicted boolean;
BEGIN
  -- Fixtures as table owner (bypass RLS)
  RESET ROLE;
  PERFORM set_config('request.jwt.claims', '', true);

  INSERT INTO public.rooms (uid, number, room_type, floor, status, active)
  VALUES (room_uid, 'N12', 'Estándar', 1, 'available', true);

  INSERT INTO public.rate_plans (
    uid, name, kind, base_amount_cents, extra_hour_cents, included_hours
  )
  VALUES (rate_uid, 'N12 hora', 'hourly', 80000, 20000, 2);

  INSERT INTO public.guests (uid, name) VALUES (guest_uid, 'Huésped N12');

  INSERT INTO public.stays (
    uid, room_uid, guest_uid, rate_plan_uid, check_in_at, status, notes
  )
  VALUES (stay_uid, room_uid, guest_uid, rate_uid, now(), 'open', 'original');

  -- 1. Admin cannot update stays (GRANT + RLS)
  PERFORM set_config('request.jwt.claims', admin_jwt, true);
  n := -1;
  BEGIN
    SET LOCAL ROLE authenticated;
    UPDATE public.stays SET notes = 'admin-hack' WHERE uid = stay_uid;
    GET DIAGNOSTICS n = ROW_COUNT;
    RESET ROLE;
  EXCEPTION
    WHEN insufficient_privilege THEN
      RESET ROLE;
      n := 0;
  END;
  PERFORM set_config('request.jwt.claims', '', true);
  IF n <> 0 THEN
    RAISE EXCEPTION 'FAIL: admin updated stays (% rows)', n;
  END IF;
  SELECT notes INTO notes_val FROM public.stays WHERE uid = stay_uid;
  IF notes_val IS DISTINCT FROM 'original' THEN
    RAISE EXCEPTION 'FAIL: stay notes changed to %', notes_val;
  END IF;
  RAISE NOTICE 'PASS: admin cannot update stays';

  -- 2. Admin cannot change rooms.status
  PERFORM set_config('request.jwt.claims', admin_jwt, true);
  SET LOCAL ROLE authenticated;
  BEGIN
    UPDATE public.rooms SET status = 'blocked' WHERE uid = room_uid;
    RESET ROLE;
    RAISE EXCEPTION 'FAIL: admin changed rooms.status';
  EXCEPTION
    WHEN others THEN
      RESET ROLE;
      PERFORM set_config('request.jwt.claims', '', true);
      IF SQLERRM LIKE 'FAIL:%' THEN
        RAISE;
      END IF;
      IF SQLERRM NOT LIKE '%rooms.status%' AND SQLSTATE <> 'P0001' THEN
        RAISE EXCEPTION 'FAIL: unexpected error changing rooms.status: %', SQLERRM;
      END IF;
  END;
  SELECT status INTO status_val FROM public.rooms WHERE uid = room_uid;
  IF status_val IS DISTINCT FROM 'available' THEN
    RAISE EXCEPTION 'FAIL: rooms.status became %', status_val;
  END IF;
  RAISE NOTICE 'PASS: admin cannot change rooms.status';

  -- 3. Device cannot write catalog via PostgREST
  PERFORM set_config('request.jwt.claims', device_jwt, true);
  SET LOCAL ROLE authenticated;
  BEGIN
    INSERT INTO public.rate_plans (uid, name, kind, base_amount_cents)
    VALUES (gen_random_uuid(), 'device-hack', 'hourly', 1);
    GET DIAGNOSTICS n = ROW_COUNT;
    RESET ROLE;
    IF n <> 0 THEN
      RAISE EXCEPTION 'FAIL: device inserted rate_plans via PostgREST';
    END IF;
  EXCEPTION
    WHEN insufficient_privilege OR check_violation THEN
      RESET ROLE;
    WHEN others THEN
      RESET ROLE;
      IF SQLERRM LIKE 'FAIL:%' THEN
        RAISE;
      END IF;
      -- RLS typically raises insufficient_privilege (42501)
      IF SQLSTATE NOT IN ('42501', '42501') AND SQLERRM NOT ILIKE '%policy%' AND SQLERRM NOT ILIKE '%permission%' THEN
        -- 0-row update/insert from RLS can also appear as success with 0 rows
        NULL;
      END IF;
  END;
  PERFORM set_config('request.jwt.claims', '', true);
  IF EXISTS (SELECT 1 FROM public.rate_plans WHERE name = 'device-hack') THEN
    RAISE EXCEPTION 'FAIL: device catalog row persisted';
  END IF;
  RAISE NOTICE 'PASS: device cannot write catalog via PostgREST';

  -- 4. Same operation_id twice does not duplicate
  PERFORM set_config('request.jwt.claims', device_jwt, true);
  apply_result := nightdesk.sync_apply_ops(
    device_id,
    jsonb_build_array(
      jsonb_build_object(
        'operation_id', op_id,
        'entity', 'stay',
        'entity_uid', stay_uid,
        'op', 'upsert',
        'payload', jsonb_build_object(
          'uid', stay_uid,
          'room_uid', room_uid,
          'guest_uid', guest_uid,
          'rate_plan_uid', rate_uid,
          'check_in_at', now(),
          'status', 'open',
          'notes', 'from-device'
        )
      )
    )
  );
  IF (apply_result ->> 'applied')::int <> 1 THEN
    RAISE EXCEPTION 'FAIL: first apply expected applied=1 got %', apply_result;
  END IF;

  apply_result := nightdesk.sync_apply_ops(
    device_id,
    jsonb_build_array(
      jsonb_build_object(
        'operation_id', op_id,
        'entity', 'stay',
        'entity_uid', stay_uid,
        'op', 'upsert',
        'payload', jsonb_build_object(
          'uid', stay_uid,
          'room_uid', room_uid,
          'guest_uid', guest_uid,
          'rate_plan_uid', rate_uid,
          'check_in_at', now(),
          'status', 'open',
          'notes', 'duplicate'
        )
      )
    )
  );
  IF (apply_result ->> 'skipped')::int <> 1 OR (apply_result ->> 'applied')::int <> 0 THEN
    RAISE EXCEPTION 'FAIL: replay expected skipped=1 got %', apply_result;
  END IF;
  SELECT notes INTO notes_val FROM public.stays WHERE uid = stay_uid;
  IF notes_val IS DISTINCT FROM 'from-device' THEN
    RAISE EXCEPTION 'FAIL: replay changed stay notes to %', notes_val;
  END IF;
  IF (SELECT count(*) FROM public.stays WHERE uid = stay_uid) <> 1 THEN
    RAISE EXCEPTION 'FAIL: stay duplicated';
  END IF;
  IF (SELECT count(*) FROM public.sync_applied_ops WHERE operation_id = op_id) <> 1 THEN
    RAISE EXCEPTION 'FAIL: operation_id stored more than once';
  END IF;
  RAISE NOTICE 'PASS: same operation_id is idempotent';

  -- 5. Stale expected_version fails with P0001 / conflict
  PERFORM set_config('request.jwt.claims', admin_jwt, true);
  conflicted := false;
  BEGIN
    PERFORM nightdesk.catalog_upsert_room(
      room_uid,
      0,
      'N12',
      'Estándar',
      1,
      'stale',
      true,
      NULL
    );
  EXCEPTION
    WHEN others THEN
      IF SQLSTATE = 'P0001' AND SQLERRM LIKE 'conflict:%' THEN
        conflicted := true;
      ELSE
        RAISE EXCEPTION 'FAIL: unexpected conflict error % %', SQLSTATE, SQLERRM;
      END IF;
  END;
  IF NOT conflicted THEN
    RAISE EXCEPTION 'FAIL: stale expected_version did not conflict';
  END IF;
  RAISE NOTICE 'PASS: stale expected_version returns conflict';

  PERFORM set_config('request.jwt.claims', '', true);
  RAISE NOTICE 'N12 RLS/RPC tests passed';
END $$;
