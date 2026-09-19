# N07 Remote Admin Supabase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (inline in this session). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship N07 (MOT-54/55/56 + remote catalog): device mode, `supabaseInvoke`, Auth-only-remote, catalog writes, sync UI stubs until I07.

**Architecture:** Same Tauri binary; `cmd()` routes `invoke` | `supabaseInvoke` | `mockInvoke` by mode, with always-local allowlist (`device_mode_*`, `remote_configure`, `hash_password`). Reception stays SQLite offline; remote uses Supabase Auth + PostgREST/RPC. Sync commands are stubs.

**Tech Stack:** Tauri 2, React, `@supabase/supabase-js`, SQLite settings key `device_mode`, Windows keyring when available (stub OK), existing N12 RPCs.

**Spec:** `docs/superpowers/specs/2026-09-19-n07-remote-admin-supabase-design.md`  
**Skills:** `@nightdesk-conventions` `@nightdesk-add-command` `@nightdesk-design`

---

## File map

| File | Responsibility |
|---|---|
| `src-tauri/src/models.rs` | `DeviceMode`, payloads for mode/configure/hash/sync stubs |
| `src-tauri/src/service.rs` | `device_mode_get/set`, `hash_password`, sync stub status |
| `src-tauri/src/commands.rs` + `lib.rs` | IPC adapters + register |
| `src-tauri/src/credentials.rs` (new) | Keyring store for remote URL/anon and sync device (stub file-based fallback if no keyring crate) |
| `src/lib/types.ts` | Mirror types |
| `src/lib/api.ts` | `cmd()` routing + allowlist + api methods |
| `src/lib/supabase.ts` (new) | Client, `supabaseInvoke`, Realtime wrappers |
| `src/lib/mock.ts` | Mode + remote cases + sync stubs |
| `src/pages/DeviceModePage.tsx` (new) | First-boot role UI (no «Nightdesk») |
| `src/pages/RemoteConfigPage.tsx` (new) | URL + anon for remote |
| `src/pages/LoginPage.tsx` | Branch local vs Auth |
| `src/App.tsx` | Boot: mode → config → login |
| `src/components/layout/AppShell.tsx` | Sync indicator (reception) |
| `src/pages/SettingsPage.tsx` | Sección Sincronización |
| `package.json` | `@supabase/supabase-js` |
| `docs/contrato-ipc-api.md` | `null` mode, `remote_configure`, stubs |

---

### Task 1: device_mode IPC + first-boot page

**Files:** models, service, commands, lib.rs, types, api, mock, DeviceModePage, App.tsx, contrato

- [ ] **Step 1:** Add `device_mode_get` / `device_mode_set` storing in SQLite `settings` key `device_mode` (`reception`|`remote`). Get returns `Option<String>` / null when unset. Public (no session).
- [ ] **Step 2:** Wire api + mock; `DeviceModePage` (Spanish UI, Night Ops, no brand name); App gates on null mode before setup/login.
- [ ] **Step 3:** `cargo test` + `npm run build`; commit.

### Task 2: cmd routing allowlist + supabase package + skeleton supabaseInvoke

**Files:** api.ts, package.json, supabase.ts, mock.ts

- [ ] **Step 1:** Install `@supabase/supabase-js`.
- [ ] **Step 2:** `cmd()`: load mode (cache); if remote and name not in `LOCAL_ALWAYS`, call `supabaseInvoke`; else invoke/mock.
- [ ] **Step 3:** `supabaseInvoke` switch: default `forbidden` for unknown; implement `contract_info` + a few list reads as stubs returning empty/error until Task 4; commit.

### Task 3: remote_configure + hash_password

**Files:** credentials.rs, commands, service (hash reuse from auth), api, mock, RemoteConfigPage, App.tsx

- [ ] **Step 1:** `remote_configure { project_url, anon_key }` → keyring or app-data file outside git (never settings UI).
- [ ] **Step 2:** `hash_password { password }` → `{ hash }` Argon2id same as auth.
- [ ] **Step 3:** Remote boot shows RemoteConfigPage if no credentials; commit.

### Task 4: Remote Auth + supabaseInvoke reads

**Files:** supabase.ts, LoginPage, api, mock

- [ ] **Step 1:** `auth_login`/`auth_session`/`auth_logout` via Supabase Auth in remote; map to `SessionInfo` shape (synthetic local ids OK if needed for UI).
- [ ] **Step 2:** Implement reads: list_board, list_rooms, list_rate_plans, list_products, list_reservations, list_history, get_stay_detail, get_settings, preview_bill (billing.ts estimativo).
- [ ] **Step 3:** Operational mutations + print → `forbidden`; commit.

### Task 5: Realtime wrappers + live board

**Files:** supabase.ts, api.ts, BoardPage (and history/reservations as needed)

- [ ] **Step 1:** `subscribeOperational(onEvent)` / `unsubscribeOperational` in supabase.ts; expose via api.
- [ ] **Step 2:** Board (and related) refresh on event in remote mode; connection indicator; commit.

### Task 6: Remote catalog writes

**Files:** supabase.ts (catalog_upsert_* RPC), RoomsPage/Catalog/Settings/Users already call api

- [ ] **Step 1:** Map save_room (no status), save_rate_plan, save_product, set_product_active, save_settings (business keys), auth_create_user (+ hash_password) to RPCs with expected_version.
- [ ] **Step 2:** Surface `conflict` as ApiError; commit.

### Task 7: Sync stubs + AppShell + Settings

**Files:** models/service/commands, api, mock, AppShell, SettingsPage

- [ ] **Step 1:** Stub `sync_status`, `sync_pull_now`, `sync_configure_device`.
- [ ] **Step 2:** Indicator + Ajustes sección; listen `sync:catalog-updated` (Tauri event or window CustomEvent in mock).
- [ ] **Step 3:** Update contrato-ipc-api.md; verify build/tests; Jira comment on MOT-54/55/56 if connected; commit.

---

## Verification checklist

- Reception path unchanged without choosing remote.
- Browser mock can flip remote mode and get `forbidden` on check_in.
- No secrets in repo.
- `cargo test` + `npm run build`.
