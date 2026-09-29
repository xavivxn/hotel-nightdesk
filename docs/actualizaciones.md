# Actualizaciones desde la app

La app de Windows se actualiza sola desde un botón **Actualizar** en la barra de título. Usa el plugin oficial de Tauri 2 (`tauri-plugin-updater`): descarga el instalador, verifica su firma, cierra la app, instala en modo pasivo y la vuelve a abrir.

## Cómo funciona

- La app consulta `updates/windows/latest.json` en Supabase Storage 20 s después de abrir, cada 4 h y cuando vuelve internet.
- Se autentica con la cuenta del dispositivo (la misma de sync y respaldos). El bucket `updates` es **privado** porque el instalador lleva embebidas esas credenciales (`embedded_device.local.json`).
- Si la versión publicada es mayor que la instalada, aparece **Actualizar X.Y.Z** junto a los botones de ventana. Al confirmar: descarga con progreso, verifica la firma, la app se cierra, el instalador NSIS corre en modo pasivo (barra de progreso, sin preguntas ni permisos de administrador) y la app se vuelve a abrir.
- Los datos de `%APPDATA%\com.nightdesk.hotel\` no se tocan. Las migraciones de SQLite corren al abrir, como en cualquier instalación.
- Sin internet o sin credenciales de dispositivo el botón no aparece y la recepción sigue funcionando igual.

| Pieza | Archivo |
|-------|---------|
| Consulta e instalación | `src-tauri/src/updater.rs` |
| Comandos IPC | `app_update_check`, `app_update_install` en `commands.rs` |
| Botón y diálogo | `src/components/layout/UpdateButton.tsx` (dentro de `TitleBar.tsx`) |
| Clave pública y modo pasivo | `src-tauri/tauri.conf.json` → `plugins.updater` |
| Bucket y permisos | `supabase/migrations/20260929120000_updates_bucket.sql` |

## Configuración (una sola vez)

### 1. Clave de firma

En la PC donde se compila, desde PowerShell en la raíz del repo:

```powershell
npx tauri signer generate -w "$env:USERPROFILE\.tauri\nightdesk.key"
```

Pide una contraseña y genera dos archivos:

- `nightdesk.key.pub` (pública): copiá su contenido completo en `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`, reemplazando `PEGAR_AQUI_LA_CLAVE_PUBLICA_nightdesk.key.pub`.
- `nightdesk.key` (privada): **nunca** va al repo. Guardá una copia junto con su contraseña en un lugar seguro. Si se pierde, las PCs instaladas no aceptan más actualizaciones y hay que reinstalar a mano en cada una.

### 2. Bucket en Supabase

Aplicá la migración `20260929120000_updates_bucket.sql` (`supabase db push`, o pegala en el SQL Editor del dashboard). Crea el bucket privado `updates` y permite lectura solo a cuentas de dispositivo y administradores.

### 3. Última instalación manual

Las PCs con 0.1.7 o anterior no saben actualizarse. La primera versión que incluye el botón se instala a mano una última vez en cada PC. Desde ahí, todo se hace con el botón.

## Publicar una versión

1. Compilá con la clave en el entorno:

   ```powershell
   $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\nightdesk.key" -Raw
   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<contraseña de la clave>"
   npm run build:installer:windows
   ```

   En `src-tauri/target/release/bundle/nsis/` quedan `Love Nestt Motel App_X.Y.Z_x64-setup.exe` y `Love Nestt Motel App_X.Y.Z_x64-setup.exe.sig`. Si faltan las variables, el build falla al final (la versión ya subió; usá la siguiente sin problema).

2. En el dashboard: **Storage → updates → carpeta `windows`**, subí el `.exe` renombrado sin espacios: `nightdesk_X.Y.Z_x64-setup.exe`. Renombrar no afecta la firma.

3. Armá `latest.json` y subilo a `updates/windows/latest.json`, reemplazando el anterior. Subilo **después** del `.exe`.

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

   Para copiar la firma: `Get-Content "src-tauri\target\release\bundle\nsis\Love Nestt Motel App_X.Y.Z_x64-setup.exe.sig" -Raw | Set-Clipboard`.

4. Las PCs muestran el botón al abrir la app o en la próxima consulta (cada 4 h). Si tarda, puede ser la caché de Storage (hasta 1 h).

## Reglas

- `version` en `latest.json` tiene que ser exactamente la del instalador subido y mayor que la instalada. No se puede bajar de versión: para volver atrás, publicá una versión nueva con el código anterior.
- La firma tiene que ser la del mismo `.exe`. Si no coincide, la app rechaza la instalación y queda como estaba.
- Conservá los `.exe` anteriores en el bucket: sirven para reinstalar a mano si hace falta.
- No cambies el identificador `com.nightdesk.hotel` ni el nombre del producto: el instalador los usa para reemplazar la instalación existente.

## Verificar una versión nueva

1. El botón aparece en la barra de título con la versión publicada.
2. **Más tarde** cierra el diálogo y deja el botón visible.
3. **Actualizar ahora** muestra el progreso, la app se cierra, aparece la barra del instalador y la app se vuelve a abrir sola.
4. La versión nueva está instalada, el tablero y el historial siguen iguales y la app sigue abriéndose al iniciar sesión en Windows.
