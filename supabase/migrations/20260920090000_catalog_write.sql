-- I11: atomic, idempotent catalog mutations and redacted audit.
CREATE TABLE public.catalog_audit (
 operation_id uuid PRIMARY KEY, entity text NOT NULL, actor text NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(), before_json jsonb, after_json jsonb
);
ALTER TABLE public.catalog_audit ENABLE ROW LEVEL SECURITY;
CREATE POLICY catalog_audit_read ON public.catalog_audit FOR SELECT TO authenticated
 USING (public.is_admin() OR public.is_device());
GRANT SELECT ON public.catalog_audit TO authenticated;
CREATE TABLE nightdesk.catalog_operations (
 operation_id uuid PRIMARY KEY, caller uuid NOT NULL, fingerprint text NOT NULL, result jsonb NOT NULL
);
REVOKE ALL ON nightdesk.catalog_operations FROM PUBLIC,anon,authenticated;

CREATE FUNCTION nightdesk.catalog_write(p_entity text,p_payload jsonb,p_operation_id uuid,p_actor text DEFAULT NULL)
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
        p_active=>COALESCE((p_payload->>'active')::boolean,true),p_local_id=>v_local_id);
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
REVOKE ALL ON FUNCTION nightdesk.catalog_write(text,jsonb,uuid,text) FROM PUBLIC,anon;
GRANT EXECUTE ON FUNCTION nightdesk.catalog_write(text,jsonb,uuid,text) TO authenticated;
CREATE FUNCTION public.catalog_write(p_entity text,p_payload jsonb,p_operation_id uuid,p_actor text DEFAULT NULL)
RETURNS jsonb LANGUAGE sql SECURITY INVOKER SET search_path='' AS $$
 SELECT nightdesk.catalog_write(p_entity,p_payload,p_operation_id,p_actor);
$$;
REVOKE ALL ON FUNCTION public.catalog_write(text,jsonb,uuid,text) FROM PUBLIC,anon;
GRANT EXECUTE ON FUNCTION public.catalog_write(text,jsonb,uuid,text) TO authenticated;
-- Clients must use the audited RPC instead of bypassing version/idempotency guards.
REVOKE INSERT,UPDATE,DELETE ON public.rooms,public.rate_plans,public.products,public.app_users,public.business_settings FROM authenticated;
DO $$ DECLARE f record; BEGIN
 FOR f IN SELECT p.oid::regprocedure signature FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
   WHERE n.nspname IN ('nightdesk','public') AND p.proname LIKE 'catalog_upsert_%'
 LOOP EXECUTE format('REVOKE EXECUTE ON FUNCTION %s FROM PUBLIC, anon, authenticated',f.signature); END LOOP;
END $$;
