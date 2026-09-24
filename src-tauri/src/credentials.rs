//! Supabase configuration. On Windows, reception device credentials live in
//! Credential Manager. Elsewhere they stay in the app data directory, same place
//! as the remote admin URL and anon key. A legacy plaintext file on Windows is
//! migrated on first load and then removed.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RemoteCreds {
    project_url: String,
    anon_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct DeviceCreds {
    pub(crate) project_url: String,
    pub(crate) anon_key: String,
    pub(crate) device_email: String,
    /// Stored only for the sync worker; never returned to the UI.
    pub(crate) device_password: String,
}

pub(crate) fn load_device(app_data: &Path) -> AppResult<DeviceCreds> {
    if !device_marker_path(app_data).is_file() && !device_path(app_data).is_file() {
        return Err(AppError::storage("No se configuró este dispositivo en esta instalación"));
    }
    if let Some(raw) = read_device_secret(app_data)? {
        // The marker prevents an orphaned vault entry from reconnecting a clean
        // installation after the user deletes the application's data directory.
        if !device_marker_path(app_data).is_file() && !device_path(app_data).is_file() {
            return Err(AppError::storage("No se configuró este dispositivo en esta instalación"));
        }
        let creds: DeviceCreds = serde_json::from_slice(&raw)
            .map_err(|_| AppError::storage("Credencial del dispositivo inválida"))?;
        validated_project_url(&creds.project_url)?;
        if !device_marker_path(app_data).is_file() {
            write_device_marker(app_data)?;
        }
        remove_legacy_device_file(app_data)?;
        return Ok(creds);
    }

    let raw = std::fs::read(device_path(app_data))
        .map_err(|_| AppError::storage("No se pudo leer la configuración del dispositivo"))?;
    let creds: DeviceCreds = serde_json::from_slice(&raw)
        .map_err(|_| AppError::storage("Configuración del dispositivo inválida"))?;
    validated_project_url(&creds.project_url)?;
    write_device_secret(app_data, &raw)?;
    write_device_marker(app_data)?;
    remove_legacy_device_file(app_data)?;
    Ok(creds)
}

fn device_marker_path(app_data: &Path) -> PathBuf {
    app_data.join("device_credential_store.txt")
}

fn write_device_marker(app_data: &Path) -> AppResult<()> {
    std::fs::create_dir_all(app_data)?;
    std::fs::write(device_marker_path(app_data), b"windows-credential-manager-v1\n")?;
    Ok(())
}

fn remove_legacy_device_file(app_data: &Path) -> AppResult<()> {
    // Windows keeps the secret in Credential Manager, so the JSON is only a
    // migration source. On other systems that JSON is the store itself.
    #[cfg(windows)]
    {
        let path = device_path(app_data);
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|e| AppError::storage(format!("No se pudo eliminar la credencial anterior: {e}")))?;
        }
    }
    #[cfg(not(windows))]
    {
        let _ = app_data;
    }
    Ok(())
}

fn remote_path(app_data: &Path) -> PathBuf {
    app_data.join("remote_supabase.json")
}

fn device_path(app_data: &Path) -> PathBuf {
    app_data.join("device_supabase.json")
}

fn validated_project_url(value: &str) -> AppResult<String> {
    let parsed = reqwest::Url::parse(value.trim())
        .map_err(|_| AppError::msg("URL del proyecto inválida"))?;
    let local = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if parsed.scheme() != "https" && !(parsed.scheme() == "http" && local) {
        return Err(AppError::msg("La URL del proyecto debe usar HTTPS"));
    }
    Ok(parsed.as_str().trim_end_matches('/').to_string())
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
    Ok(Some((validated_project_url(&creds.project_url)?, creds.anon_key)))
}

