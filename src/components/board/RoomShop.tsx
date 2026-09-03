import { api } from "@/lib/api";
import { formatMoney } from "@/lib/format";
import { PRODUCT_CATEGORIES, type ProductCategory } from "@/lib/products";
import type { Charge, Product } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useEffect, useMemo, useState } from "react";

export function RoomShop({
  stayId,
  currency,
  charges,
  onChanged,
}: {
  stayId: number;
  currency: string;
  charges: Charge[];
  onChanged: () => Promise<void> | void;
}) {
  const [products, setProducts] = useState<Product[]>([]);
  const [category, setCategory] = useState<ProductCategory>("bebidas");
  const [busyId, setBusyId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.listProducts(true)
      .then(setProducts)
      .catch((e) => setError(String(e)));
  }, []);

  const counts = useMemo(() => {
    const map = new Map<string, number>();
    for (const charge of charges) {
      if (charge.kind !== "surcharge") continue;
      map.set(charge.description, (map.get(charge.description) ?? 0) + 1);
    }
    return map;
  }, [charges]);

  const shopTotal = useMemo(
    () => charges.filter((c) => c.kind === "surcharge").reduce((sum, c) => sum + c.amount_cents, 0),
    [charges],
  );

  const visible = products.filter((p) => p.category === category);

  async function addProduct(product: Product) {
    setBusyId(product.id);
    setError(null);
    try {
      await api.addProductCharge({ stay_id: stayId, product_id: product.id });
      await onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyId(null);
    }
  }

  return (
    <div className="space-y-3">
      <div className="flex items-end justify-between gap-3">
        <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">Tienda</p>
        {shopTotal > 0 ? (
          <p className="font-mono text-sm font-semibold tabular-nums">{formatMoney(shopTotal, currency)}</p>
        ) : null}
      </div>
      <div className="flex flex-wrap gap-1.5">
        {PRODUCT_CATEGORIES.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setCategory(item.id)}
            className={cn(
              "rounded-lg px-2.5 py-1.5 text-[11px] font-semibold uppercase tracking-[0.06em]",
              category === item.id
                ? "bg-[var(--accent)] text-[var(--accent-ink)]"
                : "bg-[var(--surface-2)] text-[var(--muted)] hover:text-[var(--ink)]",
            )}
          >
            {item.label}
          </button>
        ))}
      </div>
      <div className="grid max-h-[220px] grid-cols-2 gap-2 overflow-y-auto pr-1 scrollbar-thin sm:grid-cols-3">
        {visible.map((product) => {
          const qty = counts.get(product.name) ?? 0;
          return (
            <button
              key={product.id}
              type="button"
              disabled={busyId === product.id}
              onClick={() => addProduct(product)}
              className={cn(
                "rounded-lg border border-[var(--line)] bg-[var(--bg)] px-3 py-2.5 text-left transition-colors hover:bg-[var(--surface-2)]",
                qty > 0 && "border-[var(--accent)] bg-[var(--accent-soft)]",
              )}
            >
              <div className="flex items-start justify-between gap-2">
                <span className="text-sm font-semibold leading-tight">{product.name}</span>
                {qty > 0 ? (
                  <span className="font-mono text-[11px] font-bold tabular-nums text-[var(--accent)]">×{qty}</span>
                ) : null}
              </div>
              <span className="mt-1 block font-mono text-xs tabular-nums text-[var(--muted)]">
                {formatMoney(product.price_cents, currency)}
              </span>
            </button>
          );
        })}
      </div>
      {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
    </div>
  );
}
