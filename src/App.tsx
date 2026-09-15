import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { useEffect, useState } from "react";
import { AppShell } from "@/components/layout/AppShell";
import { BoardPage } from "@/pages/BoardPage";
import { HistoryPage } from "@/pages/HistoryPage";
import { ReservationsPage } from "@/pages/ReservationsPage";
import { RoomsPage } from "@/pages/RoomsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { CatalogPage } from "@/pages/CatalogPage";
import { LoginPage } from "@/pages/LoginPage";
import { UsersPage } from "@/pages/UsersPage";
import { api } from "@/lib/api";
import { useTheme } from "@/lib/theme";
import type { AppSettings, SessionInfo } from "@/lib/types";
import { Button } from "@/components/ui/Button";
import { RoleContext } from "@/lib/permissions";

const fallbackSettings: AppSettings = {
  business_name: "Nightdesk Inn",
  address: "",
  phone: "",
  tax_percent: 10,
  currency_symbol: "Gs.",
  theme: "dark",
  receipt_footer: "Gracias por su visita",
  printer_enabled: false,
  printer_path: "",
  printer_name: "",
  paper_width: 80,
  auto_print_on_checkout: true,
  require_guest_name: false,
  pin_hash: "",
  has_pin: false,
};


export default function App() {
  const { setTheme } = useTheme();
  const [settings, setSettings] = useState<AppSettings>(fallbackSettings);
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [setup, setSetup] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  async function initialize() {
    setError(null);
    try { setSetup(await api.setupRequired()); }
    catch(e) { setError(String(e)); }
  }
  useEffect(() => { document.getElementById("boot-splash")?.remove(); void initialize(); }, []);
  useEffect(() => {
    function expire() { api.clearSession(); setSession(null); setLoaded(false); setNotice("Tu sesión terminó. Volvé a ingresar."); }
    window.addEventListener("nightdesk-session-expired", expire);
    return () => window.removeEventListener("nightdesk-session-expired", expire);
  }, []);
  useEffect(() => {
    if (!session) return;
    let cancelled = false;
    async function refresh() {
      try {
        await api.session();
        if (!cancelled) setError(null);
      } catch(e) { if (!cancelled) setError(String(e)); }
    }
    void api.getSettings().then(current => {
      if (cancelled) return;
      setSettings(current);
      if (current.theme === "light" || current.theme === "dark") setTheme(current.theme);
      setLoaded(true);
    }).catch(e => { if (!cancelled) setError(String(e)); });
    const timer = window.setInterval(refresh, 15000);
    const expiration = window.setTimeout(() => { api.clearSession(); window.dispatchEvent(new Event("nightdesk-session-expired")); }, Math.max(0, session.expires_at * 1000 - Date.now()));
    window.addEventListener("focus", refresh);
    return () => { cancelled = true; clearInterval(timer); clearTimeout(expiration); window.removeEventListener("focus", refresh); };
  }, [session, setTheme]);

  async function logout() {
    try { await api.logout(); } catch { /* The local session is always cleared. */ }
    setSession(null); setLoaded(false); setError(null); setNotice("Sesión cerrada.");
  }
  if (setup === null) return <div className="p-8"><p role="alert">{error || "Preparando acceso…"}</p>{error && <Button onClick={initialize}>Reintentar</Button>}</div>;
  if (!session) return <LoginPage setup={setup} notice={notice} onLogin={s => { setSession(s); setSetup(false); setError(null); setNotice(null); }} />;
  if (!loaded) return <div className="p-8"><p role="alert">{error || "Cargando tu espacio…"}</p><Button onClick={logout}>Volver al acceso</Button></div>;
  const admin = session.user.role === "admin";
  return <RoleContext.Provider value={session.user.role}><BrowserRouter>
    {error && <div role="alert" className="bg-[var(--danger-soft)] p-3 text-[var(--danger)]">No se pudo verificar la conexión: {error}</div>}
    <Routes><Route element={<AppShell user={session.user} onLogout={logout} />}>
      <Route path="/" element={<BoardPage settings={settings} />} />
      <Route path="/reservas" element={<ReservationsPage />} />
      <Route path="/historial" element={<HistoryPage settings={settings} />} />
      <Route path="/habitaciones" element={admin ? <RoomsPage settings={settings} /> : <Navigate to="/" replace />} />
      <Route path="/catalogo" element={admin ? <CatalogPage settings={settings} /> : <Navigate to="/" replace />} />
      <Route path="/usuarios" element={admin ? <UsersPage /> : <Navigate to="/" replace />} />
      <Route path="/ajustes" element={admin ? <SettingsPage settings={settings} onSaved={setSettings} /> : <Navigate to="/" replace />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Route></Routes>
  </BrowserRouter></RoleContext.Provider>;
}