pub fn save_remote(app_data: &Path, project_url: &str, anon_key: &str) -> AppResult<()> {
    let url = validated_project_url(project_url)?;
    let key = anon_key.trim();
    if key.is_empty() {
        return Err(AppError::msg("Ingresá la URL del proyecto y la clave anónima"));
    }
    std::fs::create_dir_all(app_data).map_err(|e| AppError::msg(e.to_string()))?;
    let body = serde_json::to_string_pretty(&RemoteCreds {
        project_url: url,
        anon_key: key.to_string(),
    })
    .map_err(|e| AppError::msg(e.to_string()))?;
    std::fs::write(remote_path(app_data), body).map_err(|e| AppError::msg(e.to_string()))?;
    Ok(())
}

pub fn device_configured(app_data: &Path) -> AppResult<bool> {
    if !device_path(app_data).is_file() && !device_marker_path(app_data).is_file() {
        return Ok(false);
    }
    load_device(app_data)?;
    Ok(true)
}

fn backup_key_path(app_data: &Path) -> PathBuf {
    app_data.join("backup_aes_key.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupKeyFile {
    /// Hex-encoded 32-byte AES-256 key. Never shown in UI.
    key_hex: String,
}

/// Ensures a 32-byte AES key exists under app data. Generates one on first use.
pub fn ensure_backup_key(app_data: &Path) -> AppResult<[u8; 32]> {
    let path = backup_key_path(app_data);
    if path.is_file() {
        let raw = std::fs::read_to_string(&path).map_err(|e| AppError::storage(e.to_string()))?;
        let file: BackupKeyFile = serde_json::from_str(&raw).map_err(|_| AppError::storage("Clave de respaldo inválida"))?;
        let bytes = hex::decode(file.key_hex.trim()).map_err(|_| AppError::storage("Clave de respaldo inválida"))?;
        if bytes.len() != 32 {
            return Err(AppError::storage("Clave de respaldo inválida"));
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        return Ok(out);
    }
    use rand_core::{OsRng, RngCore};
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    std::fs::create_dir_all(app_data).map_err(|e| AppError::storage(e.to_string()))?;
    let body = serde_json::to_string_pretty(&BackupKeyFile {
        key_hex: hex::encode(key),
    })
    .map_err(|e| AppError::msg(e.to_string()))?;
    std::fs::write(&path, body).map_err(|e| AppError::storage(e.to_string()))?;
    Ok(key)
}

pub fn backup_key_configured(app_data: &Path) -> bool {
    backup_key_path(app_data).is_file()
}

/// Writes the custodian AES-256 key (64 hex chars). Overwrites the local file. Never returns the key.
pub fn import_backup_key(app_data: &Path, key_hex: &str) -> AppResult<()> {
    let cleaned: String = key_hex.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = hex::decode(cleaned.trim()).map_err(|_| AppError::msg("La clave debe ser 64 caracteres hexadecimales"))?;
    if bytes.len() != 32 {
        return Err(AppError::msg("La clave debe ser 64 caracteres hexadecimales"));
    }
    std::fs::create_dir_all(app_data).map_err(|e| AppError::storage(e.to_string()))?;
    let body = serde_json::to_string_pretty(&BackupKeyFile {
        key_hex: hex::encode(bytes),
    })
    .map_err(|e| AppError::msg(e.to_string()))?;
    std::fs::write(backup_key_path(app_data), body).map_err(|e| AppError::storage(e.to_string()))?;
    Ok(())
}

pub fn save_device(
    app_data: &Path,
    project_url: &str,
    anon_key: &str,
    device_email: &str,
    device_password: &str,
) -> AppResult<()> {
    let url = validated_project_url(project_url)?;
    let key = anon_key.trim();
    let email = device_email.trim();
    if key.is_empty() || email.is_empty() || device_password.is_empty() {
        return Err(AppError::msg("Completá URL, clave anónima y credenciales del dispositivo"));
    }
    let body = serde_json::to_vec(&DeviceCreds {
        project_url: url,
        anon_key: key.to_string(),
        device_email: email.to_string(),
        device_password: device_password.to_string(),
    })
    .map_err(|e| AppError::msg(e.to_string()))?;
    write_device_secret(app_data, &body)?;
    write_device_marker(app_data)?;
    remove_legacy_device_file(app_data)?;
    Ok(())
}

#[cfg(windows)]
fn device_target(app_data: &Path) -> Vec<u16> {
    use sha2::{Digest, Sha256};
    let path = app_data.to_string_lossy().to_lowercase();
    let target = format!(
        "com.nightdesk.hotel/device/{}",
        hex::encode(Sha256::digest(path.as_bytes()))
    );
    target.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn read_device_secret(app_data: &Path) -> AppResult<Option<Vec<u8>>> {
    use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
    use windows_sys::Win32::Security::Credentials::{CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC};
    let target = device_target(app_data);
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: target is a NUL-terminated UTF-16 string. CredReadW initializes
    // credential on success and CredFree releases that buffer exactly once.
    if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } == 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_NOT_FOUND as i32) {
            return Ok(None);
        }
        return Err(AppError::storage(format!("No se pudo leer el Administrador de credenciales de Windows: {error}")));
    }
    if credential.is_null() {
        return Err(AppError::storage("Windows devolvió una credencial vacía"));
    }
    // SAFETY: the returned credential and blob are valid until CredFree.
    let raw = unsafe {
        let stored = &*credential;
        let len = stored.CredentialBlobSize as usize;
        let bytes = if len == 0 {
            Vec::new()
        } else if stored.CredentialBlob.is_null() {
            CredFree(credential.cast());
            return Err(AppError::storage("Windows devolvió una credencial inválida"));
        } else {
            std::slice::from_raw_parts(stored.CredentialBlob, len).to_vec()
        };
        CredFree(credential.cast());
        bytes
    };
    Ok(Some(raw))
}

