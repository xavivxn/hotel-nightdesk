# Piloto LAN: dos PCs Windows (principal + adicional)

Registro de la aceptación física pendiente de `docs/recepciones-lan.md`. Solo datos ficticios. No anotar contraseñas, PIN, claves ni datos de clientes reales. No configurar Supabase ni publicar este instalador.

## Compilación e instalador

| Campo | Valor |
|---|---|
| Rama / commit | `feature/recepciones-lan` / `` (`git rev-parse --short HEAD`) |
| Comando | `powershell -ExecutionPolicy Bypass -File .\scripts\build-lan-pilot-windows.ps1` |
| Aviso "cloud credentials are not embedded" | Sí / No |
| Instalador | `src-tauri\target\debug\bundle\nsis\Nightdesk LAN Pilot Auto_0.1.11_x64-setup.exe` |
| SHA-256 (PC principal) | |
| SHA-256 verificado en adicional | Coincide / No coincide |
| Versión mostrada en la app | |

## Equipos

| Rol | Modelo PC | Windows (`winver`) | Conexión (Ethernet/Wi-Fi) | IP local | Perfil de red | Impresora (modelo, driver, cola, 58/80 mm) |
|---|---|---|---|---|---|---|
| Principal | | | | | Privada | |
| Adicional | | | | | Privada | |

Router/switch (modelo): 
Forma de cortar internet conservando la LAN: 

## Resultados

Resultado: OK / Falla / Pendiente / N/A. En falla, copiar el texto exacto del error y si quedó una solicitud pendiente.

| # | Hora | Puesto | Prueba | Resultado | Detalle / error exacto |
|---|---|---|---|---|---|
| 1 | | Principal | Compilación con el script | | |
| 2 | | Principal | Instalación (SmartScreen, WebView2) | | |
| 3 | | Adicional | Instalación con el mismo .exe | | |
| 4 | | Principal | Recepción principal, admin y recepcionista ficticios | | |
| 5 | | Principal | Puestos: interfaz, LAN 17443, Configurar firewall (UAC) | | |
| 6 | | Adicional | Vinculación automática sin "Buscar ahora" (segundos) | | |
| 7 | | Ambos | Huella completa y código coinciden; aprobado | | |
| 8 | | Adicional | Inicio de sesión con usuario distinto | | |
| 9 | | A→B | Ingreso en principal visible en adicional | | |
| 10 | | B→A | Consumo en adicional visible una sola vez en principal | | |
| 11 | | Ambos | Misma habitación al mismo tiempo: una válida + conflicto | | |
| 12 | | Ambos | Consumo en A invalida cierre mostrado en B (importe nuevo) | | |
| 13 | | Principal | Impresión de prueba local | | |
| 14 | | Adicional | Impresión de prueba local | | |
| 15 | | Adicional | Impresión hacia la impresora de la principal | | |
| 16 | | | Cierre: dos copias | | |
| 17 | | | Reimpresión: una copia | | |
| 18 | | | Sin papel / impresora apagada: cierre confirmado, sin reenvío automático | | |
| 19 | | Ambos | Sin internet con LAN: ingreso, consumo y cierre desde ambos | | |
| 20 | | Adicional | Corte de red de la adicional: muestra últimos datos, bloquea operaciones, reconecta sola | | |
| 21 | | Principal | Cerrar ventana: sigue en bandeja, adicional opera | | |
| 22 | | Principal | Detener recepción y salir: adicional bloquea; al reabrir, nuevo ingreso | | |
| 23 | | Ambos | Actividad atribuye cada cierre a su usuario | | |
| 24 | | Principal | Regreso de internet (sin Supabase en piloto: sync N/A) | | |

## Propagación (segundos desde la acción hasta verla en el otro puesto)

| Medición | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---|---|---|---|---|---|---|---|---|---|
| Segundos | | | | | | | | | | |

p50: 
p95 (aprox., el mayor de 10): 

## Incidencias

- 06/10/2026 19:52 · Impresora (pantalla **Impresora**): `Command printer_config_get not found`. Causa: `PrinterPage` llama a `api.deviceModeGet()` en paralelo con `printer_config_get`; `refreshDeviceMode()` vaciaba `cachedMode` y la consulta se enviaba directo a Tauri en lugar de `reception_invoke`. Corregido en `src/lib/api.ts` (conserva el modo mientras se vuelve a consultar). Requiere recompilar e instalar encima en ambas PCs.
- 06/10/2026 20:02 · Ventana: se abría con 1680×1050 fijos (mínimo 1100×820) y centrada; en pantallas más chicas o con escala 125–150 % quedaba fuera del monitor y se perdían los botones minimizar/maximizar/cerrar. Corregido: `src-tauri/src/window_fit.rs` ajusta tamaño y posición al área de trabajo del monitor (maximiza si no entra), la ventana se muestra recién ajustada y el botón central maximiza/restaura en vez de pantalla completa.
