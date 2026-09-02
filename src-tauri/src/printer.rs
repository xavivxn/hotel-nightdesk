use crate::error::AppResult;
use crate::models::{AppSettings, BillPreview, Stay};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn build_receipt(
    settings: &AppSettings,
    stay: &Stay,
    bill: &BillPreview,
    payment_method: &str,
) -> Vec<u8> {
    let width: usize = if settings.paper_width <= 58 { 32 } else { 48 };
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1B, 0x40]); // init
    out.extend_from_slice(&[0x1B, 0x74, 16]); // code page windows-1252 when supported
    out.extend_from_slice(&[0x1B, 0x61, 1]); // center
    out.extend_from_slice(&[0x1D, 0x21, 0x11]); // double size
    writeln_ascii(&mut out, &sanitize(&settings.business_name));
    out.extend_from_slice(&[0x1D, 0x21, 0x00]);
    if !settings.address.is_empty() {
        writeln_ascii(&mut out, &sanitize(&settings.address));
    }
    if !settings.phone.is_empty() {
        writeln_ascii(&mut out, &sanitize(&settings.phone));
    }
    writeln_ascii(&mut out, "");
    out.extend_from_slice(&[0x1B, 0x61, 0]); // left
    writeln_ascii(&mut out, &"-".repeat(width));
    writeln_ascii(&mut out, &format!("Habitacion: {}", stay.room_number));
    writeln_ascii(
        &mut out,
        &format!("Huesped: {}", sanitize(&stay.guest_name)),
    );
    writeln_ascii(
        &mut out,
        &format!("Entrada: {}", short_dt(&stay.check_in_at)),
    );
    if let Some(out_at) = &stay.check_out_at {
        writeln_ascii(&mut out, &format!("Salida:  {}", short_dt(out_at)));
    }
    writeln_ascii(&mut out, &format!("Duracion: {}", bill.duration_label));
    writeln_ascii(&mut out, &"-".repeat(width));
    for line in &bill.lines {
        writeln_ascii(
            &mut out,
            &kv_line(
                width,
                &sanitize(&line.description),
                line.amount_cents,
                &settings.currency_symbol,
            ),
        );
    }
    writeln_ascii(&mut out, &"-".repeat(width));
    if bill.tax_cents != 0 {
        writeln_ascii(
            &mut out,
            &kv_line(
                width,
                "Subtotal",
                bill.subtotal_cents,
                &settings.currency_symbol,
            ),
        );
        writeln_ascii(
            &mut out,
            &kv_line(
                width,
                &format!("IVA {}%", bill.tax_percent),
                bill.tax_cents,
                &settings.currency_symbol,
            ),
        );
    }
    out.extend_from_slice(&[0x1B, 0x45, 1]);
    writeln_ascii(
        &mut out,
        &kv_line(width, "TOTAL", bill.total_cents, &settings.currency_symbol),
    );
    out.extend_from_slice(&[0x1B, 0x45, 0]);
    writeln_ascii(
        &mut out,
        &format!("Pago: {}", payment_label(payment_method)),
    );
    writeln_ascii(&mut out, &"-".repeat(width));
    out.extend_from_slice(&[0x1B, 0x61, 1]);
    writeln_ascii(&mut out, &sanitize(&settings.receipt_footer));
    writeln_ascii(&mut out, "");
    writeln_ascii(&mut out, "");
    out.extend_from_slice(&[0x1D, 0x56, 0x41, 0x10]); // partial cut
    out
}

pub fn print_bytes(
    bytes: &[u8],
    settings: &AppSettings,
    app_data: &Path,
    label: &str,
) -> AppResult<Option<String>> {
    archive_ticket(bytes, app_data, label);
    if !settings.printer_enabled {
        return Ok(None);
    }
    if let Err(e) = send_to_printer(bytes, settings) {
        return Ok(Some(e));
    }
    Ok(None)
}

pub fn build_test_receipt(settings: &AppSettings) -> Vec<u8> {
    let width: usize = if settings.paper_width <= 58 { 32 } else { 48 };
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1B, 0x40]);
    out.extend_from_slice(&[0x1B, 0x61, 1]);
    out.extend_from_slice(&[0x1D, 0x21, 0x11]);
    writeln_ascii(&mut out, &sanitize(&settings.business_name));
    out.extend_from_slice(&[0x1D, 0x21, 0x00]);
    writeln_ascii(&mut out, "Prueba de impresion");
    writeln_ascii(&mut out, &format!("Ancho: {} mm", settings.paper_width));
    writeln_ascii(&mut out, &"-".repeat(width.min(32)));
    writeln_ascii(&mut out, "Nightdesk listo");
    writeln_ascii(&mut out, "");
    out.extend_from_slice(&[0x1D, 0x56, 0x41, 0x10]);
    out
}

