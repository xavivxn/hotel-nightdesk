# N11 — Catálogo de consumos para administración

La pantalla **Catálogo admin** permite mantener el catálogo que usa recepción: buscar por nombre, filtrar por categoría y estado, crear productos, editar nombre/categoría/precio y aplicar baja lógica mediante activar/desactivar.

## Reglas de negocio

- El precio se guarda como un entero positivo en guaraníes (`price_cents` en el contrato existente, sin decimales).
- Un producto inactivo no aparece en el selector de recepción y no puede generar nuevos cargos.
- Desactivar un producto no elimina filas ni cargos anteriores. Cada cargo conserva la descripción y el precio aplicado al momento de agregarlo; el cierre persiste esas líneas para el historial.
- La base local SQLite es la fuente de operación de recepción y sigue disponible sin internet.

## Contrato privado

La UI usa los comandos de dominio `list_products(active_only)`, `save_product(payload)` y `set_product_active(product_id, active)`. La misma forma de datos debe exponerse en la API privada para la aplicación de administración:

```json
{
  "id": 12,
  "name": "Agua mineral",
  "category": "bebidas",
  "price_cents": 15000,
  "active": true,
  "sort_order": 42
}
```

Las escrituras remotas van a Supabase (`catalog_write` → `catalog_upsert_*` con `expected_version`; ver [arquitectura](arquitectura-offline-supabase.md) y [contrato](contrato-ipc-api.md)). La aplicación administradora no accede al archivo SQLite de recepción. Sin conexión con administración la edición de catálogo se rechaza y recepción sigue operando. N11 dejó el menú local; I11/N07 cierran el write-through.

## Revisión de uso

1. Abrir **Catálogo admin** y comprobar los contadores de activos/inactivos.
2. Crear un producto con precio `15.000`; probar que un precio vacío, decimal o cero muestra un error claro.
3. Editar nombre, categoría y precio; verificar que el cambio aparece al recargar.
4. Desactivar el producto; comprobar que desaparece de **Tienda** en una cuenta abierta.
5. Reactivarlo y agregarlo a una cuenta; cerrar la cuenta y confirmar que el historial conserva descripción y precio aunque el catálogo se vuelva a editar.
6. Repetir la lectura/escritura desde el modo Administración remota (Supabase) y comprobar que sin conectividad sólo falla la edición de catálogo, no el flujo local de recepción.
