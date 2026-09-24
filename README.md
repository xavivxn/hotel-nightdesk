# Love Nestt Motel App

Aplicación Windows para recepción y administración remota. Recepción opera con SQLite local, incluso sin internet; administración consulta y modifica catálogo mediante Supabase.

Un mismo instalador sirve para las dos PCs. El modo se elige en el primer arranque.

## Arquitectura

La operación de recepción sigue siendo local sobre SQLite. El admin remoto consulta y edita catálogo vía Supabase; recepción sincroniza con cola offline. El respaldo diario cifrado se envía a Supabase Storage.

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

## Requisitos para compilar

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

## Instalador Windows

Compilar en Windows desde la raíz del repositorio:

```powershell
npm ci
npm run build:installer:windows
```

El instalador NSIS por usuario queda en `src-tauri/target/release/bundle/nsis/`. Es el mismo archivo `.exe` para recepción y administración. La compilación requiere Node, Rust y Visual C++ Build Tools; **las PCs donde se instala no necesitan estas herramientas**. Si WebView2 no está presente, el instalador predeterminado necesita internet para descargarlo una vez. La operación de recepción posterior funciona sin internet.

`npm run build:installer:windows` incrementa automáticamente la versión de parche antes de cada instalador (`0.1.1` → `0.1.2`) y la sincroniza en `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` y `src-tauri/Cargo.lock`. Los builds de desarrollo no consumen versiones; usá este comando para generar entregables Windows.

Desde la versión 0.1.1, el instalador registra la aplicación para abrirla automáticamente cuando inicia sesión el usuario de Windows. La entrada es por usuario (HKCU), se conserva durante una actualización y se elimina al desinstalar. Esto inicia la ventana después del inicio de sesión; no instala un servicio de Windows ni arranca antes de que el usuario inicie sesión.

Para una compilación rápida de desarrollo:

```bash
npm run tauri build -- --debug
```

El instalador conserva `com.nightdesk.hotel` como identificador interno para mantener la ruta de datos al actualizar. No cambies ese identificador ni instales una versión anterior sobre una base ya migrada. Cerrá la aplicación antes de actualizar y verificá que exista un respaldo reciente. La actualización reemplaza los binarios y conserva `%APPDATA%\com.nightdesk.hotel\`.

En el primer arranque de una instalación nueva, elegí **Recepción** o **Administración remota**. En recepción configurá la impresora y el dispositivo Supabase; sus credenciales se guardan en el Administrador de credenciales de Windows. Una configuración anterior en `device_supabase.json` se migra al abrir y se elimina tras guardar la credencial en Windows. En administración configurá URL/clave anónima e iniciá sesión con Supabase Auth. No se instala un servicio Windows ni WireGuard. El procedimiento y la evidencia pendiente por cada equipo están en [validación MOT-23](docs/validacion-instalacion-mot23.md).

## Datos locales

La base SQLite se crea al primer arranque en el data dir de la app, no dentro de esta carpeta.

- macOS: `~/Library/Application Support/com.nightdesk.hotel/`
- Windows: `%APPDATA%\com.nightdesk.hotel\`

Con WAL activo, no copiar únicamente `nightdesk.db` mientras la app está trabajando. Usá el flujo de respaldos consistente de la aplicación; ver la [guía de recuperación](docs/guia-respaldos-recuperacion.md).

## Impresora

En **Ajustes** se configura el nombre de la impresora (`lp` en macOS/Linux, cola de Windows) o la ruta del puerto USB/serial, y el ancho de papel (58 u 80 mm). El ticket de checkout se puede reimprimir desde el historial.

## Estructura

```
src/                 Frontend React
src-tauri/           Backend Rust, SQLite, ESC/POS
src-tauri/migrations Schema inicial
```
