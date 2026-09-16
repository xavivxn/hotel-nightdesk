import { useContext, useEffect, useMemo, useState } from "react";
import { Ban, Bath, Clock3, LogIn } from "lucide-react";
import { RoleContext } from "@/lib/permissions";
import { api } from "@/lib/api";
import { formatDateTime, formatMoney, parseGuaranies, rateKindLabel } from "@/lib/format";
import { dormidaEnd, dormidaStartsAtMidnight, dormidaUnavailableMessage, dormidaWindowOpen } from "@/lib/billing";
import type { AppSettings, BoardRoom, Charge, RatePlan } from "@/lib/types";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Input } from "@/components/ui/Field";
import { RoomShop } from "@/components/board/RoomShop";
import { cn } from "@/lib/utils";

function formatClock(date: Date) {
  return date.toLocaleTimeString("es-AR", { hour: "2-digit", minute: "2-digit" });
}

function isDormidaKind(kind: string) {
  return kind === "overnight" || kind === "night";
}

function useNow(ms = 30000) {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), ms);
    return () => window.clearInterval(id);
  }, [ms]);
  return now;
}

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
  const now = useNow();
  const activeRates = [...rates.filter((r) => r.active)].sort((a, b) => {
    const order = { hourly: 0, overnight: 1, night: 2 };
    return (order[a.kind] ?? 9) - (order[b.kind] ?? 9);
  });
  const [rateId, setRateId] = useState(activeRates[0]?.id ?? 0);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const selected = activeRates.find((r) => r.id === rateId);
  const blocked = item.display_status === "blocked";
  const dirty = item.display_status === "dirty";
  const dormidaOpen = selected
    ? !isDormidaKind(selected.kind) || dormidaWindowOpen(now, selected.night_cutoff_hour)
    : false;
  const canCheckIn = !blocked && !dirty && Boolean(selected) && dormidaOpen;
  const isJacuzzi = /jacc?uz+i/.test(item.room.room_type.trim().toLowerCase());
  const checkoutAt = selected?.kind === "hourly"
    ? new Date(Date.now() + 60 * 60 * 1000)
    : selected?.kind === "overnight" || selected?.kind === "night"
      ? dormidaEnd(now, selected.night_cutoff_hour)
      : null;
  const dormidaMidnight = selected
    ? dormidaStartsAtMidnight(now, selected.night_cutoff_hour)
    : false;

  useEffect(() => {
    if (!selected || !isDormidaKind(selected.kind) || dormidaWindowOpen(now, selected.night_cutoff_hour)) return;
    const hourly = rates.find((rate) => rate.active && rate.kind === "hourly");
    if (hourly) setRateId(hourly.id);
  }, [now, rates, selected]);

  async function submit() {
    setBusy(true);
    setError(null);
    try {
      await api.checkIn({
        room_id: item.room.id,
        guest_name: "",
        document: null,
        phone: null,
        rate_plan_id: rateId,
        expected_hours: selected?.kind === "hourly" ? 1 : null,
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
    <Dialog open title={`Habitación ${item.room.number}`} subtitle="Ingreso sin reserva" onClose={onClose}>
      <div className="space-y-5">
        <div className="flex flex-wrap items-center gap-2 text-sm text-[var(--muted)]">
          <span className="rounded-lg border border-[var(--line)] bg-[var(--bg)] px-2.5 py-1 font-medium text-[var(--ink)]">
            {item.room.room_type}
          </span>
          <span>Piso {item.room.floor}</span>
          {isJacuzzi ? (
            <span className="inline-flex items-center gap-1 text-[var(--gold)]">
              <Bath size={14} /> Con jacuzzi
            </span>
          ) : null}
        </div>

        {dirty || blocked ? (
          <div className={cn("rounded-lg px-4 py-3 text-sm", dirty ? "bg-[var(--dirty-soft)]" : "bg-[var(--danger-soft)]")}>
            <p className="font-semibold">{dirty ? "Pendiente de aseo" : "Fuera de servicio"}</p>
            <p className="mt-1 text-[var(--ink)]">
              {dirty
                ? "Marcala como libre cuando esté limpia. Recién ahí se puede ingresar."
                : "Esta habitación está bloqueada. Desbloqueala para volver a usarla."}
            </p>
            <Button
              className="mt-3 w-full"
              variant="secondary"
              onClick={async () => {
                await api.setRoomStatus(item.room.id, "available");
                onChanged();
              }}
            >
              {dirty ? "Marcar como libre" : "Desbloquear"}
            </Button>
          </div>
        ) : null}

        <div>
          <p className="mb-2 text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">Elegí la tarifa</p>
          <div className="grid gap-2">
            {activeRates.map((rate) => {
              const active = rate.id === rateId;
              const locked = isDormidaKind(rate.kind) && !dormidaWindowOpen(now, rate.night_cutoff_hour);
              const midnight = dormidaStartsAtMidnight(now, rate.night_cutoff_hour);
              return (
                <button
                  key={rate.id}
                  type="button"
                  disabled={locked}
                  aria-disabled={locked}
                  onClick={() => { if (!locked) setRateId(rate.id); }}
                  className={cn(
                    "flex min-h-11 items-center justify-between gap-3 rounded-lg border px-3.5 py-3 text-left",
                    locked && "cursor-not-allowed opacity-50",
                    !locked && active
                      ? "border-[var(--accent)] bg-[var(--accent-soft)]"
                      : "border-[var(--line)] bg-[var(--surface)]",
                    !locked && !active && "hover:bg-[var(--surface-2)]",
                  )}
                >
                  <span>
                    <span className="block text-sm font-semibold text-[var(--ink)]">{rate.name}</span>
                    <span className="mt-0.5 block text-xs text-[var(--muted)]">
                      {rate.kind === "hourly"
                        ? "Después, adicional de 30 min o otra hora"
                        : locked
                          ? midnight
                            ? "Disponible de 00:00 a 10:00"
                            : "Disponible de 22:00 a 10:00"
                          : midnight
                            ? "00:00 a 10:00"
                            : "22:00 a 10:00"}
                    </span>
                  </span>
                  <span className="font-mono text-sm font-semibold tabular-nums text-[var(--ink)]">
                    {formatMoney(rate.base_amount_cents, settings.currency_symbol)}
                  </span>
                </button>
              );
            })}
          </div>
        </div>

        {selected ? (
          <div className="rounded-lg border border-[var(--line)] bg-[var(--bg)] px-4 py-3">
            <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">Resumen</p>
            <div className="mt-2 flex items-start justify-between gap-3">
              <span className="inline-flex items-center gap-1.5 text-sm text-[var(--muted)]">
                <Clock3 size={15} />
                {checkoutAt
                  ? selected.kind === "hourly"
                    ? `Primera hora hasta ${formatClock(checkoutAt)}`
                    : `Salida a las ${formatClock(checkoutAt)}`
                  : rateKindLabel(selected.kind)}
              </span>
              <span className="font-mono text-lg font-semibold tabular-nums">
                {formatMoney(selected.base_amount_cents, settings.currency_symbol)}
              </span>
            </div>
            <p className="mt-2 text-xs text-[var(--muted)]">
              {selected.kind === "hourly"
                ? "Si se pasan 5 minutos de la hora, se cobra adicional de 30 min. Si se pasan 5 minutos de esos 30, se cobra otra hora. Es automático."
                : dormidaMidnight
                  ? "Viernes, sábado y feriados: de 00:00 a 10:00. Si el ingreso es de madrugada, la salida es a las 10:00 de hoy."
                  : "Domingo a jueves: de 22:00 a 10:00. Si el ingreso es de madrugada, la salida es a las 10:00 de hoy."}
            </p>
          </div>
        ) : null}

        {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
        <Button className="w-full" size="lg" disabled={busy || !canCheckIn} onClick={submit}>
          <LogIn size={18} />
          {busy ? "Ingresando…" : `Ingresar a ${item.room.number}`}
        </Button>
        {item.display_status === "available" ? (
          <button
            type="button"
            className="flex min-h-11 w-full items-center justify-center gap-2 text-sm text-[var(--muted)] hover:text-[var(--danger)]"
            onClick={async () => {
              await api.setRoomStatus(item.room.id, "blocked");
              onChanged();
              onClose();
            }}
          >
            <Ban size={15} />
            Bloquear habitación
          </button>
        ) : null}
      </div>
    </Dialog>
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
    <Dialog open title={`Habitación ${item.room.number}`} subtitle="Reserva de hoy" onClose={onClose}>
      <div className="space-y-4">
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
    </Dialog>
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
  const now = useNow();
  const [bill, setBill] = useState(item.estimated_total_cents);
  const [lines, setLines] = useState<{ description: string; amount_cents: number }[]>([]);
  const [charges, setCharges] = useState<Charge[]>([]);
  const [overnight, setOvernight] = useState(stay.converted_to_overnight);
  const admin = useContext(RoleContext) === "admin";
  const [extraDesc, setExtraDesc] = useState("Consumo");
  const [extraAmount, setExtraAmount] = useState("");
  const [print, setPrint] = useState(settings.auto_print_on_checkout);
  const [error, setError] = useState<string | null>(null);
  const [printError, setPrintError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [mutationBusy, setMutationBusy] = useState(false);
  const [closedStayId, setClosedStayId] = useState<number | null>(null);

  async function refresh() {
    const [, preview, currentCharges] = await api.getStayDetail(stay.id);
    setBill(preview.total_cents);
    setLines(preview.lines);
    setCharges(currentCharges.filter((c) => c.kind === "surcharge" || c.kind === "discount"));
    setOvernight(preview.overnight_applied);
  }

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
    if (closedStayId) return;
    const id = window.setInterval(() => refresh().catch(() => undefined), 15000);
    return () => window.clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stay.id, closedStayId]);

  const total = bill ?? 0;
  const subtitle = useMemo(() => `desde ${formatDateTime(stay.check_in_at)}`, [stay]);

  if (closedStayId) {
    function finish() {
      onChanged();
      onClose();
    }
    return (
      <Dialog open title={`Habitación ${item.room.number}`} subtitle="Estadía cerrada" onClose={finish}>
        <div className="space-y-4">
          <p className="font-mono text-3xl font-semibold tabular-nums">{formatMoney(total, settings.currency_symbol)}</p>
          <p className="text-sm text-[var(--muted)]">La cuenta quedó cerrada y la habitación pasa a sucia. No se registra ningún pago.</p>
          {printError ? <p className="text-sm text-[var(--warn)]">Impresora: {printError}. Podés reintentar.</p> : null}
          <Button
            className="w-full"
            variant="secondary"
            onClick={async () => {
              setBusy(true);
              try { setPrintError(await api.reprintReceipt(closedStayId)); }
              catch (e) { setPrintError(String(e)); }
              finally { setBusy(false); }
            }}
            disabled={busy}
          >
            {busy ? "Enviando…" : "Reimprimir ticket"}
          </Button>
          <Button className="w-full" onClick={finish}>
            Volver al tablero
          </Button>
        </div>
      </Dialog>
    );
  }

  return (
    <Dialog
      open
      size="lg"
      dismissible={false}
      title={`Habitación ${item.room.number}`}
      subtitle={subtitle}
      onClose={onClose}
    >
      <div className="grid min-h-0 flex-1 grid-cols-1 overflow-hidden min-[800px]:grid-cols-2">
        <div className="min-h-0 space-y-5 overflow-y-auto border-[var(--line)] p-6 scrollbar-thin min-[800px]:border-r">
          <div className="rounded-lg border border-[var(--line)] bg-[var(--bg)] p-4">
            <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">Cuenta en vivo</p>
            <p className="mt-1 font-mono text-4xl font-semibold tabular-nums tracking-tight">
              {formatMoney(total, settings.currency_symbol)}
            </p>
            {overnight ? (
              <p className="mt-2 text-sm text-[var(--accent)]">Se está aplicando tarifa de dormida</p>
            ) : null}
          </div>
          <ul className="space-y-2 text-sm">
            {lines.map((line, i) => (
              <li key={`${line.description}-${i}`} className="flex justify-between gap-4">
                <span>{line.description}</span>
                <span className="font-mono font-medium tabular-nums">
                  {formatMoney(line.amount_cents, settings.currency_symbol)}
                </span>
              </li>
            ))}
          </ul>
          {!stay.converted_to_overnight && stay.rate_kind === "hourly" ? (
            <div className="space-y-2">
              <Button
                variant="secondary"
                className="w-full"
                onClick={async () => {
                  setMutationBusy(true);
                  setError(null);
                  try {
                    await api.convertToOvernight(stay.id);
                    await refresh();
                    onChanged();
                  } catch (e) {
                    setError(String(e));
                  } finally {
                    setMutationBusy(false);
                  }
                }}
                disabled={mutationBusy || busy || !dormidaWindowOpen(now)}
              >
                Convertir a dormida
              </Button>
              {!dormidaWindowOpen(now) ? (
                <p className="text-xs text-[var(--muted)]">{dormidaUnavailableMessage(now)}</p>
              ) : null}
            </div>
          ) : null}
          <RoomShop
            stayId={stay.id}
            currency={settings.currency_symbol}
            charges={charges}
            onBusyChange={setMutationBusy}
            onChanged={async () => {
              await refresh();
              onChanged();
            }}
          />
          {admin && <div className="grid grid-cols-[1fr_120px_auto] gap-2">
            <Input value={extraDesc} onChange={(e) => setExtraDesc(e.target.value)} placeholder="Otro cargo" />
            <Input
              value={extraAmount}
              onChange={(e) => setExtraAmount(e.target.value)}
              placeholder="0"
              inputMode="numeric"
            />
            <Button
              variant="secondary"
              onClick={async () => {
                setMutationBusy(true);
                setError(null);
                try {
                  await api.addCharge({
                    stay_id: stay.id,
                    kind: parseGuaranies(extraAmount) < 0 ? "discount" : "surcharge",
                    description: extraDesc,
                    amount_cents: parseGuaranies(extraAmount),
                  });
                  setExtraAmount("");
                  await refresh();
                } catch (e) {
                  setError(String(e));
                } finally {
                  setMutationBusy(false);
                }
              }}
              disabled={mutationBusy || busy}
            >
              Sumar
            </Button>
          </div>}
          {charges.map((charge) => (
            <div key={charge.id} className="flex items-center justify-between text-sm">
              <span>
                {charge.description} · {formatMoney(charge.amount_cents, settings.currency_symbol)}
              </span>
              {admin && <button
                className="text-[var(--danger)]"
                onClick={async () => {
                  setMutationBusy(true);
                  setError(null);
                  try {
                    await api.deleteCharge(charge.id);
                    await refresh();
                  } catch (e) {
                    setError(String(e));
                  } finally {
                    setMutationBusy(false);
                  }
                }}
                disabled={mutationBusy || busy}
              >
                Quitar
              </button>}
            </div>
          ))}
        </div>
        <div className="flex min-h-0 flex-col p-6">
          <div className="min-h-0 flex-1 space-y-4 overflow-y-auto scrollbar-thin">
            <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">Cerrar cuenta</p>
            <div className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4 text-sm">
              <p className="font-semibold">Total operativo</p>
              <p className="mt-1 font-mono text-2xl font-semibold tabular-nums">{formatMoney(total, settings.currency_symbol)}</p>
              <p className="mt-2 text-[var(--muted)]">El total se calcula en SQLite y queda guardado para el historial.</p>
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input type="checkbox" checked={print} onChange={(e) => setPrint(e.target.checked)} />
              Imprimir ticket al cerrar
            </label>
          </div>
          <div className="mt-4 shrink-0 space-y-3 border-t border-[var(--line)] pt-4">
            {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
            {printError ? <p className="text-sm text-[var(--warn)]">{printError}</p> : null}
            <Button
              className="w-full"
              variant="ok"
              size="lg"
              disabled={busy || mutationBusy}
              onClick={async () => {
                setBusy(true);
                setError(null);
                try {
                  const result = await api.checkOut({
                    stay_id: stay.id,
                    print,
                  });
                  setClosedStayId(result.stay.id);
                  setBill(result.bill.total_cents);
                  if (result.print_error) setPrintError(result.print_error);
                } catch (e) {
                  setError(String(e));
                } finally {
                  setBusy(false);
                }
              }}
            >
              Cerrar cuenta
            </Button>
          </div>
        </div>
      </div>
    </Dialog>
  );
}
