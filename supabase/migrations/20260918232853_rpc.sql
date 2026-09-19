-- N12 RPC (SECURITY DEFINER in private schema nightdesk) + Realtime.
-- Public wrappers are SECURITY INVOKER so PostgREST can call them without
-- exposing definer functions in an API schema.

CREATE OR REPLACE FUNCTION nightdesk.require_device(p_device_id uuid)
RETURNS void
LANGUAGE plpgsql
STABLE
SET search_path = nightdesk, public, pg_temp
AS $$
BEGIN
  IF NOT public.is_device() THEN
    RAISE EXCEPTION 'forbidden: device role required'
      USING ERRCODE = '42501';
  END IF;
  IF public.jwt_device_id() IS DISTINCT FROM p_device_id THEN
    RAISE EXCEPTION 'forbidden: device_id mismatch'
      USING ERRCODE = '42501';
  END IF;
END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.require_catalog_writer()
RETURNS void
LANGUAGE plpgsql
STABLE
SET search_path = nightdesk, public, pg_temp
AS $$
BEGIN
  IF NOT (public.is_admin() OR public.is_device()) THEN
    RAISE EXCEPTION 'forbidden: admin or device role required'
      USING ERRCODE = '42501';
  END IF;
END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.json_uuid(payload jsonb, field text)
RETURNS uuid
LANGUAGE sql
IMMUTABLE
SET search_path = nightdesk, public, pg_temp
AS $$
  SELECT CASE
    WHEN payload -> field IS NULL OR payload -> field = 'null'::jsonb THEN NULL
    WHEN btrim(payload ->> field) = '' THEN NULL
    ELSE (payload ->> field)::uuid
  END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.json_bigint(payload jsonb, field text)
RETURNS bigint
LANGUAGE sql
IMMUTABLE
SET search_path = nightdesk, public, pg_temp
AS $$
  SELECT CASE
    WHEN payload -> field IS NULL OR payload -> field = 'null'::jsonb THEN NULL
    WHEN btrim(payload ->> field) = '' THEN NULL
    ELSE (payload ->> field)::bigint
  END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.json_int(payload jsonb, field text)
RETURNS integer
LANGUAGE sql
IMMUTABLE
SET search_path = nightdesk, public, pg_temp
AS $$
  SELECT CASE
    WHEN payload -> field IS NULL OR payload -> field = 'null'::jsonb THEN NULL
    WHEN btrim(payload ->> field) = '' THEN NULL
    ELSE (payload ->> field)::integer
  END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.json_bool(payload jsonb, field text, default_value boolean)
RETURNS boolean
LANGUAGE sql
IMMUTABLE
SET search_path = nightdesk, public, pg_temp
AS $$
  SELECT CASE
    WHEN payload -> field IS NULL OR payload -> field = 'null'::jsonb THEN default_value
    ELSE (payload ->> field)::boolean
  END;
$$;

-- ---------------------------------------------------------------------------
-- sync_apply_ops: one TX, skip known operation_id, reception wins
-- Each op: { operation_id, entity, entity_uid, op, payload }
-- ---------------------------------------------------------------------------

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
        closed_applied_kind, closed_tax_percent, closed_duration_label
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
        payload ->> 'closed_duration_label'
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
        closed_duration_label = EXCLUDED.closed_duration_label;

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

-- ---------------------------------------------------------------------------
-- Bootstrap: empty remote accepts the local catalog; otherwise return remote.
-- ---------------------------------------------------------------------------

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
      included_hours, grace_minutes, night_cutoff_hour, active, version
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
      COALESCE(nightdesk.json_int(item, 'version'), 1)
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

