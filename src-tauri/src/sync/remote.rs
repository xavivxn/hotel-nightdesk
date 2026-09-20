use super::catalog::RemoteClient;
use crate::{credentials, error::{AppError, AppResult}};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};

pub struct SupabaseRemote { creds: credentials::DeviceCreds }
impl SupabaseRemote {
    pub fn load(path: &Path) -> AppResult<Self> { Ok(Self { creds: credentials::load_device(path)? }) }
}
fn offline() -> AppError { AppError::storage("Requiere conexión con administración. No se modificó el catálogo local; si hubo un corte, reintentá con la misma operación.") }
impl RemoteClient for SupabaseRemote {
    fn write(&self, request: &Value) -> AppResult<Value> {
        let url = reqwest::Url::parse(&self.creds.project_url).map_err(|_|AppError::msg("URL de administración inválida"))?;
        let local = matches!(url.host_str(),Some("localhost"|"127.0.0.1"|"[::1]"));
        if url.scheme() != "https" && !(url.scheme()=="http" && local) { return Err(AppError::msg("Administración requiere HTTPS")); }
        let client=reqwest::blocking::Client::builder().timeout(Duration::from_secs(12))
            .connect_timeout(Duration::from_secs(4)).redirect(reqwest::redirect::Policy::none())
            .build().map_err(|_|offline())?;
        let base=self.creds.project_url.trim_end_matches('/');
        let login=client.post(format!("{base}/auth/v1/token?grant_type=password"))
            .header("apikey",&self.creds.anon_key)
            .json(&json!({"email":self.creds.device_email,"password":self.creds.device_password}))
            .send().map_err(|_|offline())?;
        if !login.status().is_success() { return Err(AppError::forbidden("No se pudo autenticar el dispositivo de recepción")); }
        let auth:Value=login.json().map_err(|_|offline())?;
        let token=auth["access_token"].as_str().ok_or_else(offline)?;
        let reply=client.post(format!("{base}/rest/v1/rpc/catalog_write"))
            .header("apikey",&self.creds.anon_key).bearer_auth(token).json(request).send().map_err(|_|offline())?;
        let status=reply.status();
        let body:Value=reply.json().map_err(|_|offline())?;
        if status.is_success() { return Ok(body); }
        let msg=body["message"].as_str().unwrap_or("");
        if msg.contains("conflict:") { return Err(AppError::conflict("La ficha cambió; recargá antes de guardar")); }
        if status.as_u16()==401 || status.as_u16()==403 || body["code"]=="42501" { return Err(AppError::forbidden("Sin permiso para modificar el catálogo")); }
        if status.is_server_error() { return Err(offline()); }
        Err(AppError::msg("Administración rechazó los datos. Revisá los campos y la migración de catálogo I11."))
    }
}
