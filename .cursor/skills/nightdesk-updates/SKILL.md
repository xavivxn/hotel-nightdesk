---
name: nightdesk-updates
description: >-
  In-app Windows updates for Nightdesk: tauri-plugin-updater, signing keys,
  bump-version.mjs, first manual 0.1.8 install, then latest.json on the private
  updates bucket. Use when changing updater.rs, UpdateButton, tauri.conf.json
  updater pubkey, bump-version, build:installer:windows, latest.json, or when
  the user asks how to build, sign, publish, or version a release.
---

# Actualizaciones Nightdesk

Procedimiento humano y reglas: [docs/actualizaciones.md](docs/actualizaciones.md). No duplicar esa guía acá.

## Qué no hacer

- No generar un par de firma nuevo si `plugins.updater.pubkey` ya no es el placeholder. El par vigente vive fuera del repo (`~/.tauri/nightdesk.key` en la Mac de desarrollo).
- No commitear `nightdesk.key`, contraseñas, `.exe` ni `.sig`.
- No editar `version` a mano. Solo `npm run build:installer:windows` → `scripts/bump-version.mjs` (patch).
- No rebobinar el bump si el build falló después: el siguiente número es el correcto.
- No publicar `latest.json` de la **primera** build con el botón (hoy: 0.1.7 → 0.1.8). Esa se instala a mano. El manifiesto empieza en la build **siguiente**.
- Subir primero el `.exe` a `updates/windows/`, después `latest.json`. `version` = la del instalador y mayor que la instalada.
- No cambiar `identifier` `com.nightdesk.hotel` ni `productName`.

## Código

Consulta autenticada (JWT de dispositivo) a `updates/windows/latest.json`: `src-tauri/src/updater.rs`. UI: `src/components/layout/UpdateButton.tsx`. Bucket: ya creado; no reaplicar la migración salvo que falte en un proyecto nuevo.
