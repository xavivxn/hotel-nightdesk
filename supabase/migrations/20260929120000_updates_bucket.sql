-- In-app updates (tauri-plugin-updater). See docs/actualizaciones.md.
-- Private bucket: the Windows installer embeds the reception device credentials,
-- so it must never be publicly downloadable. The app reads latest.json and the
-- installer with the device (or admin) JWT. Uploads: Supabase dashboard / service_role only.

INSERT INTO storage.buckets (id, name, public, file_size_limit)
VALUES ('updates', 'updates', false, 52428800)
ON CONFLICT (id) DO UPDATE
SET public = excluded.public,
    file_size_limit = excluded.file_size_limit;

DROP POLICY IF EXISTS updates_objects_app_select ON storage.objects;

CREATE POLICY updates_objects_app_select
  ON storage.objects
  FOR SELECT
  TO authenticated
  USING (bucket_id = 'updates' AND (public.is_device() OR public.is_admin()));