#[cfg(windows)]
fn write_device_secret(app_data: &Path, raw: &[u8]) -> AppResult<()> {
    use windows_sys::Win32::Security::Credentials::{CredWriteW, CREDENTIALW, CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC};
    // Generic credentials support at most 2560 bytes. Report an actionable error
    // rather than letting CredWriteW fail with a generic Windows code.
    if raw.len() > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
        return Err(AppError::storage("Las credenciales del dispositivo son demasiado largas para Windows"));
    }
    let mut target = device_target(app_data);
    let mut credential = CREDENTIALW::default();
    credential.Type = CRED_TYPE_GENERIC;
    credential.TargetName = target.as_mut_ptr();
    credential.CredentialBlobSize = raw.len() as u32;
    credential.CredentialBlob = raw.as_ptr() as *mut u8;
    credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
    // SAFETY: target and raw remain alive during CredWriteW; Windows copies them.
    if unsafe { CredWriteW(&credential, 0) } == 0 {
        let error = std::io::Error::last_os_error();
        return Err(AppError::storage(format!("No se pudo guardar la credencial del dispositivo en Windows: {error}")));
    }
    Ok(())
}

#[cfg(not(windows))]
fn read_device_secret(app_data: &Path) -> AppResult<Option<Vec<u8>>> {
    let path = device_path(app_data);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read(&path).map_err(|e| {
        AppError::storage(format!("No se pudo leer la configuración del dispositivo: {e}"))
    })?;
    if raw.is_empty() {
        return Ok(None);
    }
    Ok(Some(raw))
}

