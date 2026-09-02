import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { useEffect, useState } from "react";
import { AppShell } from "@/components/layout/AppShell";
import { BootSplash } from "@/components/layout/BootSplash";
import { BoardPage } from "@/pages/BoardPage";
import { HistoryPage } from "@/pages/HistoryPage";
import { ReservationsPage } from "@/pages/ReservationsPage";
import { RoomsPage } from "@/pages/RoomsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { api } from "@/lib/api";
import { useTheme } from "@/lib/theme";
import type { AppSettings } from "@/lib/types";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Field";

/** Seconds the splash holds after boot. Integers like 10, 8, 5. */
const BOOT_SPLASH_SECONDS = 2;
const BOOT_FADE_MS = 180;

function wait(ms: number) {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

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
  require_guest_name: true,
  pin_hash: "",
  has_pin: false,
};

export default function App() {
  const { setTheme } = useTheme();
  const [settings, setSettings] = useState<AppSettings>(fallbackSettings);
  const [locked, setLocked] = useState(false);
  const [pin, setPin] = useState("");
  const [pinError, setPinError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [leaving, setLeaving] = useState(false);

  async function loadSettings() {
    const current = await api.getSettings();
    setSettings(current);
    if (current.theme === "light" || current.theme === "dark") {
      setTheme(current.theme);
    }
    const needsPin = await api.pinRequired();
    setLocked(needsPin);
  }

  useEffect(() => {
    let cancelled = false;
    document.getElementById("boot-splash")?.remove();

    void (async () => {
      await loadSettings().catch(() => undefined);
      if (cancelled) return;
      await wait(BOOT_SPLASH_SECONDS * 1000);
      if (cancelled) return;
      setLeaving(true);
      await wait(BOOT_FADE_MS);
      if (cancelled) return;
      setReady(true);
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  if (!ready) {
    return <BootSplash leaving={leaving} holdSeconds={BOOT_SPLASH_SECONDS} />;
  }

  if (locked) {
    return (
      <div className="grid min-h-screen place-items-center px-6">
        <form
          className="card w-full max-w-sm rounded-lg p-8"
          onSubmit={async (e) => {
            e.preventDefault();
            const ok = await api.verifyPin(pin);
            if (ok) {
              setLocked(false);
              setPin("");
              setPinError(null);
            } else {
              setPinError("PIN incorrecto");
            }
          }}
        >
          <p className="font-mono text-xs font-semibold tracking-[0.22em] text-[var(--accent)]">NIGHTDESK</p>
          <p className="mt-3 text-xl font-semibold tracking-tight">Desbloqueo</p>
          <p className="mt-2 text-sm text-[var(--muted)]">Ingresá el PIN para abrir la recepción.</p>
          <div className="mt-6">
            <Input
              type="password"
              inputMode="numeric"
              autoFocus
              value={pin}
              onChange={(e) => setPin(e.target.value)}
              placeholder="PIN"
            />
          </div>
          {pinError ? <p className="mt-3 text-sm text-[var(--danger)]">{pinError}</p> : null}
          <Button className="mt-5 w-full" type="submit">
            Entrar
          </Button>
        </form>
      </div>
    );
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppShell businessName={settings.business_name} />}>
          <Route path="/" element={<BoardPage settings={settings} />} />
          <Route path="/reservas" element={<ReservationsPage />} />
          <Route path="/habitaciones" element={<RoomsPage settings={settings} />} />
          <Route path="/historial" element={<HistoryPage settings={settings} />} />
          <Route
            path="/ajustes"
            element={<SettingsPage settings={settings} onSaved={setSettings} />}
          />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
