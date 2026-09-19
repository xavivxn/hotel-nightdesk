import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Field, Input, Select } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { formatMoney } from "@/lib/format";
import { PRODUCT_CATEGORIES, type ProductCategory } from "@/lib/products";
import type { AppSettings, Product } from "@/lib/types";
import { cn } from "@/lib/utils";
import { Package, Pencil, Plus, Power, Search, ShieldCheck, Wifi, WifiOff } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

type ProductForm = {
  id: number | null;
  name: string;
  category: ProductCategory;
  price: string;
  active: boolean;
  sort_order: number;
  version: number;
};

const emptyForm: ProductForm = {
  id: null,
  name: "",
  category: "bebidas",
  price: "",
  active: true,
  sort_order: 0,
  version: 1,
};

function categoryLabel(category: string) {
  return PRODUCT_CATEGORIES.find((item) => item.id === category)?.label ?? category;
}

function parseIntegerGuaranies(value: string) {
  const normalized = value.replace(/^Gs\.?/i, "").replace(/[\s.]/g, "").replace(/,/g, "").trim();
  if (!/^\d+$/.test(normalized)) return null;
  const amount = Number(normalized);
  return Number.isSafeInteger(amount) && amount > 0 ? amount : null;
}

function inputPrice(value: number) {
  return value > 0 ? value.toLocaleString("es-PY") : "";
}

