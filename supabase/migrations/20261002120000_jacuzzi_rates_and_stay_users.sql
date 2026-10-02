-- Jacuzzi and normal rooms get their own hour / adicional 30 min / dormida prices, and each
-- stay records who opened it (check-in) and who charged it (checkout).
ALTER TABLE public.rate_plans
  ADD COLUMN IF NOT EXISTS room_category text NOT NULL DEFAULT 'normal'
  CHECK (room_category IN ('normal', 'jacuzzi'));
ALTER TABLE public.stays ADD COLUMN IF NOT EXISTS checked_in_by text;
ALTER TABLE public.stays ADD COLUMN IF NOT EXISTS checked_out_by text;

-- Jacuzzi plans start as copies of the normal ones. The uids are fixed and shared with the
-- reception seed (`db::seed_jacuzzi_plans_if_missing`), so pull matches instead of duplicating.
INSERT INTO public.rate_plans (
  uid, local_id, name, kind, base_amount_cents, extra_hour_cents,
  included_hours, grace_minutes, night_cutoff_hour, active, room_category
)
SELECT seed.uid,
       (SELECT COALESCE(MAX(local_id), 0) FROM public.rate_plans) + seed.position,
       src.name || ' Jacuzzi', src.kind, src.base_amount_cents, src.extra_hour_cents,
       src.included_hours, src.grace_minutes, src.night_cutoff_hour, true, 'jacuzzi'
FROM (VALUES
  ('7a3c0f10-2026-4930-8000-000000000001'::uuid, 'hourly', 1),
  ('7a3c0f10-2026-4930-8000-000000000002'::uuid, 'overnight', 2)
) AS seed(uid, kind, position)
CROSS JOIN LATERAL (
  SELECT rp.* FROM public.rate_plans rp
  WHERE rp.kind = seed.kind AND rp.room_category = 'normal'
  ORDER BY rp.active DESC, rp.local_id NULLS LAST
  LIMIT 1
) AS src
ON CONFLICT (uid) DO NOTHING;

DROP FUNCTION IF EXISTS nightdesk.catalog_upsert_rate_plan(
  uuid, integer, text, text, bigint, bigint, integer, integer, integer, boolean, bigint
);

