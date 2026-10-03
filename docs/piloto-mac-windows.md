# Piloto LAN inicial: Mac principal y Windows adicional

## Ajuste de tablero: indicador LAN compacto

La principal y la adicional muestran ahora un botón **LAN** con punto de estado en la cabecera del tablero. La franja superior de conexión y la tarjeta grande de vinculación ya no ocupan espacio. Al pulsar el botón se abre un panel flotante con el estado, el descubrimiento, la huella, las solicitudes de vinculación y la recuperación de operaciones. Escape, clic fuera y el botón de cierre lo cierran. La pantalla completa de primer vínculo de la adicional sigue intacta.

El verde de la principal indica que su servidor LAN está disponible; no confirma que la adicional esté conectada. En la adicional, verde indica conexión con la principal, ámbar una reconexión reciente y rojo una pérdida de conexión. Gris indica LAN sin configurar. Un aviso pequeño de desconexión desaparece tras siete segundos; una operación con resultado incierto mantiene su aviso hasta que se consulte o se resuelva. Administración remota no consulta ni muestra este estado LAN.

Los paquetes de esta revisión conservan la identidad `com.nightdesk.hotel.pilot.auto` y la versión piloto `0.1.11`. Para actualizar la adicional, **cerrar Nightdesk e instalar encima del piloto Auto existente, sin desinstalarlo primero**. En Mac, esa ruta conservó vínculo y datos. Si se desinstala Windows y la app vuelve a «Vincular con principal», la principal verá una identidad de puesto nueva: un administrador debe revocar el registro anterior en **Puestos** y repetir la vinculación presencial. Las operaciones confirmadas siguen en la base de la principal. Los paquetes siguen sin contener credenciales de Supabase ni ser una actualización de producción.

| Equipo | Paquete compacto | SHA-256 |
|---|---|---|
| Mac arm64 | `pilot-artifacts/Nightdesk-LAN-Pilot-Auto-0.1.11-macOS-arm64-compact.zip` | `2e39e93f678faa9b82273acce64ec1ac0e4d8f0a7b4be7986595bb4c1fbaa2b2` |
| Windows x64 | `pilot-artifacts/Nightdesk-LAN-Pilot-Auto-0.1.11-Windows-x64-compact-setup.exe` | `ec9dcc4e86ae583333f035eb84a9e3edf8757447f9705000fd4ab7eca6d2a079` |

Para la aceptación física: reinstalar ambos paquetes, abrir el tablero en ambos temas y comprobar que no aparecen la franja ni la tarjeta; abrir/cerrar el panel con ratón y teclado, verificar la búsqueda y una operación en cada sentido, cortar brevemente Wi-Fi de la adicional y observar reconexión y aviso. Forzar un resultado incierto requiere cortar la red en el instante del commit; no repetir manualmente un cierre sin consultar la solicitud retenida. Comprobar por separado que Administración remota carece de control LAN.

Comprobación Mac de esta revisión: la principal conservó su base, su usuario y el puesto vinculado tras reinstalar y reiniciar. El tablero mostró solo el botón LAN junto a la ocupación, sin franja ni tarjeta; el panel flotó por encima de las habitaciones en ambos temas. Escape y clic fuera lo cerraron. La principal mostró correctamente «Servicio LAN disponible» y el puesto vinculado, sin afirmar que el otro equipo estuviera en línea. Los recursos HTML, CSS y JS generados para Mac y Windows coincidieron byte a byte en la compilación; la prueba en la interfaz Windows queda a completar en ese equipo.

Durante la instalación Windows de esta revisión se desinstaló primero el piloto Auto anterior. La adicional regresó al asistente de primer vínculo con una identidad nueva y la principal rechazó el intento mientras seguía activo el registro de la identidad anterior. Se revocó solo ese registro obsoleto en **Puestos**, se abrió otra ventana de dos minutos, se comparó el nuevo código en ambos equipos y se aprobó la nueva identidad. Este caso confirma que una reinstalación limpia exige revinculación; todavía se debe comprobar una actualización instalada encima sin desinstalar.

