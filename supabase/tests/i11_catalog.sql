-- Run against a disposable Supabase/Postgres test database after all migrations.
-- Rolls back all fixtures. Never use a production session.
BEGIN;
SET LOCAL ROLE authenticated;
SELECT set_config('request.jwt.claims','{"sub":"11111111-1111-4111-8111-111111111111","app_metadata":{"role":"admin"}}',true);
DO $$
DECLARE
 op uuid:=gen_random_uuid(); product_uid uuid:=gen_random_uuid(); req jsonb; first_result jsonb; replay jsonb;
BEGIN
 req:=jsonb_build_object('uid',product_uid,'expected_version',0,'name','I11 disposable product','category','Test','price_cents',15000,'active',true,'sort_order',0);
 first_result:=public.catalog_write('products',req,op,NULL);
 replay:=public.catalog_write('products',req,op,NULL);
 IF first_result<>replay THEN RAISE EXCEPTION 'replay changed result'; END IF;
 IF (SELECT count(*) FROM public.catalog_audit WHERE operation_id=op)<>1 THEN RAISE EXCEPTION 'duplicate audit'; END IF;
 BEGIN
  PERFORM public.catalog_write('products',req||'{"price_cents":20000}'::jsonb,op,NULL);
  RAISE EXCEPTION 'accepted changed operation';
 EXCEPTION WHEN SQLSTATE 'P0001' THEN
  IF SQLERRM NOT LIKE 'conflict:%' THEN RAISE; END IF;
 END;
 BEGIN
  PERFORM public.catalog_write('products',req,gen_random_uuid(),NULL);
  RAISE EXCEPTION 'accepted stale version';
 EXCEPTION WHEN SQLSTATE 'P0001' THEN
  IF SQLERRM NOT LIKE 'conflict:%' THEN RAISE; END IF;
 END;
 BEGIN
  UPDATE public.products SET price_cents=1 WHERE products.uid=product_uid;
  RAISE EXCEPTION 'direct write bypassed RPC';
 EXCEPTION WHEN insufficient_privilege THEN NULL;
 END;
 -- Other catalog entities, nullable before state and redacted account credentials.
 PERFORM public.catalog_write('rooms',jsonb_build_object('uid',gen_random_uuid(),'expected_version',0,'number','I11-test','room_type','Normal','floor',1,'active',true),gen_random_uuid(),NULL);
 PERFORM public.catalog_write('rate_plans',jsonb_build_object('uid',gen_random_uuid(),'expected_version',0,'name','I11-rate','kind','hourly','base_amount_cents',80000,'extra_hour_cents',15000,'included_hours',1,'grace_minutes',5,'night_cutoff_hour',10,'active',true),gen_random_uuid(),NULL);
 op:=gen_random_uuid();
 PERFORM public.catalog_write('app_users',jsonb_build_object('uid',gen_random_uuid(),'expected_version',0,'username','i11-test','role','recepcion','password_hash','$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0c2FsdA$aGFzaGhhc2hoYXNoaGFzaGhhc2hoYXNoaGFzaGhhc2g','active',true),op,NULL);
 IF (SELECT after_json ? 'password_hash' FROM public.catalog_audit WHERE operation_id=op) THEN RAISE EXCEPTION 'hash leaked into audit'; END IF;
 PERFORM public.catalog_write('settings','{"values":{"business_name":"I11","phone":"123"},"versions":{"business_name":0,"phone":0}}',gen_random_uuid(),NULL);
 BEGIN
  PERFORM public.catalog_write('settings','{"values":{"business_name":"Wrong","phone":"Wrong"},"versions":{"business_name":1,"phone":0}}',gen_random_uuid(),NULL);
  RAISE EXCEPTION 'accepted partial settings conflict';
 EXCEPTION WHEN SQLSTATE 'P0001' THEN
  IF SQLERRM NOT LIKE 'conflict:%' THEN RAISE; END IF;
 END;
 IF (SELECT value FROM public.business_settings WHERE key='business_name')<>'I11' THEN RAISE EXCEPTION 'settings partly committed'; END IF;
 PERFORM set_config('request.jwt.claims','{"sub":"33333333-3333-4333-8333-333333333333","app_metadata":{"role":"device","device_id":"44444444-4444-4444-8444-444444444444"}}',true);
 PERFORM public.catalog_write('products',req||jsonb_build_object('expected_version',1,'price_cents',17000),gen_random_uuid(),'local-admin-id');
 PERFORM set_config('request.jwt.claims','{"sub":"22222222-2222-4222-8222-222222222222","app_metadata":{"role":"recepcion"}}',true);
 BEGIN
  PERFORM public.catalog_write('products',req,gen_random_uuid(),NULL);
  RAISE EXCEPTION 'reception wrote catalog';
 EXCEPTION WHEN insufficient_privilege THEN NULL;
 END;
END $$;
SELECT 'I11_CATALOG_TESTS_OK';
ROLLBACK;
