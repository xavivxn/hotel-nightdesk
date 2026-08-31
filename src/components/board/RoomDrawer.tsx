import { api } from "@/lib/api";
import { formatDateTime, formatMoney, pesosToCents, rateKindLabel } from "@/lib/format";
import type { AppSettings, BoardRoom, Charge, RatePlan } from "@/lib/types";
import { Button } from "@/components/ui/Button";
import { Drawer } from "@/components/ui/Drawer";
import { Field, Input, Select } from "@/components/ui/Field";
import { useEffect, useMemo, useState } from "react";

export function RoomDrawer({
  item,
  rates,
  settings,
  onClose,
  onChanged,
}: {
  item: BoardRoom | null;
  rates: RatePlan[];
  settings: AppSettings;
  onClose: () => void;
  onChanged: () => void;
}) {
  if (!item) return null;
  if (item.stay) {
    return (
      <StayDrawer
        item={item}
        settings={settings}
        onClose={onClose}
        onChanged={onChanged}
      />
    );
  }
  if (item.display_status === "reserved" && item.reservation) {
    return (
      <ReservedDrawer item={item} settings={settings} onClose={onClose} onChanged={onChanged} />
    );
  }
  return (
    <CheckInDrawer item={item} rates={rates} settings={settings} onClose={onClose} onChanged={onChanged} />
  );
}

