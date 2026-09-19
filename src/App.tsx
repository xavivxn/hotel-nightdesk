import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { useEffect, useState, type ReactNode } from "react";
import { AppShell } from "@/components/layout/AppShell";
import { TitleBar } from "@/components/layout/TitleBar";
import { BoardPage } from "@/pages/BoardPage";
import { HistoryPage } from "@/pages/HistoryPage";
import { ReservationsPage } from "@/pages/ReservationsPage";
import { RoomsPage } from "@/pages/RoomsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { CatalogPage } from "@/pages/CatalogPage";
import { LoginPage } from "@/pages/LoginPage";
import { UsersPage } from "@/pages/UsersPage";
import { DeviceModePage } from "@/pages/DeviceModePage";
import { RemoteConfigPage } from "@/pages/RemoteConfigPage";
import { api, refreshDeviceMode } from "@/lib/api";
import { useTheme } from "@/lib/theme";
import type { AppSettings, DeviceMode, SessionInfo } from "@/lib/types";
import { Button } from "@/components/ui/Button";
import { RoleContext } from "@/lib/permissions";

const fallbackSettings: AppSettings = {
  business_name: "MotelApp",
  address: "",
  phone: "",
  tax_percent: 0,
  currency_symbol: "Gs.",
  theme: "light",
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
  const [deviceMode, setDeviceMode] = useState<DeviceMode | null | undefined>(undefined);
  const [remoteReady, setRemoteReady] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  async function initialize() {
    setError(null);
    try {
      let mode = await refreshDeviceMode();
      if (mode === null) {
        const needsSetup = await api.setupRequired();
        if (!needsSetup) {
          await api.deviceModeSet("reception");
          mode = "reception";
        }
      }
      setDeviceMode(mode);
      if (mode === "remote") {
        const configured = await api.remoteConfigured();
        if (configured) {
          const cfg = await api.remoteGetConfig();
          if (cfg) {
            const { initSupabase } = await import("@/lib/supabase");
            initSupabase(cfg.project_url, cfg.anon_key);
          }
        }
        setRemoteReady(configured);
        setSetup(false);
      } else if (mode === "reception") {
        setRemoteReady(null);
        setSetup(await api.setupRequired());
      } else {
        setSetup(null);
        setRemoteReady(null);
      }
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    document.getElementById("boot-splash")?.remove();
    void initialize();
  }, []);
  useEffect(() => {
    setTheme("light");
  }, [session, setTheme]);
  useEffect(() => {
    function expire() {
      api.clearSession();
      setSession(null);
      setLoaded(false);
      setNotice("Tu sesión terminó. Volvé a ingresar.");
    }
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
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    }
    void api
      .getSettings()
      .then((current) => {
        if (cancelled) return;
        setSettings({ ...current, theme: "light" });
        setTheme("light");
        setLoaded(true);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    const timer = window.setInterval(refresh, 15000);
    const expiration = window.setTimeout(() => {
      api.clearSession();
      window.dispatchEvent(new Event("nightdesk-session-expired"));
    }, Math.max(0, session.expires_at * 1000 - Date.now()));
    window.addEventListener("focus", refresh);
    return () => {
      cancelled = true;
      clearInterval(timer);
      clearTimeout(expiration);
      window.removeEventListener("focus", refresh);
    };
  }, [session, setTheme]);

  async function logout() {
    try {
      await api.logout();
    } catch {
      /* session always cleared */
    }
    setSession(null);
    setLoaded(false);
    setError(null);
    setNotice("Sesión cerrada.");
    setTheme("light");
  }

  let body: ReactNode;
  if (deviceMode === undefined || (deviceMode === "reception" && setup === null)) {
    body = (
      <div className="p-8">
        <p role="alert">{error || "Preparando acceso…"}</p>
        {error && <Button onClick={initialize}>Reintentar</Button>}
      </div>
    );
  } else if (deviceMode === null) {
    body = (
      <DeviceModePage
        onChosen={(mode) => {
          setDeviceMode(mode);
          if (mode === "remote") {
            setSetup(false);
            setRemoteReady(false);
          } else {
            void api.setupRequired().then(setSetup);
          }
        }}
      />
    );
  } else if (deviceMode === "remote" && remoteReady === false) {
    body = <RemoteConfigPage onConfigured={() => setRemoteReady(true)} />;
  } else if (!session) {
    body = (
      <LoginPage
        setup={Boolean(setup)}
        remote={deviceMode === "remote"}
        notice={notice}
        onLogin={(s) => {
          setSession(s);
          setSetup(false);
          setError(null);
          setNotice(null);
        }}
      />
    );
  } else if (!loaded) {
    body = (
      <div className="p-8">
        <p role="alert">{error || "Cargando tu espacio…"}</p>
        <Button onClick={logout}>Volver al acceso</Button>
      </div>
    );
  } else {
    const admin = session.user.role === "admin";
    body = (
      <RoleContext.Provider value={session.user.role}>
        <BrowserRouter>
          {error && (
            <div role="alert" className="bg-[var(--danger-soft)] p-3 text-[var(--danger)]">
              No se pudo verificar la conexión: {error}
            </div>
          )}
          <Routes>
            <Route element={<AppShell user={session.user} deviceMode={deviceMode} onLogout={logout} />}>
              <Route path="/" element={<BoardPage settings={settings} deviceMode={deviceMode} />} />
              <Route path="/reservas" element={<ReservationsPage />} />
              <Route path="/historial" element={<HistoryPage settings={settings} />} />
              <Route path="/habitaciones" element={admin ? <RoomsPage settings={settings} /> : <Navigate to="/" replace />} />
              <Route path="/catalogo" element={admin ? <CatalogPage settings={settings} /> : <Navigate to="/" replace />} />
              <Route path="/usuarios" element={admin ? <UsersPage /> : <Navigate to="/" replace />} />
              <Route
                path="/ajustes"
                element={admin ? <SettingsPage settings={settings} deviceMode={deviceMode} onSaved={setSettings} /> : <Navigate to="/" replace />}
              />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Route>
          </Routes>
        </BrowserRouter>
      </RoleContext.Provider>
    );
  }

  return (
    <div className="flex h-screen flex-col overflow-hidden">
      <TitleBar />
      <div className="min-h-0 flex-1 overflow-hidden">{body}</div>
    </div>
  );
}
