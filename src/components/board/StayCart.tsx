import { api } from "@/lib/api";
import { formatMoney } from "@/lib/format";
import type { Charge, Product } from "@/lib/types";
import { Minus, Plus, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

type ChargeGroup = {
  key: string;
  description: string;
  kind: string;
  unitCents: number;
  ids: number[];
};

function groupCharges(charges: Charge[]): ChargeGroup[] {
  const groups: ChargeGroup[] = [];
  for (const charge of charges) {
    if (charge.kind !== "surcharge" && charge.kind !== "discount") continue;
    const key = `${charge.kind}\0${charge.description}\0${charge.amount_cents}`;
    const existing = groups.find((g) => g.key === key);
    if (existing) {
      existing.ids.push(charge.id);
    } else {
      groups.push({
        key,
        description: charge.description,
        kind: charge.kind,
        unitCents: charge.amount_cents,
        ids: [charge.id],
      });
    }
  }
  return groups;
}

export function StayCart({
  stayId,
  currency,
  charges,
  onChanged,
  onBusyChange,
}: {
  stayId: number;
  currency: string;
  charges: Charge[];
  onChanged: () => Promise<void> | void;
  onBusyChange?: (busy: boolean) => void;
}) {
  const [products, setProducts] = useState<Product[]>([]);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.listProducts(true)
      .then(setProducts)
      .catch((e) => setError(String(e)));
  }, []);

  const groups = useMemo(() => groupCharges(charges), [charges]);

  async function run(busyKey: number, action: () => Promise<void>) {
    setBusyId(busyKey);
    onBusyChange?.(true);
    setError(null);
    try {
      await action();
      await onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyId(null);
      onBusyChange?.(false);
    }
  }

  async function increment(group: ChargeGroup) {
    const product = products.find(
      (p) => p.name === group.description && p.price_cents === group.unitCents,
    );
    if (product) {
      await run(-group.ids[0], () =>
        api.addProductCharge({ stay_id: stayId, product_id: product.id }),
      );
      return;
    }
    await run(-group.ids[0], () =>
      api.addCharge({
        stay_id: stayId,
        kind: group.kind,
        description: group.description,
        amount_cents: group.unitCents,
      }),
    );
  }

  async function decrement(group: ChargeGroup) {
    const id = group.ids[group.ids.length - 1];
    await run(-id, () => api.deleteCharge(id));
  }

  async function removeGroup(group: ChargeGroup) {
    await run(-group.ids[0], async () => {
      for (const id of group.ids) {
        await api.deleteCharge(id);
      }
    });
  }

  const busy = busyId !== null;

  return (
    <div className="space-y-2">
      <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">
        Consumos
      </p>
      {groups.length === 0 ? (
        <p className="text-sm text-[var(--muted)]">Sin consumos</p>
      ) : (
        <ul className="space-y-1.5">
          {groups.map((group) => {
            const lineTotal = group.unitCents * group.ids.length;
            return (
              <li key={group.key} className="flex items-center gap-3 text-sm">
                <span className="min-w-0 flex-1 truncate">
                  {group.description}
                  <span className="text-[var(--muted)]">
                    {" "}
                    · {formatMoney(lineTotal, currency)}
                  </span>
                </span>
                <div className="flex shrink-0 items-center gap-0.5">
                  <button
                    type="button"
                    aria-label="Restar"
                    disabled={busy}
                    onClick={() => decrement(group)}
                    className="flex h-7 w-7 items-center justify-center rounded-lg text-[var(--ink)] hover:bg-[var(--surface-2)] disabled:opacity-50"
                  >
                    <Minus className="h-3.5 w-3.5" strokeWidth={2.5} />
                  </button>
                  <span className="min-w-[1.5rem] text-center font-mono text-sm font-semibold tabular-nums">
                    {group.ids.length}
                  </span>
                  <button
                    type="button"
                    aria-label="Sumar"
                    disabled={busy}
                    onClick={() => increment(group)}
                    className="flex h-7 w-7 items-center justify-center rounded-lg text-[var(--ink)] hover:bg-[var(--surface-2)] disabled:opacity-50"
                  >
                    <Plus className="h-3.5 w-3.5" strokeWidth={2.5} />
                  </button>
                  <button
                    type="button"
                    aria-label="Quitar"
                    disabled={busy}
                    onClick={() => removeGroup(group)}
                    className="ml-1 flex h-7 w-7 items-center justify-center rounded-lg text-[var(--danger)] hover:bg-[var(--danger-soft)] disabled:opacity-50"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
      )}
      {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
    </div>
  );
}
