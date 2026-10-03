use super::*;
use crate::{
    auth, device,
    operations::{encode, field},
};
use rusqlite::params;

async fn require_local(
    app: &AppHandle,
    token: Option<String>,
    admin: bool,
) -> AppResult<crate::models::SessionUser> {
    let is_client = app
        .state::<AppState>()
        .device
        .lock()
        .unwrap()
        .mode
        .as_deref()
        == Some("reception_client");
    let user = if is_client {
        let session = client::call(app, "auth_session", json!({}), token).await?;
        crate::operations::decode(session["user"].clone())?
    } else {
        auth::require(&app.state::<AppState>(), token.as_deref(), admin)?
    };
    if admin && !user.role.is_admin() {
        return Err(AppError::forbidden(
            "Esta operación requiere administración",
        ));
    }
    Ok(user)
}

#[tauri::command]
pub async fn reception_invoke(
    app: AppHandle,
    command: String,
    args: Value,
    session_token: Option<String>,
) -> AppResult<Value> {
    let state = app.state::<AppState>();
    let config = state.device.lock().unwrap().clone();
    let is_client = config.mode.as_deref() == Some("reception_client");
    match command.as_str() {
        "list_printers" => {
            require_local(&app, session_token, false).await?;
            return encode(crate::printer::list_printers()?);
        }
        "printer_config_get" => {
            require_local(&app, session_token, false).await?;
            return Ok(
                json!({"settings":config.printer.unwrap_or_default(),"target":config.print_target}),
            );
        }
        "printer_config_save" => {
            require_local(&app, session_token, false).await?;
            let mut p: crate::models::AppSettings = field(&args, "settings")?;
            p.pin_hash.clear();
            let target = field::<String>(&args, "target")?;
            if !matches!(target.as_str(), "local" | "principal")
                || !matches!(p.paper_width, 58 | 80)
                || !p.printer_path.is_empty()
            {
                return Err(AppError::msg(
                    "Elegí una cola de Windows, un ancho de 58 u 80 mm y un destino válido",
                ));
            }
            let mut config = state.device.lock().unwrap();
            let mut next = config.clone();
            next.printer = Some(p);
            next.print_target = target;
            device::save(&state.data_dir, &next)?;
            *config = next;
            return Ok(Value::Null);
        }
        "receipt_print" | "print_test" => {
            require_local(&app, session_token.clone(), false).await?;
            return crate::printing::print_from_station(&app, &command, args, session_token).await;
        }
        "save_daily_pdf" | "save_analytics_pdf" => {
            require_local(&app, session_token, command == "save_analytics_pdf").await?;
            return crate::printing::save_pdf(&state.data_dir, &args);
        }
        _ => {}
    }
    let station = config.station_id.clone();
    let result = if is_client {
        client::call(&app, &command, args, session_token).await?
    } else {
        crate::backend::dispatch(&app, &station, &command, args, session_token).await?
    };
    if command == "get_settings" || command == "save_settings" {
        let business = crate::operations::decode(result)?;
        return encode(device::local_settings(&config, business));
    }
    Ok(result)
}

