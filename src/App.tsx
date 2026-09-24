import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { useEffect, useState, type ReactNode } from "react";
import { AppShell } from "@/components/layout/AppShell";
import { TitleBar } from "@/components/layout/TitleBar";
import { BoardPage } from "@/pages/BoardPage";
import { HistoryPage } from "@/pages/HistoryPage";
import { AnalyticsPage } from "@/pages/AnalyticsPage";
import { ReservationsPage } from "@/pages/ReservationsPage";
import { RoomsPage } from "@/pages/RoomsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { CatalogPage } from "@/pages/CatalogPage";
import { LoginPage } from "@/pages/LoginPage";
import { UsersPage } from "@/pages/UsersPage";
import { DeviceModePage } from "@/pages/DeviceModePage";
import { RemoteConfigPage } from "@/pages/RemoteConfigPage";
import { LoveNestLogo } from "@/components/layout/BrandLogo";
import { api, refreshDeviceMode } from "@/lib/api";
import { LOGIN_MOTION, type HandoffPhase } from "@/lib/login-transition";
import { useTheme } from "@/lib/theme";
import type { AppSettings, DeviceMode, SessionInfo } from "@/lib/types";
import { Button } from "@/components/ui/Button";
import { RoleContext } from "@/lib/permissions";

type Handoff = "none" | HandoffPhase;

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
  // Login → shell choreography. "hold": login overlay plays E2/E3 while settings load;
  // "reveal": logo flies to the sidebar and the shell un-blurs; "none": shell only.
  const [handoff, setHandoff] = useState<Handoff>("none");
  const [sequenceDone, setSequenceDone] = useState(false);
  const [handoffSlow, setHandoffSlow] = useState(false);

  function resetHandoff() {
    setHandoff("none");
    setSequenceDone(false);
    setHandoffSlow(false);
  }

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
    if (deviceMode !== "reception") return;
    function refreshSetup() {
      void api.setupRequired().then(setSetup).catch(() => undefined);
    }
    window.addEventListener("sync:catalog-updated", refreshSetup);
    return () => window.removeEventListener("sync:catalog-updated", refreshSetup);
  }, [deviceMode]);
  useEffect(() => {
    setTheme("light");
  }, [session, setTheme]);
  useEffect(() => {
    function expire() {
      api.clearSession();
      setSession(null);
      setLoaded(false);
      resetHandoff();
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

  // Reveal starts once settings are loaded and the login sequence marked done.
  useEffect(() => {
    if (handoff === "hold" && loaded && sequenceDone) setHandoff("reveal");
  }, [handoff, loaded, sequenceDone]);
  // If settings fail while holding, drop the overlay so the error + "Volver al acceso" show.
  useEffect(() => {
    if (handoff !== "none" && error && !loaded) resetHandoff();
  }, [handoff, error, loaded]);
  useEffect(() => {
    if (handoff !== "hold" || loaded) return;
    const id = window.setTimeout(() => setHandoffSlow(true), LOGIN_MOTION.slowLoadMs);
    return () => window.clearTimeout(id);
  }, [handoff, loaded]);

  async function logout() {
    try {
      await api.logout();
    } catch {
      /* session always cleared */
    }
    setSession(null);
    setLoaded(false);
    resetHandoff();
    setError(null);
    setNotice("La sesión anterior se cerró correctamente.");
    setTheme("light");
  }

  // `body` is the shell (or a loading block); `login` stays mounted on top during the handoff
  // so the logo can fly from the login stage to the sidebar without remounting.
  let body: ReactNode = null;
  let login: ReactNode = null;
  const inHandoff = session !== null && handoff !== "none";
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
  } else if (!session || inHandoff) {
    login = (
      <LoginPage
        setup={Boolean(setup)}
        remote={deviceMode === "remote"}
        notice={notice}
        handoff={
          inHandoff
            ? { phase: handoff as HandoffPhase, slow: handoffSlow, onDone: resetHandoff }
            : undefined
        }
        onLogin={(s, animated) => {
          setSession(s);
          setSetup(false);
          setError(null);
          setNotice(null);
          setSequenceDone(false);
          setHandoffSlow(false);
          setHandoff(animated ? "hold" : "none");
        }}
        onSequenceDone={() => setSequenceDone(true)}
      />
    );
  }
  if (session && !loaded && !inHandoff) {
    body = (
      <div className="login-stage login-stage--handoff h-full min-h-0 p-6">
        <div className="login-stack">
          <div className="login-logo-slot">
            <div className="login-logo-glow login-logo-glow--hold" />
            <LoveNestLogo className="login-logo" />
          </div>
          <p className="mt-4 text-sm text-[var(--muted)]" role="status">
            {error || "Cargando tu espacio…"}
          </p>
          {error ? (
            <Button className="mt-4" onClick={logout}>
              Volver al acceso
            </Button>
          ) : null}
        </div>
      </div>
    );
  } else if (session && loaded && deviceMode) {
    const admin = session.user.role === "admin";
    const reveal = handoff === "hold" ? "pre" : handoff === "reveal" ? "go" : undefined;
    body = (
      <RoleContext.Provider value={session.user.role}>
        <BrowserRouter>
          {error && (
            <div role="alert" className="bg-[var(--danger-soft)] p-3 text-[var(--danger)]">
              No se pudo verificar la conexión: {error}
            </div>
          )}
          <Routes>
            <Route
              element={
                <AppShell user={session.user} deviceMode={deviceMode} reveal={reveal} onLogout={logout} />
              }
            >
              <Route path="/" element={<BoardPage settings={settings} deviceMode={deviceMode} />} />
              <Route path="/reservas" element={<ReservationsPage />} />
              <Route path="/historial" element={<HistoryPage settings={settings} />} />
              <Route path="/analisis" element={admin ? <AnalyticsPage deviceMode={deviceMode} /> : <Navigate to="/" replace />} />
              <Route path="/habitaciones" element={admin ? <RoomsPage settings={settings} /> : <Navigate to="/" replace />} />
              <Route path="/catalogo" element={admin ? <CatalogPage settings={settings} /> : <Navigate to="/" replace />} />
              <Route path="/usuarios" element={admin ? <UsersPage user={session.user} /> : <Navigate to="/" replace />} />
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
      <div className="relative min-h-0 flex-1 overflow-hidden">
        {body}
        {login}
      </div>
    </div>
  );
}
