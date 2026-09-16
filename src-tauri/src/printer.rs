use crate::error::AppResult;
use crate::models::{AppSettings, BillPreview, Stay};
#[cfg(windows)]
#[path = "windows_spooler.rs"]
mod windows_spooler;
#[cfg(not(windows))]
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(not(windows))]
use std::process::Command;
#[cfg(not(windows))]
use std::process::Stdio;

pub fn build_receipt(
    settings: &AppSettings,
    stay: &Stay,
    bill: &BillPreview,
) -> Vec<u8> {
    let width = line_width(settings.paper_width);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1B, 0x40]); // init
    select_code_page(&mut out);
    write_brand_header(&mut out);
    set_font(&mut out, 1, 1);
    set_emphasis(&mut out, true);
    if !settings.address.is_empty() {
        writeln_ticket(&mut out, &settings.address);
    }
    if !settings.phone.is_empty() {
        writeln_ticket(&mut out, &settings.phone);
    }
    writeln_ascii(&mut out, "");
    out.extend_from_slice(&[0x1B, 0x61, 0]); // left
    writeln_ascii(&mut out, &"-".repeat(width));
    writeln_ascii(&mut out, &format!("Habitacion: {}", stay.room_number));
    if !stay.guest_name.trim().is_empty() {
        writeln_ticket(&mut out, &format!("Huesped: {}", stay.guest_name));
    }
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
        let description = encode_ticket(&line.description);
        let amount = format_money(line.amount_cents, &settings.currency_symbol);
        let room = width.saturating_sub(amount.len() + 1).max(1);
        let chunks: Vec<&[u8]> = description.chunks(room).collect();
        for part in chunks.iter().take(chunks.len().saturating_sub(1)) {
            writeln_raw(&mut out, part);
        }
        write_money_line(
            &mut out,
            width,
            chunks.last().copied().unwrap_or_default(),
            &amount,
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
    writeln_ascii(&mut out, "Cuenta interna - sin cobro");
    writeln_ascii(&mut out, &"-".repeat(width));
    out.extend_from_slice(&[0x1B, 0x61, 1]);
    writeln_ticket(&mut out, &settings.receipt_footer);
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
    archive_ticket(bytes, app_data, label)?;
    if !settings.printer_enabled {
        return Ok(Some("Impresora desactivada. Ticket archivado; no enviado a papel.".into()));
    }
    if let Err(e) = send_to_printer(bytes, settings) {
        return Ok(Some(e));
    }
    Ok(None)
}

pub fn build_test_receipt(settings: &AppSettings) -> Vec<u8> {
    let width = line_width(settings.paper_width);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1B, 0x40]);
    select_code_page(&mut out);
    write_brand_header(&mut out);
    set_font(&mut out, 1, 1);
    set_emphasis(&mut out, true);
    writeln_ascii(&mut out, "Prueba de impresion");
    writeln_ascii(&mut out, &format!("Ancho: {} mm", settings.paper_width));
    writeln_ascii(&mut out, &"-".repeat(width));
    writeln_ascii(&mut out, &"1234567890".repeat(5)[..width]);
    writeln_ticket(&mut out, "Acentos: áéíóú ÁÉÍÓÚ ñÑ");
    writeln_ascii(&mut out, &kv_line(width, "Consumo", 15000, "Gs."));
    writeln_ascii(&mut out, &kv_line(width, "Descuento", -5000, "Gs."));
    writeln_ascii(&mut out, "Verificar margenes y corte");
    writeln_ascii(&mut out, "");
    out.extend_from_slice(&[0x1D, 0x56, 0x41, 0x10]);
    out
}

pub fn list_printers() -> AppResult<Vec<String>> {
    #[cfg(windows)]
    { windows_spooler::list().map_err(crate::error::AppError::printer) }
    #[cfg(not(windows))]
    { Ok(vec![]) }
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
        return windows_spooler::send(bytes, name);
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

fn archive_ticket(bytes: &[u8], app_data: &Path, label: &str) -> AppResult<()> {
    let dir = app_data.join("tickets");
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%f");
    let path: PathBuf = dir.join(format!("{label}-{stamp}.bin"));
    std::fs::write(&path, bytes)?;
    std::fs::write(dir.join("last-ticket.bin"), bytes)?;
    let readable = bytes
        .iter()
        .filter(|b| **b == b'\n' || (**b >= 32 && **b < 127))
        .copied()
        .collect::<Vec<_>>();
    std::fs::write(dir.join("last-ticket.txt"), readable)?;
    Ok(())
}

/// Columns that fit once glyphs are larger than Font A. 80 mm holds 32 at 18 dots.
fn line_width(paper_width: i64) -> usize {
    if paper_width <= 58 { 21 } else { 32 }
}

/// Ticket brand matching the logo wordmark. ESC/POS cannot load that typeface.
fn write_brand_header(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0x1B, 0x61, 1]); // center
    set_font(out, 2, 2);
    set_emphasis(out, true);
    writeln_ticket(out, "LOVE NESTT");
    set_font(out, 1, 1);
    set_emphasis(out, true);
    writeln_ticket(out, "- M O T E L -");
    set_emphasis(out, false);
    writeln_ascii(out, "");
}

/// GS ! n. Width stays 1 for the body so lines do not wrap; height 2 makes them easier to read.
fn set_font(out: &mut Vec<u8>, width: u8, height: u8) {
    let w = width.saturating_sub(1).min(7);
    let h = height.saturating_sub(1).min(7);
    out.extend_from_slice(&[0x1B, 0x4D, 0]); // Font A, same resident font as typical delivery tickets
    out.extend_from_slice(&[0x1D, 0x21, w | (h << 4)]);
}

