# Actualizaciones desde la app

La app de Windows se actualiza sola desde el botón **Actualizar** del encabezado del Tablero (`tauri-plugin-updater`). Descarga el instalador, verifica la firma, cierra la app, instala en modo pasivo y la vuelve a abrir. Los datos de `%APPDATA%\com.nightdesk.hotel\` no se tocan.

Esta guía es el procedimiento operativo: primer build con versionado e instalaciones a mano, y cada update posterior al bucket `updates`.

## Cómo funciona

- La app consulta `updates/windows/latest.json` en Supabase Storage 5 s después de abrir, cada 4 h y cuando vuelve internet.
- Se autentica con la cuenta del dispositivo (la misma de sync y respaldos). El bucket `updates` es **privado**: el instalador lleva embebidas esas credenciales y, si está en `embedded_device.local.json`, las de administración remota (`remote_email`, `remote_password`, `remote_auth_version`). Para rotar esa cuenta: subí `remote_auth_version`, compilá y publicá; las PCs que actualizan reemplazan la credencial guardada.
- Si la versión publicada es **mayor** que la instalada, aparece **Actualizar X.Y.Z** al lado del título «Tablero de habitaciones». Con un clic: descarga con progreso en el mismo botón, verifica la firma, NSIS en modo pasivo (sin preguntas ni permisos de administrador), la app se reabre.
- Las migraciones de SQLite corren al abrir, como en cualquier instalación.
- Sin internet o sin credenciales de dispositivo el botón no aparece y la recepción sigue igual.

| Pieza | Dónde |
|-------|--------|
| Consulta e instalación | `src-tauri/src/updater.rs` |
| Comandos IPC | `app_update_check`, `app_update_install` |
| Botón | `src/components/layout/UpdateButton.tsx` (en el encabezado de `BoardPage.tsx`) |
| Clave pública y modo pasivo | `src-tauri/tauri.conf.json` → `plugins.updater` |
| Subir versión al build | `scripts/bump-version.mjs` (lo llama `npm run build:installer:windows`) |
| Bucket | Storage → `updates` (migración ya aplicada) |

## Versionado

No edites `version` a mano. El comando de instalador corre `scripts/bump-version.mjs` **antes** de compilar y suma el patch en:

- `package.json` y `package-lock.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml` y `src-tauri/Cargo.lock`

Hoy el repo está en **0.1.7**. El primer `npm run build:installer:windows` deja **0.1.8**. El siguiente deja **0.1.9**, y así.

Si el build falla **después** del bump (por ejemplo faltan las variables de firma), la versión ya quedó subida. No rebobines esos archivos: la próxima compilación usa el número siguiente.

## Qué ya está hecho (no lo repitas)

1. **Clave de firma.** Par generado en esta Mac, fuera del repo:
   - privada: `~/.tauri/nightdesk.key`
   - pública: `~/.tauri/nightdesk.key.pub`
   - contraseña: `~/.tauri/nightdesk.key.password`
2. **Pública en el repo.** Ya está en `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`. No la reemplaces ni generes otra clave: las PCs que instalen un build firmado con esta clave rechazarían updates firmados con otra.
3. **Bucket.** `updates` ya existe en el proyecto nightdesk, privado, lectura para dispositivo y admin. No vuelvas a aplicar la migración.

**Nunca** commitees `nightdesk.key` ni la contraseña. `*.key` está en `.gitignore`. Guardá una copia de la privada y de la contraseña fuera de esta Mac.

## Copiar la clave a la PC de Windows (una vez)

El instalador se arma en Windows. Esa PC necesita la **misma** privada.

1. En Windows creá `%USERPROFILE%\.tauri\` si no existe.
2. Copiá `nightdesk.key` (y, si querés, `nightdesk.key.password`) desde la Mac.
3. No corras `tauri signer generate` otra vez. Eso crea un par distinto y rompe la pública del repo.

---

## Parte A — Primer build con versionado (última instalación a mano)

Las PCs con 0.1.7 **no tienen** el botón. Esta build se instala a mano. **No** publiques `latest.json` de esta versión.

### A1. PowerShell, raíz del repo, con la clave en el entorno

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\nightdesk.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content "$env:USERPROFILE\.tauri\nightdesk.key.password" -Raw
npm run build:installer:windows
```

Si no copiaste el archivo de contraseña, asigná `$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD` a mano.

### A2. Artefactos