fn send_to_printer(bytes: &[u8], settings: &AppSettings) -> Result<(), String> {
    if !settings.printer_path.trim().is_empty() {
        std::fs::write(settings.printer_path.trim(), bytes)
            .map_err(|e| format!("No se pudo escribir en el puerto: {e}"))?;
        return Ok(());
    }
    if settings.printer_name.trim().is_empty() {
        return Err("Configurá el nombre o la ruta de la impresora".into());
    }
    print_named(bytes, settings.printer_name.trim())
}

fn print_named(bytes: &[u8], name: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let tmp = std::env::temp_dir().join("nightdesk-ticket.bin");
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        let status = Command::new("print")
            .args(["/D:", &format!("\\\\localhost\\{name}")])
            .arg(&tmp)
            .status()
            .or_else(|_| {
                Command::new("cmd")
                    .args([
                        "/C",
                        &format!("copy /B \"{}\" \"\\\\localhost\\{}\"", tmp.display(), name),
                    ])
                    .status()
            })
            .map_err(|e| format!("No se pudo enviar al spooler: {e}"))?;
        if !status.success() {
            return Err("El spooler de Windows rechazó el ticket".into());
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let mut child = Command::new("lp")
            .args(["-d", name, "-o", "raw"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("No se encontró el comando lp: {e}"))?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(bytes).map_err(|e| e.to_string())?;
        }
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("lp falló: {err}"));
        }
        Ok(())
    }
}

fn archive_ticket(bytes: &[u8], app_data: &Path, label: &str) {
    let dir = app_data.join("tickets");
    let _ = std::fs::create_dir_all(&dir);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path: PathBuf = dir.join(format!("{label}-{stamp}.bin"));
    let _ = std::fs::write(&path, bytes);
    let _ = std::fs::write(dir.join("last-ticket.bin"), bytes);
    let readable = bytes
        .iter()
        .filter(|b| **b == b'\n' || (**b >= 32 && **b < 127))
        .copied()
        .collect::<Vec<_>>();
    let _ = std::fs::write(dir.join("last-ticket.txt"), readable);
}

fn writeln_ascii(out: &mut Vec<u8>, line: &str) {
    out.extend_from_slice(line.as_bytes());
    out.extend_from_slice(b"\n");
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'Á' | 'À' | 'Ä' | 'Â' => 'A',
            'É' | 'È' | 'Ë' | 'Ê' => 'E',
            'Í' | 'Ì' | 'Ï' | 'Î' => 'I',
            'Ó' | 'Ò' | 'Ö' | 'Ô' => 'O',
            'Ú' | 'Ù' | 'Ü' | 'Û' => 'U',
            'Ñ' => 'N',
            '¿' | '¡' => ' ',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

fn kv_line(width: usize, label: &str, cents: i64, symbol: &str) -> String {
    let amount = format_money(cents, symbol);
    let max_label = width.saturating_sub(amount.len() + 1);
    let mut label = label.chars().take(max_label).collect::<String>();
    if label.len() < max_label {
        label.push_str(&" ".repeat(max_label - label.len()));
    }
    format!("{label} {amount}")
}

fn format_money(amount: i64, symbol: &str) -> String {
    let negative = amount < 0;
    let whole_s = thousand_sep(amount.abs());
    let mut s = format!("{whole_s} {symbol}");
    if negative {
        s = format!("-{s}");
    }
    s
}

fn thousand_sep(value: i64) -> String {
    let s = value.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn short_dt(rfc: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc)
        .map(|dt| dt.format("%d/%m %H:%M").to_string())
        .unwrap_or_else(|_| rfc.chars().take(16).collect())
}

fn payment_label(method: &str) -> &'static str {
    match method {
        "card" => "Tarjeta",
        "transfer" => "Transferencia",
        _ => "Efectivo",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_money_pyg_has_no_decimals_and_symbol_after() {
        assert_eq!(format_money(0, "Gs."), "0 Gs.");
        assert_eq!(format_money(5_000, "Gs."), "5.000 Gs.");
        assert_eq!(format_money(1_250_000, "Gs."), "1.250.000 Gs.");
        assert_eq!(format_money(-5_000, "Gs."), "-5.000 Gs.");
    }

    #[test]
    fn thousand_sep_groups_by_thousands() {
        assert_eq!(thousand_sep(0), "0");
        assert_eq!(thousand_sep(80_000), "80.000");
        assert_eq!(thousand_sep(1_250_000), "1.250.000");
    }
}
