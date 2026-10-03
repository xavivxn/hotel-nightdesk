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

## Ampliación LAN aprobada (03/10/2026)

Leer `docs/recepciones-lan.md` y contrato IPC v3 / LAN v1. Se autoriza Axum HTTPS + WebSocket + mDNS en el proceso principal. Modos: `reception`, `reception_client`, `remote`. La adicional no crea base operativa ni inicia workers cloud. `api.ts` sigue siendo el único puente UI.

Operaciones locales y LAN usan `lan/ipc.rs` → `backend.rs` → `operations.rs` → servicios. Tipos compartidos en `models.rs`/`types.ts`; tipos de transporte/configuración en `lan`/`device`. No agregar adaptadores operativos paralelos en `commands.rs` que omitan el registro durable. Efectos, resultado, auditoría y outbox se confirman en una misma transacción inmediata; savepoints en servicios. Las versiones operativas están separadas del catálogo. Cierre exige quote vigente y guarda comprobante antes de imprimir; impresión por puesto en `printing.rs`, fuera del lock DB. Nuevos pagos/arqueos quedan fuera del alcance.

Actualización/restauración de principal: pausa coordinada y LAN deshabilitada. Restaurar cambia generación fuera del snapshot para impedir replay de solicitudes antiguas. Credenciales del puesto en Credential Manager, nunca frontend. No habilitar VPN, servicio Windows, carpeta SQLite compartida ni escrituras aisladas reconciliables. Los ensayos automatizados no sustituyen el piloto físico de dos PCs/impresoras Windows.