CREATE FUNCTION nightdesk.catalog_upsert_rate_plan(
  p_uid uuid,
  p_expected_version integer,
  p_name text,
  p_kind text,
  p_base_amount_cents bigint,
  p_extra_hour_cents bigint DEFAULT 0,
  p_included_hours integer DEFAULT 1,
  p_grace_minutes integer DEFAULT 10,
  p_night_cutoff_hour integer DEFAULT 12,
  p_active boolean DEFAULT true,
  p_local_id bigint DEFAULT NULL,
  p_room_category text DEFAULT 'normal'
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = nightdesk, public, pg_temp
AS $$
DECLARE
  rec public.rate_plans%ROWTYPE;
BEGIN
  PERFORM nightdesk.require_catalog_writer();

  SELECT * INTO rec FROM public.rate_plans rp WHERE rp.uid = p_uid;
  IF FOUND THEN
    IF rec.version IS DISTINCT FROM p_expected_version THEN
      RAISE EXCEPTION 'conflict: expected_version does not match'
        USING ERRCODE = 'P0001';
    END IF;
    UPDATE public.rate_plans
    SET
      name = p_name,
      kind = p_kind,
      base_amount_cents = p_base_amount_cents,
      extra_hour_cents = p_extra_hour_cents,
      included_hours = p_included_hours,
      grace_minutes = p_grace_minutes,
      night_cutoff_hour = p_night_cutoff_hour,
      active = p_active,
      room_category = COALESCE(p_room_category, room_category),
      local_id = COALESCE(p_local_id, local_id)
    WHERE uid = p_uid
    RETURNING * INTO rec;
  ELSE
    IF p_expected_version IS DISTINCT FROM 0 THEN
      RAISE EXCEPTION 'conflict: expected_version does not match'
        USING ERRCODE = 'P0001';
    END IF;
    INSERT INTO public.rate_plans (
      uid, local_id, name, kind, base_amount_cents, extra_hour_cents,
      included_hours, grace_minutes, night_cutoff_hour, active, room_category
    )
    VALUES (
      p_uid, p_local_id, p_name, p_kind, p_base_amount_cents, p_extra_hour_cents,
      p_included_hours, p_grace_minutes, p_night_cutoff_hour, p_active,
      COALESCE(p_room_category, 'normal')
    )
    RETURNING * INTO rec;
  END IF;

  RETURN to_jsonb(rec);
END;
$$;

REVOKE EXECUTE ON FUNCTION nightdesk.catalog_upsert_rate_plan(
  uuid, integer, text, text, bigint, bigint, integer, integer, integer, boolean, bigint, text
) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION nightdesk.catalog_write(p_entity text,p_payload jsonb,p_operation_id uuid,p_actor text DEFAULT NULL)
RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path='' AS $$
DECLARE
 old_op nightdesk.catalog_operations%ROWTYPE;
 v_uid uuid; v_expected integer; v_local_id bigint;
 before_row jsonb; result_row jsonb; audit jsonb; response jsonb;
 pair record; fingerprint text; actor text;
BEGIN
 PERFORM nightdesk.require_catalog_writer();
 IF auth.uid() IS NULL OR p_operation_id IS NULL THEN RAISE EXCEPTION 'forbidden: authenticated operation required' USING ERRCODE='42501'; END IF;
 fingerprint := md5(jsonb_build_object('entity',p_entity,'payload',p_payload,'actor',p_actor)::text);
 -- Serializes the catalog allocator/version check, including competing creates.
 PERFORM pg_advisory_xact_lock(731173);
 SELECT * INTO old_op FROM nightdesk.catalog_operations WHERE operation_id=p_operation_id;
 IF FOUND THEN
   IF old_op.caller<>auth.uid() OR old_op.fingerprint<>fingerprint THEN RAISE EXCEPTION 'conflict: operation_id reused'; END IF;
   RETURN old_op.result;
 END IF;
 actor := auth.uid()::text || CASE WHEN public.is_device() THEN '/local:' || COALESCE(p_actor,'unknown') ELSE '' END;
 IF p_entity='settings' THEN
   before_row := '{}'::jsonb; result_row := '[]'::jsonb;
   FOR pair IN SELECT key,value FROM jsonb_each_text(p_payload->'values') ORDER BY key LOOP
     before_row := before_row || jsonb_build_object(pair.key,(SELECT value FROM public.business_settings WHERE key=pair.key));
     result_row := result_row || jsonb_build_array(nightdesk.catalog_upsert_settings(pair.key,pair.value,(p_payload->'versions'->>pair.key)::integer));
   END LOOP;
 ELSE
   v_uid := (p_payload->>'uid')::uuid;
   v_expected := (p_payload->>'expected_version')::integer;
   IF v_uid IS NULL OR v_expected IS NULL OR v_expected<0 THEN RAISE EXCEPTION 'validation: uid and expected_version required'; END IF;
   CASE p_entity
   WHEN 'rooms' THEN
      SELECT to_jsonb(t) INTO before_row FROM public.rooms t WHERE uid=v_uid;
      IF p_payload->>'local_id' IS NULL THEN
        SELECT COALESCE(MAX(local_id),0)+1 INTO v_local_id FROM public.rooms;
      ELSE v_local_id := (p_payload->>'local_id')::bigint; END IF;
      result_row := nightdesk.catalog_upsert_room(p_uid=>v_uid,p_expected_version=>v_expected,
        p_number=>(p_payload->>'number')::text,
        p_room_type=>(p_payload->>'room_type')::text,
        p_floor=>(p_payload->>'floor')::integer,
        p_notes=>(p_payload->>'notes')::text,
        p_active=>COALESCE((p_payload->>'active')::boolean,true),p_local_id=>v_local_id);
   WHEN 'rate_plans' THEN
      SELECT to_jsonb(t) INTO before_row FROM public.rate_plans t WHERE uid=v_uid;
      IF p_payload->>'local_id' IS NULL THEN
        SELECT COALESCE(MAX(local_id),0)+1 INTO v_local_id FROM public.rate_plans;
      ELSE v_local_id := (p_payload->>'local_id')::bigint; END IF;
      result_row := nightdesk.catalog_upsert_rate_plan(p_uid=>v_uid,p_expected_version=>v_expected,
        p_name=>(p_payload->>'name')::text,
        p_kind=>(p_payload->>'kind')::text,
        p_base_amount_cents=>(p_payload->>'base_amount_cents')::bigint,
        p_extra_hour_cents=>(p_payload->>'extra_hour_cents')::bigint,
        p_included_hours=>(p_payload->>'included_hours')::integer,
        p_grace_minutes=>(p_payload->>'grace_minutes')::integer,
        p_night_cutoff_hour=>(p_payload->>'night_cutoff_hour')::integer,
        p_active=>COALESCE((p_payload->>'active')::boolean,true),p_local_id=>v_local_id,
        p_room_category=>(p_payload->>'room_category')::text);
   WHEN 'products' THEN
      SELECT to_jsonb(t) INTO before_row FROM public.products t WHERE uid=v_uid;
      IF p_payload->>'local_id' IS NULL THEN
        SELECT COALESCE(MAX(local_id),0)+1 INTO v_local_id FROM public.products;
      ELSE v_local_id := (p_payload->>'local_id')::bigint; END IF;
      result_row := nightdesk.catalog_upsert_product(p_uid=>v_uid,p_expected_version=>v_expected,
        p_name=>(p_payload->>'name')::text,
        p_category=>(p_payload->>'category')::text,
        p_price_cents=>(p_payload->>'price_cents')::bigint,
        p_active=>COALESCE((p_payload->>'active')::boolean,true),
        p_sort_order=>COALESCE((p_payload->>'sort_order')::integer,0),p_local_id=>v_local_id);
   WHEN 'app_users' THEN
      IF COALESCE(p_payload->>'password_hash','') !~ '^\$argon2id\$' OR length(p_payload->>'password_hash')>512 THEN RAISE EXCEPTION 'validation: Argon2id hash required'; END IF;
      IF length(trim(COALESCE(p_payload->>'username','')))=0 OR length(p_payload->>'username')>64 THEN RAISE EXCEPTION 'validation: invalid username'; END IF;
      SELECT to_jsonb(t) INTO before_row FROM public.app_users t WHERE uid=v_uid;
      IF p_payload->>'local_id' IS NULL THEN
        SELECT COALESCE(MAX(local_id),0)+1 INTO v_local_id FROM public.app_users;
      ELSE v_local_id := (p_payload->>'local_id')::bigint; END IF;
      result_row := nightdesk.catalog_upsert_user(p_uid=>v_uid,p_expected_version=>v_expected,
        p_username=>(p_payload->>'username')::text,
        p_password_hash=>(p_payload->>'password_hash')::text,
        p_role=>(p_payload->>'role')::text,
        p_active=>COALESCE((p_payload->>'active')::boolean,true),p_local_id=>v_local_id);

   ELSE RAISE EXCEPTION 'validation: invalid catalog entity';
   END CASE;
 END IF;
 audit := jsonb_build_object('actor',actor,'created_at',now(),
   'before',CASE WHEN jsonb_typeof(before_row)='object' THEN before_row-'password_hash'-'password'-'pin_hash' ELSE before_row END,
   'after',result_row-'password_hash'-'password'-'pin_hash');
 INSERT INTO public.catalog_audit VALUES(p_operation_id,p_entity,actor,now(),audit->'before',audit->'after');
 response := jsonb_build_object('row',result_row,'audit',audit);
 INSERT INTO nightdesk.catalog_operations VALUES(p_operation_id,auth.uid(),fingerprint,response);
 RETURN response;
END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.sync_bootstrap_catalog(
  device_id uuid,
  rooms jsonb DEFAULT '[]'::jsonb,
  rate_plans jsonb DEFAULT '[]'::jsonb,
  products jsonb DEFAULT '[]'::jsonb,
  settings jsonb DEFAULT '[]'::jsonb,
  app_users jsonb DEFAULT '[]'::jsonb
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = nightdesk, public, pg_temp
AS $$
DECLARE
  item jsonb;
  remote_empty boolean;
BEGIN
  PERFORM nightdesk.require_device(device_id);

  remote_empty :=
    NOT EXISTS (SELECT 1 FROM public.rooms)
    AND NOT EXISTS (SELECT 1 FROM public.rate_plans)
    AND NOT EXISTS (SELECT 1 FROM public.products)
    AND NOT EXISTS (SELECT 1 FROM public.business_settings)
    AND NOT EXISTS (SELECT 1 FROM public.app_users);

  IF NOT remote_empty THEN
    RETURN jsonb_build_object(
      'accepted', false,
      'rooms', COALESCE((SELECT jsonb_agg(to_jsonb(r) ORDER BY r.number) FROM public.rooms r), '[]'::jsonb),
      'rate_plans', COALESCE((SELECT jsonb_agg(to_jsonb(rp) ORDER BY rp.name) FROM public.rate_plans rp), '[]'::jsonb),
      'products', COALESCE((SELECT jsonb_agg(to_jsonb(p) ORDER BY p.sort_order, p.name) FROM public.products p), '[]'::jsonb),
      'settings', COALESCE((SELECT jsonb_agg(to_jsonb(s) ORDER BY s.key) FROM public.business_settings s), '[]'::jsonb),
      'app_users', COALESCE((SELECT jsonb_agg(to_jsonb(u) ORDER BY u.username) FROM public.app_users u), '[]'::jsonb)
    );
  END IF;

  FOR item IN SELECT value FROM jsonb_array_elements(COALESCE(rooms, '[]'::jsonb))
  LOOP
    INSERT INTO public.rooms (
      uid, local_id, number, room_type, floor, status, notes, active, version
    )
    VALUES (
      COALESCE(nightdesk.json_uuid(item, 'uid'), gen_random_uuid()),
      nightdesk.json_bigint(item, 'local_id'),
      item ->> 'number',
      COALESCE(item ->> 'room_type', 'Estándar'),
      COALESCE(nightdesk.json_int(item, 'floor'), 1),
      COALESCE(item ->> 'status', 'available'),
      item ->> 'notes',
      nightdesk.json_bool(item, 'active', true),
      COALESCE(nightdesk.json_int(item, 'version'), 1)
    );
  END LOOP;

  FOR item IN SELECT value FROM jsonb_array_elements(COALESCE(rate_plans, '[]'::jsonb))
  LOOP
    INSERT INTO public.rate_plans (
      uid, local_id, name, kind, base_amount_cents, extra_hour_cents,
      included_hours, grace_minutes, night_cutoff_hour, active, version, room_category
    )
    VALUES (
      COALESCE(nightdesk.json_uuid(item, 'uid'), gen_random_uuid()),
      nightdesk.json_bigint(item, 'local_id'),
      item ->> 'name',
      item ->> 'kind',
      nightdesk.json_bigint(item, 'base_amount_cents'),
      COALESCE(nightdesk.json_bigint(item, 'extra_hour_cents'), 0),
      COALESCE(nightdesk.json_int(item, 'included_hours'), 1),
      COALESCE(nightdesk.json_int(item, 'grace_minutes'), 10),
      COALESCE(nightdesk.json_int(item, 'night_cutoff_hour'), 12),
      nightdesk.json_bool(item, 'active', true),
      COALESCE(nightdesk.json_int(item, 'version'), 1),
      COALESCE(item ->> 'room_category', 'normal')
    );
  END LOOP;

  FOR item IN SELECT value FROM jsonb_array_elements(COALESCE(products, '[]'::jsonb))
  LOOP
    INSERT INTO public.products (
      uid, local_id, name, category, price_cents, active, sort_order, version
    )
    VALUES (
      COALESCE(nightdesk.json_uuid(item, 'uid'), gen_random_uuid()),
      nightdesk.json_bigint(item, 'local_id'),
      item ->> 'name',
      item ->> 'category',
      nightdesk.json_bigint(item, 'price_cents'),
      nightdesk.json_bool(item, 'active', true),
      COALESCE(nightdesk.json_int(item, 'sort_order'), 0),
      COALESCE(nightdesk.json_int(item, 'version'), 1)
    );
  END LOOP;

  FOR item IN SELECT value FROM jsonb_array_elements(COALESCE(settings, '[]'::jsonb))
  LOOP
    INSERT INTO public.business_settings (key, value, version)
    VALUES (
      item ->> 'key',
      COALESCE(item ->> 'value', ''),
      COALESCE(nightdesk.json_int(item, 'version'), 1)
    );
  END LOOP;

  FOR item IN SELECT value FROM jsonb_array_elements(COALESCE(app_users, '[]'::jsonb))
  LOOP
    INSERT INTO public.app_users (
      uid, local_id, username, password_hash, role, active, version
    )
    VALUES (
      COALESCE(nightdesk.json_uuid(item, 'uid'), gen_random_uuid()),
      nightdesk.json_bigint(item, 'local_id'),
      item ->> 'username',
      item ->> 'password_hash',
      item ->> 'role',
      nightdesk.json_bool(item, 'active', true),
      COALESCE(nightdesk.json_int(item, 'version'), 1)
    );
  END LOOP;

  INSERT INTO public.sync_devices (device_id, last_seen_at, last_push_at)
  VALUES (sync_bootstrap_catalog.device_id, now(), now())
  ON CONFLICT ON CONSTRAINT sync_devices_pkey DO UPDATE SET last_seen_at = now();

  RETURN jsonb_build_object('accepted', true);
END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.sync_apply_ops(device_id uuid, ops jsonb)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = nightdesk, public, pg_temp
AS $$
DECLARE
  op jsonb;
  payload jsonb;
  v_op_id uuid;
  v_uid uuid;
  v_entity text;
  v_kind text;
  applied integer := 0;
  skipped integer := 0;
BEGIN
  PERFORM nightdesk.require_device(device_id);

  IF ops IS NULL OR jsonb_typeof(ops) <> 'array' THEN
    RAISE EXCEPTION 'validation: ops must be a JSON array'
      USING ERRCODE = 'P0001';
  END IF;

  FOR op IN SELECT value FROM jsonb_array_elements(ops)
  LOOP
    BEGIN
      v_op_id := (op ->> 'operation_id')::uuid;
    EXCEPTION
      WHEN invalid_text_representation THEN
        RAISE EXCEPTION 'validation: operation_id must be a uuid'
          USING ERRCODE = 'P0001';
    END;

    IF v_op_id IS NULL THEN
      RAISE EXCEPTION 'validation: operation_id required'
        USING ERRCODE = 'P0001';
    END IF;

    IF EXISTS (
      SELECT 1 FROM public.sync_applied_ops a WHERE a.operation_id = v_op_id
    ) THEN
      skipped := skipped + 1;
      CONTINUE;
    END IF;

    payload := COALESCE(op -> 'payload', '{}'::jsonb);
    v_uid := COALESCE(
      nightdesk.json_uuid(payload, 'uid'),
      (op ->> 'entity_uid')::uuid
    );
    v_entity := lower(COALESCE(op ->> 'entity', ''));
    v_kind := lower(COALESCE(op ->> 'op', 'upsert'));

    IF v_entity IN ('guests') THEN v_entity := 'guest'; END IF;
    IF v_entity IN ('reservations') THEN v_entity := 'reservation'; END IF;
    IF v_entity IN ('stays') THEN v_entity := 'stay'; END IF;
    IF v_entity IN ('charges') THEN v_entity := 'charge'; END IF;
    IF v_entity IN ('payments') THEN v_entity := 'payment'; END IF;
    IF v_entity IN ('rooms') THEN v_entity := 'room'; END IF;

    IF v_uid IS NULL THEN
      RAISE EXCEPTION 'validation: entity_uid required'
        USING ERRCODE = 'P0001';
    END IF;

    IF v_entity = 'guest' THEN
      INSERT INTO public.guests (uid, local_id, name, document, phone)
      VALUES (
        v_uid,
        nightdesk.json_bigint(payload, 'local_id'),
        payload ->> 'name',
        payload ->> 'document',
        payload ->> 'phone'
      )
      ON CONFLICT (uid) DO UPDATE SET
        local_id = COALESCE(EXCLUDED.local_id, public.guests.local_id),
        name = EXCLUDED.name,
        document = EXCLUDED.document,
        phone = EXCLUDED.phone;

    ELSIF v_entity = 'reservation' THEN
      INSERT INTO public.reservations (
        uid, local_id, guest_uid, room_uid, rate_plan_uid,
        expected_arrival_at, expected_nights, status, notes
      )
      VALUES (
        v_uid,
        nightdesk.json_bigint(payload, 'local_id'),
        COALESCE(nightdesk.json_uuid(payload, 'guest_uid'), nightdesk.json_uuid(payload, 'guest_id')),
        COALESCE(nightdesk.json_uuid(payload, 'room_uid'), nightdesk.json_uuid(payload, 'room_id')),
        COALESCE(nightdesk.json_uuid(payload, 'rate_plan_uid'), nightdesk.json_uuid(payload, 'rate_plan_id')),
        (payload ->> 'expected_arrival_at')::timestamptz,
        COALESCE(nightdesk.json_int(payload, 'expected_nights'), 1),
        COALESCE(payload ->> 'status', 'hold'),
        payload ->> 'notes'
      )
      ON CONFLICT (uid) DO UPDATE SET
        local_id = COALESCE(EXCLUDED.local_id, public.reservations.local_id),
        guest_uid = EXCLUDED.guest_uid,
        room_uid = EXCLUDED.room_uid,
        rate_plan_uid = EXCLUDED.rate_plan_uid,
        expected_arrival_at = EXCLUDED.expected_arrival_at,
        expected_nights = EXCLUDED.expected_nights,
        status = EXCLUDED.status,
        notes = EXCLUDED.notes;

    ELSIF v_entity = 'stay' THEN
      INSERT INTO public.stays (
        uid, local_id, room_uid, guest_uid, rate_plan_uid, reservation_uid,
        check_in_at, expected_checkout_at, check_out_at, status,
        converted_to_overnight, overnight_rate_plan_uid, notes,
        closed_applied_kind, closed_tax_percent, closed_duration_label,
        closed_total_cents, closed_line_count, checked_in_by, checked_out_by
      )
      VALUES (
        v_uid,
        nightdesk.json_bigint(payload, 'local_id'),
        COALESCE(nightdesk.json_uuid(payload, 'room_uid'), nightdesk.json_uuid(payload, 'room_id')),
        COALESCE(nightdesk.json_uuid(payload, 'guest_uid'), nightdesk.json_uuid(payload, 'guest_id')),
        COALESCE(nightdesk.json_uuid(payload, 'rate_plan_uid'), nightdesk.json_uuid(payload, 'rate_plan_id')),
        COALESCE(nightdesk.json_uuid(payload, 'reservation_uid'), nightdesk.json_uuid(payload, 'reservation_id')),
        (payload ->> 'check_in_at')::timestamptz,
        NULLIF(payload ->> 'expected_checkout_at', '')::timestamptz,
        NULLIF(payload ->> 'check_out_at', '')::timestamptz,
        COALESCE(payload ->> 'status', 'open'),
        nightdesk.json_bool(payload, 'converted_to_overnight', false),
        COALESCE(nightdesk.json_uuid(payload, 'overnight_rate_plan_uid'), nightdesk.json_uuid(payload, 'overnight_rate_plan_id')),
        payload ->> 'notes',
        payload ->> 'closed_applied_kind',
        NULLIF(payload ->> 'closed_tax_percent', '')::numeric,
        payload ->> 'closed_duration_label',
        NULLIF(payload ->> 'closed_total_cents', '')::bigint,
        NULLIF(payload ->> 'closed_line_count', '')::bigint,
        NULLIF(payload ->> 'checked_in_by', ''),
        NULLIF(payload ->> 'checked_out_by', '')
      )
      ON CONFLICT (uid) DO UPDATE SET
        local_id = COALESCE(EXCLUDED.local_id, public.stays.local_id),
        room_uid = EXCLUDED.room_uid,
        guest_uid = EXCLUDED.guest_uid,
        rate_plan_uid = EXCLUDED.rate_plan_uid,
        reservation_uid = EXCLUDED.reservation_uid,
        check_in_at = EXCLUDED.check_in_at,
        expected_checkout_at = EXCLUDED.expected_checkout_at,
        check_out_at = EXCLUDED.check_out_at,
        status = EXCLUDED.status,
        converted_to_overnight = EXCLUDED.converted_to_overnight,
        overnight_rate_plan_uid = EXCLUDED.overnight_rate_plan_uid,
        notes = EXCLUDED.notes,
        closed_applied_kind = EXCLUDED.closed_applied_kind,
        closed_tax_percent = EXCLUDED.closed_tax_percent,
        closed_duration_label = EXCLUDED.closed_duration_label,
        closed_total_cents = EXCLUDED.closed_total_cents,
        closed_line_count = EXCLUDED.closed_line_count,
        checked_in_by = COALESCE(EXCLUDED.checked_in_by, public.stays.checked_in_by),
        checked_out_by = COALESCE(EXCLUDED.checked_out_by, public.stays.checked_out_by);

    ELSIF v_entity = 'charge' THEN
      IF v_kind = 'delete' THEN
        UPDATE public.charges
        SET deleted_at = COALESCE(NULLIF(payload ->> 'deleted_at', '')::timestamptz, now())
        WHERE uid = v_uid;
        IF NOT FOUND THEN
          INSERT INTO public.charges (
            uid, local_id, stay_uid, kind, description, amount_cents, deleted_at
          )
          VALUES (
            v_uid,
            nightdesk.json_bigint(payload, 'local_id'),
            COALESCE(nightdesk.json_uuid(payload, 'stay_uid'), nightdesk.json_uuid(payload, 'stay_id')),
            COALESCE(payload ->> 'kind', 'surcharge'),
            COALESCE(payload ->> 'description', ''),
            COALESCE(nightdesk.json_bigint(payload, 'amount_cents'), 0),
            COALESCE(NULLIF(payload ->> 'deleted_at', '')::timestamptz, now())
          );
        END IF;
      ELSE
        INSERT INTO public.charges (
          uid, local_id, stay_uid, kind, description, amount_cents, deleted_at
        )
        VALUES (
          v_uid,
          nightdesk.json_bigint(payload, 'local_id'),
          COALESCE(nightdesk.json_uuid(payload, 'stay_uid'), nightdesk.json_uuid(payload, 'stay_id')),
          payload ->> 'kind',
          payload ->> 'description',
          nightdesk.json_bigint(payload, 'amount_cents'),
          NULLIF(payload ->> 'deleted_at', '')::timestamptz
        )
        ON CONFLICT (uid) DO UPDATE SET
          local_id = COALESCE(EXCLUDED.local_id, public.charges.local_id),
          stay_uid = EXCLUDED.stay_uid,
          kind = EXCLUDED.kind,
          description = EXCLUDED.description,
          amount_cents = EXCLUDED.amount_cents,
          deleted_at = EXCLUDED.deleted_at;
      END IF;

    ELSIF v_entity = 'payment' THEN
      INSERT INTO public.payments (uid, local_id, stay_uid, method, amount_cents)
      VALUES (
        v_uid,
        nightdesk.json_bigint(payload, 'local_id'),
        COALESCE(nightdesk.json_uuid(payload, 'stay_uid'), nightdesk.json_uuid(payload, 'stay_id')),
        payload ->> 'method',
        nightdesk.json_bigint(payload, 'amount_cents')
      )
      ON CONFLICT (uid) DO UPDATE SET
        local_id = COALESCE(EXCLUDED.local_id, public.payments.local_id),
        stay_uid = EXCLUDED.stay_uid,
        method = EXCLUDED.method,
        amount_cents = EXCLUDED.amount_cents;

    ELSIF v_entity = 'room' THEN
      -- Reception owns status only. Existing catalog fields stay put.
      IF EXISTS (SELECT 1 FROM public.rooms r WHERE r.uid = v_uid) THEN
        UPDATE public.rooms
        SET
          status = COALESCE(payload ->> 'status', status),
          local_id = COALESCE(nightdesk.json_bigint(payload, 'local_id'), local_id)
        WHERE uid = v_uid;
      ELSE
        INSERT INTO public.rooms (
          uid, local_id, number, room_type, floor, status, notes, active
        )
        VALUES (
          v_uid,
          nightdesk.json_bigint(payload, 'local_id'),
          payload ->> 'number',
          COALESCE(payload ->> 'room_type', 'Estándar'),
          COALESCE(nightdesk.json_int(payload, 'floor'), 1),
          COALESCE(payload ->> 'status', 'available'),
          payload ->> 'notes',
          nightdesk.json_bool(payload, 'active', true)
        );
      END IF;

    ELSE
      RAISE EXCEPTION 'validation: unknown entity %', v_entity
        USING ERRCODE = 'P0001';
    END IF;

    INSERT INTO public.sync_applied_ops (operation_id, device_id)
    VALUES (v_op_id, sync_apply_ops.device_id);

    applied := applied + 1;
  END LOOP;

  INSERT INTO public.sync_devices (device_id, last_seen_at, last_push_at, pending_hint)
  VALUES (sync_apply_ops.device_id, now(), now(), 0)
  ON CONFLICT ON CONSTRAINT sync_devices_pkey DO UPDATE SET
    last_seen_at = now(),
    last_push_at = now(),
    pending_hint = 0;

  RETURN jsonb_build_object('applied', applied, 'skipped', skipped);
END;
$$;
