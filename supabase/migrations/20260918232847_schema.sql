-- N12 schema: Postgres mirror of SQLite reception (uid PK, local_id = IPC id).
-- Catalog: rooms (except status), rate_plans, products, business_settings, app_users.
-- Operation: guests, reservations, stays, charges, payments, rooms.status.
-- Do not sync: receipt_snapshots, login_attempts, device keys.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ---------------------------------------------------------------------------
-- Trigger helpers (INVOKER; not security definer)
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION public.set_updated_at()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = public
AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$;

-- Bump catalog version. rooms.status / local_id are operational and do not bump.
CREATE OR REPLACE FUNCTION public.bump_version()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = public
AS $$
BEGIN
  IF TG_TABLE_NAME = 'rooms' THEN
    IF NEW.number IS NOT DISTINCT FROM OLD.number
       AND NEW.room_type IS NOT DISTINCT FROM OLD.room_type
       AND NEW.floor IS NOT DISTINCT FROM OLD.floor
       AND NEW.notes IS NOT DISTINCT FROM OLD.notes
       AND NEW.active IS NOT DISTINCT FROM OLD.active THEN
      NEW.version := OLD.version;
      RETURN NEW;
    END IF;
  END IF;
  NEW.version := OLD.version + 1;
  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION public.set_updated_at() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.bump_version() FROM PUBLIC;

-- ---------------------------------------------------------------------------
-- Catalog
-- ---------------------------------------------------------------------------

