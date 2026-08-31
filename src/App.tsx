import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { useEffect, useState } from "react";
import { AppShell } from "@/components/layout/AppShell";
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

const fallbackSettings: AppSettings = {
  business_name: "Nightdesk Inn",
  address: "",
  phone: "",
  tax_percent: 0,
  currency_symbol: "$",
  theme: "dark",
  receipt_footer: "Gracias por su visita",
  printer_enabled: false,
  printer_path: "",
  printer_name: "",
  paper_width: 80,
  auto_print_on_checkout: true,
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

  async function loadSettings() {
    const current = await api.getSettings();
    setSettings(current);
    if (current.theme === "light" || current.theme === "dark") {
      setTheme(current.theme);
    }
    const needsPin = await api.pinRequired();
    setLocked(needsPin);
    setReady(true);
  }

  useEffect(() => {
    loadSettings().catch(() => setReady(true));
  }, []);

  if (!ready) {
    return (
      <div className="grid min-h-screen place-items-center text-[var(--muted)]">
        Abriendo recepción…
      </div>
    );
  }

  if (locked) {
    return (
      <div className="grid min-h-screen place-items-center px-6">
        <form
          className="card w-full max-w-sm rounded-3xl p-8"
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
          <p className="font-display text-3xl">Nightdesk</p>
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
