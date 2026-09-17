# I03 — Viabilidad de conectividad y destino de respaldos

**Nota 17/09/2026:** WireGuard quedó descartado. La arquitectura vigente es [arquitectura-offline-supabase.md](arquitectura-offline-supabase.md). D08 (handshake VPN) ya no aplica; D09 (destino S3/VPS) pasa a Supabase Storage. El dimensionamiento de snapshot §3 y la custodia de clave §5 se conservan y los usa I08. El cierre formal de filas V05 / §2 / §4 y de D08–D09 en [configuración operativa](configuracion-operativa.md) es [MOT-38 · I03.1](https://naserfer.atlassian.net/browse/MOT-38).

**Entregable:** [MOT-14 · I03](https://naserfer.atlassian.net/browse/MOT-14), con [MOT-38 · I03.1](https://naserfer.atlassian.net/browse/MOT-38) y [MOT-39 · I03.2](https://naserfer.atlassian.net/browse/MOT-39).  
**Versión:** 1.0 · 16 de septiembre de 2026.  
**Responsable técnico de este registro:** Iván Ortiz.  
**Datos de red y destino operativo:** Naser facilita; no constan en el repositorio ni en Jira.  
**Estado:** dimensionamiento de snapshot vigente. Handshake WireGuard **no se ejecutará**. Este documento no contiene secretos.

Complementa [N01 — Configuración operativa](configuracion-operativa.md) y [arquitectura Supabase](arquitectura-offline-supabase.md) §7–§8. No implementa sync ni subida cifrada (I07/I08).

## 1. Decisiones

| ID | Fecha | Contexto | Decisión | Responsable |
|---|---|---|---|---|
| V01 | 16/09/2026 | No hay proveedor, router, IP WAN ni ubicación de la PC admin en N01 ni en Jira. Inventar un handshake no cumple MOT-38. | **WireGuard sigue siendo el transporte propuesto.** La viabilidad **directa vs intermediario queda no comprobada**. D08 es impedimento real hasta el relevamiento y la prueba del §3. | Iván registra; Naser facilita datos de ambas ubicaciones. |
| V02 | 16/09/2026 | CGNAT es frecuente en ISP residenciales/PYME de Paraguay. WireGuard no tiene cuota; un relay sí. | Si recepción está en CGNAT IPv4 y sin IPv6 alcanzable: **intermediario** (VPS propio con WireGuard o servicio autorizado). Costo mensual **a cotizar**; no se asume gratuito. No contratar en I03. | Naser cotiza/autoriza gasto; I07 implementa. |
| V03 | 16/09/2026 | Snapshot medido: un mes sintético a 3 estadías/hab/día comprime a **132 KiB**. Retención 7/30/12 cabe en pocos MiB. | **Mantener la retención propuesta 7 locales / 30 diarias remotas / 12 mensuales remotas.** Horario **04:00** sigue como propuesta hasta confirmación operativa. Destino: objeto S3-compatible **o** mismo VPS que un eventual relay, con permisos separados. No contratar en I03. | Iván dimensiona; Naser nombra destino y custodio (D09/D10). |
| V04 | 16/09/2026 | La clave de descifrado en la PC de recepción no sobrevive a un disco perdido. | Custodia **fuera** de esa PC: persona nombrada + copia en ubicación protegida. En git/Jira solo «custodio: nombre». Procedimiento: §5. Naser nombra a la persona; no consta hoy. | Naser nombra; I08 implementa cifrado; la prueba de restauración en otro equipo es criterio de I08/N10. |

## 2. Conectividad (MOT-38)

### 2.1 Checklist por ubicación

Completar una fila para **recepción** y otra para **administración**. No pegar claves, PSK ni IPs internas de producción en este archivo.

| Dato | Recepción | Administración |
|---|---|---|
| Proveedor de internet | No informado | No informado |
| Modelo de router y acceso a su configuración | No informado | No informado |
| IPv4 WAN del router | No informado | No informado |
| IPv4 pública vista desde fuera (comparar con WAN) | No informado | No informado |
| ¿CGNAT? (WAN privada `10/8`, `100.64/10`, `192.168/16` o distinta de la IP pública) | No comprobado | No comprobado |
| IPv6 habilitado y alcanzable | No comprobado | No comprobado |
| UDP 51820 (abrir o port-forward hacia la PC) | No comprobado | No comprobado |
| Firewall / CGNAT del ISP que bloquee UDP | No comprobado | No comprobado |
| Endpoint WireGuard (hostname o IP, sin clave) | No definido | No definido |

Comparar la IPv4 WAN del router con un visor externo (por ejemplo la IP que muestra un servicio «what's my IP» desde esa red). Si coinciden y UDP 51820 llega, hay camino para **WireGuard directo** (peer recepción = servidor, admin = cliente, o al revés si la pública está en admin). Si la WAN es CGNAT y no hay IPv6, hace falta **intermediario**.

### 2.2 Protocolo de prueba (reproducible)

Ejecutar cuando existan las dos PCs Windows y los datos del §2.1. No forma parte de este cierre.

1. Instalar el cliente oficial WireGuard en ambas PCs. Generar un par de claves **en cada equipo** (`wg genkey` / `wg pubkey`). No copiar claves privadas a chat, git ni Jira.
2. Decidir roles: si hay IPv4/IPv6 pública en un lado, ese equipo (o el router con port-forward UDP 51820) es el listener. Si ambos están en CGNAT, el listener es el intermediario.
3. Configurar un peer por lado: `AllowedIPs` de un `/30` o `/24` interno de la VPN (ejemplo de documentación: `10.13.0.0/24`; elegir otro rango si choca con la LAN). MTU 1420 si hay problemas de fragmentación.
4. Traer la interfaz: handshake visible en `wg show` (campo *latest handshake* reciente) y `ping` a la IP VPN del otro extremo.
5. Registrar aquí: fecha, quién ejecutó, resultado (éxito / fallo / timeout), RTT. Registrar en Jira «prueba ejecutada, handshake OK/FAIL», sin pegar la config.

**Resultado al 16/09/2026:** no ejecutado. Faltan las dos ubicaciones reales y D08.

### 2.3 Intermediario (si la prueba directa falla o es imposible)

Opciones, costos **a cotizar**, no contratadas:

| Opción | Qué cubre | Orden de magnitud (público, 2026) | Notas |
|---|---|---|---|
| VPS pequeño (p. ej. Hetzner CX22 o equivalente) con WireGuard | Relay/endpoint estable; puede alojar también el destino de copias | ≈ 4–6 EUR/mes + IVA | Relay VPN y backup son funciones distintas: usuarios y discos/ACL separados. |
| Servicio mesh autorizado (Tailscale, Netbird u homólogo) | Conectividad sin abrir puertos en el motel | plan gratuito limitado o ≈ 5+ USD/usuario/mes | Evaluar residencia de datos y si el motel acepta un tercero. |
| Equipo de Naser con IP pública | Endpoint propio | costo de ese enlace, no del software | Solo si hay IP estable y UDP permitido. |

WireGuard en sí **no tiene licencia de pago**. El costo es el de la IP alcanzable (router/ISP o VPS/servicio).

## 3. Medición de snapshot SQLite (MOT-39)

Fecha: 16/09/2026. Máquina de desarrollo macOS. CLI: `sqlite3` (Android SDK `platform-tools`). Comandos: `.backup` y `VACUUM INTO`. Compresión: gzip nivel 6. Integridad del snapshot sintético: `PRAGMA integrity_check` = `ok`. Script auxiliar solo en `/tmp/nightdesk-i03-medida`, no en el repo.

**Por qué no copiar `nightdesk.db` a secas:** en el directorio de datos de la app (`com.nightdesk.hotel`) el archivo principal medía **4,0 KiB** y el WAL **1,06 MiB**. Casi todo el estado vivo estaba en el WAL. I08 debe usar Online Backup / `VACUUM INTO` con la base abierta.

| Escenario | Cómo se obtuvo | `.backup` / `VACUUM INTO` | gzip -6 |
|---|---|---:|---:|
| Base local de desarrollo | `.backup` y `VACUUM INTO` sobre `nightdesk.db` en uso (WAL activo) | 104,0 KiB | 5,7 KiB |
| Mes sintético de alcance | 23 habitaciones, 3 estadías cerradas/hab/día, 30 días: 2.070 huéspedes, 2.070 estadías, 6.210 cargos, 41 productos. Migraciones 001–006 aplicadas. | 908,0 KiB (vacuum) | **132,4 KiB** |

Tiempos de `.backup` / `VACUUM INTO` del mes sintético: ≈ 0,02 s en esta máquina. No es un dato de la PC de recepción.

**Crecimiento:** con este perfil el gzip diario ronda 0,13 MiB y crece de forma lineal con estadías y cargos. Una ocupación más intensa (5 estadías/hab/día) se estima ≈ 0,22 MiB gzip. Un año de operación real con tickets, auditoría e I04/I06 (usuarios, `operation_id`) puede subir el orden de magnitud; conviene volver a medir en I08 sobre datos de instalación. Aun así permanece muy por debajo de 1 GiB para la retención acordada.

### 3.1 Dimensionamiento de retención 7 / 30 / 12

Sobre el gzip del mes sintético (cota diaria = una copia de ese tamaño):

| Cola | Cálculo | Espacio |
|---|---|---|
| 7 copias locales sin comprimir (`VACUUM`) | 7 × 908 KiB | **6,2 MiB** |
| 7 copias locales gzip | 7 × 132 KiB | **0,93 MiB** |
| 30 diarias remotas gzip | 30 × 132 KiB | **3,9 MiB** |
| 12 mensuales remotos gzip | 12 × 132 KiB | **1,6 MiB** |
| Remoto 30 + 12 gzip | 42 × 132 KiB | **5,4 MiB** |
| Mismo remoto a 5 estadías/hab/día | × 5/3 | **≈ 9 MiB** |

I08 cifrará antes de subir: el tamaño cifrado es del mismo orden (ligero overhead). **La retención 7/30/12 no está limitada por capacidad** a este volumen. El cuello será el ancho de subida del motel (no medido): 132 KiB son unos 2 s a 512 Kib/s. Ancho de banda del enlace: **pendiente de medir** en el relevamiento D08.

## 4. Destino de respaldos (MOT-39)

El servidor externo **no** es base operativa ni origen de sync hacia recepción. Credenciales de subida y de borrado separadas cuando el proveedor lo permita. Credenciales por canal seguro, nunca en este Markdown.

| Opción | Protocolo | Capacidad frente a 7/30/12 | Costo explícito | Cuándo preferirla |
|---|---|---|---|---|
| A. Almacenamiento de objetos S3-compatible (Backblaze B2, Cloudflare R2, AWS S3 u homólogo regional) | HTTPS (API de objetos), nombre de objeto único por respaldo | 10 MiB de retención es despreciable frente a 1 GiB+ de los planes | Almacenamiento típico ≈ 6 USD/TB·mes; a este volumen **≪ 1 USD/mes** o tramo gratuito. Egreso y mínimo de cuenta **a cotizar**. | Destino solo de copias, sin VPS que administrar. |
| B. VPS propio (puede ser el mismo de V02) | SFTP o HTTPS a un bucket/disco con ACL de backup | Disco de 20–40 GiB de sobra | Ya cubierto por el VPS ≈ 4–6 EUR/mes si se comparte infra; si es solo backup, el objeto (A) suele ser más barato | Si ya hay relay WireGuard y se quiere un solo proveedor. |
| C. Equipo de Naser | SFTP/HTTPS en red controlada | Según disco de ese equipo | Costo del hardware/enlace existente | Solo con responsable operativo nombrado y copias fuera del motel. |

**Decisión de I03:** no contratar. I08 implementa contra el destino que Naser elija (A, B o C) usando las cifras de §3. Mientras D09 siga sin proveedor nombrado, la subida real permanece bloqueada.

Responsable operativo de copias y restauraciones: **no informado** (D09). Naser lo registra.

## 5. Custodia de la clave de descifrado

- La clave **no** vive solo en la PC de recepción.
- Custodio: persona nombrada por Naser (aún no consta). Segunda copia en ubicación protegida (caja / gestor de secretos / sobre lacrado — el medio lo elige el custodio).
- Recuperación: el custodio entrega la clave por canal seguro al administrador que restaura; se rotará después de usarla si hubo compromiso del disco.
- Quién prueba la restauración en otro equipo: el mismo responsable operativo de D09, en I08/N10. Una copia solo se considera recuperable después de esa prueba.
- En Jira: «clave configurada, custodio [nombre]». Nunca el material criptográfico.

## 6. Qué queda para I07 / I08

- I07: instalar WireGuard (directo o con intermediario según el resultado del §2.2), servicio Windows, abrir UDP solo donde V01/V02 lo permitan.
- I08: snapshot Online Backup, cifrado, cola, subida al destino de §4, retención 7/30/12, aviso si pasan 24 h sin copia remota confirmada, restauración en otro equipo.
- Naser: completar D08 (ISP/router/CGNAT/IPv6) y D09 (proveedor + responsable). Sin eso no hay handshake ni contrato de almacenamiento.

## 7. Evidencia de este cierre

| Criterio | Cómo se cubre |
|---|---|
| Router/ISP, CGNAT/IPv6 y prueba documentados, o impedimento | §2.1 vacío a propósito; §2.2 protocolo; V01 = impedimento D08 |
| Servidor, capacidad, retención, custodia, intermediario, costos | V02–V04, §3–§5; nada contratado |
| Sin secretos en git ni Jira | Este archivo no incluye claves, contraseñas, endpoints privados ni IPs de producción |
| Snapshot medido | §3, comandos `.backup` / `VACUUM INTO`, cifras en bytes |