#[tauri::command]
pub async fn lan_control(
    app: AppHandle,
    action: String,
    args: Value,
    session_token: Option<String>,
) -> AppResult<Value> {
    let state = app.state::<AppState>();
    match action.as_str() {
        "status" => return Ok(status(&app)),
        "reconnect" => {
            let mut config = state.device.lock().unwrap();
            if config.mode.as_deref() != Some("reception_client") {
                return Err(AppError::forbidden(
                    "Solo disponible en recepción adicional",
                ));
            }
            let mut next = config.clone();
            next.paired = false;
            device::save(&state.data_dir, &next)?;
            *config = next;
            *state.lan.client_session.lock().unwrap() = None;
            return Ok(Value::Null);
        }
        "discover" => {
            return encode(
                tauri::async_runtime::spawn_blocking(discovery::discover)
                    .await
                    .map_err(|_| AppError::storage("No se pudo buscar recepción"))??,
            )
        }
        "pair" => {
            if state.device.lock().unwrap().paired {
                return Err(AppError::forbidden("Este puesto ya está vinculado"));
            }
            return client::pair(&app, field(&args, "host")?, field(&args, "name")?).await;
        }
        "pair_status" => return client::pair_status(&app).await,
        "review_pending" => {
            return client::review_pending(
                &app,
                session_token,
                field(&args, "operation_id")?,
                field(&args, "note")?,
            )
            .await
        }
        "pending" | "retry" => {
            return client::pending(
                &app,
                session_token,
                if action == "retry" {
                    Some(field(&args, "operation_id")?)
                } else {
                    None
                },
            )
            .await;
        }
        _ => {}
    }
    require_local(&app, session_token, true).await?;
    if state.device.lock().unwrap().mode.as_deref() != Some("reception") {
        return Err(AppError::forbidden(
            "Esto se configura en la recepción principal",
        ));
    }
    match action.as_str() {
        "interfaces" => {
            let list = if_addrs::get_if_addrs()
                .map_err(|_| AppError::storage("No se pudieron listar las interfaces"))?
                .into_iter()
                .filter_map(|i| match i.ip() {
                    std::net::IpAddr::V4(ip)
                        if !ip.is_loopback() && local_ip(&ip.to_string()).is_ok() =>
                    {
                        Some(json!({"name":i.name,"address":ip.to_string()}))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            Ok(json!(list))
        }
        "configure" => {
            let mut config = state.device.lock().unwrap().clone();
            let enabled = field(&args, "enabled")?;
            let address = field::<String>(&args, "address")?;
            let port = field::<u16>(&args, "port")?;
            if enabled {
                local_ip(&address)?;
                if port < 1024 {
                    return Err(AppError::msg("Puerto inválido"));
                }
            }
            config.bind_interface = if_addrs::get_if_addrs()
                .unwrap_or_default()
                .into_iter()
                .find(|i| i.ip().to_string() == address)
                .map(|i| i.name)
                .unwrap_or_default();
            if enabled && config.bind_interface.is_empty() {
                return Err(AppError::msg("Elegí una interfaz disponible en esta PC"));
            }
            config.lan_enabled = enabled;
            config.bind_address = address;
            config.port = port;
            device::save(&state.data_dir, &config)?;
            *state.device.lock().unwrap() = config;
            stop_and_wait(&app).await?;
            server::start(app.clone()).await?;
            Ok(status(&app))
        }
        "admin_info" => {
            let identity = if state.device.lock().unwrap().lan_enabled {
                Some(server::identity(&app)?)
            } else {
                None
            };
            let mut pending = state.lan.pending.lock().unwrap();
            pending.retain(|_, p| p.expires > Instant::now());
            let requests=pending.values().map(|p|json!({"station_id":p.station_id,"name":p.name,"verification_code":&p.token_hash[..12]})).collect::<Vec<_>>();
            let conn = state.db.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT id,name,active FROM lan_stations ORDER BY created_at")?;
            let stations=stmt.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"active":r.get::<_,bool>(2)?})))?.collect::<Result<Vec<_>,_>>()?;
            Ok(
                json!({"identity":identity,"pending":requests,"stations":stations,"pairing_open":state.lan.pairing_until.lock().unwrap().is_some_and(|t|Instant::now()<t)}),
            )
        }
        "pairing_open" => {
            *state.lan.pairing_until.lock().unwrap() =
                Some(Instant::now() + std::time::Duration::from_secs(120));
            Ok(Value::Null)
        }
        "approve" => {
            server::approve(&app, &field::<String>(&args, "station_id")?)?;
            Ok(Value::Null)
        }
        "revoke" => {
            state.db.lock().unwrap().execute(
                "UPDATE lan_stations SET active=0 WHERE id=?1",
                params![field::<String>(&args, "station_id")?],
            )?;
            Ok(Value::Null)
        }
        "firewall" => configure_firewall(&app),
        _ => Err(AppError::msg("Acción de conexión inválida")),
    }
}

fn configure_firewall(app: &AppHandle) -> AppResult<Value> {
    #[cfg(windows)]
    {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let config = app.state::<AppState>().device.lock().unwrap().clone();
        local_ip(&config.bind_address)?;
        let data=STANDARD.encode(json!({"exe":std::env::current_exe()?.to_string_lossy(),"address":config.bind_address,"port":config.port}).to_string());
        let script=format!("$ErrorActionPreference='Stop'; $d=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{data}'))|ConvertFrom-Json; Get-NetFirewallRule -DisplayName 'Nightdesk LAN*' -ErrorAction SilentlyContinue|Remove-NetFirewallRule; New-NetFirewallRule -DisplayName 'Nightdesk LAN TCP' -Direction Inbound -Action Allow -Program $d.exe -Protocol TCP -LocalPort $d.port -RemoteAddress LocalSubnet -Profile Private; New-NetFirewallRule -DisplayName 'Nightdesk LAN Discovery' -Direction Inbound -Action Allow -Program $d.exe -Protocol UDP -LocalPort 5353 -RemoteAddress LocalSubnet -Profile Private;");
        let encoded = STANDARD.encode(
            script
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        let launch=format!("$p=Start-Process powershell.exe -Verb RunAs -Wait -PassThru -ArgumentList '-NoProfile -NonInteractive -EncodedCommand {encoded}'; exit $p.ExitCode");
        let status = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &launch])
            .status()?;
        if !status.success() {
            return Err(AppError::storage("No se pudo configurar el firewall. Aceptá el permiso de Windows o pedí ayuda al instalador"));
        }
        Ok(Value::Null)
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err(AppError::msg(
            "El asistente de firewall está disponible en Windows",
        ))
    }
}