function CheckInDrawer({
  item,
  rates,
  settings,
  onClose,
  onChanged,
}: {
  item: BoardRoom;
  rates: RatePlan[];
  settings: AppSettings;
  onClose: () => void;
  onChanged: () => void;
}) {
  const activeRates = rates.filter((r) => r.active);
  const [guestName, setGuestName] = useState("");
  const [document, setDocument] = useState("");
  const [phone, setPhone] = useState("");
  const [rateId, setRateId] = useState(activeRates[0]?.id ?? 0);
  const [hours, setHours] = useState(3);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const selected = activeRates.find((r) => r.id === rateId);

  async function submit() {
    setBusy(true);
    setError(null);
    try {
      await api.checkIn({
        room_id: item.room.id,
        guest_name: guestName,
        document: document || null,
        phone: phone || null,
        rate_plan_id: rateId,
        expected_hours: selected?.kind === "hourly" ? hours : null,
      });
      onChanged();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Drawer open title={`Habitación ${item.room.number}`} subtitle="Check-in walk-in" onClose={onClose}>
      {item.display_status === "dirty" || item.display_status === "blocked" ? (
        <div className="mb-4 rounded-xl bg-[var(--warn-soft)] px-4 py-3 text-sm">
          Esta habitación está {item.display_status === "dirty" ? "sucia" : "bloqueada"}. Podés marcarla libre desde acá.
          <Button
            className="mt-3 w-full"
            variant="secondary"
            onClick={async () => {
              await api.setRoomStatus(item.room.id, "available");
              onChanged();
            }}
          >
            Marcar como libre
          </Button>
        </div>
      ) : null}
      <div className="space-y-4">
        <Field label="Huésped">
          <Input value={guestName} onChange={(e) => setGuestName(e.target.value)} placeholder="Nombre y apellido" />
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Documento">
            <Input value={document} onChange={(e) => setDocument(e.target.value)} />
          </Field>
          <Field label="Teléfono">
            <Input value={phone} onChange={(e) => setPhone(e.target.value)} />
          </Field>
        </div>
        <Field label="Tarifa">
          <Select value={rateId} onChange={(e) => setRateId(Number(e.target.value))}>
            {activeRates.map((rate) => (
              <option key={rate.id} value={rate.id}>
                {rate.name} · {rateKindLabel(rate.kind)} · {formatMoney(rate.base_amount_cents, settings.currency_symbol)}
              </option>
            ))}
          </Select>
        </Field>
        {selected?.kind === "hourly" ? (
          <Field label="Horas esperadas">
            <Input type="number" min={1} value={hours} onChange={(e) => setHours(Number(e.target.value))} />
          </Field>
        ) : null}
        {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
        <Button className="w-full" disabled={busy || item.display_status === "blocked" || item.display_status === "dirty"} onClick={submit}>
          Confirmar check-in
        </Button>
        {item.display_status === "available" ? (
          <Button
            className="w-full"
            variant="ghost"
            onClick={async () => {
              await api.setRoomStatus(item.room.id, "blocked");
              onChanged();
              onClose();
            }}
          >
            Bloquear habitación
          </Button>
        ) : null}
      </div>
    </Drawer>
  );
}

function ReservedDrawer({
  item,
  onClose,
  onChanged,
}: {
  item: BoardRoom;
  settings: AppSettings;
  onClose: () => void;
  onChanged: () => void;
}) {
  const res = item.reservation!;
  const [error, setError] = useState<string | null>(null);
  return (
    <Drawer open title={`Habitación ${item.room.number}`} subtitle="Reserva de hoy" onClose={onClose}>
      <div className="space-y-4">
        <p className="text-lg font-semibold">{res.guest_name}</p>
        <p className="text-sm text-[var(--muted)]">
          {formatDateTime(res.expected_arrival_at)} · {res.expected_nights} noche(s) · {res.rate_plan_name}
        </p>
        {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
        <Button
          className="w-full"
          onClick={async () => {
            try {
              await api.checkInReservation(res.id);
              onChanged();
              onClose();
            } catch (e) {
              setError(String(e));
            }
          }}
        >
          Hacer check-in
        </Button>
        <p className="text-sm text-[var(--muted)]">{res.rate_plan_name}</p>
      </div>
    </Drawer>
  );
}

function StayDrawer({
  item,
  settings,
  onClose,
  onChanged,
}: {
  item: BoardRoom;
  settings: AppSettings;
  onClose: () => void;
  onChanged: () => void;
}) {
  const stay = item.stay!;
  const [bill, setBill] = useState(item.estimated_total_cents);
  const [lines, setLines] = useState<{ description: string; amount_cents: number }[]>([]);
  const [charges, setCharges] = useState<Charge[]>([]);
  const [overnight, setOvernight] = useState(stay.converted_to_overnight);
  const [method, setMethod] = useState("cash");
  const [received, setReceived] = useState("");
  const [extraDesc, setExtraDesc] = useState("Recargo");
  const [extraAmount, setExtraAmount] = useState("");
  const [print, setPrint] = useState(settings.auto_print_on_checkout);
  const [error, setError] = useState<string | null>(null);
  const [printError, setPrintError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [closedStayId, setClosedStayId] = useState<number | null>(null);

  async function refresh() {
    const [, preview, currentCharges] = await api.getStayDetail(stay.id);
    setBill(preview.total_cents);
    setLines(preview.lines);
    setCharges(currentCharges.filter((c) => c.kind === "surcharge" || c.kind === "discount"));
    setOvernight(preview.overnight_applied);
    if (!received) setReceived((preview.total_cents / 100).toFixed(2));
  }

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
    const id = window.setInterval(() => refresh().catch(() => undefined), 15000);
    return () => window.clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stay.id]);

  const total = bill ?? 0;
  const receivedCents = pesosToCents(received || "0");
  const change = receivedCents - total;

  const subtitle = useMemo(
    () => `${stay.guest_name} · desde ${formatDateTime(stay.check_in_at)}`,
    [stay],
  );

  if (closedStayId) {
    return (
      <Drawer open wide title={`Habitación ${item.room.number}`} subtitle="Estadía cerrada" onClose={onClose}>
        <div className="space-y-4">
          <p className="font-display text-3xl">{formatMoney(total, settings.currency_symbol)}</p>
          <p className="text-sm text-[var(--muted)]">El cobro quedó registrado. La habitación pasa a sucia.</p>
          {printError ? <p className="text-sm text-[var(--warn)]">Impresora: {printError}. Podés reintentar.</p> : null}
          <Button
            className="w-full"
            variant="secondary"
            onClick={async () => {
              const nextError = await api.reprintReceipt(closedStayId);
              setPrintError(nextError);
            }}
          >
            Reimprimir ticket
          </Button>
          <Button className="w-full" onClick={onClose}>
            Volver al tablero
          </Button>
        </div>
      </Drawer>
    );
  }

  return (
    <Drawer open wide title={`Habitación ${item.room.number}`} subtitle={subtitle} onClose={onClose}>
      <div className="space-y-5">
        <div className="rounded-2xl bg-[var(--surface-2)] p-4">
          <p className="text-xs uppercase tracking-[0.16em] text-[var(--muted)]">Cuenta en vivo</p>
          <p className="mt-1 font-display text-4xl">{formatMoney(total, settings.currency_symbol)}</p>
          {overnight ? <p className="mt-2 text-sm text-[var(--accent)]">Se está aplicando tarifa de pernocte/noche</p> : null}
        </div>
        <ul className="space-y-2 text-sm">
          {lines.map((line, i) => (
            <li key={`${line.description}-${i}`} className="flex justify-between gap-4">
              <span>{line.description}</span>
              <span className="font-medium">{formatMoney(line.amount_cents, settings.currency_symbol)}</span>
            </li>
          ))}
        </ul>
        {!stay.converted_to_overnight ? (
          <Button
            variant="secondary"
            className="w-full"
            onClick={async () => {
              await api.convertToOvernight(stay.id);
              await refresh();
              onChanged();
            }}
          >
            Convertir a pernocte
          </Button>
        ) : null}
        <div className="grid grid-cols-[1fr_120px_auto] gap-2">
          <Input value={extraDesc} onChange={(e) => setExtraDesc(e.target.value)} />
          <Input value={extraAmount} onChange={(e) => setExtraAmount(e.target.value)} placeholder="0,00" />
          <Button
            variant="secondary"
            onClick={async () => {
              await api.addCharge({
                stay_id: stay.id,
                kind: pesosToCents(extraAmount) < 0 ? "discount" : "surcharge",
                description: extraDesc,
                amount_cents: pesosToCents(extraAmount),
              });
              setExtraAmount("");
              await refresh();
            }}
          >
            Sumar
          </Button>
        </div>
        {charges.map((charge) => (
          <div key={charge.id} className="flex items-center justify-between text-sm">
            <span>
              {charge.description} · {formatMoney(charge.amount_cents, settings.currency_symbol)}
            </span>
            <button className="text-[var(--danger)]" onClick={async () => { await api.deleteCharge(charge.id); await refresh(); }}>
              Quitar
            </button>
          </div>
        ))}
        <Field label="Medio de pago">
          <Select value={method} onChange={(e) => setMethod(e.target.value)}>
            <option value="cash">Efectivo</option>
            <option value="card">Tarjeta</option>
            <option value="transfer">Transferencia</option>
          </Select>
        </Field>
        <Field label="Monto recibido">
          <Input value={received} onChange={(e) => setReceived(e.target.value)} />
        </Field>
        {method === "cash" && change > 0 ? (
          <p className="text-sm">Vuelto: {formatMoney(change, settings.currency_symbol)}</p>
        ) : null}
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={print} onChange={(e) => setPrint(e.target.checked)} />
          Imprimir ticket al cerrar
        </label>
        {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
        {printError ? <p className="text-sm text-[var(--warn)]">{printError}</p> : null}
        <Button
          className="w-full"
          variant="ok"
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            setError(null);
            try {
              const result = await api.checkOut({
                stay_id: stay.id,
                method,
                amount_cents: receivedCents,
                print,
              });
              setClosedStayId(result.stay.id);
              if (result.print_error) setPrintError(result.print_error);
              onChanged();
            } catch (e) {
              setError(String(e));
            } finally {
              setBusy(false);
            }
          }}
        >
          Cobrar y cerrar
        </Button>
      </div>
    </Drawer>
  );
}