CREATE TABLE public.rooms (
  uid uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  local_id bigint,
  number text NOT NULL UNIQUE,
  room_type text NOT NULL DEFAULT 'Estándar',
  floor integer NOT NULL DEFAULT 1,
  status text NOT NULL DEFAULT 'available'
    CHECK (status IN ('available', 'dirty', 'blocked', 'occupied')),
  notes text,
  active boolean NOT NULL DEFAULT true,
  version integer NOT NULL DEFAULT 1,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE public.rate_plans (
  uid uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  local_id bigint,
  name text NOT NULL,
  kind text NOT NULL CHECK (kind IN ('hourly', 'night', 'overnight')),
  base_amount_cents bigint NOT NULL,
  extra_hour_cents bigint NOT NULL DEFAULT 0,
  included_hours integer NOT NULL DEFAULT 1,
  grace_minutes integer NOT NULL DEFAULT 10,
  night_cutoff_hour integer NOT NULL DEFAULT 12
    CHECK (night_cutoff_hour BETWEEN 0 AND 23),
  active boolean NOT NULL DEFAULT true,
  version integer NOT NULL DEFAULT 1,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE public.products (
  uid uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  local_id bigint,
  name text NOT NULL,
  category text NOT NULL,
  price_cents bigint NOT NULL,
  active boolean NOT NULL DEFAULT true,
  sort_order integer NOT NULL DEFAULT 0,
  version integer NOT NULL DEFAULT 1,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE public.app_users (
  uid uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  local_id bigint,
  username text NOT NULL,
  password_hash text NOT NULL,
  role text NOT NULL CHECK (role IN ('admin', 'recepcion')),
  active boolean NOT NULL DEFAULT true,
  version integer NOT NULL DEFAULT 1,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX app_users_username_lower ON public.app_users (lower(username));

-- Business whitelist only (I12 §4). Local keys (theme, printer_*, pin, sync_*) stay on SQLite.
CREATE TABLE public.business_settings (
  key text PRIMARY KEY
    CHECK (key IN (
      'business_name',
      'address',
      'phone',
      'tax_percent',
      'currency_symbol',
      'receipt_footer',
      'require_guest_name',
      'ticket_header',
      'ticket_header_name'
    )),
  value text NOT NULL,
  version integer NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- Operation
-- ---------------------------------------------------------------------------

CREATE TABLE public.guests (
  uid uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  local_id bigint,
  name text NOT NULL,
  document text,
  phone text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE public.reservations (
  uid uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  local_id bigint,
  guest_uid uuid NOT NULL REFERENCES public.guests (uid),
  room_uid uuid NOT NULL REFERENCES public.rooms (uid),
  rate_plan_uid uuid NOT NULL REFERENCES public.rate_plans (uid),
  expected_arrival_at timestamptz NOT NULL,
  expected_nights integer NOT NULL DEFAULT 1,
  status text NOT NULL DEFAULT 'hold'
    CHECK (status IN ('hold', 'checked_in', 'cancelled', 'no_show')),
  notes text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE public.stays (
  uid uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  local_id bigint,
  room_uid uuid NOT NULL REFERENCES public.rooms (uid),
  guest_uid uuid NOT NULL REFERENCES public.guests (uid),
  rate_plan_uid uuid NOT NULL REFERENCES public.rate_plans (uid),
  reservation_uid uuid REFERENCES public.reservations (uid),
  check_in_at timestamptz NOT NULL,
  expected_checkout_at timestamptz,
  check_out_at timestamptz,
  status text NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
  converted_to_overnight boolean NOT NULL DEFAULT false,
  overnight_rate_plan_uid uuid REFERENCES public.rate_plans (uid),
  notes text,
  closed_applied_kind text,
  closed_tax_percent numeric,
  closed_duration_label text,
  closed_total_cents bigint,
  closed_line_count bigint,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_stays_one_open_per_room
  ON public.stays (room_uid) WHERE status = 'open';

CREATE TABLE public.charges (
  uid uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  local_id bigint,
  stay_uid uuid NOT NULL REFERENCES public.stays (uid),
  kind text NOT NULL
    CHECK (kind IN ('stay', 'extra_hour', 'tax', 'surcharge', 'discount')),
  description text NOT NULL,
  amount_cents bigint NOT NULL,
  deleted_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE public.payments (
  uid uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  local_id bigint,
  stay_uid uuid NOT NULL REFERENCES public.stays (uid),
  method text NOT NULL CHECK (method IN ('cash', 'card', 'transfer')),
  amount_cents bigint NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- Sync + backup manifesto (I07 / I08)
-- ---------------------------------------------------------------------------

CREATE TABLE public.sync_applied_ops (
  operation_id uuid PRIMARY KEY,
  device_id uuid NOT NULL,
  applied_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE public.sync_devices (
  device_id uuid PRIMARY KEY,
  name text,
  last_seen_at timestamptz,
  last_push_at timestamptz,
  pending_hint integer NOT NULL DEFAULT 0
);

CREATE TABLE public.backups (
  uid uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  motel_id text NOT NULL,
  backup_id text NOT NULL UNIQUE,
  backuped_at timestamptz NOT NULL,
  schema_version text NOT NULL,
  app_version text NOT NULL,
  size_bytes bigint NOT NULL,
  checksum text NOT NULL,
  storage_path text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- Triggers
-- ---------------------------------------------------------------------------

CREATE TRIGGER rooms_set_updated_at
  BEFORE UPDATE ON public.rooms
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();
CREATE TRIGGER rooms_bump_version
  BEFORE UPDATE ON public.rooms
  FOR EACH ROW EXECUTE FUNCTION public.bump_version();

CREATE TRIGGER rate_plans_set_updated_at
  BEFORE UPDATE ON public.rate_plans
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();
CREATE TRIGGER rate_plans_bump_version
  BEFORE UPDATE ON public.rate_plans
  FOR EACH ROW EXECUTE FUNCTION public.bump_version();

CREATE TRIGGER products_set_updated_at
  BEFORE UPDATE ON public.products
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();
CREATE TRIGGER products_bump_version
  BEFORE UPDATE ON public.products
  FOR EACH ROW EXECUTE FUNCTION public.bump_version();

CREATE TRIGGER app_users_set_updated_at
  BEFORE UPDATE ON public.app_users
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();
CREATE TRIGGER app_users_bump_version
  BEFORE UPDATE ON public.app_users
  FOR EACH ROW EXECUTE FUNCTION public.bump_version();

CREATE TRIGGER business_settings_set_updated_at
  BEFORE UPDATE ON public.business_settings
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();
CREATE TRIGGER business_settings_bump_version
  BEFORE UPDATE ON public.business_settings
  FOR EACH ROW EXECUTE FUNCTION public.bump_version();

CREATE TRIGGER guests_set_updated_at
  BEFORE UPDATE ON public.guests
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();
CREATE TRIGGER reservations_set_updated_at
  BEFORE UPDATE ON public.reservations
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();
CREATE TRIGGER stays_set_updated_at
  BEFORE UPDATE ON public.stays
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();
CREATE TRIGGER charges_set_updated_at
  BEFORE UPDATE ON public.charges
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();
CREATE TRIGGER payments_set_updated_at
  BEFORE UPDATE ON public.payments
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();
CREATE TRIGGER backups_set_updated_at
  BEFORE UPDATE ON public.backups
  FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();

-- ---------------------------------------------------------------------------
-- Pull indexes (cursor = updated_at)
-- ---------------------------------------------------------------------------

CREATE INDEX idx_rooms_updated_at ON public.rooms (updated_at);
CREATE INDEX idx_rate_plans_updated_at ON public.rate_plans (updated_at);
CREATE INDEX idx_products_updated_at ON public.products (updated_at);
CREATE INDEX idx_app_users_updated_at ON public.app_users (updated_at);
CREATE INDEX idx_business_settings_updated_at ON public.business_settings (updated_at);
CREATE INDEX idx_guests_updated_at ON public.guests (updated_at);
CREATE INDEX idx_reservations_updated_at ON public.reservations (updated_at);
CREATE INDEX idx_stays_updated_at ON public.stays (updated_at);
CREATE INDEX idx_charges_updated_at ON public.charges (updated_at);
CREATE INDEX idx_payments_updated_at ON public.payments (updated_at);

CREATE INDEX idx_reservations_room_uid ON public.reservations (room_uid);
CREATE INDEX idx_reservations_room_status ON public.reservations (room_uid, status);
CREATE INDEX idx_stays_room_uid ON public.stays (room_uid);
CREATE INDEX idx_charges_stay_uid ON public.charges (stay_uid);
CREATE INDEX idx_charges_stay_kind ON public.charges (stay_uid, kind);
CREATE INDEX idx_payments_stay_uid ON public.payments (stay_uid);
CREATE INDEX idx_sync_applied_ops_device ON public.sync_applied_ops (device_id, applied_at);