En `src-tauri\target\release\bundle\nsis\`:

- `Love Nestt Motel App_0.1.8_x64-setup.exe`
- `Love Nestt Motel App_0.1.8_x64-setup.exe.sig`

El nombre lleva espacios; para Storage se renombra después. Renombrar **no** invalida la firma.

### A3. Instalar en cada recepción

1. Cerrá la app si está abierta.
2. Corré el `.exe` de 0.1.8.
3. Comprobá que arranca, que el tablero y el historial siguen, y que el identificador sigue siendo `com.nightdesk.hotel`.

Opcional: subí el instalador a Storage → `updates` → carpeta `windows` como `nightdesk_0.1.8_x64-setup.exe` por si hay que reinstalar a mano. **No** subas `latest.json` con `"version": "0.1.8"`: las PCs que acaban de instalar 0.1.8 no verían botón, y las de 0.1.7 no saben consultar el bucket.

---

## Parte B — Cada update siguiente (botón Actualizar)

Cuando haya un cambio que valga la pena entregar por el botón (el próximo bump será **0.1.9**, o el número que muestre el script).

### B1. Mismo build firmado

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\nightdesk.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content "$env:USERPROFILE\.tauri\nightdesk.key.password" -Raw
npm run build:installer:windows
```

Anotá la versión que imprimió el script (`Versión actualizada: A.B.C → X.Y.Z`). El `.exe` y el `.sig` quedan en la misma carpeta `nsis\` con ese `X.Y.Z`.

### B2. Subir el instalador (antes que el manifiesto)

Dashboard → **Storage** → bucket **updates** → carpeta **windows**:

1. Subí el `.exe` como `nightdesk_X.Y.Z_x64-setup.exe` (sin espacios).
2. Dejá los `.exe` viejos: sirven para reinstalar a mano.

### B3. Armar y subir `latest.json` (después del `.exe`)

Copiá la firma:

```powershell
Get-Content "src-tauri\target\release\bundle\nsis\Love Nestt Motel App_X.Y.Z_x64-setup.exe.sig" -Raw | Set-Clipboard
```

Subí `updates/windows/latest.json` **reemplazando** el anterior:

```json
{
  "version": "X.Y.Z",
  "notes": "Qué cambia, en una o dos líneas para recepción.",
  "pub_date": "2026-09-29T15:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<contenido completo del .exe.sig>",
      "url": "https://rkbilukqaoafpojkoljm.supabase.co/storage/v1/object/updates/windows/nightdesk_X.Y.Z_x64-setup.exe"
    }
  }
}
```

`version` tiene que ser **exactamente** la del `.exe` y **mayor** que la instalada (después de la parte A: mayor que 0.1.8). `pub_date` en UTC. La firma es la de **ese** archivo.

### B4. Comprobar en una PC que ya tiene 0.1.8 (o la última instalada)

1. Abrí la app (con internet y dispositivo configurado). A los ~5 s, o al volver la red, aparece **Actualizar X.Y.Z** en el encabezado del Tablero.
2. Un clic muestra «Descargando N %» en el mismo botón; la app se cierra, aparece la barra del instalador y la app se vuelve a abrir sola.
3. Si algo falla, el botón pasa a «Reintentar actualización» y muestra el motivo debajo.
4. Tablero e historial iguales; la app sigue abriéndose al iniciar sesión en Windows.

Si el botón tarda: caché de Storage (hasta 1 h) o el equipo no tiene credenciales de dispositivo.

---

## Reglas

- No se puede bajar de versión con el botón. Para volver atrás, publicá una versión **nueva** con el código anterior.
- Firma y `.exe` tienen que ser del mismo build. Si no coinciden, la app rechaza el update y queda como estaba.
- No cambies `com.nightdesk.hotel` ni el `productName`: el instalador los usa para reemplazar la instalación existente.
- Commiteá el bump de versión que dejó el script (queda en el working tree después del build). No subas `.exe`, `.sig` ni la clave privada.

## Si algo falla

| Qué ves | Qué hacer |
|---------|-----------|
| El build termina sin `.sig` | Faltaban `TAURI_SIGNING_PRIVATE_KEY` / `_PASSWORD`. La versión ya subió; repetí el build (siguiente número). |
| No aparece el botón | Versión publicada no es mayor; sin internet; sin cuenta de dispositivo; o `latest.json` se subió antes que el `.exe`. |
| “No se pudo actualizar” / firma | Pública del instalado ≠ par que firmó este `.exe`, o pegaste mal el `.sig` en `latest.json`. |
| Generaste otra clave por error | No la uses. Seguí con `~/.tauri/nightdesk.key` (el par cuya pública está en `tauri.conf.json`). |
