-- Admin can publish installers and latest.json to the private updates bucket.
-- Device remains SELECT-only. The bucket stays private.

DROP POLICY IF EXISTS updates_objects_admin_insert ON storage.objects;
CREATE POLICY updates_objects_admin_insert
  ON storage.objects
  FOR INSERT
  TO authenticated
  WITH CHECK (bucket_id = 'updates' AND public.is_admin());

DROP POLICY IF EXISTS updates_objects_admin_update ON storage.objects;
CREATE POLICY updates_objects_admin_update
  ON storage.objects
  FOR UPDATE
  TO authenticated
  USING (bucket_id = 'updates' AND public.is_admin())
  WITH CHECK (bucket_id = 'updates' AND public.is_admin());
