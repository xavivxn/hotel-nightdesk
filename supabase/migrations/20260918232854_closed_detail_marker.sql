-- Existing installations need the closure completeness marker before the
-- analytics page can trust a remote total. This migration adds the columns
-- and replaces the sync RPC so new stay payloads carry both marker fields.
ALTER TABLE public.stays ADD COLUMN IF NOT EXISTS closed_total_cents bigint;
ALTER TABLE public.stays ADD COLUMN IF NOT EXISTS closed_line_count bigint;

UPDATE public.stays s
SET closed_total_cents = totals.total_cents,
    closed_line_count = totals.line_count
FROM (
  SELECT c.stay_uid,
         COALESCE(SUM(c.amount_cents), 0)::bigint AS total_cents,
         COUNT(*)::bigint AS line_count
  FROM public.charges c
  WHERE c.deleted_at IS NULL
  GROUP BY c.stay_uid
) totals
WHERE s.uid = totals.stay_uid
  AND s.status = 'closed'
  AND s.closed_applied_kind IS NOT NULL
  AND s.closed_total_cents IS NULL;

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
        closed_total_cents, closed_line_count
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
        NULLIF(payload ->> 'closed_line_count', '')::bigint
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
        closed_line_count = EXCLUDED.closed_line_count;

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
