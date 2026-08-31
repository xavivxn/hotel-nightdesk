import { NavLink, Outlet } from "react-router-dom";
import { BedDouble, CalendarClock, History, LayoutGrid, Moon, Settings2, Sun } from "lucide-react";
import { useEffect, useState } from "react";
import { useTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";

const links = [
  { to: "/", label: "Tablero", icon: LayoutGrid },
  { to: "/reservas", label: "Reservas", icon: CalendarClock },
  { to: "/habitaciones", label: "Habitaciones", icon: BedDouble },
  { to: "/historial", label: "Historial", icon: History },
  { to: "/ajustes", label: "Ajustes", icon: Settings2 },
];

export function AppShell({ businessName }: { businessName: string }) {
  const { theme, toggle } = useTheme();
  const [clock, setClock] = useState(() => new Date());

  useEffect(() => {
    const id = window.setInterval(() => setClock(new Date()), 1000);
    return () => window.clearInterval(id);
  }, []);

  return (
    <div className="flex min-h-screen">
      <aside className="sticky top-0 flex h-screen w-[88px] flex-col border-r border-[var(--line)] bg-[var(--surface)]/90 px-3 py-5 backdrop-blur-md lg:w-64 lg:px-4">
        <div className="px-2">
          <p className="font-display text-xl leading-none">Nightdesk</p>
          <p className="mt-1 hidden truncate text-xs text-[var(--muted)] lg:block">{businessName}</p>
        </div>
        <nav className="mt-8 flex flex-1 flex-col gap-1">
          {links.map((link) => (
            <NavLink
              key={link.to}
              to={link.to}
              end={link.to === "/"}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 rounded-xl px-3 py-3 text-sm font-medium text-[var(--muted)] hover:bg-[var(--surface-2)] hover:text-[var(--ink)]",
                  isActive && "bg-[var(--accent-soft)] text-[var(--ink)]",
                )
              }
            >
              <link.icon size={18} />
              <span className="hidden lg:inline">{link.label}</span>
            </NavLink>
          ))}
        </nav>
        <div className="space-y-3 px-1">
          <p className="hidden font-mono text-sm text-[var(--muted)] lg:block">
            {clock.toLocaleTimeString("es-AR", { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
          </p>
          <button
            onClick={toggle}
            className="flex w-full items-center justify-center gap-2 rounded-xl border border-[var(--line)] px-3 py-2 text-sm hover:bg-[var(--surface-2)] lg:justify-start"
          >
            {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
            <span className="hidden lg:inline">{theme === "dark" ? "Modo claro" : "Modo oscuro"}</span>
          </button>
        </div>
      </aside>
      <main className="min-w-0 flex-1">
        <Outlet />
      </main>
    </div>
  );
}
