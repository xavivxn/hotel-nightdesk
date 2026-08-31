import { createContext, createElement, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

type Theme = "light" | "dark";

type ThemeContextValue = {
  theme: Theme;
  toggle: () => void;
  setTheme: (theme: Theme) => void;
};

const ThemeContext = createContext<ThemeContextValue | null>(null);

function applyTheme(theme: Theme) {
  document.documentElement.classList.toggle("dark", theme === "dark");
  document.documentElement.style.colorScheme = theme;
}

export function ThemeProvider({ children, initial = "dark" }: { children: ReactNode; initial?: Theme }) {
  const [theme, setThemeState] = useState<Theme>(() => {
    const stored = localStorage.getItem("nightdesk.theme");
    if (stored === "light" || stored === "dark") return stored;
    return initial;
  });

  useEffect(() => {
    applyTheme(theme);
    localStorage.setItem("nightdesk.theme", theme);
  }, [theme]);

  const value = useMemo(
    () => ({
      theme,
      setTheme: (next: Theme) => setThemeState(next),
      toggle: () => setThemeState((current) => (current === "dark" ? "light" : "dark")),
    }),
    [theme],
  );

  return createElement(ThemeContext.Provider, { value }, children);
}

export function useTheme() {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("ThemeProvider requerido");
  return ctx;
}
