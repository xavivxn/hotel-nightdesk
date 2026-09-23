-- I08.3: nonce on remote manifesto + device SELECT for restore on another PC.
ALTER TABLE public.backups
  ADD COLUMN IF NOT EXISTS nonce_hex text;

DROP POLICY IF EXISTS backups_device_select ON public.backups;
CREATE POLICY backups_device_select ON public.backups
  FOR SELECT TO authenticated
  USING (public.is_device());

-- Device on a new PC is a different Auth user; owner = auth.uid() blocked restore.
DROP POLICY IF EXISTS backups_objects_device_select ON storage.objects;
CREATE POLICY backups_objects_device_select
  ON storage.objects
  FOR SELECT
  TO authenticated
  USING (bucket_id = 'backups' AND public.is_device());
