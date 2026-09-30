use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Validation,
    NotFound,
    Conflict,
    Forbidden,
    SessionExpired,
    RateLimited,
    InvalidCredentials,
    Storage,
    #[allow(dead_code)]
    Printer,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    App { code: ErrorCode, message: String },
    #[error("No se pudo guardar en este equipo")]
    Sqlite(#[from] rusqlite::Error),
    #[error("No se pudo guardar en este equipo")]
    Io(#[from] std::io::Error),
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::App {
            code,
            message: message.into(),
        }
    }

    pub fn msg(text: impl Into<String>) -> Self {
        Self::new(ErrorCode::Validation, text)
    }

    pub fn not_found(text: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, text)
    }

    pub fn conflict(text: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, text)
    }

    pub fn forbidden(text: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, text)
    }

    pub fn session_expired(text: impl Into<String>) -> Self {
        Self::new(ErrorCode::SessionExpired, text)
    }

    pub fn rate_limited(text: impl Into<String>) -> Self {
        Self::new(ErrorCode::RateLimited, text)
    }

    pub fn invalid_credentials(text: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidCredentials, text)
    }

    #[allow(dead_code)]
    pub fn storage(text: impl Into<String>) -> Self {
        Self::new(ErrorCode::Storage, text)
    }

    #[allow(dead_code)]
    pub fn printer(text: impl Into<String>) -> Self {
        Self::new(ErrorCode::Printer, text)
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::App { code, .. } => *code,
            Self::Sqlite(_) | Self::Io(_) => ErrorCode::Storage,
        }
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: ErrorCode,
    message: &'a str,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let message = self.to_string();
        ErrorBody {
            code: self.code(),
            message: &message,
        }
        .serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_and_spanish_message() {
        let error = AppError::conflict("La habitación ya está ocupada");
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"code\":\"conflict\""));
        assert!(json.contains("La habitación ya está ocupada"));
        assert_eq!(error.to_string(), "La habitación ya está ocupada");
    }

    #[test]
    fn storage_and_printer_codes_serialize() {
        let storage = AppError::storage("No se pudo leer la base");
        let printer = AppError::printer("La impresora no responde");
        assert!(serde_json::to_string(&storage).unwrap().contains("\"code\":\"storage\""));
        assert!(serde_json::to_string(&printer).unwrap().contains("\"code\":\"printer\""));
        assert_eq!(storage.code(), ErrorCode::Storage);
        assert_eq!(printer.code(), ErrorCode::Printer);
    }

    #[test]
    fn sqlite_and_io_hide_engine_details() {
        let sqlite = AppError::from(rusqlite::Error::InvalidQuery);
        let io = AppError::from(std::io::Error::other("disk"));
        let sqlite_json = serde_json::to_string(&sqlite).unwrap();
        let io_json = serde_json::to_string(&io).unwrap();
        assert!(sqlite_json.contains("No se pudo guardar en este equipo"));
        assert!(io_json.contains("No se pudo guardar en este equipo"));
        assert!(!sqlite_json.to_lowercase().contains("sqlite"));
        assert!(!sqlite_json.to_lowercase().contains("rusqlite"));
        assert!(!io_json.contains("disk"));
    }
}
