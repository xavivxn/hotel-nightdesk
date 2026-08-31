---
name: nightdesk-design
description: >-
  Night Ops UI-UX for Nightdesk: tokens, IBM Plex, room status colors, stamps,
  cards, sidebar DutyClock, light/dark. Use when adding or changing pages,
  components, CSS, Button, Drawer, Field, RoomCard, AppShell, DutyClock,
  index.css, layout, or any visual work.
---

# Diseño Night Ops

Recepción = **mesa de control**, no landing de hotel. Teal de marca; el color de estado no es decoración.

Leer este skill **antes** de crear o restilar un componente. Tokens viven en `src/index.css` (`:root` + `.dark`). No hardcodear hex en JSX.

## Personalidad

- IBM Plex Sans (UI) + IBM Plex Mono (números, habitación, dinero, reloj, sellos).
- Radios `rounded-lg`. Bordes 1px `--line` salvo cards de habitación (2px + barra).
- Superficie plana. Sin glow, serif, oro, sombra boutique, ni `hover:-translate-y`.
- Copy UI en español (es-AR). Código en inglés.

## Tokens

Usar solo `var(--…)`. Si hace falta un color nuevo: mismo **hue** en claro y oscuro, distinta luminancia. Acento de marca ≠ estado.

| Token | Uso |
|-------|-----|
| `--bg` `--bg-2` `--surface` `--surface-2` | fondos |
| `--ink` `--muted` `--line` | texto y bordes |
| `--accent` `--accent-ink` `--accent-soft` | marca, nav activo, CTA primary, riel de día |
| `--ok` / `--ok-ink` / `--ok-soft` | libre |
| `--warn` / `--warn-ink` / `--warn-soft` | ocupada; riel turno noche (22:00–05:59) |
| `--dirty` / `--dirty-ink` / `--dirty-soft` | sucia (violeta, no ámbar) |
| `--info` / `--info-ink` / `--info-soft` | reservada |
| `--danger` / `--danger-ink` / `--danger-soft` | bloqueada / error |

`--*-soft` = fondos de aviso o sello, **no** el cuerpo de una room card.

## Tipografía

- Título de pantalla: `page-kicker` + `page-title` (definidos en `index.css`).
- Habitación, montos, duración, reloj: `font-mono tabular-nums`. Dinero solo con `formatMoney`.
- Labels de field: 11px, uppercase, tracking `0.08em`, `--muted`.
- No Fraunces / Figtree / `font-display` editorial.

## Habitaciones (tablero)

Fuente: `RoomCard` + clases `.room-*` en `index.css`.

- Cuerpo: `--surface`. Color **solo** en borde 2px y barra izquierda (`.room-card::before`, 0.7rem).
- Sello `.stamp` relleno (`bg` del estado + `*-ink`). Leyenda = `RoomStatusLegend`.
- Marca grande abajo-derecha (40px, ~80% opacity), **no** en ocupada (ahí hay huésped / timer / total):

| Estado | Color | Icono Lucide |
|--------|-------|--------------|
| `available` | `--ok` | `Check` |
| `occupied` | `--warn` | — (sello `Timer` nada más) |
| `dirty` | `--dirty` | `BrushCleaning` |
| `reserved` | `--info` | `CalendarClock` |
| `blocked` | `--danger` | `Ban` |

Sucia ≠ ocupada (violeta ≠ ámbar). Sucia ≠ error (no rojo). Rojo = bloqueada o fallo.

## Primitivos

Reusar, no reinventar:

| Pieza | Archivo | Regla |
|-------|---------|-------|
| Botón | `components/ui/Button.tsx` | `primary` / `secondary` / `ghost` / `danger` / `ok`. Un CTA fuerte por bloque. |
| Field | `components/ui/Field.tsx` | focus `border-accent` + `ring-2 ring-accent-soft`. |
| Drawer | `components/ui/Drawer.tsx` | panel derecho; título `text-xl font-semibold tracking-tight`. |
| Card genérica | `.card` | surface + line. `rounded-lg`. |
| Reloj | `layout/DutyClock.tsx` | no otro reloj. Flip solo HH/MM; SS estático; riel 2px. |
| Shell | `layout/AppShell.tsx` | nav; activo = `accent-soft` + texto `accent`. |

`ok` = cobrar/cerrar. `danger` = destructivo. Primary = una acción por pantalla.

## Layout de pantalla

```
px-6 py-6 lg:px-8
header: kicker + page-title | acciones a la derecha
```

Tablas: thead `surface-2`, headers uppercase muted. Habitación y totales en mono.

## Movimiento

- Cards: `hover:bg-surface-2`, sin lift.
- DutyClock: flip 180ms en hora/minuto; colon late; `prefers-reduced-motion` apaga flip y pulso.
- Drawer: ok entrar/salir. No animar el grid del tablero.
- Touch ≥ 44px.

## Checklist componente nuevo

1. ¿Tokens `var(--*)` en claro y oscuro? ¿Cero hex sueltos?
2. ¿Sans para texto, mono para números?
3. ¿Color = semántica (marca o estado), no adorno?
4. ¿Reusa Button / Field / Drawer / `.card` / `.stamp`?
5. ¿Un primary? ¿Empty/error en español?
6. ¿Se ve en **los dos temas**? Verificar ambos.

Prohibido: Fraunces, oro, fill de card de habitación, `--warn` para sucia, `--accent` como estado de room, store global, `invoke` desde UI.
