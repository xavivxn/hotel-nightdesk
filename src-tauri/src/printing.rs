//! Print jobs are separate from account transactions. A spooler ACK is not paper confirmation.
use crate::{
    device,
    error::{AppError, AppResult},
    models::*,
    operations::{digest, field},
    AppState,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

pub fn document(conn: &Connection, stay_id: i64) -> AppResult<Value> {
    let bytes = crate::service::receipt_bytes(conn, stay_id)?;
    let raw: Option<String> = conn.query_row(
        "SELECT document_json FROM receipt_snapshots WHERE stay_id=?1",
        [stay_id],
        |r| r.get(0),
    )?;
    let data = raw
        .map(|s| {
            serde_json::from_str::<Value>(&s).map_err(|_| AppError::storage("Comprobante inválido"))
        })
        .transpose()?;
    Ok(json!({"stay_id":stay_id,"bytes":bytes,"document":data}))
}
fn render(data: &Value, local: &AppSettings) -> AppResult<Vec<u8>> {
    if data["document"].is_null() {
        return field(data, "bytes");
    }
    let doc = &data["document"];
    let mut settings: AppSettings = field(doc, "settings")?;
    settings.paper_width = local.paper_width;
    let stay: Stay = field(doc, "stay")?;
    let bill: BillPreview = field(doc, "bill")?;
    Ok(crate::printer::build_receipt(&settings, &stay, &bill))
}
pub fn submit(
    dir: &std::path::Path,
    settings: &AppSettings,
    bytes: &[u8],
    job_id: &str,
    copies: u32,
) -> AppResult<Option<String>> {
    uuid::Uuid::parse_str(job_id).map_err(|_| AppError::msg("Trabajo de impresión inválido"))?;
    if !(1..=2).contains(&copies) {
        return Err(AppError::msg("Cantidad de copias inválida"));
    }
    if !settings.printer_enabled || settings.printer_name.trim().is_empty() {
        return Ok(Some("Elegí y habilitá una impresora en este puesto".into()));
    }
    let hash = digest(
        &json!({"bytes":bytes,"printer":settings.printer_name,"width":settings.paper_width,"copies":copies}),
    );
    let conn = device::journal(dir)?;
    for copy in 0..copies {
        let id = format!("{job_id}:{copy}");
        let previous: Option<(String, String, Option<String>)> = conn
            .query_row(
                "SELECT request_hash,status,error FROM print_jobs WHERE id=?1",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((original, status, error)) = previous {
            if original != hash {
                return Err(AppError::conflict(
                    "El trabajo de impresión tiene otros datos",
                ));
            }
            if status == "submitted" {
                continue;
            }
            return Ok(Some(error.unwrap_or_else(|| {
                "Resultado de impresión incierto. Comprobá el papel antes de reimprimir".into()
            })));
        }
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO print_jobs VALUES (?1,?2,'sending',NULL,?3)",
            params![id, hash, crate::db::now_rfc3339()],
        )?;
        if inserted == 0 {
            return Ok(Some(
                "Este ticket ya se está enviando. Comprobá el papel antes de reimprimir".into(),
            ));
        }
        // No SQLite business lock is held while the spooler runs.
        let result =
            crate::printer::print_bytes(bytes, settings, dir, &format!("job-{job_id}-{copy}"));
        let error = match result {
            Ok(error) => error,
            Err(e) => Some(e.to_string()),
        };
        conn.execute(
            "UPDATE print_jobs SET status=?1,error=?2 WHERE id=?3",
            params![
                if error.is_some() {
                    "failed"
                } else {
                    "submitted"
                },
                error,
                id
            ],
        )?;
        if error.is_some() {
            return Ok(error);
        }
    }
    Ok(None)
}
pub fn host_print(app: &AppHandle, token: Option<&str>, args: Value) -> AppResult<Value> {
    let state = app.state::<AppState>();
    crate::auth::require(&state, token, false)?;
    let (data, business) = {
        let conn = state.db.lock().unwrap();
        (
            document(&conn, field(&args, "stay_id")?)?,
            crate::service::get_settings(&conn)?,
        )
    };
    let settings = device::local_settings(&state.device.lock().unwrap(), business);
    let bytes = render(&data, &settings)?;
    Ok(json!(submit(
        &state.data_dir,
        &settings,
        &bytes,
        &field::<String>(&args, "job_id")?,
        field(&args, "copies")?
    )?))
}
pub async fn print_from_station(
    app: &AppHandle,
    command: &str,
    args: Value,
    token: Option<String>,
) -> AppResult<Value> {
    let state = app.state::<AppState>();
    let config = state.device.lock().unwrap().clone();
    let client = config.mode.as_deref() == Some("reception_client");
    let target = args["target"].as_str().unwrap_or(&config.print_target);
    if !matches!(target, "local" | "principal") {
        return Err(AppError::msg("Destino inválido"));
    }
    if command == "receipt_print" && target == "principal" && client {
        return crate::lan::client::call(app, "receipt_print_host", args, token).await;
    }
    let settings = config.printer.unwrap_or_default();
    let bytes = if command == "print_test" {
        crate::printer::build_test_receipt(&settings)
    } else {
        let data = if client {
            crate::lan::client::call(
                app,
                "receipt_document",
                json!({"stay_id":field::<i64>(&args,"stay_id")?}),
                token,
            )
            .await?
        } else {
            document(&state.db.lock().unwrap(), field(&args, "stay_id")?)?
        };
        render(&data, &settings)?
    };
    let copies = if command == "print_test" {
        1
    } else {
        field(&args, "copies")?
    };
    let id = field::<String>(&args, "job_id")?;
    let dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok(json!(submit(&dir, &settings, &bytes, &id, copies)?))
    })
    .await
    .map_err(|_| AppError::storage("No se pudo enviar el ticket"))?
}
pub fn save_pdf(dir: &std::path::Path, args: &Value) -> AppResult<Value> {
    let bytes: Vec<u8> = field(args, "bytes")?;
    if bytes.len() > 20_000_000 || !bytes.starts_with(b"%PDF-") || !bytes.ends_with(b"%%EOF\n") {
        return Err(AppError::msg("PDF inválido"));
    }
    let reports = dir.join("informes");
    std::fs::create_dir_all(&reports)?;
    let path = reports.join(format!("informe-{}.pdf", uuid::Uuid::new_v4()));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(json!(path.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn submitted_copies_and_uncertain_spooler_jobs_are_never_resent() {
        let dir =
            std::env::temp_dir().join(format!("nightdesk-print-job-{}", uuid::Uuid::new_v4()));
        let settings = AppSettings {
            printer_enabled: true,
            printer_name: "NEVER_SEND_THIS_TEST".into(),
            paper_width: 58,
            ..AppSettings::default()
        };
        let bytes = b"frozen receipt";
        let job = uuid::Uuid::new_v4().to_string();
        let hash = digest(
            &json!({"bytes":bytes.as_slice(),"printer":settings.printer_name,"width":58,"copies":2}),
        );
        let conn = device::journal(&dir).unwrap();
        for copy in 0..2 {
            conn.execute(
                "INSERT INTO print_jobs VALUES (?1,?2,'submitted',NULL,'now')",
                params![format!("{job}:{copy}"), hash],
            )
            .unwrap();
        }
        assert!(submit(&dir, &settings, bytes, &job, 2).unwrap().is_none());
        conn.execute(
            "UPDATE print_jobs SET status='sending' WHERE id=?1",
            [format!("{job}:1")],
        )
        .unwrap();
        assert!(submit(&dir, &settings, bytes, &job, 2)
            .unwrap()
            .unwrap()
            .contains("incierto"));
        assert_eq!(
            submit(&dir, &settings, b"changed", &job, 2)
                .unwrap_err()
                .code(),
            crate::error::ErrorCode::Conflict
        );
        drop(conn);
        let _ = std::fs::remove_dir_all(dir);
    }
}
