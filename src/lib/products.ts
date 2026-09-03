/** In-room shop catalog. Prices are provisional Gs. (fields named *_cents). */

export type ProductCategory =
  | "bebidas"
  | "snacks"
  | "tabaco"
  | "higiene"
  | "adulto"
  | "licores";

export type Product = {
  id: number;
  name: string;
  category: ProductCategory;
  price_cents: number;
  active: boolean;
  sort_order: number;
};

export const PRODUCT_CATEGORIES: { id: ProductCategory; label: string }[] = [
  { id: "bebidas", label: "Bebidas" },
  { id: "snacks", label: "Snacks" },
  { id: "tabaco", label: "Tabaco" },
  { id: "higiene", label: "Higiene" },
  { id: "adulto", label: "Adulto" },
  { id: "licores", label: "Licores" },
];

/** Seed rows without ids — used by mock and mirrored in Rust seed. */
export const PRODUCT_SEED: Array<{
  name: string;
  category: ProductCategory;
  price_cents: number;
}> = [
  // Bebidas
  { name: "Agua", category: "bebidas", price_cents: 5_000 },
  { name: "Coca", category: "bebidas", price_cents: 8_000 },
  { name: "Pulp", category: "bebidas", price_cents: 8_000 },
  { name: "Fanta", category: "bebidas", price_cents: 8_000 },
  { name: "Tónica", category: "bebidas", price_cents: 7_000 },
  { name: "Del valle", category: "bebidas", price_cents: 10_000 },
  { name: "Energy", category: "bebidas", price_cents: 12_000 },
  { name: "Power", category: "bebidas", price_cents: 12_000 },
  { name: "Bud 66", category: "bebidas", price_cents: 12_000 },
  { name: "Skol", category: "bebidas", price_cents: 12_000 },
  { name: "Smirnoff", category: "bebidas", price_cents: 18_000 },
  // Snacks
  { name: "Beldent", category: "snacks", price_cents: 5_000 },
  { name: "Halls", category: "snacks", price_cents: 5_000 },
  { name: "Papa", category: "snacks", price_cents: 10_000 },
  { name: "Gullón", category: "snacks", price_cents: 8_000 },
  { name: "Turrón", category: "snacks", price_cents: 8_000 },
  { name: "Bonbon", category: "snacks", price_cents: 7_000 },
  { name: "Chocolate", category: "snacks", price_cents: 10_000 },
  // Tabaco
  { name: "Kent Conv.", category: "tabaco", price_cents: 18_000 },
  { name: "Lucky", category: "tabaco", price_cents: 18_000 },
  { name: "Encendedor", category: "tabaco", price_cents: 8_000 },
  // Higiene
  { name: "Crema D.", category: "higiene", price_cents: 10_000 },
  { name: "Cepillo D.", category: "higiene", price_cents: 8_000 },
  { name: "Baño E.", category: "higiene", price_cents: 12_000 },
  { name: "Gel Pant.", category: "higiene", price_cents: 15_000 },
  { name: "Prestobarba", category: "higiene", price_cents: 12_000 },
  // Adulto
  { name: "Prime", category: "adulto", price_cents: 15_000 },
  { name: "Control", category: "adulto", price_cents: 15_000 },
  { name: "Lubricante", category: "adulto", price_cents: 25_000 },
  { name: "Prot. 100", category: "adulto", price_cents: 20_000 },
  { name: "Prot. 150", category: "adulto", price_cents: 30_000 },
  { name: "Prot. 200", category: "adulto", price_cents: 40_000 },
  { name: "Capa P.", category: "adulto", price_cents: 25_000 },
  { name: "Agrandador", category: "adulto", price_cents: 35_000 },
  { name: "Anillo v.", category: "adulto", price_cents: 45_000 },
  { name: "Estimulador", category: "adulto", price_cents: 50_000 },
  { name: "Fantasía", category: "adulto", price_cents: 60_000 },
  // Licores
  { name: "Quinta", category: "licores", price_cents: 45_000 },
  { name: "Sta. Helena", category: "licores", price_cents: 50_000 },
  { name: "Monje", category: "licores", price_cents: 55_000 },
  { name: "Johnnie W.", category: "licores", price_cents: 180_000 },
];

export function buildSeedProducts(): Product[] {
  return PRODUCT_SEED.map((item, index) => ({
    id: index + 1,
    name: item.name,
    category: item.category,
    price_cents: item.price_cents,
    active: true,
    sort_order: index + 1,
  }));
}
