# Nightdesk

Sistema local de recepción para hotel/motel: tablero de habitaciones, check-in por hora o noche, cobro al checkout e impresión de tickets ESC/POS. Tauri + React + SQLite, 100% offline.

Pensado para el dueño o el personal de recepción. Los datos quedan en el equipo: no hace falta internet ni un servidor en la nube.

## Qué incluye el MVP

- Tablero visual de habitaciones (libre, ocupada, sucia, bloqueada, reservada)
- Check-in walk-in con tarifa por hora, por noche o pernocte
- Cuenta en vivo, recargos/descuentos y cobro al checkout (efectivo, tarjeta o transferencia)
- Reservas simples: en espera, check-in, cancelar o no-show
- Historial del día y reimpresión de ticket
- Tema claro/oscuro y PIN opcional de desbloqueo
- Impresión ESC/POS (58 mm / 80 mm); si la impresora falla, el cobro igual se cierra

## Stack

- **App:** [Tauri 2](https://v2.tauri.app/)
- **UI:** React, TypeScript, Vite, Tailwind CSS
- **Datos:** SQLite (en el directorio de datos de la app)
- **Cobro e impresión:** Rust (`rusqlite` + bytes ESC/POS)

## Requisitos

- Node.js 20+
- Rust (rustup, toolchain stable)
- En macOS: Xcode Command Line Tools
- En Windows: [Microsoft Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) y [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)

## Cómo correrlo

```bash
npm install
npm run tauri dev
```

Eso abre la app de escritorio con SQLite local.

Solo la interfaz en el navegador (datos de prueba en el propio browser, sin Tauri):

```bash
npm run dev
```

Luego abrí [http://localhost:1420](http://localhost:1420).

## Build

Instalador de producción (en la misma plataforma donde lo compiles):

```bash
npm run tauri build
```

Build de prueba, más rápido:

```bash
npm run tauri build -- --debug
```

Los artefactos quedan en `src-tauri/target/release/bundle/` (o `debug/bundle/`).

- **macOS:** `.app` / `.dmg`
- **Windows:** instalador NSIS / `.exe` (hay que compilarlo en un PC con Windows)

## Datos locales

La base SQLite se crea al primer arranque en el data dir de la app, no dentro de esta carpeta.

- macOS: `~/Library/Application Support/com.nightdesk.hotel/`
- Windows: `%APPDATA%\com.nightdesk.hotel\`

Un backup es copiar el archivo `nightdesk.db`.

## Impresora

En **Ajustes** se configura el nombre de la impresora (`lp` en macOS/Linux, cola de Windows) o la ruta del puerto USB/serial, y el ancho de papel (58 u 80 mm). El ticket de checkout se puede reimprimir desde el historial.

## Estructura

```
src/                 Frontend React
src-tauri/           Backend Rust, SQLite, ESC/POS
src-tauri/migrations Schema inicial
```
