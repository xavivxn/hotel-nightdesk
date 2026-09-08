import { NavLink, Outlet } from "react-router-dom";
import { BedDouble, CalendarClock, History, LayoutGrid, Moon, Settings2, Sun } from "lucide-react";
import { DutyClock } from "@/components/layout/DutyClock";
import { TitleBar } from "@/components/layout/TitleBar";
import { useTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";

const links = [
  { to: "/", label: "Tablero", icon: LayoutGrid },
  { to: "/reservas", label: "Reservas", icon: CalendarClock },
  { to: "/habitaciones", label: "Habitaciones", icon: BedDouble },
  { to: "/historial", label: "Historial", icon: History },
  { to: "/ajustes", label: "Ajustes", icon: Settings2 },
];

export function AppShell() {
  const { theme, toggle } = useTheme();

  return (
    <div className="flex h-screen flex-col overflow-hidden">
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <aside className="app-sidebar flex h-full w-[88px] flex-col border-r border-[var(--line)] bg-[var(--surface)] px-3 py-5 lg:w-56 lg:px-4">
          <div className="sidebar-brand">
            <div className="brand-symbol"><LayoutGrid size={23} /></div>
            <div className="hidden lg:block"><p className="brand-caption">CONTROL DE<br />RECEPCIÓN</p></div>
          </div>
          <p className="nav-section hidden lg:block">OPERACIÓN</p>
          <nav aria-label="Navegación principal" className="flex flex-1 flex-col gap-1">
            {links.map((link) => (
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
          <div className="space-y-3 px-1">
            <DutyClock />
            <button
              onClick={toggle}
              className="flex w-full items-center justify-center gap-2 rounded-lg border border-[var(--line)] px-3 py-2 text-sm hover:bg-[var(--surface-2)] lg:justify-start"
            >
              {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
              <span className="hidden lg:inline">{theme === "dark" ? "Modo claro" : "Modo oscuro"}</span>
            </button>
          </div>
        </aside>
        <main className="min-w-0 flex-1 overflow-auto">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
