# Nightdesk

Sistema local de recepción para hotel/motel: tablero de habitaciones, check-in por hora o noche, cobro al checkout e impresión de tickets ESC/POS. Tauri + React + SQLite, 100% offline.

Pensado para el dueño o el personal de recepción. Los datos quedan en el equipo: no hace falta internet ni un servidor en la nube.

## Evolución acordada (pendiente de implementación)

La operación de recepción sigue siendo local sobre SQLite. El admin remoto consulta y edita catálogo vía Supabase; recepción sincroniza con cola offline. Respaldo diario cifrado a Supabase Storage.

Ver [Arquitectura, requisitos y criterios de aceptación](docs/arquitectura-offline-supabase.md). El documento [VPN/API privada](docs/arquitectura-offline-vpn-backups.md) quedó sustituido el 17/09/2026.

Configuración de trabajo documentada en [N01 — Configuración operativa acordada](docs/configuracion-operativa.md): habitaciones, tarifas y catálogo inicial, usuarios, equipos, impresión, respaldos, capacidad del mes y datos pendientes para instalar.

Recorrido de recepción documentado en [N02 — Control de cuentas](docs/recepcion-cuentas.md): cierre local, cargos, limpieza, historial y reimpresión.

Medición de snapshot y retención 7/30/12 en [I03 — Conectividad y respaldos](docs/viabilidad-conectividad-respaldos.md) §3. WireGuard no se implementa; D08/D09 se cierran en I03.1. Contrato: [contrato-ipc-api.md](docs/contrato-ipc-api.md).

## Qué incluye el MVP

- Tablero visual de habitaciones (libre, ocupada, sucia, bloqueada, reservada)
- Check-in walk-in con tarifa por hora, por noche o pernocte
- Cuenta en vivo, recargos/descuentos y cobro al checkout (efectivo, tarjeta o transferencia)
- Reservas simples: en espera, check-in, cancelar o no-show
- Historial del día y reimpresión de ticket
- Tema claro/oscuro y PIN opcional de desbloqueo
- Impresión ESC/POS (58 mm / 80 mm); si la impresora falla, el cobro igual se cierra
- Catálogo de consumos para administración: altas, edición, precios en Gs. y baja lógica

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

Con WAL activo, no copiar únicamente `nightdesk.db` mientras la app está trabajando. El mecanismo previsto generará snapshots consistentes con SQLite Online Backup API y verificará su restauración. Ver la especificación de respaldos vinculada arriba.

## Impresora

En **Ajustes** se configura el nombre de la impresora (`lp` en macOS/Linux, cola de Windows) o la ruta del puerto USB/serial, y el ancho de papel (58 u 80 mm). El ticket de checkout se puede reimprimir desde el historial.

## Estructura

```
src/                 Frontend React
src-tauri/           Backend Rust, SQLite, ESC/POS
src-tauri/migrations Schema inicial
```
