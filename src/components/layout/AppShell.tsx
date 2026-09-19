import { NavLink, Outlet } from "react-router-dom";
import { BedDouble, CalendarClock, History, LayoutGrid, Moon, Package, Settings2, Sun } from "lucide-react";
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
  { to: "/catalogo", label: "Catálogo admin", icon: Package },
  { to: "/ajustes", label: "Ajustes", icon: Settings2 },
  { to: "/usuarios", label: "Usuarios", icon: Settings2 },
];

function syncLabel(status: SyncStatus | null): string {
  if (!status) return "Sync…";
  if (!status.configured) return "Sin configurar";
  if (status.last_error && !status.connected) return "Error sync";
  if (status.pending_outbox > 0) return `${status.pending_outbox} pend.`;
  if (status.connected) return "Conectado";
  return "Sin conexión";
}

export function AppShell({
  user,
  deviceMode,
  onLogout,
}: {
  user: SessionUser;
  deviceMode: DeviceMode;
  onLogout: () => void;
}) {
  const { theme, toggle } = useTheme();
  const [sync, setSync] = useState<SyncStatus | null>(null);
  const [enter] = useState(() => consumeShellEnter());

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
    <div className={cn("flex h-full min-h-0 overflow-hidden", enter && "app-shell--enter")}>
      <aside className="app-sidebar app-shell-aside flex h-full w-[88px] flex-col border-r border-[var(--line)] bg-[var(--surface)] px-2 py-5 lg:w-56 lg:px-4">
        <div className="sidebar-brand">
          <LoveNestLogo />
        </div>
        <p className="nav-section hidden lg:block">OPERACIÓN</p>
        <nav aria-label="Navegación principal" className="flex flex-1 flex-col gap-1">
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
          {deviceMode === "reception" && (
            <p className="mb-2 truncate text-xs text-[var(--muted)]" title={sync?.last_error ?? undefined}>
              Sync: {syncLabel(sync)}
            </p>
          )}
          {deviceMode === "remote" && (
            <p className="mb-2 truncate text-xs text-[var(--muted)]">Modo remoto</p>
          )}
          <p className="truncate text-xs" title={`${user.username} · ${user.role}`}>
            {user.username} · {user.role === "admin" ? "Admin" : "Recepción"}
          </p>
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
        <main className="app-shell-content min-h-0 min-w-0 flex-1 overflow-auto">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
