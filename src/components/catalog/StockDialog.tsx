import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Field, Input } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import type { Product, ProductStock, StockMovement, StockMovementReason } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useEffect, useState } from "react";

const REASONS: Record<StockMovementReason, string> = {
  sale: "Venta",
  void: "Venta anulada",
  restock: "Llegó mercadería",
  count: "Conteo físico",
};

/** Whole units: "24", "1.000" or "1 000". Rejects decimals and signs. */
function parseUnits(value: string, min: number) {
  const digits = value.replace(/[\s.]/g, "");
  if (!/^\d+$/.test(digits)) return null;
  const units = Number(digits);
  return units >= min && units <= 100_000 ? units : null;
}

export function stockLevel(stock: ProductStock | undefined) {
  if (!stock) return "untracked" as const;
  if (stock.quantity <= 0) return "out" as const;
  if (stock.quantity <= stock.min_quantity) return "low" as const;
  return "ok" as const;
}

export function StockBadge({ stock, className }: { stock: ProductStock | undefined; className?: string }) {
  const level = stockLevel(stock);
  if (!stock) return <span className={cn("text-xs text-[var(--muted)]", className)}>Sin control</span>;
  return (
    <span
      className={cn(
        "rounded-md px-2 py-0.5 font-mono text-xs font-semibold tabular-nums",
        level === "out" && "bg-[var(--danger-soft)] text-[var(--danger)]",
        level === "low" && "bg-[var(--warn-soft)] text-[var(--warn)]",
        level === "ok" && "bg-[var(--surface-2)] text-[var(--ink)]",
        className,
      )}
      title={stock.min_quantity > 0 ? `Aviso con ${stock.min_quantity} u.` : undefined}
    >
      {stock.quantity} u.
    </span>
  );
}

export function StockDialog({
  product,
  stock,
  onClose,
  onSaved,
}: {
  product: Product | null;
  stock: ProductStock | undefined;
  onClose: () => void;
  onSaved: () => Promise<void> | void;
}) {
  const [mode, setMode] = useState<"add" | "set">("add");
  const [quantity, setQuantity] = useState("");
  const [minimum, setMinimum] = useState("");
  const [note, setNote] = useState("");
  const [movements, setMovements] = useState<StockMovement[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!product) return;
    setMode(stock ? "add" : "set");
    setQuantity("");
    setMinimum(String(stock?.min_quantity ?? 0));
    setNote("");
    setError(null);
    setMovements([]);
    api.listStockMovements(product.id).then(setMovements).catch(() => setMovements([]));
    // Reset only when another product opens; a refreshed `stock` keeps what is being typed.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [product?.id]);

  if (!product) return null;

  async function save(nextMode: "add" | "set" | "untrack") {
    if (!product) return;
    const amount = nextMode === "untrack" ? 0 : parseUnits(quantity, nextMode === "add" ? 1 : 0);
    if (amount == null) {
      setError(nextMode === "add" ? "Ingresá cuántas unidades llegaron (1 a 100.000)." : "Ingresá cuántas unidades contaste (0 a 100.000).");
      return;
    }
    const min = parseUnits(minimum || "0", 0);
    if (min == null) {
      setError("El aviso de stock bajo va de 0 a 100.000 unidades.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.updateProductStock({
        product_id: product.id,
        mode: nextMode,
        quantity: amount,
        min_quantity: min,
        note: note.trim() || null,
      });
      await onSaved();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const subtitle = stock
    ? `Hay ${stock.quantity} u. · aviso con ${stock.min_quantity} u.`
    : "Todavía no se controla su stock. Cargá un conteo para empezar.";

  return (
    <Dialog open title={`Stock · ${product.name}`} subtitle={subtitle} onClose={() => !busy && onClose()}>
      <form
        className="space-y-4"
        onSubmit={(e) => {
          e.preventDefault();
          void save(mode);
        }}
      >
        <div className="grid grid-cols-2 gap-1 rounded-lg bg-[var(--surface-2)] p-1" role="radiogroup" aria-label="Tipo de movimiento">
          {([
            ["add", "Llegó mercadería"],
            ["set", "Conteo físico"],
          ] as const).map(([value, label]) => (
            <button
              key={value}
              type="button"
              role="radio"
              aria-checked={mode === value}
              disabled={value === "add" && !stock}
              onClick={() => setMode(value)}
              className={cn(
                "min-h-11 rounded-md px-3 text-sm font-semibold disabled:opacity-40",
                mode === value ? "bg-[var(--surface)] text-[var(--ink)]" : "text-[var(--muted)] hover:text-[var(--ink)]",
              )}
            >
              {label}
            </button>
          ))}
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Field label={mode === "add" ? "Unidades que llegaron" : "Unidades contadas"}>
            <Input autoFocus inputMode="numeric" value={quantity} onChange={(e) => setQuantity(e.target.value)} placeholder={mode === "add" ? "24" : "0"} />
          </Field>
          <Field label="Avisar cuando queden">
            <Input inputMode="numeric" value={minimum} onChange={(e) => setMinimum(e.target.value)} />
          </Field>
        </div>
        <Field label="Nota (opcional)">
          <Input value={note} maxLength={200} onChange={(e) => setNote(e.target.value)} placeholder="Ej. factura 123, conteo de cierre" />
        </Field>
        <p className="text-xs text-[var(--muted)]">
          Cada venta de la Tienda descuenta una unidad y anular el consumo la devuelve. Si llega a 0, la venta se permite igual y queda en rojo para revisar.
        </p>
        {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
        <div className="flex flex-wrap items-center justify-between gap-2 pt-1">
          {stock ? (
            <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={() => void save("untrack")}>
              Dejar de controlar
            </Button>
          ) : <span />}
          <div className="flex gap-2">
            <Button type="button" variant="secondary" disabled={busy} onClick={onClose}>Cancelar</Button>
            <Button type="submit" disabled={busy}>{busy ? "Guardando…" : "Guardar"}</Button>
          </div>
        </div>
      </form>
      {movements.length ? (
        <div className="mt-6 border-t border-[var(--line)] pt-4">
          <p className="mb-2 text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--muted)]">Últimos movimientos</p>
          <ul className="divide-y divide-[var(--line)] text-sm">
            {movements.map((movement) => (
              <li key={movement.id} className="flex items-start justify-between gap-3 py-2">
                <div className="min-w-0">
                  <p className="font-medium">{REASONS[movement.reason] ?? movement.reason}</p>
                  <p className="truncate text-xs text-[var(--muted)]">
                    {formatDateTime(movement.created_at)}
                    {movement.username ? ` · ${movement.username}` : ""}
                    {movement.note ? ` · ${movement.note}` : ""}
                  </p>
                </div>
                <div className="shrink-0 text-right font-mono text-xs tabular-nums">
                  <p className={cn("font-semibold", movement.delta < 0 ? "text-[var(--danger)]" : "text-[var(--ok)]")}>
                    {movement.delta > 0 ? `+${movement.delta}` : movement.delta}
                  </p>
                  <p className="text-[var(--muted)]">quedan {movement.quantity_after}</p>
                </div>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </Dialog>
  );
}