fn set_emphasis(out: &mut Vec<u8>, on: bool) {
    let bit = u8::from(on);
    out.extend_from_slice(&[0x1B, 0x45, bit]); // bold
    out.extend_from_slice(&[0x1B, 0x47, bit]); // double-strike
}

fn writeln_ascii(out: &mut Vec<u8>, line: &str) {
    writeln_ticket(out, line);
}

fn writeln_ticket(out: &mut Vec<u8>, line: &str) {
    select_code_page(out);
    writeln_raw(out, &encode_ticket(line));
}

/// Page 2 is PC850. áéíóúñ use the same bytes as the default page 0, so they still print if ESC t is ignored.
fn select_code_page(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0x1B, 0x74, 2]);
}

fn writeln_raw(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(bytes);
    out.push(b'\n');
}

fn write_money_line(out: &mut Vec<u8>, width: usize, label: &[u8], amount: &str) {
    let amount = encode_ticket(amount);
    let max_label = width.saturating_sub(amount.len() + 1);
    let mut line = label.iter().copied().take(max_label).collect::<Vec<_>>();
    line.resize(max_label, b' ');
    line.push(b' ');
    line.extend_from_slice(&amount);
    writeln_raw(out, &line);
}

/// PC850 / shared page-0 bytes. Controls, including ESC, become `?` so text cannot inject commands.
fn encode_ticket(value: &str) -> Vec<u8> {
    value.chars().map(pc850_byte).collect()
}

fn pc850_byte(c: char) -> u8 {
    match c {
        ' '..='~' => c as u8,
        'á' => 0xA0, 'é' => 0x82, 'í' => 0xA1, 'ó' => 0xA2, 'ú' => 0xA3,
        'Á' => 0xB5, 'É' => 0x90, 'Í' => 0xD6, 'Ó' => 0xE0, 'Ú' => 0xE9,
        'ñ' => 0xA4, 'Ñ' => 0xA5,
        'ü' => 0x81, 'Ü' => 0x9A,
        'à' => 0x85, 'è' => 0x8A, 'ì' => 0x8D, 'ò' => 0x95, 'ù' => 0x97,
        'ä' => 0x84, 'ë' => 0x89, 'ï' => 0x8B, 'ö' => 0x94,
        'â' => 0x83, 'ê' => 0x88, 'î' => 0x8C, 'ô' => 0x93, 'û' => 0x96,
        '¿' => 0xA8, '¡' => 0xAD,
        _ => b'?',
    }
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
        .map(|dt| dt.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string())
        .unwrap_or_else(|_| rfc.chars().take(16).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticket_contains_full_width_and_no_injected_commands() {
        for width in [58, 80] {
            let mut settings = AppSettings::default(); settings.paper_width = width;
            settings.business_name = "Café\u{1b}@".into();
            let bytes = build_test_receipt(&settings);
            let text = String::from_utf8_lossy(&bytes);
            let rule = "-".repeat(if width == 58 { 21 } else { 32 });
            assert!(text.contains(&rule));
            assert!(bytes.windows(b"LOVE NESTT".len()).any(|part| part == b"LOVE NESTT"));
            assert!(bytes.windows(b"M O T E L".len()).any(|part| part == b"M O T E L"));
            let name = encode_ticket("Café\u{1b}@");
            assert_eq!(name, b"Caf\x82?@");
            assert!(!name.contains(&0x1B));
            assert!(bytes.windows(4).any(|part| part == [0xA0, 0x82, 0xA1, 0xA2]));
            assert!(text.contains("-5.000 Gs."));
        }
    }

    #[test]
    fn archive_failure_and_disabled_printer_are_reported() {
        let dir = std::env::temp_dir().join(format!("receipt-test-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        let settings = AppSettings::default();
        assert!(print_bytes(b"test", &settings, &dir, "test").unwrap().unwrap().contains("desactivada"));
        assert!(print_bytes(b"test", &settings, &dir.join("tickets/last-ticket.bin"), "test").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "Requires an explicitly selected physical printer"]
    fn physical_printer_sample() {
        println!("Colas disponibles: {:?}", list_printers());
        let name = std::env::var("RECEIPT_TEST_PRINTER").expect("Set RECEIPT_TEST_PRINTER explicitly");
        let mut settings = AppSettings::default();
        settings.business_name = "MotelApp".into(); settings.paper_width = 80;
        settings.printer_enabled = true; settings.printer_name = name;
        let bytes = build_test_receipt(&settings);
        send_to_printer(&bytes, &settings).expect("Windows spooler must accept the sample");
    }

    #[test]
    fn unknown_queue_is_an_error() {
        let mut settings = AppSettings::default();
        settings.printer_enabled = true;
        settings.printer_name = "COLA-INEXISTENTE-N05".into();
        settings.printer_path = String::new();
        let err = send_to_printer(b"test", &settings).expect_err("missing queue");
        assert!(err.contains("cola") || err.contains("impresora") || err.contains("OpenPrinter") || !err.is_empty());
    }

    #[test]
    #[ignore = "Requires an explicitly selected physical printer"]
    fn concurrent_spooler_jobs_are_separate() {
        let name = std::env::var("RECEIPT_TEST_PRINTER").expect("Set RECEIPT_TEST_PRINTER explicitly");
        let mut settings = AppSettings::default();
        settings.business_name = "MotelApp".into();
        settings.paper_width = 80;
        settings.printer_enabled = true;
        settings.printer_name = name.clone();
        let bytes = build_test_receipt(&settings);
        let (first, second) = std::thread::scope(|scope| {
            let a = scope.spawn(|| print_named(&bytes, &name));
            let b = scope.spawn(|| print_named(&bytes, &name));
            (a.join().unwrap(), b.join().unwrap())
        });
        first.expect("cola concurrente 1");
        second.expect("cola concurrente 2");
    }

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