Tras revincular, Windows mostró el tablero compacto. Un ingreso de la habitación 01 en Mac apareció automáticamente como ocupada en Windows. Los tres consumos de prueba agregados en Windows —Agua 5.000 Gs., Tónica 7.000 Gs. y Smirnoff 18.000 Gs.— aparecieron en la misma cuenta de Mac, con total de 75.000 Gs. incluyendo la primera hora. Al apagar solo el Wi-Fi de Windows durante unos 15 segundos, el indicador dejó de estar verde, mostró el aviso breve y recuperó la conexión por sí solo al encender el Wi-Fi, sin revincular.
En Windows también se confirmó que el botón LAN abre el panel por encima de las habitaciones sin desplazar el tablero y que Escape o clic fuera lo cierran.

## Segunda instalación limpia: búsqueda automática

La compilación **Nightdesk LAN Pilot Auto** usa el identificador `com.nightdesk.hotel.pilot.auto` en Mac y Windows. Crea directorios de datos propios, así que permite probar el asistente desde cero sin borrar el piloto ya vinculado ni su historial. No ejecutar simultáneamente el piloto anterior y este nuevo en la Mac: ambos podrían intentar usar el mismo puerto LAN.

Paquetes de esta instalación, compilados desde el mismo código fuente y sin credenciales de Supabase incrustadas:

| Equipo | Archivo | SHA-256 |
|---|---|---|
| Mac arm64 | `pilot-artifacts/Nightdesk-LAN-Pilot-Auto-0.1.11-macOS-arm64.zip` | `c7715888bac076d5f22ffa2e2fdd742d2624844b47d2cb757f0634170d418441` |
| Windows x64 | `pilot-artifacts/Nightdesk-LAN-Pilot-Auto-0.1.11-Windows-x64-setup.exe` | `1883b673f920ebc842cfe254f9b30b42e89458ab64f24f25dad99751e75cd32c` |

La compilación Mac usa `NIGHTDESK_PILOT_BUILD=1 npm run tauri -- build --debug --bundles app --no-sign --config src-tauri/tauri.pilot.auto.conf.json`. Para Windows, `scripts/build-lan-pilot-windows.ps1` permite construir en Windows; en esta prueba se compiló en Mac desde el paquete de fuente `nightdesk-lan-pilot-source-0.1.11.zip` con `cargo-xwin`, objetivo `x86_64-pc-windows-msvc` y NSIS.

1. Cerrar el piloto anterior en la Mac desde la bandeja y abrir **Nightdesk LAN Pilot Auto**. Seleccionar **Recepción principal**, crear un administrador de prueba e ingresar al tablero.
2. En el tablero, abrir **Vincular puesto** → **Activar red local**. Si la Mac tiene varias interfaces y se eligió otra, ajustarla en **Puestos**. Mantener el panel abierto: debe encontrar la nueva adicional cuando se inicie.
3. Instalar **Nightdesk LAN Pilot Auto** en Windows x64 junto al piloto anterior, cerrar el anterior y abrir el nuevo. Seleccionar **Recepción adicional**. La pantalla **Vincular con principal** debe descubrir la Mac sin pegar JSON ni pulsar **Buscar ahora**.
4. Seleccionar la Mac, comparar la huella completa, abrir la ventana de dos minutos en la principal, solicitar vinculación en Windows y aprobar únicamente el código coincidente. Iniciar sesión en Windows con una cuenta creada en la Mac.
5. Comprobar desde ambos tableros un ingreso y un consumo. Reiniciar el Wi-Fi de Windows y confirmar que reconecta. Registrar si el descubrimiento se completó solo, cuánto tardó y si apareció un aviso de firewall o permiso de red local.

La prueba física desde ambas máquinas sigue siendo necesaria: una compilación y un descubrimiento comprobado solo en Mac no certifican que el firewall y mDNS de Windows se comporten igual. La instalación inicial puede requerir WebView2 y el instalador de ensayo no está firmado.

