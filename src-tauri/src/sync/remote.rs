use super::catalog::RemoteClient;
use super::client::SupabaseClient;
use crate::{
    credentials,
    error::{AppError, AppResult, ErrorCode},
};
use serde_json::Value;
use std::path::Path;

pub struct SupabaseRemote {
    client: SupabaseClient,
}

impl SupabaseRemote {
    pub fn load(path: &Path) -> AppResult<Self> {
        Ok(Self {
            client: SupabaseClient::from_device(credentials::load_device(path)?)?,
        })
    }
}

impl RemoteClient for SupabaseRemote {
    fn write(&self, request: &Value) -> AppResult<Value> {
        self.client.rpc("catalog_write", request).map_err(|error| {
            if error.code() == ErrorCode::Storage {
                AppError::storage("Requiere conexión con administración. No se modificó el catálogo local; si hubo un corte, reintentá con la misma operación.")
            } else if error.code() == ErrorCode::Forbidden {
                AppError::forbidden("Sin permiso para modificar el catálogo")
            } else {
                error
            }
        })
    }
}
