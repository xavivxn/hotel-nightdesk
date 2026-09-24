-- Local-only Auth users for `supabase db reset`.
-- Passwords are development placeholders, not production secrets.
-- Hosted users are created in the dashboard / SQL editor; rotate them.

INSERT INTO auth.users (
  instance_id,
  id,
  aud,
  role,
  email,
  encrypted_password,
  email_confirmed_at,
  raw_app_meta_data,
  raw_user_meta_data,
  created_at,
  updated_at,
  confirmation_token,
  email_change,
  email_change_token_new,
  recovery_token
)
VALUES
(
  '00000000-0000-0000-0000-000000000000',
  '11111111-1111-1111-1111-111111111111',
  'authenticated',
  'authenticated',
  'admin@nightdesk.local',
  crypt('123456', gen_salt('bf')),
  now(),
  '{"provider":"email","providers":["email"],"role":"admin"}'::jsonb,
  '{}'::jsonb,
  now(),
  now(),
  '',
  '',
  '',
  ''
),
(
  '00000000-0000-0000-0000-000000000000',
  '22222222-2222-2222-2222-222222222222',
  'authenticated',
  'authenticated',
  'device@nightdesk.local',
  crypt('123456', gen_salt('bf')),
  now(),
  '{"provider":"email","providers":["email"],"role":"device","device_id":"33333333-3333-3333-3333-333333333333"}'::jsonb,
  '{}'::jsonb,
  now(),
  now(),
  '',
  '',
  '',
  ''
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO auth.identities (
  id,
  user_id,
  identity_data,
  provider,
  provider_id,
  last_sign_in_at,
  created_at,
  updated_at
)
VALUES
(
  '11111111-1111-1111-1111-111111111111',
  '11111111-1111-1111-1111-111111111111',
  jsonb_build_object(
    'sub', '11111111-1111-1111-1111-111111111111',
    'email', 'admin@nightdesk.local',
    'email_verified', true
  ),
  'email',
  '11111111-1111-1111-1111-111111111111',
  now(),
  now(),
  now()
),
(
  '22222222-2222-2222-2222-222222222222',
  '22222222-2222-2222-2222-222222222222',
  jsonb_build_object(
    'sub', '22222222-2222-2222-2222-222222222222',
    'email', 'device@nightdesk.local',
    'email_verified', true
  ),
  'email',
  '22222222-2222-2222-2222-222222222222',
  now(),
  now(),
  now()
)
ON CONFLICT (id) DO NOTHING;