export function CatalogPage({ settings }: { settings: AppSettings }) {
  const [products, setProducts] = useState<Product[]>([]);
  const [category, setCategory] = useState<"all" | ProductCategory>("all");
  const [status, setStatus] = useState<"all" | "active" | "inactive">("active");
  const [query, setQuery] = useState("");
  const [form, setForm] = useState<ProductForm | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function load() {
    setProducts(await api.listProducts(false));
  }

  useEffect(() => {
    load().catch((e) => setError(String(e)));
  }, []);

  const filtered = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return products.filter((product) => {
      if (category !== "all" && product.category !== category) return false;
      if (status === "active" && !product.active) return false;
      if (status === "inactive" && product.active) return false;
      return !normalizedQuery || `${product.name} ${product.category}`.toLocaleLowerCase().includes(normalizedQuery);
    });
  }, [category, products, query, status]);

  const activeCount = products.filter((product) => product.active).length;
  const inactiveCount = products.length - activeCount;

  function edit(product: Product) {
    setError(null);
    setNotice(null);
    setForm({
      id: product.id,
      name: product.name,
      category: (PRODUCT_CATEGORIES.some((item) => item.id === product.category) ? product.category : "bebidas") as ProductCategory,
      price: inputPrice(product.price_cents),
      active: product.active,
      sort_order: product.sort_order,
      version: product.version,
    });
  }

  async function saveProduct() {
    if (!form) return;
    const name = form.name.trim();
    const price = parseIntegerGuaranies(form.price);
    if (!name) {
      setError("El nombre del producto es obligatorio.");
      return;
    }
    if (!price) {
      setError("El precio debe ser un número entero mayor que 0 Gs. Ejemplo: 15.000");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.saveProduct({
        id: form.id,
        name,
        category: form.category,
        price_cents: price,
        active: form.active,
        sort_order: form.sort_order || undefined,
        expected_version: form.id ? form.version : null,
      });
      await load();
      setForm(null);
      setNotice(form.id ? "Producto actualizado en este equipo." : "Producto agregado al catálogo local.");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function toggle(product: Product) {
    setBusy(true);
    setError(null);
    try {
      await api.setProductActive(product.id, !product.active);
      await load();
      setNotice(product.active ? `${product.name} quedó inactivo. Sus cargos históricos se conservan.` : `${product.name} volvió a estar activo.`);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="px-6 py-6 lg:px-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="page-kicker">Administración · N11</p>
          <h1 className="page-title">Catálogo de consumos</h1>
          <p className="page-description">Productos, categorías y precios que recepción puede cargar en una cuenta.</p>
        </div>
        <Button onClick={() => { setError(null); setNotice(null); setForm({ ...emptyForm }); }}>
          <Plus size={17} /> Nuevo producto
        </Button>
      </header>

      <div className="mt-6 grid gap-3 sm:grid-cols-3">
        <div className="card rounded-lg p-4"><p className="page-kicker">Total</p><p className="mt-1 font-mono text-2xl tabular-nums">{products.length}</p><p className="mt-1 text-xs text-[var(--muted)]">Productos registrados</p></div>
        <div className="card rounded-lg p-4"><p className="page-kicker">Activos</p><p className="mt-1 font-mono text-2xl tabular-nums text-[var(--ok)]">{activeCount}</p><p className="mt-1 text-xs text-[var(--muted)]">Visibles en recepción</p></div>
        <div className="card rounded-lg p-4"><p className="page-kicker">Inactivos</p><p className="mt-1 font-mono text-2xl tabular-nums text-[var(--muted)]">{inactiveCount}</p><p className="mt-1 text-xs text-[var(--muted)]">Conservados para el historial</p></div>
      </div>

      <section className="card mt-6 rounded-lg p-4">
        <div className="flex flex-wrap items-center gap-3">
          <label className="relative min-w-[220px] flex-1">
            <span className="sr-only">Buscar producto</span>
            <Search size={16} className="pointer-events-none absolute left-3 top-3.5 text-[var(--muted)]" />
            <Input className="pl-9" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Buscar por nombre o categoría" />
          </label>
          <Select className="w-auto min-w-[150px]" value={category} onChange={(e) => setCategory(e.target.value as "all" | ProductCategory)}>
            <option value="all">Todas las categorías</option>
            {PRODUCT_CATEGORIES.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}
          </Select>
          <Select className="w-auto min-w-[130px]" value={status} onChange={(e) => setStatus(e.target.value as "all" | "active" | "inactive")}>
            <option value="active">Activos</option>
            <option value="inactive">Inactivos</option>
            <option value="all">Todos</option>
          </Select>
        </div>
        <div className="mt-4 divide-y divide-[var(--line)]">
          {filtered.map((product) => (
            <div key={product.id} className="flex flex-wrap items-center justify-between gap-3 py-3 first:pt-1 last:pb-1">
              <div className="flex min-w-0 items-center gap-3">
                <div className={cn("grid h-10 w-10 shrink-0 place-items-center rounded-lg", product.active ? "bg-[var(--accent-soft)] text-[var(--accent)]" : "bg-[var(--surface-2)] text-[var(--muted)]")}><Package size={18} /></div>
                <div className="min-w-0"><p className={cn("truncate font-semibold", !product.active && "text-[var(--muted)]")}>{product.name}</p><p className="text-xs text-[var(--muted)]">{categoryLabel(product.category)} · {product.active ? "Activo" : "Inactivo"}</p></div>
              </div>
              <div className="flex items-center gap-3"><span className="font-mono text-sm font-semibold tabular-nums">{formatMoney(product.price_cents, settings.currency_symbol)}</span><Button size="sm" variant="ghost" aria-label={`Editar ${product.name}`} onClick={() => edit(product)}><Pencil size={15} /> Editar</Button><Button size="sm" variant={product.active ? "danger" : "secondary"} disabled={busy} onClick={() => toggle(product)}><Power size={15} /> {product.active ? "Desactivar" : "Activar"}</Button></div>
            </div>
          ))}
          {filtered.length === 0 ? <p className="py-8 text-center text-sm text-[var(--muted)]">No hay productos para esos filtros.</p> : null}
        </div>
      </section>

      <div className="mt-4 grid gap-3 md:grid-cols-2">
        <div className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4"><div className="flex items-center gap-2 font-semibold"><ShieldCheck size={17} className="text-[var(--accent)]" /> Permisos y trazabilidad</div><p className="mt-2 text-sm text-[var(--muted)]">Este menú corresponde al rol administrador. Desactivar aplica baja lógica: nunca se borra un producto usado en cuentas.</p></div>
        <div className="rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4"><div className="flex items-center gap-2 font-semibold"><Wifi size={17} className="text-[var(--accent)]" /> Operación local y remota</div><p className="mt-2 text-sm text-[var(--muted)]">La recepción sigue funcionando sin internet. La publicación remota se autentica contra la API privada únicamente cuando WireGuard está conectado.</p><div className="mt-3 flex items-center gap-2 text-xs text-[var(--muted)]"><WifiOff size={14} /> Sin acceso directo a la SQLite de recepción</div></div>
      </div>

      {error ? <p className="mt-4 text-sm text-[var(--danger)]">{error}</p> : null}
      {notice ? <p className="mt-4 text-sm text-[var(--muted)]">{notice}</p> : null}

      <Dialog open={Boolean(form)} title={form?.id ? "Editar producto" : "Nuevo producto"} subtitle="Los precios se guardan como guaraníes enteros." onClose={() => !busy && setForm(null)}>
        {form ? <form className="space-y-4" onSubmit={(e) => { e.preventDefault(); void saveProduct(); }}>
          <Field label="Nombre"><Input autoFocus value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="Ej. Agua mineral" /></Field>
          <Field label="Categoría"><Select value={form.category} onChange={(e) => setForm({ ...form, category: e.target.value as ProductCategory })}>{PRODUCT_CATEGORIES.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</Select></Field>
          <Field label="Precio (Gs.)"><Input inputMode="numeric" value={form.price} onChange={(e) => setForm({ ...form, price: e.target.value })} placeholder="15.000" aria-describedby="price-help" /><span id="price-help" className="mt-1 block text-xs text-[var(--muted)]">Usá números enteros, con o sin separador de miles.</span></Field>
          <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={form.active} onChange={(e) => setForm({ ...form, active: e.target.checked })} /> Disponible para recepción</label>
          <div className="flex justify-end gap-2 pt-2"><Button type="button" variant="secondary" disabled={busy} onClick={() => setForm(null)}>Cancelar</Button><Button type="submit" disabled={busy}>{busy ? "Guardando…" : "Guardar producto"}</Button></div>
        </form> : null}
      </Dialog>
    </div>
  );
}
