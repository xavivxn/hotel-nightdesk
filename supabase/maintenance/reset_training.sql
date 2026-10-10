-- One-off data reset, NOT a schema migration or an RPC exposed to the app.
-- Run as postgres in the maintenance window described in docs/reinicio-capacitacion.md.
-- All old clients must be closed; install the reset build OFFLINE before reconnecting.
-- In the same session, explicitly set:
-- SET nightdesk.confirm_training_reset = 'post-training-2026-10';
-- Auth users, roles, grants, functions, policies and Storage buckets are preserved.
-- Storage objects require separate removal via Storage API/dashboard, never SQL.
DO $reset$
BEGIN
  IF current_setting('nightdesk.confirm_training_reset', true)
     IS DISTINCT FROM 'post-training-2026-10' THEN
    RAISE EXCEPTION 'Falta confirmar el reinicio de capacitación; revisar docs/reinicio-capacitacion.md';
  END IF;

  -- No CASCADE: an unexpected dependency must abort the entire reset.
  -- app_users is a replica of the preserved LOCAL users: bootstrap restores it.
  -- It must be empty too, otherwise bootstrap will not accept the new catalog.
  TRUNCATE TABLE
    public.charges,
    public.payments,
    public.stays,
    public.reservations,
    public.guests,
    public.rooms,
    public.rate_plans,
    public.products,
    public.business_settings,
    public.app_users,
    public.catalog_audit,
    public.catalog_deletes,
    public.operational_audit,
    public.sync_applied_ops,
    public.sync_devices,
    public.backups,
    nightdesk.catalog_operations
  RESTART IDENTITY;

  PERFORM set_config('nightdesk.confirm_training_reset', '', false);
END;
$reset$;
