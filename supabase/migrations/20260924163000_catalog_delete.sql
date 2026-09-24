-- Hard-delete catalog users and leave a tombstone so reception pull removes the local row.
CREATE TABLE public.catalog_deletes (
  entity text NOT NULL,
  uid uuid NOT NULL,
  deleted_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (entity, uid)
);
ALTER TABLE public.catalog_deletes ENABLE ROW LEVEL SECURITY;
CREATE POLICY catalog_deletes_select ON public.catalog_deletes FOR SELECT TO authenticated
  USING (public.is_admin() OR public.is_device());
GRANT SELECT ON public.catalog_deletes TO authenticated;
REVOKE INSERT, UPDATE, DELETE ON public.catalog_deletes FROM PUBLIC, anon, authenticated;

ALTER PUBLICATION supabase_realtime ADD TABLE public.catalog_deletes;

CREATE FUNCTION nightdesk.catalog_delete(
  p_entity text,
  p_uid uuid,
  p_expected_version integer,
  p_operation_id uuid,
  p_actor text DEFAULT NULL
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  old_op nightdesk.catalog_operations%ROWTYPE;
  before_row jsonb;
  rec public.app_users%ROWTYPE;
  fingerprint text;
  actor text;
  response jsonb;
  others integer;
BEGIN
  PERFORM nightdesk.require_catalog_writer();
  IF auth.uid() IS NULL OR p_operation_id IS NULL THEN
    RAISE EXCEPTION 'forbidden: authenticated operation required' USING ERRCODE = '42501';
  END IF;
  IF p_entity IS DISTINCT FROM 'app_users' THEN
    RAISE EXCEPTION 'validation: invalid catalog entity';
  END IF;
  IF p_uid IS NULL OR p_expected_version IS NULL OR p_expected_version < 0 THEN
    RAISE EXCEPTION 'validation: uid and expected_version required';
  END IF;

  fingerprint := md5(jsonb_build_object('entity', p_entity, 'uid', p_uid, 'expected_version', p_expected_version, 'actor', p_actor)::text);
  PERFORM pg_advisory_xact_lock(731173);
  SELECT * INTO old_op FROM nightdesk.catalog_operations WHERE operation_id = p_operation_id;
  IF FOUND THEN
    IF old_op.caller <> auth.uid() OR old_op.fingerprint <> fingerprint THEN
      RAISE EXCEPTION 'conflict: operation_id reused';
    END IF;
    RETURN old_op.result;
  END IF;

  actor := auth.uid()::text || CASE WHEN public.is_device() THEN '/local:' || COALESCE(p_actor, 'unknown') ELSE '' END;
  SELECT * INTO rec FROM public.app_users WHERE uid = p_uid;
  IF FOUND THEN
    IF rec.version IS DISTINCT FROM p_expected_version AND p_expected_version IS DISTINCT FROM 0 THEN
      RAISE EXCEPTION 'conflict: expected_version does not match' USING ERRCODE = 'P0001';
    END IF;
    IF rec.role = 'admin' THEN
      SELECT COUNT(*) INTO others FROM public.app_users WHERE role = 'admin' AND active AND uid IS DISTINCT FROM p_uid;
      IF others = 0 THEN
        RAISE EXCEPTION 'forbidden: last admin' USING ERRCODE = '42501';
      END IF;
    END IF;
    before_row := to_jsonb(rec) - 'password_hash' - 'password' - 'pin_hash';
    DELETE FROM public.app_users WHERE uid = p_uid;
  ELSE
    before_row := NULL;
  END IF;

  INSERT INTO public.catalog_deletes (entity, uid, deleted_at)
  VALUES (p_entity, p_uid, now())
  ON CONFLICT (entity, uid) DO UPDATE SET deleted_at = excluded.deleted_at;

  response := jsonb_build_object(
    'row', jsonb_build_object('uid', p_uid, 'entity', p_entity, 'deleted', true),
    'audit', jsonb_build_object('actor', actor, 'created_at', now(), 'before', before_row, 'after', NULL)
  );
  INSERT INTO public.catalog_audit VALUES (p_operation_id, p_entity, actor, now(), before_row, NULL);
  INSERT INTO nightdesk.catalog_operations VALUES (p_operation_id, auth.uid(), fingerprint, response);
  RETURN response;
END;
$$;

REVOKE ALL ON FUNCTION nightdesk.catalog_delete(text, uuid, integer, uuid, text) FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION nightdesk.catalog_delete(text, uuid, integer, uuid, text) TO authenticated;

CREATE FUNCTION public.catalog_delete(
  p_entity text,
  p_uid uuid,
  p_expected_version integer,
  p_operation_id uuid,
  p_actor text DEFAULT NULL
)
RETURNS jsonb
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT nightdesk.catalog_delete(p_entity, p_uid, p_expected_version, p_operation_id, p_actor);
$$;

REVOKE ALL ON FUNCTION public.catalog_delete(text, uuid, integer, uuid, text) FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.catalog_delete(text, uuid, integer, uuid, text) TO authenticated;
