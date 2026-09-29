import { api } from "@/lib/api";
import { formatMoney } from "@/lib/format";
import { PRODUCT_CATEGORIES, type ProductCategory } from "@/lib/products";
import type { Charge, Product, ProductStock } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useEffect, useMemo, useState } from "react";

export function RoomShop({
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
  const [stock, setStock] = useState<Map<number, ProductStock>>(new Map());
  const [category, setCategory] = useState<ProductCategory>("bebidas");
  const [busyId, setBusyId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function loadStock() {
    const rows = await api.listProductStock();
    setStock(new Map(rows.map((row) => [row.product_id, row])));
  }

  useEffect(() => {
    api.listProducts(true)
      .then(setProducts)
      .catch((e) => setError(String(e)));
    // Stock is informative here: a failure must not hide the shop.
    loadStock().catch(() => undefined);
  }, []);

  const counts = useMemo(() => {
    const map = new Map<string, number>();
    for (const charge of charges) {
      if (charge.kind !== "surcharge") continue;
      map.set(charge.description, (map.get(charge.description) ?? 0) + 1);
    }
    return map;
  }, [charges]);

  const visible = products.filter((p) => p.category === category);

  async function addProduct(product: Product) {
    setBusyId(product.id);
    onBusyChange?.(true);
    setError(null);
    try {
      await api.addProductCharge({ stay_id: stayId, product_id: product.id });
      await onChanged();
      await loadStock().catch(() => undefined);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyId(null);
      onBusyChange?.(false);
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <p className="shrink-0 text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">
        Tienda
      </p>
      <div className="flex shrink-0 flex-wrap gap-1.5">
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
      <div className="grid min-h-0 flex-1 grid-cols-2 content-start gap-2 overflow-y-auto pr-1 scrollbar-thin sm:grid-cols-3">
        {visible.map((product) => {
          const qty = counts.get(product.name) ?? 0;
          const left = stock.get(product.id);
          return (
            <button
              key={product.id}
              type="button"
              disabled={busyId !== null}
              onClick={() => addProduct(product)}
              className={cn(
                "rounded-lg border border-[var(--line)] bg-[var(--bg)] px-3 py-2.5 text-left transition-colors hover:bg-[var(--surface-2)]",
                qty > 0 && "border-[var(--accent)] bg-[var(--accent-soft)]",
              )}
            >
              <div className="flex items-start justify-between gap-2">
                <span className="text-sm font-semibold leading-tight">{product.name}</span>
                {qty > 0 ? (
                  <span className="font-mono text-[11px] font-bold tabular-nums text-[var(--accent)]">
                    ×{qty}
                  </span>
                ) : null}
              </div>
              <span className="mt-1 flex items-center justify-between gap-2 font-mono text-xs tabular-nums text-[var(--muted)]">
                {formatMoney(product.price_cents, currency)}
                {left ? (
                  <span
                    className={cn(
                      left.quantity <= 0
                        ? "font-semibold text-[var(--danger)]"
                        : left.quantity <= left.min_quantity && "font-semibold text-[var(--warn)]",
                    )}
                  >
                    {left.quantity <= 0 ? "Sin stock" : `Quedan ${left.quantity}`}
                  </span>
                ) : null}
              </span>
            </button>
          );
        })}
      </div>
      {error ? <p className="shrink-0 text-sm text-[var(--danger)]">{error}</p> : null}
    </div>
  );
}