Ensayo observado de esta segunda instalación: la Mac abrió con una base y un administrador nuevos; al activar LAN desde el tablero eligió `192.168.0.8:17443` y detectó por mDNS la adicional Windows en `192.168.0.16`. Windows encontró la Mac automáticamente en su pantalla de primer vínculo, sin pulsar **Buscar ahora** ni pegar datos manuales. Se comparó la huella SHA-256 completa y luego el código de solicitud; la principal mostró **Recepción adicional vinculada**. Windows inició sesión con la cuenta creada en esta Mac y cargó el tablero. Un ingreso de la habitación 01 desde Mac pasó a **Ocupada** automáticamente en Windows; un consumo **Agua** de 5.000 Gs. agregado en Windows apareció una sola vez en Mac y elevó el total de la cuenta de 45.000 a 50.000 Gs. sin recargar. Queda por verificar en esta segunda instalación la recuperación tras pérdida temporal de Wi-Fi y las aceptaciones prolongadas indicadas abajo.

El piloto inicial de abajo usó la versión y el protocolo compatibles `0.1.11` en ambas máquinas. El identificador `com.nightdesk.hotel.pilot` separaba la base y la configuración de la instalación habitual. La compilación con `NIGHTDESK_PILOT_BUILD=1` no incrusta credenciales de Supabase. **No configurar sincronización, administración remota ni actualización dentro del piloto.** Crear únicamente usuarios y operaciones ficticios.

La prueba Mac + Windows valida transporte LAN, vínculo, sesiones, concurrencia, recuperación y la impresora de Windows si está disponible. No sustituye el piloto final de dos PCs Windows, sus dos impresoras y seis horas sin WAN.

## Preparar los binarios

En la Mac, desde la raíz del repositorio:

```bash
NIGHTDESK_PILOT_BUILD=1 npm run tauri -- build --debug --bundles app --no-sign --config src-tauri/tauri.pilot.conf.json
```

El paquete queda en `src-tauri/target/debug/bundle/macos/Nightdesk LAN Pilot.app`. Es de desarrollo y no está firmado para distribución; ejecutarlo solo en esta Mac. Comprobar en `Info.plist` el identificador `com.nightdesk.hotel.pilot` antes de abrir. No usar `npm run tauri dev` sin `--config`: abriría el directorio de datos habitual.

El instalador Windows x64 se generó **en esta Mac** desde el mismo código, usando `cargo-xwin` y NSIS. Está en `pilot-artifacts/Nightdesk-LAN-Pilot-0.1.11-Windows-x64-setup.exe` (SHA-256: `520e7e903502b76ffaaba49e915ac0e3986c7f8eb05eacc1625b04520bd99967`). Copiar ese `.exe` a Windows por USB o un medio de transferencia local y verificar su hash antes de abrirlo. **Windows no necesita el repositorio, Rust ni Node.js** para instalar el paquete.

Instalar **Nightdesk LAN Pilot** sin desinstalar ni actualizar la app habitual. El instalador NSIS de ensayo no está firmado, no cambia la versión del proyecto y no registra arranque automático en Windows. Si WebView2 falta, la instalación inicial puede necesitar internet; completar la instalación antes de probar el corte de WAN.

## Verificación guiada en casa

Anotar resultado, hora y captura o texto exacto del error de cada paso. No compartir contraseñas, claves ni archivos de credenciales.

