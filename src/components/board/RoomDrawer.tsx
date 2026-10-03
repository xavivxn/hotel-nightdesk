import { useOperationalRefresh } from "@/lib/useOperationalRefresh";
import type { AccountQuote } from "@/lib/types";
import { useContext, useEffect, useMemo, useRef, useState } from "react";
import { Ban, Bath, Clock3, LogIn } from "lucide-react";
import { RoleContext } from "@/lib/permissions";
import { api } from "@/lib/api";
import { FIELD_EMPTY, formatDateTime, formatMoney, parseMoneyInteger, rateKindLabel, requireTrimmed, roomCategory } from "@/lib/format";
import { dormidaEnd, dormidaStartsAtMidnight, dormidaUnavailableMessage, dormidaWindowOpen } from "@/lib/billing";
import type { AppSettings, BoardRoom, Charge, EffectivePrice, RatePlan } from "@/lib/types";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Input, Select, reportInputIssue } from "@/components/ui/Field";
import { RoomShop } from "@/components/board/RoomShop";
import { StayCart } from "@/components/board/StayCart";
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
  readOnly = false,
  onClose,
  onChanged,
}: {
  item: BoardRoom | null;
  rates: RatePlan[];
  settings: AppSettings;
  readOnly?: boolean;
  onClose: () => void;
  onChanged: () => void;
}) {
  if (!item) return null;
  if (item.stay) {
    return (
      <StayDrawer
        item={item}
        settings={settings}
        readOnly={readOnly}
        onClose={onClose}
        onChanged={onChanged}
      />
    );
  }
  if (item.display_status === "reserved" && item.reservation) {
    return (
      <ReservedDrawer item={item} settings={settings} readOnly={readOnly} onClose={onClose} onChanged={onChanged} />
    );
  }
  return (
    <CheckInDrawer item={item} rates={rates} settings={settings} readOnly={readOnly} onClose={onClose} onChanged={onChanged} />
  );
}