-- ---------------------------------------------------------------------------
-- Catalog upserts: admin or device (write-through). expected_version conflict.
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION nightdesk.catalog_upsert_room(
  p_uid uuid,
  p_expected_version integer,
  p_number text,
  p_room_type text DEFAULT 'Estándar',
  p_floor integer DEFAULT 1,
  p_notes text DEFAULT NULL,
  p_active boolean DEFAULT true,
  p_local_id bigint DEFAULT NULL
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = nightdesk, public, pg_temp
AS $$
DECLARE
  rec public.rooms%ROWTYPE;
BEGIN
  PERFORM nightdesk.require_catalog_writer();

  SELECT * INTO rec FROM public.rooms r WHERE r.uid = p_uid;
  IF FOUND THEN
    IF rec.version IS DISTINCT FROM p_expected_version THEN
      RAISE EXCEPTION 'conflict: expected_version does not match'
        USING ERRCODE = 'P0001';
    END IF;
    UPDATE public.rooms
    SET
      number = p_number,
      room_type = p_room_type,
      floor = p_floor,
      notes = p_notes,
      active = p_active,
      local_id = COALESCE(p_local_id, local_id)
    WHERE uid = p_uid
    RETURNING * INTO rec;
  ELSE
    IF p_expected_version IS DISTINCT FROM 0 THEN
      RAISE EXCEPTION 'conflict: expected_version does not match'
        USING ERRCODE = 'P0001';
    END IF;
    INSERT INTO public.rooms (
      uid, local_id, number, room_type, floor, notes, active
    )
    VALUES (
      p_uid, p_local_id, p_number, p_room_type, p_floor, p_notes, p_active
    )
    RETURNING * INTO rec;
  END IF;

  RETURN to_jsonb(rec);
END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.catalog_upsert_rate_plan(
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
  p_local_id bigint DEFAULT NULL
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
      included_hours, grace_minutes, night_cutoff_hour, active
    )
    VALUES (
      p_uid, p_local_id, p_name, p_kind, p_base_amount_cents, p_extra_hour_cents,
      p_included_hours, p_grace_minutes, p_night_cutoff_hour, p_active
    )
    RETURNING * INTO rec;
  END IF;

  RETURN to_jsonb(rec);
END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.catalog_upsert_product(
  p_uid uuid,
  p_expected_version integer,
  p_name text,
  p_category text,
  p_price_cents bigint,
  p_active boolean DEFAULT true,
  p_sort_order integer DEFAULT 0,
  p_local_id bigint DEFAULT NULL
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = nightdesk, public, pg_temp
AS $$
DECLARE
  rec public.products%ROWTYPE;
BEGIN
  PERFORM nightdesk.require_catalog_writer();

  SELECT * INTO rec FROM public.products p WHERE p.uid = p_uid;
  IF FOUND THEN
    IF rec.version IS DISTINCT FROM p_expected_version THEN
      RAISE EXCEPTION 'conflict: expected_version does not match'
        USING ERRCODE = 'P0001';
    END IF;
    UPDATE public.products
    SET
      name = p_name,
      category = p_category,
      price_cents = p_price_cents,
      active = p_active,
      sort_order = p_sort_order,
      local_id = COALESCE(p_local_id, local_id)
    WHERE uid = p_uid
    RETURNING * INTO rec;
  ELSE
    IF p_expected_version IS DISTINCT FROM 0 THEN
      RAISE EXCEPTION 'conflict: expected_version does not match'
        USING ERRCODE = 'P0001';
    END IF;
    INSERT INTO public.products (
      uid, local_id, name, category, price_cents, active, sort_order
    )
    VALUES (
      p_uid, p_local_id, p_name, p_category, p_price_cents, p_active, p_sort_order
    )
    RETURNING * INTO rec;
  END IF;

  RETURN to_jsonb(rec);
END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.catalog_upsert_settings(
  p_key text,
  p_value text,
  p_expected_version integer
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = nightdesk, public, pg_temp
AS $$
DECLARE
  rec public.business_settings%ROWTYPE;
BEGIN
  PERFORM nightdesk.require_catalog_writer();

  SELECT * INTO rec FROM public.business_settings s WHERE s.key = p_key;
  IF FOUND THEN
    IF rec.version IS DISTINCT FROM p_expected_version THEN
      RAISE EXCEPTION 'conflict: expected_version does not match'
        USING ERRCODE = 'P0001';
    END IF;
    UPDATE public.business_settings
    SET value = p_value
    WHERE key = p_key
    RETURNING * INTO rec;
  ELSE
    IF p_expected_version IS DISTINCT FROM 0 THEN
      RAISE EXCEPTION 'conflict: expected_version does not match'
        USING ERRCODE = 'P0001';
    END IF;
    INSERT INTO public.business_settings (key, value)
    VALUES (p_key, p_value)
    RETURNING * INTO rec;
  END IF;

  RETURN to_jsonb(rec);
END;
$$;

CREATE OR REPLACE FUNCTION nightdesk.catalog_upsert_user(
  p_uid uuid,
  p_expected_version integer,
  p_username text,
  p_password_hash text,
  p_role text,
  p_active boolean DEFAULT true,
  p_local_id bigint DEFAULT NULL
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = nightdesk, public, pg_temp
AS $$
DECLARE
  rec public.app_users%ROWTYPE;
BEGIN
  PERFORM nightdesk.require_catalog_writer();

  SELECT * INTO rec FROM public.app_users u WHERE u.uid = p_uid;
  IF FOUND THEN
    IF rec.version IS DISTINCT FROM p_expected_version THEN
      RAISE EXCEPTION 'conflict: expected_version does not match'
        USING ERRCODE = 'P0001';
    END IF;
    UPDATE public.app_users
    SET
      username = p_username,
      password_hash = p_password_hash,
      role = p_role,
      active = p_active,
      local_id = COALESCE(p_local_id, local_id)
    WHERE uid = p_uid
    RETURNING * INTO rec;
  ELSE
    IF p_expected_version IS DISTINCT FROM 0 THEN
      RAISE EXCEPTION 'conflict: expected_version does not match'
        USING ERRCODE = 'P0001';
    END IF;
    INSERT INTO public.app_users (
      uid, local_id, username, password_hash, role, active
    )
    VALUES (
      p_uid, p_local_id, p_username, p_password_hash, p_role, p_active
    )
    RETURNING * INTO rec;
  END IF;

  RETURN to_jsonb(rec);
END;
$$;

-- Public INVOKER wrappers (PostgREST / supabase-js .rpc)
CREATE OR REPLACE FUNCTION public.sync_apply_ops(device_id uuid, ops jsonb)
RETURNS jsonb
LANGUAGE sql
SECURITY INVOKER
SET search_path = nightdesk, public
AS $$
  SELECT nightdesk.sync_apply_ops(device_id, ops);
$$;

CREATE OR REPLACE FUNCTION public.sync_bootstrap_catalog(
  device_id uuid,
  rooms jsonb DEFAULT '[]'::jsonb,
  rate_plans jsonb DEFAULT '[]'::jsonb,
  products jsonb DEFAULT '[]'::jsonb,
  settings jsonb DEFAULT '[]'::jsonb,
  app_users jsonb DEFAULT '[]'::jsonb
)
RETURNS jsonb
LANGUAGE sql
SECURITY INVOKER
SET search_path = nightdesk, public
AS $$
  SELECT nightdesk.sync_bootstrap_catalog(
    device_id, rooms, rate_plans, products, settings, app_users
  );
$$;

CREATE OR REPLACE FUNCTION public.catalog_upsert_room(
  p_uid uuid,
  p_expected_version integer,
  p_number text,
  p_room_type text DEFAULT 'Estándar',
  p_floor integer DEFAULT 1,
  p_notes text DEFAULT NULL,
  p_active boolean DEFAULT true,
  p_local_id bigint DEFAULT NULL
)
RETURNS jsonb
LANGUAGE sql
SECURITY INVOKER
SET search_path = nightdesk, public
AS $$
  SELECT nightdesk.catalog_upsert_room(
    p_uid, p_expected_version, p_number, p_room_type, p_floor, p_notes, p_active, p_local_id
  );
$$;

CREATE OR REPLACE FUNCTION public.catalog_upsert_rate_plan(
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
  p_local_id bigint DEFAULT NULL
)
RETURNS jsonb
LANGUAGE sql
SECURITY INVOKER
SET search_path = nightdesk, public
AS $$
  SELECT nightdesk.catalog_upsert_rate_plan(
    p_uid, p_expected_version, p_name, p_kind, p_base_amount_cents, p_extra_hour_cents,
    p_included_hours, p_grace_minutes, p_night_cutoff_hour, p_active, p_local_id
  );
$$;

CREATE OR REPLACE FUNCTION public.catalog_upsert_product(
  p_uid uuid,
  p_expected_version integer,
  p_name text,
  p_category text,
  p_price_cents bigint,
  p_active boolean DEFAULT true,
  p_sort_order integer DEFAULT 0,
  p_local_id bigint DEFAULT NULL
)
RETURNS jsonb
LANGUAGE sql
SECURITY INVOKER
SET search_path = nightdesk, public
AS $$
  SELECT nightdesk.catalog_upsert_product(
    p_uid, p_expected_version, p_name, p_category, p_price_cents, p_active, p_sort_order, p_local_id
  );
$$;

CREATE OR REPLACE FUNCTION public.catalog_upsert_settings(
  p_key text,
  p_value text,
  p_expected_version integer
)
RETURNS jsonb
LANGUAGE sql
SECURITY INVOKER
SET search_path = nightdesk, public
AS $$
  SELECT nightdesk.catalog_upsert_settings(p_key, p_value, p_expected_version);
$$;

CREATE OR REPLACE FUNCTION public.catalog_upsert_user(
  p_uid uuid,
  p_expected_version integer,
  p_username text,
  p_password_hash text,
  p_role text,
  p_active boolean DEFAULT true,
  p_local_id bigint DEFAULT NULL
)
RETURNS jsonb
LANGUAGE sql
SECURITY INVOKER
SET search_path = nightdesk, public
AS $$
  SELECT nightdesk.catalog_upsert_user(
    p_uid, p_expected_version, p_username, p_password_hash, p_role, p_active, p_local_id
  );
$$;

REVOKE ALL ON ALL FUNCTIONS IN SCHEMA nightdesk FROM PUBLIC, anon;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA public FROM anon;

GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA nightdesk TO authenticated, service_role;
GRANT EXECUTE ON FUNCTION public.sync_apply_ops(uuid, jsonb) TO authenticated;
GRANT EXECUTE ON FUNCTION public.sync_bootstrap_catalog(uuid, jsonb, jsonb, jsonb, jsonb, jsonb) TO authenticated;
GRANT EXECUTE ON FUNCTION public.catalog_upsert_room(uuid, integer, text, text, integer, text, boolean, bigint) TO authenticated;
GRANT EXECUTE ON FUNCTION public.catalog_upsert_rate_plan(uuid, integer, text, text, bigint, bigint, integer, integer, integer, boolean, bigint) TO authenticated;
GRANT EXECUTE ON FUNCTION public.catalog_upsert_product(uuid, integer, text, text, bigint, boolean, integer, bigint) TO authenticated;
GRANT EXECUTE ON FUNCTION public.catalog_upsert_settings(text, text, integer) TO authenticated;
GRANT EXECUTE ON FUNCTION public.catalog_upsert_user(uuid, integer, text, text, text, boolean, bigint) TO authenticated;

ALTER DEFAULT PRIVILEGES IN SCHEMA nightdesk
  GRANT EXECUTE ON FUNCTIONS TO authenticated;

-- Realtime: catalog + operational tables the admin board watches. Payload is a
-- pull trigger, not the source of truth (I07).
ALTER PUBLICATION supabase_realtime ADD TABLE public.rooms;
ALTER PUBLICATION supabase_realtime ADD TABLE public.rate_plans;
ALTER PUBLICATION supabase_realtime ADD TABLE public.products;
ALTER PUBLICATION supabase_realtime ADD TABLE public.business_settings;
ALTER PUBLICATION supabase_realtime ADD TABLE public.app_users;
ALTER PUBLICATION supabase_realtime ADD TABLE public.stays;
ALTER PUBLICATION supabase_realtime ADD TABLE public.charges;