#[cfg(not(windows))]
fn write_device_secret(app_data: &Path, raw: &[u8]) -> AppResult<()> {
    std::fs::create_dir_all(app_data).map_err(|e| AppError::storage(e.to_string()))?;
    let path = device_path(app_data);
    std::fs::write(&path, raw).map_err(|e| {
        AppError::storage(format!("No se pudo guardar la configuración del dispositivo: {e}"))
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(|e| {
            AppError::storage(format!("No se pudo proteger la configuración del dispositivo: {e}"))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nightdesk-creds-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn import_backup_key_accepts_64_hex_and_rejects_short() {
        let dir = temp_dir();
        assert!(import_backup_key(&dir, "abcd").is_err());
        let hex = "aa".repeat(32);
        import_backup_key(&dir, &hex).unwrap();
        let loaded = ensure_backup_key(&dir).unwrap();
        assert_eq!(loaded, [0xaa; 32]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an interactive Windows logon session with Credential Manager"]
    fn device_credentials_roundtrip_and_legacy_migration() -> AppResult<()> {
        use windows_sys::Win32::Security::Credentials::{CredDeleteW, CRED_TYPE_GENERIC};

        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let target = device_target(&self.0);
                // SAFETY: NUL-terminated UTF-16 target belongs to this test.
                unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) };
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let dir = temp_dir();
        let _cleanup = Cleanup(dir.clone());
        save_device(&dir, "https://example.supabase.co", "anon-test", "device@example.test", "password-test")?;
        assert!(device_configured(&dir)?);
        assert!(device_marker_path(&dir).is_file());
        assert!(!device_path(&dir).exists(), "new credentials must not be written to plaintext JSON");
        let loaded = load_device(&dir)?;
        assert_eq!(loaded.device_email, "device@example.test");
        assert_eq!(loaded.device_password, "password-test");

        std::fs::remove_file(device_marker_path(&dir))?;
        assert!(!device_configured(&dir)?, "a removed data directory must not reuse an orphaned credential");
        assert!(load_device(&dir).is_err());
        save_device(&dir, "https://example.supabase.co", "anon-test", "device@example.test", "password-test")?;

        let target = device_target(&dir);
        // SAFETY: the target is valid and the credential was created above.
        assert_ne!(unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) }, 0);
        let legacy = DeviceCreds {
            project_url: "https://legacy.supabase.co".into(),
            anon_key: "legacy-anon".into(),
            device_email: "legacy@example.test".into(),
            device_password: "legacy-password".into(),
        };
        std::fs::write(device_path(&dir), serde_json::to_vec(&legacy).expect("serialize test credentials"))?;
        let migrated = load_device(&dir)?;
        assert_eq!(migrated.device_password, "legacy-password");
        assert!(!device_path(&dir).exists(), "migration must remove the plaintext copy");
        assert_eq!(load_device(&dir)?.device_email, "legacy@example.test");
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn device_credential_target_is_stable_across_path_case() {
        assert_eq!(
            device_target(Path::new("C:\\Users\\Naser\\AppData\\Roaming\\com.nightdesk.hotel")),
            device_target(Path::new("c:\\users\\naser\\appdata\\roaming\\COM.NIGHTDESK.HOTEL"))
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn device_credentials_persist_in_app_data_outside_windows() -> AppResult<()> {
        let dir = temp_dir();
        save_device(
            &dir,
            "http://127.0.0.1:54321/",
            "anon-test",
            "device@example.test",
            "password-test",
        )?;
        assert!(device_configured(&dir)?);
        assert!(device_marker_path(&dir).is_file());
        assert!(device_path(&dir).is_file(), "the device secret stays in app data");
        let loaded = load_device(&dir)?;
        assert_eq!(loaded.project_url, "http://127.0.0.1:54321");
        assert_eq!(loaded.device_email, "device@example.test");
        assert_eq!(loaded.device_password, "password-test");
        assert!(device_path(&dir).is_file(), "a later load must keep the stored secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(device_path(&dir)).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(!device_configured(&dir).unwrap_or(true));
        Ok(())
    }

    #[test]
    fn project_url_rejects_remote_http_and_allows_local_development() {
        assert!(validated_project_url("http://remote.example.test").is_err());
        assert_eq!(
            validated_project_url("http://localhost:54321/").unwrap(),
            "http://localhost:54321"
        );
        assert_eq!(
            validated_project_url("https://example.supabase.co/").unwrap(),
            "https://example.supabase.co"
        );
    }

    #[test]
    fn remote_config_rejects_insecure_saved_url() -> AppResult<()> {
        let dir = temp_dir();
        assert!(save_remote(&dir, "http://remote.example.test", "anon-test").is_err());
        std::fs::write(
            remote_path(&dir),
            r#"{"project_url":"http://remote.example.test","anon_key":"anon-test"}"#,
        )?;
        assert!(load_remote(&dir).is_err());
        save_remote(&dir, "https://example.supabase.co/", "anon-test")?;
        assert_eq!(load_remote(&dir)?.unwrap().0, "https://example.supabase.co");
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }
}