function CheckInDrawer({
  item,
  rates,
  settings,
  readOnly,
  onClose,
  onChanged,
}: {
  item: BoardRoom;
  rates: RatePlan[];
  settings: AppSettings;
  readOnly: boolean;
  onClose: () => void;
  onChanged: () => void;
}) {
  const now = useNow();
  const category = roomCategory(item.room.room_type);
  const activeRates = useMemo(() => rates.filter((r) => r.active && r.room_category === category).sort((a, b) => {
    const order = { hourly: 0, overnight: 1, night: 2 };
    return (order[a.kind] ?? 9) - (order[b.kind] ?? 9);
  }), [rates, category]);
  const [rateId, setRateId] = useState(activeRates[0]?.id ?? 0);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [prices, setPrices] = useState<Map<number, EffectivePrice>>(new Map());
  const selected = activeRates.find((r) => r.id === rateId);
  const selectedPrice = selected ? prices.get(selected.id) : undefined;

  // Promotions depend on the hour: refresh with the clock. The charge itself is decided in Rust.
  useEffect(() => {
    api.currentPrices()
      .then((rows) => setPrices(new Map(rows.map((row) => [row.rate_plan_id, row]))))
      .catch(() => undefined);
  }, [now]);
  const blocked = item.display_status === "blocked";
  const dirty = item.display_status === "dirty";
  const dormidaOpen = selected
    ? !isDormidaKind(selected.kind) || dormidaWindowOpen(now, selected.night_cutoff_hour)
    : false;
  const canCheckIn = !blocked && !dirty && Boolean(selected) && dormidaOpen;
  const isJacuzzi = category === "jacuzzi";
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
    const hourly = activeRates.find((rate) => rate.kind === "hourly");
    if (hourly) setRateId(hourly.id);
  }, [now, activeRates, selected]);

  async function submit() {
    setBusy(true);
    setError(null);
    try {
      await api.checkIn({
        room_id: item.room.id,
        expected_version: item.room.operational_version,
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
                ? readOnly
                  ? "Pendiente de aseo en recepción."
                  : "Marcala como libre cuando esté limpia. Recién ahí se puede ingresar."
                : readOnly
                  ? "Esta habitación está bloqueada."
                  : "Esta habitación está bloqueada. Desbloqueala para volver a usarla."}
            </p>
            {readOnly ? null : (
              <Button
                className="mt-3 w-full"
                variant="secondary"
                onClick={async () => {
                  await api.setRoomStatus(item.room.id, "available", item.room.operational_version);
                  onChanged();
                }}
              >
                {dirty ? "Marcar como libre" : "Desbloquear"}
              </Button>
            )}
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
                  <span className="shrink-0 text-right">
                    <span className="block font-mono text-sm font-semibold tabular-nums text-[var(--ink)]">
                      {formatMoney(prices.get(rate.id)?.base_amount_cents ?? rate.base_amount_cents, settings.currency_symbol)}
                    </span>
                    {prices.get(rate.id)?.rule_name ? (
                      <span className="block text-[11px] font-semibold text-[var(--accent)]">
                        {prices.get(rate.id)?.rule_name} · antes {formatMoney(rate.base_amount_cents, settings.currency_symbol)}
                      </span>
                    ) : null}
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
                {formatMoney(selectedPrice?.base_amount_cents ?? selected.base_amount_cents, settings.currency_symbol)}
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
        {readOnly ? (
          <p className="text-sm text-[var(--muted)]">El ingreso se hace en recepción.</p>
        ) : (
          <Button className="w-full" size="lg" disabled={busy || !canCheckIn} onClick={submit}>
            <LogIn size={18} />
            {busy ? "Ingresando…" : `Ingresar a ${item.room.number}`}
          </Button>
        )}
        {!readOnly && item.display_status === "available" ? (
          <button
            type="button"
            className="flex min-h-11 w-full items-center justify-center gap-2 text-sm text-[var(--muted)] hover:text-[var(--danger)]"
            onClick={async () => {
              await api.setRoomStatus(item.room.id, "blocked", item.room.operational_version);
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
  readOnly,
  onClose,
  onChanged,
}: {
  item: BoardRoom;
  settings: AppSettings;
  readOnly: boolean;
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
        {readOnly ? (
          <p className="text-sm text-[var(--muted)]">El check-in se hace en recepción.</p>
        ) : (
          <Button
            className="w-full"
            onClick={async () => {
              try {
                await api.checkInReservation(res.id, res.operational_version);
                onChanged();
                onClose();
              } catch (e) {
                setError(String(e));
              }
            }}
          >
            Hacer check-in
          </Button>
        )}
        <p className="text-sm text-[var(--muted)]">{res.rate_plan_name}</p>
      </div>
    </Dialog>
  );
}

function StayDrawer({
  item,
  settings,
  readOnly,
  onClose,
  onChanged,
}: {
  item: BoardRoom;
  settings: AppSettings;
  readOnly: boolean;
  onClose: () => void;
  onChanged: () => void;
}) {
  const stay = item.stay!;
  const now = useNow();
  const [quote, setQuote] = useState<AccountQuote | null>(null);
  const [bill, setBill] = useState(item.estimated_total_cents);
  const [lines, setLines] = useState<{ kind: string; description: string; amount_cents: number }[]>([]);
  const [charges, setCharges] = useState<Charge[]>([]);
  const [overnight, setOvernight] = useState(stay.converted_to_overnight);
  const admin = useContext(RoleContext) === "admin";
  const [extraDesc, setExtraDesc] = useState("Consumo");
  const [extraAmount, setExtraAmount] = useState("");
  const extraDescRef = useRef<HTMLInputElement>(null);
  const extraAmountRef = useRef<HTMLInputElement>(null);
  const [print, setPrint] = useState(settings.auto_print_on_checkout);
  const [error, setError] = useState<string | null>(null);
  const [printError, setPrintError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [mutationBusy, setMutationBusy] = useState(false);
  const [reprintTarget, setReprintTarget] = useState<"local" | "principal" | undefined>();
  const [closedStayId, setClosedStayId] = useState<number | null>(null);

  async function refresh() {
    const [[currentStay, detail, currentCharges], currentQuote] = await Promise.all([api.getStayDetail(stay.id), readOnly ? Promise.resolve(null) : api.accountQuote(stay.id)]);
    const preview = currentQuote?.bill ?? detail;
    setQuote(currentQuote);
    if (currentStay.status === "closed") setClosedStayId(currentStay.id);
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

  useOperationalRefresh(async () => { if (!closedStayId && !busy) await refresh(); });
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
          <Select aria-label="Destino de reimpresión" value={reprintTarget ?? ""} onChange={e => setReprintTarget(e.target.value as "local" | "principal" || undefined)}><option value="">Destino guardado de este puesto</option><option value="local">Impresora local</option><option value="principal">Impresora de la principal</option></Select>
          <Button
            className="w-full"
            variant="secondary"
            onClick={async () => {
              setBusy(true);
              try { setPrintError(await api.reprintReceipt(closedStayId, reprintTarget)); }
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
      <div className={cn(
        "grid min-h-0 flex-1 grid-cols-1 overflow-hidden",
        !readOnly && "min-[800px]:grid-cols-[1.2fr_0.9fr]",
      )}>
        {readOnly ? null : (
          <div className="flex min-h-0 flex-col gap-4 overflow-hidden border-[var(--line)] p-6 min-[800px]:border-r">
            <div className="shrink-0 space-y-3">
              {overnight ? (
                <p className="text-sm text-[var(--accent)]">Se está aplicando tarifa de dormida</p>
              ) : null}
              {!stay.converted_to_overnight && stay.rate_kind === "hourly" ? (
                <div className="space-y-2">
                  <Button
                    variant="secondary"
                    className="w-full"
                    onClick={async () => {
                      setMutationBusy(true);
                      setError(null);
                      try {
                        await api.convertToOvernight(stay.id, quote?.version);
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
            </div>
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
          </div>
        )}
        <div className="flex min-h-0 flex-col p-6">
          <div className="min-h-0 flex-1 space-y-4 overflow-y-auto scrollbar-thin">
            {readOnly && overnight ? (
              <p className="text-sm text-[var(--accent)]">Se está aplicando tarifa de dormida</p>
            ) : null}
            <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">
              Cuenta
            </p>
            <ul className="space-y-2 text-sm">
              {lines
                .filter((line) => line.kind !== "surcharge" && line.kind !== "discount")
                .map((line, i) => (
                  <li key={`${line.description}-${i}`} className="flex justify-between gap-4">
                    <span>{line.description}</span>
                    <span className="font-mono font-medium tabular-nums">
                      {formatMoney(line.amount_cents, settings.currency_symbol)}
                    </span>
                  </li>
                ))}
            </ul>
            <StayCart
              stayId={stay.id}
              currency={settings.currency_symbol}
              charges={charges}
              readOnly={readOnly}
              onBusyChange={setMutationBusy}
              onChanged={async () => {
                await refresh();
                onChanged();
              }}
            />
            {admin && !readOnly ? (
              <div className="grid grid-cols-[1fr_100px_auto] gap-2 border-t border-[var(--line)] pt-4">
                <Input
                  ref={extraDescRef}
                  value={extraDesc}
                  onChange={(e) => setExtraDesc(e.target.value)}
                  placeholder="Otro cargo"
                />
                <Input
                  ref={extraAmountRef}
                  value={extraAmount}
                  onChange={(e) => setExtraAmount(e.target.value)}
                  placeholder="0"
                  inputMode="numeric"
                />
                <Button
                  variant="secondary"
                  onClick={async () => {
                    const description = requireTrimmed(extraDesc);
                    if (!description) {
                      reportInputIssue(extraDescRef.current, FIELD_EMPTY);
                      return;
                    }
                    const amount = parseMoneyInteger(extraAmount);
                    if (amount == null || amount === 0) {
                      reportInputIssue(extraAmountRef.current, "El importe del cargo debe ser distinto de cero.");
                      return;
                    }
                    setMutationBusy(true);
                    setError(null);
                    try {
                      await api.addCharge({
                        stay_id: stay.id,
                        kind: amount < 0 ? "discount" : "surcharge",
                        description,
                        amount_cents: amount,
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
              </div>
            ) : null}
          </div>
          <div className="mt-4 shrink-0 space-y-3 border-t border-[var(--line)] pt-4">
            <p className="font-mono text-3xl font-semibold tabular-nums tracking-tight">
              {formatMoney(total, settings.currency_symbol)}
            </p>
            {readOnly ? (
              <p className="text-sm text-[var(--muted)]">La cuenta se cierra en recepción.</p>
            ) : (
              <>
                <label className="flex items-center gap-2 text-sm">
                  <input type="checkbox" checked={print} onChange={(e) => setPrint(e.target.checked)} />
                  Imprimir ticket al cerrar
                </label>
                {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
                {printError ? <p className="text-sm text-[var(--warn)]">{printError}</p> : null}
                <Button
                  className="w-full"
                  variant="ok"
                  size="lg"
                  disabled={busy || mutationBusy || !quote}
                  onClick={async () => {
                    setBusy(true);
                    setError(null);
                    try {
                      const result = await api.checkOut({
                        stay_id: stay.id,
                        print,
                        quote_token: quote?.token,
                        expected_version: quote?.version,
                      });
                      setClosedStayId(result.stay.id);
                      setBill(result.bill.total_cents);
                      if (result.print_error) setPrintError(result.print_error);
                    } catch (e) {
                      setError(String(e));
                      await refresh().catch(() => undefined);
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  Cerrar cuenta
                </Button>
              </>
            )}
          </div>
        </div>
      </div>
    </Dialog>
  );
}
