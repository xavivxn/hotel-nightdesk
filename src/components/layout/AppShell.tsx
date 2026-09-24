import { NavLink, Outlet, useLocation } from "react-router-dom";
import {
  BedDouble,
  ChartNoAxesCombined,
  CalendarClock,
  History,
  LayoutGrid,
  Moon,
  Package,
  Settings2,
  Sun,
  Wifi,
  WifiOff,
} from "lucide-react";
import { LoveNestLogo } from "@/components/layout/BrandLogo";
import { DutyClock } from "@/components/layout/DutyClock";
import { useTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";
import type { DeviceMode, SessionUser, SyncStatus } from "@/lib/types";
import { ADMIN_ROUTES } from "@/lib/permissions";
import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import { consumeShellEnter } from "@/lib/login-transition";
import { useEffect, useState } from "react";

const links = [
  { to: "/", label: "Tablero", icon: LayoutGrid },
  { to: "/reservas", label: "Reservas", icon: CalendarClock },
  { to: "/habitaciones", label: "Habitaciones", icon: BedDouble },
  { to: "/historial", label: "Historial", icon: History },
  { to: "/analisis", label: "Análisis", icon: ChartNoAxesCombined },
  { to: "/catalogo", label: "Catálogo admin", icon: Package },
  { to: "/ajustes", label: "Ajustes", icon: Settings2 },
  { to: "/usuarios", label: "Usuarios", icon: Settings2 },
];

function useOnline() {
  const [online, setOnline] = useState(() =>
    typeof navigator === "undefined" ? true : navigator.onLine,
  );
  useEffect(() => {
    const on = () => setOnline(true);
    const off = () => setOnline(false);
    window.addEventListener("online", on);
    window.addEventListener("offline", off);
    return () => {
      window.removeEventListener("online", on);
      window.removeEventListener("offline", off);
    };
  }, []);
  return online;
}

function sessionNet(sync: SyncStatus | null, browserOnline: boolean) {
  const cloudOnline = sync?.configured ? sync.connected : browserOnline;
  const online = browserOnline && cloudOnline;
  const pending = sync?.pending_outbox ?? 0;
  const detail = sync?.last_error?.trim() || undefined;
  let status: string;
  if (!online) status = "Sin internet";
  else if (sync?.configured && pending > 0) status = `${pending} por sincronizar`;
  else status = "En línea";
  return { online, status, detail };
}

function SessionCard({
  user,
  sync,
  browserOnline,
}: {
  user: SessionUser;
  sync: SyncStatus | null;
  browserOnline: boolean;
}) {
  const { online, status, detail } = sessionNet(sync, browserOnline);
  const initial = (user.username.trim().charAt(0) || "?").toUpperCase();
  const Icon = online ? Wifi : WifiOff;
  const tip =
    detail ??
    (online
      ? `${user.username} · ${status}`
      : `${user.username} · Sin internet — la recepción sigue operando`);

  return (
    <div
      className={cn("sidebar-session", online ? "is-online" : "is-offline")}
      title={tip}
      role="status"
      aria-live="polite"
      aria-label={`${user.username}, ${status}`}
    >
      <div className="sidebar-session-avatar" aria-hidden="true">
        <span className="sidebar-session-initial">{initial}</span>
        <span className="sidebar-session-signal">
          <Icon size={11} strokeWidth={2.5} />
        </span>
      </div>
      <div className="sidebar-session-meta hidden min-w-0 lg:block">
        <p className="sidebar-session-name truncate">{user.username}</p>
        <p className="sidebar-session-status truncate">{status}</p>
      </div>
    </div>
  );
}

export function AppShell({
  user,
  deviceMode,
  reveal,
  onLogout,
}: {
  user: SessionUser;
  deviceMode: DeviceMode;
  /**
   * Login handoff: "pre" = mounted under the login overlay (blurred, invisible, animations
   * paused); "go" = un-blur with stagger while the login logo lands on `sidebar-brand`.
   */
  reveal?: "pre" | "go";
  onLogout: () => void;
}) {
  const { theme, toggle } = useTheme();
  const [sync, setSync] = useState<SyncStatus | null>(null);
  const [enter] = useState(() => consumeShellEnter());
  const online = useOnline();
  const location = useLocation();

  useEffect(() => {
    if (deviceMode !== "reception") return;
    let cancelled = false;
    async function load() {
      try {
        const status = await api.syncStatus();
        if (!cancelled) setSync(status);
      } catch {
        if (!cancelled) setSync(null);
      }
    }
    void load();
    const id = window.setInterval(load, 30_000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [deviceMode]);

  return (
    <div
      className={cn(
        "flex h-full min-h-0 overflow-hidden",
        reveal === "pre" && "app-shell--pre-reveal",
        reveal === "go" && "app-shell--reveal",
        !reveal && enter && "app-shell--enter",
      )}
    >
      <aside className="app-sidebar app-shell-aside flex h-full min-h-0 w-[88px] shrink-0 flex-col overflow-hidden border-r border-[var(--line)] bg-[var(--surface)] px-2 py-5 lg:w-56 lg:px-4">
        <div className="sidebar-brand" data-brand-target>
          <LoveNestLogo />
        </div>
        <p className="nav-section hidden lg:block">OPERACIÓN</p>
        <nav aria-label="Navegación principal" className="min-h-0 flex-1 overflow-y-auto overscroll-contain scrollbar-thin">
          {links
            .filter((link) => user.role === "admin" || !ADMIN_ROUTES.has(link.to))
            .map((link) => (
              <NavLink
                key={link.to}
                to={link.to}
                title={link.label}
                aria-label={link.label}
                end={link.to === "/"}
                className={({ isActive }) =>
                  cn(
                    "sidebar-link flex items-center gap-3 rounded-lg px-3 py-3 text-sm font-medium text-[var(--muted)] hover:bg-[var(--surface-2)] hover:text-[var(--ink)]",
                    isActive && "bg-[var(--accent-soft)] text-[var(--accent)]",
                  )
                }
              >
                <link.icon size={18} />
                <span className="hidden lg:inline">{link.label}</span>
              </NavLink>
            ))}
        </nav>
        <div className="sidebar-foot">
          <SessionCard
            user={user}
            sync={deviceMode === "reception" ? sync : null}
            browserOnline={online}
          />
          <Button variant="secondary" className="w-full" onClick={onLogout}>
            Salir
          </Button>
          <div className="sidebar-dock">
            <DutyClock />
            <button
              onClick={toggle}
              className="theme-toggle flex w-full items-center justify-center gap-2 rounded-lg border border-[var(--line)] px-3 py-2 text-sm hover:bg-[var(--surface-2)] lg:justify-start"
            >
              {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
              <span className="hidden lg:inline">{theme === "dark" ? "Modo claro" : "Modo oscuro"}</span>
            </button>
          </div>
        </div>
      </aside>
      <div className="app-shell-main flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        <main key={location.pathname} className="app-shell-content app-route-enter min-h-0 min-w-0 flex-1 overflow-auto">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
