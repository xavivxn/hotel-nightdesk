//! Device credentials for Supabase (remote admin URL/anon, reception device).
//! Stored under the app data directory (not in SQLite `settings`, not in the repo).
//! I07 can swap the backend for Windows Credential Manager without changing callers.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RemoteCreds {
    project_url: String,
    anon_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DeviceCreds {
    project_url: String,
    anon_key: String,
    device_email: String,
    /// Stored only for the sync worker; never returned to the UI.
    device_password: String,
}

fn remote_path(app_data: &Path) -> PathBuf {
    app_data.join("remote_supabase.json")
}

fn device_path(app_data: &Path) -> PathBuf {
    app_data.join("device_supabase.json")
}

pub fn remote_configured(app_data: &Path) -> bool {
    remote_path(app_data).is_file()
}

pub fn load_remote(app_data: &Path) -> AppResult<Option<(String, String)>> {
    let path = remote_path(app_data);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| AppError::msg(e.to_string()))?;
    let creds: RemoteCreds = serde_json::from_str(&raw).map_err(|e| AppError::msg(e.to_string()))?;
    if creds.project_url.trim().is_empty() || creds.anon_key.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some((creds.project_url, creds.anon_key)))
}

pub fn save_remote(app_data: &Path, project_url: &str, anon_key: &str) -> AppResult<()> {
    let url = project_url.trim();
    let key = anon_key.trim();
    if url.is_empty() || key.is_empty() {
        return Err(AppError::msg("Ingresá la URL del proyecto y la clave anónima"));
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::msg("La URL del proyecto debe empezar con https://"));
    }
    std::fs::create_dir_all(app_data).map_err(|e| AppError::msg(e.to_string()))?;
    let body = serde_json::to_string_pretty(&RemoteCreds {
        project_url: url.to_string(),
        anon_key: key.to_string(),
    })
    .map_err(|e| AppError::msg(e.to_string()))?;
    std::fs::write(remote_path(app_data), body).map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn device_configured(app_data: &Path) -> bool {
    device_path(app_data).is_file()
}

pub fn save_device(
    app_data: &Path,
    project_url: &str,
    anon_key: &str,
    device_email: &str,
    device_password: &str,
) -> AppResult<()> {
    let url = project_url.trim();
    let key = anon_key.trim();
    let email = device_email.trim();
    if url.is_empty() || key.is_empty() || email.is_empty() || device_password.is_empty() {
        return Err(AppError::msg("Completá URL, clave anónima y credenciales del dispositivo"));
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::msg("La URL del proyecto debe empezar con https://"));
    }
    std::fs::create_dir_all(app_data).map_err(|e| AppError::msg(e.to_string()))?;
    let body = serde_json::to_string_pretty(&DeviceCreds {
        project_url: url.to_string(),
        anon_key: key.to_string(),
        device_email: email.to_string(),
        device_password: device_password.to_string(),
    })
    .map_err(|e| AppError::msg(e.to_string()))?;
    std::fs::write(device_path(app_data), body).map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}