1. Conectar ambos equipos a la misma red local (Wi-Fi o mixta) sin red de invitados ni aislamiento de clientes. Dejar el router encendido toda la prueba. Anotar versión de Windows, arquitectura x64, red Wi-Fi/Ethernet y nombre de la impresora si existe.
2. Abrir **Nightdesk LAN Pilot** en Mac y elegir **Recepción principal**. Crear un administrador y una cuenta de recepcionista ficticios. No configurar Supabase. En **Puestos**, elegir la interfaz de red local y habilitar LAN.
3. Instalar y abrir el piloto en Windows; elegir **Recepción adicional**. Buscar la Mac y comparar la huella completa. Abrir vinculación por dos minutos en la Mac, comprobar que coincide el código de ambas pantallas y aprobar. Si mDNS no descubre la Mac, usar la conexión manual mostrada en **Puestos**.
4. Ingresar con usuarios distintos en cada equipo. En Mac hacer un ingreso ficticio; comprobar que aparece en Windows. En Windows agregar un consumo; comprobarlo en Mac. Intentar actuar sobre una habitación o una cuenta que haya cambiado en el otro puesto: debe aparecer conflicto/importe actualizado, no un segundo cierre.
5. Cerrar una cuenta ficticia desde Windows. Debe existir un solo cierre y el informe de actividad debe atribuirlo al usuario de Windows. Si hay impresora Windows, configurarla en **Impresora**, probarla y verificar ticket/reimpresión; el cierre permanece confirmado aunque falle la impresión.
6. Desconectar **solo la salida a internet** del router, manteniendo Wi-Fi/LAN. Repetir ingreso, consumo y cierre desde ambos equipos. La recepción debe seguir disponible; el estado de Supabase puede mostrar sin conexión.
7. Cortar temporalmente Wi-Fi/LAN de Windows. Debe mostrar los últimos datos, detener nuevas operaciones y recuperar al reconectar. Cerrar la ventana Mac con LAN habilitada: debe permanecer en la bandeja y Windows debe seguir trabajando. Salir desde la bandeja: Windows debe detener operaciones hasta reabrir Mac y volver a iniciar sesión.

Si falla un paso, conservar el estado y registrar qué operación se intentó, hora, mensaje y si había una solicitud pendiente. No repetir manualmente un cierre hasta consultar su resultado.

## Resultado observado el 03/10/2026

- El instalador x64 abrió correctamente en Windows y ambas máquinas se alcanzaron por Wi-Fi dentro de la misma subred. La principal escuchó por HTTPS en el puerto 17443.
- La búsqueda automática inicial desde Windows no encontró la principal. La vinculación manual con los datos públicos de conexión y la comparación de huellas sí funcionó.
- Windows inició sesión con una cuenta de la principal. El ingreso de la habitación 01 desde Mac apareció en Windows sin recarga; un consumo agregado desde Windows apareció en Mac sin recarga.
- Se creó un segundo usuario con rol Recepción. El cierre de la habitación 01 desde Windows dejó la habitación sucia en Mac. **Actividad** mostró un solo cierre de 72.000 Gs. atribuido al usuario de Windows y las acciones anteriores atribuidas a `admin`.
- Pendientes: corte físico de WAN, competencia sobre una cuenta, impresión, sesión prolongada y búsqueda automática desde un Windows aún no vinculado.

Se probó el corte y regreso del Wi-Fi de Windows: la adicional detectó la pérdida de LAN y recuperó el tablero al reconectar. No se pudo cortar únicamente WAN con la infraestructura doméstica, así que esa aceptación continúa pendiente. En otra prueba, la habitación 02 quedó ocupada y visible en Mac tras reiniciar la principal.

La compilación Mac actual declara `NSLocalNetworkUsageDescription` y `NSBonjourServices` en `Info.plist` y espera confirmación del anuncio mDNS al iniciar, sin detener el servidor HTTPS si Bonjour falla. Se ejecutó `pilot-artifacts/Nightdesk-LAN-Pilot-0.1.11-macOS-arm64-discovery.zip` (SHA-256: `9fade4a108f7009274ea67af5bcc090949213d4c6c6e6e7e3ef657a5a51dc562`). Con ella, tanto Bonjour del sistema como un segundo cliente `mdns-sd` encontraron y resolvieron el anuncio de Nightdesk en `192.168.0.8:17443`. La prueba desde la pantalla de búsqueda de Windows sigue pendiente porque ese puesto ya está vinculado; no revocar el puesto funcional solo para repetir el asistente.

## Qué queda fuera de esta etapa

- La migración de auditoría de Supabase aún no está aplicada; no reconectar el piloto a producción.
- Se necesita la segunda PC Windows para comprobar el servidor, firewall, bandeja e impresora principal **en Windows**, además de seis horas de operación con dos puestos Windows.
- El instalador de ensayo no es una actualización de producción ni una versión publicada.
