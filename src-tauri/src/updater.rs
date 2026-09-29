//! In-app updates: tauri-plugin-updater against the private `updates` Storage bucket.
//! Manifest and installer are read with the device account (same credentials as sync and
//! backups), so the installer, which embeds those credentials, is never publicly downloadable.
use crate::credentials;
use crate::error::{AppError, AppResult};
use crate::models::{AppUpdateInfo, AppUpdateProgress};
use crate::sync::client::SupabaseClient;
use std::path::PathBuf;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

/// `{bucket}/{path}` of the manifest published with each release (see docs/actualizaciones.md).
const MANIFEST: &str = "updates/windows/latest.json";
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
/// Also bounds the installer download, so it must cover a slow reception connection.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);

struct Access {
    base: String,
    anon_key: String,
    token: String,
}

/// `None` when this PC has no device credentials: there is nothing to authenticate with.
fn device_access(data_dir: PathBuf) -> AppResult<Option<Access>> {
    if !credentials::device_configured(&data_dir)? {
        return Ok(None);
    }
    let client = SupabaseClient::from_device(credentials::load_device(&data_dir)?)?;
    let token = client.access_token()?;
    Ok(Some(Access {
        base: client.project_url().to_string(),
        anon_key: client.anon_key().to_string(),
        token,
    }))
}

fn updater_error(error: tauri_plugin_updater::Error) -> AppError {
    AppError::msg(format!("Actualización: {error}"))
}

async fn find(app: &AppHandle, timeout: Duration) -> AppResult<Option<Update>> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::msg(e.to_string()))?;
    // The device login uses reqwest::blocking; keep it off the async runtime.
    let access = tauri::async_runtime::spawn_blocking(move || device_access(data_dir))
        .await
        .map_err(|e| AppError::msg(e.to_string()))??;
    let Some(access) = access else {
        return Ok(None);
    };
    let endpoint = Url::parse(&format!("{}/storage/v1/object/{MANIFEST}", access.base))
        .map_err(|_| AppError::msg("URL de actualizaciones inválida"))?;
    // Headers travel with the manifest request and with the installer download.
    app.updater_builder()
        .endpoints(vec![endpoint])
        .map_err(updater_error)?
        .header("apikey", access.anon_key)
        .map_err(updater_error)?
        .header("Authorization", format!("Bearer {}", access.token))
        .map_err(updater_error)?
        .timeout(timeout)
        .build()
        .map_err(updater_error)?
        .check()
        .await
        .map_err(updater_error)
}

pub async fn check(app: &AppHandle) -> AppResult<Option<AppUpdateInfo>> {
    Ok(find(app, CHECK_TIMEOUT).await?.map(|update| AppUpdateInfo {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        notes: update.body.clone().filter(|body| !body.trim().is_empty()),
    }))
}

/// Downloads, verifies the minisign signature and runs the installer. On Windows the NSIS
/// installer (passive) closes this process and reopens the app when it finishes; SQLite
/// data in the app data dir is untouched.
pub async fn install(app: &AppHandle, on_progress: Channel<AppUpdateProgress>) -> AppResult<()> {
    // Fresh check: the token that found the update hours ago may have expired.
    let update = find(app, INSTALL_TIMEOUT)
        .await?
        .ok_or_else(|| AppError::conflict("No hay una versión nueva para instalar"))?;
    let finished = on_progress.clone();
    let mut downloaded: u64 = 0;
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = on_progress.send(AppUpdateProgress {
                    stage: "downloading",
                    downloaded,
                    total,
                });
            },
            move || {
                let _ = finished.send(AppUpdateProgress {
                    stage: "installing",
                    downloaded: 0,
                    total: None,
                });
            },
        )
        .await
        .map_err(updater_error)?;
    // Only reached where the installer does not exit the process (macOS/Linux).
    app.restart()
}
