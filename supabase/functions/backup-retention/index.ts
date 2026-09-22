/**
 * Placeholder Edge Function for remote backup retention (I08.2 / MOT-62).
 * Deploy: `supabase functions deploy backup-retention`
 * Schedule daily via Dashboard cron or pg_cron with service_role.
 * NEVER delete the single newest verified backup. Policy: 30 daily + 12 monthly.
 */

import { createClient } from "https://esm.sh/@supabase/supabase-js@2.49.1";

const DAILY_KEEP = 30;
const MONTHLY_KEEP = 12;

Deno.serve(async (_req) => {
  const url = Deno.env.get("SUPABASE_URL");
  const key = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
  if (!url || !key) {
    return new Response(JSON.stringify({ error: "missing env" }), { status: 500 });
  }
  const sb = createClient(url, key);
  const { data: rows, error } = await sb
    .from("backups")
    .select("backup_id,backuped_at,storage_path")
    .order("backuped_at", { ascending: false });
  if (error) {
    return new Response(JSON.stringify({ error: error.message }), { status: 500 });
  }
  if (!rows?.length) {
    return new Response(JSON.stringify({ deleted: 0 }), { status: 200 });
  }

  const newest = rows[0];
  const keep = new Set<string>([newest.backup_id]);
  // Keep up to DAILY_KEEP most recent.
  for (const row of rows.slice(0, DAILY_KEEP)) keep.add(row.backup_id);
  // Keep one per calendar month, up to MONTHLY_KEEP months.
  const months = new Set<string>();
  for (const row of rows) {
    const month = String(row.backuped_at).slice(0, 7);
    if (months.size >= MONTHLY_KEEP && !months.has(month)) continue;
    months.add(month);
    keep.add(row.backup_id);
  }

  let deleted = 0;
  for (const row of rows) {
    if (keep.has(row.backup_id)) continue;
    if (row.backup_id === newest.backup_id) continue;
    await sb.storage.from("backups").remove([row.storage_path]);
    await sb.from("backups").delete().eq("backup_id", row.backup_id);
    deleted += 1;
  }
  return new Response(JSON.stringify({ deleted, kept: keep.size }), {
    headers: { "Content-Type": "application/json" },
  });
});
