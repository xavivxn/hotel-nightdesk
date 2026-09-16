//! RAW ESC/POS through the Windows spooler; no shell or shared temporary file.
use std::ffi::c_void;
type Handle = *mut c_void;
#[repr(C)]
struct DocInfo { name: *const u16, output: *const u16, datatype: *const u16 }
#[repr(C)]
struct PrinterInfo { name: *const u16, server: *const u16, attributes: u32 }
#[link(name = "winspool")]
extern "system" {
    fn EnumPrintersW(flags: u32, name: *const u16, level: u32, buffer: *mut u8, size: u32, needed: *mut u32, returned: *mut u32) -> i32;
    fn OpenPrinterW(name: *const u16, handle: *mut Handle, defaults: *const c_void) -> i32;
    fn ClosePrinter(handle: Handle) -> i32;
    fn StartDocPrinterW(handle: Handle, level: u32, info: *const DocInfo) -> u32;
    fn EndDocPrinter(handle: Handle) -> i32;
    fn StartPagePrinter(handle: Handle) -> i32;
    fn EndPagePrinter(handle: Handle) -> i32;
    fn AbortPrinter(handle: Handle) -> i32;
    fn WritePrinter(handle: Handle, data: *const u8, size: u32, written: *mut u32) -> i32;
}
struct Printer(Handle);
impl Drop for Printer { fn drop(&mut self) { unsafe { ClosePrinter(self.0); } } }
fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }
fn error() -> String { format!("La cola de Windows rechazó el ticket: {}", std::io::Error::last_os_error()) }
pub fn list() -> Result<Vec<String>, String> {
    unsafe {
        let (mut needed, mut returned) = (0, 0);
        let ok = EnumPrintersW(6, std::ptr::null(), 4, std::ptr::null_mut(), 0, &mut needed, &mut returned);
        if needed == 0 {
            return if ok != 0 { Ok(vec![]) } else { Err(error()) };
        }
        // Native structs require pointer alignment, unlike a Vec<u8>.
        let mut buffer = vec![0usize; (needed as usize + std::mem::size_of::<usize>() - 1) / std::mem::size_of::<usize>()];
        let size = needed;
        if EnumPrintersW(6, std::ptr::null(), 4, buffer.as_mut_ptr().cast(), size, &mut needed, &mut returned) == 0 { return Err(error()); }
        let entries = std::slice::from_raw_parts(buffer.as_ptr().cast::<PrinterInfo>(), returned as usize);
        let mut names = Vec::new();
        for entry in entries {
            if entry.name.is_null() { continue; }
            let mut len = 0;
            while *entry.name.add(len) != 0 { len += 1; }
            names.push(String::from_utf16_lossy(std::slice::from_raw_parts(entry.name, len)));
        }
        names.sort(); names.dedup(); Ok(names)
    }
}
pub fn send(bytes: &[u8], name: &str) -> Result<(), String> {
    if name.contains('\0') { return Err("Nombre de impresora inválido".into()); }
    let bytes = rasterize(bytes).unwrap_or_else(|| bytes.to_vec());
    let name = wide(name);
    let title = wide("Ticket interno");
    let raw = wide("RAW");
    let mut handle = std::ptr::null_mut();
    unsafe {
        if OpenPrinterW(name.as_ptr(), &mut handle, std::ptr::null()) == 0 { return Err(error()); }
        let printer = Printer(handle);
        let info = DocInfo { name: title.as_ptr(), output: std::ptr::null(), datatype: raw.as_ptr() };
        if StartDocPrinterW(printer.0, 1, &info) == 0 { return Err(error()); }
        if StartPagePrinter(printer.0) == 0 { let message = error(); AbortPrinter(printer.0); return Err(message); }
        let mut offset = 0;
        while offset < bytes.len() {
            let mut written = 0;
            let size = (bytes.len() - offset).min(u32::MAX as usize) as u32;
            if WritePrinter(printer.0, bytes[offset..].as_ptr(), size, &mut written) == 0 || written == 0 {
                let message = error(); AbortPrinter(printer.0); return Err(message);
            }
            offset += written as usize;
        }
        if EndPagePrinter(printer.0) == 0 { let message = error(); AbortPrinter(printer.0); return Err(message); }
        if EndDocPrinter(printer.0) == 0 { let message = error(); AbortPrinter(printer.0); return Err(message); }
    }
    Ok(())
}

const PAPER_80: i32 = 576;

struct Line { text: Vec<u8>, align: u8, mag_w: u8, mag_h: u8 }

fn rasterize(bytes: &[u8]) -> Option<Vec<u8>> {
    let lines = text_lines(bytes);
    if lines.iter().all(|line| line.text.is_empty()) { return None; }
    let width = PAPER_80;
    let mut height = 8i32;
    for line in &lines {
        height += line_height(line);
    }
    height += 16;
    let pixels = draw(&lines, width, height)?;
    let mut out = vec![0x1B, 0x40];
    let mut y = 0;
    while y < height {
        let band = (height - y).min(512);
        out.extend_from_slice(&[0x1D, 0x76, 0x30, 0x00]);
        let row_bytes = (width / 8) as usize;
        out.extend_from_slice(&(row_bytes as u16).to_le_bytes());
        out.extend_from_slice(&(band as u16).to_le_bytes());
        for row in y..y + band {
            let start = row as usize * row_bytes;
            out.extend_from_slice(&pixels[start..start + row_bytes]);
        }
        y += band;
    }
    out.extend_from_slice(&[0x1B, 0x64, 4, 0x1D, 0x56, 0x41, 0x10]);
    Some(out)
}

/// Visible size, not the CreateFont cell. Negative height is the letter itself: bigger than Font A (24 dots), smaller than 2× (48).
fn glyph_box(line: &Line) -> (i32, i32) {
    if line.mag_w > 1 || line.mag_h > 1 { (24, -52) } else { (18, -40) }
}

fn line_height(line: &Line) -> i32 {
    if line.text.is_empty() { 16 } else { glyph_box(line).1.unsigned_abs() as i32 + 6 }
}

fn text_lines(bytes: &[u8]) -> Vec<Line> {
    let (mut lines, mut current) = (Vec::new(), Vec::new());
    let (mut align, mut mag_w, mut mag_h, mut i) = (0u8, 1u8, 1u8, 0usize);
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            lines.push(Line { text: current, align, mag_w, mag_h });
            current = Vec::new();
            i += 1;
        } else if b == 0x1B && i + 1 < bytes.len() {
            match bytes[i + 1] {
                0x61 => { align = *bytes.get(i + 2).unwrap_or(&0); i += 3; }
                0x40 => i += 2,
                0x74 | 0x4D | 0x45 | 0x47 | 0x64 => i += 3,
                _ => i += 2,
            }
        } else if b == 0x1D && i + 1 < bytes.len() {
            match bytes[i + 1] {
                0x21 => {
                    let n = *bytes.get(i + 2).unwrap_or(&0);
                    mag_w = (n & 7) + 1;
                    mag_h = ((n >> 4) & 7) + 1;
                    i += 3;
                }
                0x56 => i += 4,
                _ => i += 2,
            }
        } else if b >= 0x20 {
            current.push(b);
            i += 1;
        } else {
            i += 1;
        }
    }
    if !current.is_empty() { lines.push(Line { text: current, align, mag_w, mag_h }); }
    lines
}

fn draw(lines: &[Line], width: i32, height: i32) -> Option<Vec<u8>> {
    unsafe {
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        if dc.is_null() { return None; }
        let _dc = Dc(dc);
        let mut info = BitmapInfoHeader {
            size: 40, width, height: -height, planes: 1, bit_count: 32,
            compression: 0, size_image: 0, x_ppm: 0, y_ppm: 0, clr_used: 0, clr_important: 0,
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &mut info, 0, &mut bits, std::ptr::null_mut(), 0);
        if bitmap.is_null() || bits.is_null() { return None; }
        let previous = SelectObject(dc, bitmap);
        SetBkMode(dc, 2);
        SetBkColor(dc, 0x00FF_FFFF);
        SetTextColor(dc, 0);
        let pixel_count = (width * height) as usize;
        std::ptr::write_bytes(bits, 0xFF, pixel_count * 4);
        let mut y = 8i32;
        for line in lines {
            let (cell_w, cell_h) = glyph_box(line);
            let text = decode_pc850(&line.text);
            let text_w = line.text.len() as i32 * cell_w;
            let x = match line.align {
                1 => (width - text_w) / 2,
                2 => width - text_w,
                _ => 0,
            }.max(0);
            if !text.is_empty() {
                let font = CreateFontW(cell_h, cell_w, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Courier New").as_ptr());
                if !font.is_null() {
                    let old = SelectObject(dc, font);
                    for (index, ch) in text.chars().enumerate() {
                        let glyph = wide(&ch.to_string());
                        TextOutW(dc, x + index as i32 * cell_w, y, glyph.as_ptr(), (glyph.len() as i32 - 1).max(0));
                    }
                    SelectObject(dc, old);
                    DeleteObject(font);
                }
            }
            y += line_height(line);
        }
        let src = std::slice::from_raw_parts(bits.cast::<u8>(), pixel_count * 4);
        let mut rows = vec![0u8; pixel_count / 8];
        for row in 0..height as usize {
            for col in 0..width as usize {
                let px = src[(row * width as usize + col) * 4];
                if px < 180 {
                    rows[row * (width as usize / 8) + col / 8] |= 0x80 >> (col % 8);
                }
            }
        }
        SelectObject(dc, previous);
        DeleteObject(bitmap);
        Some(rows)
    }
}

fn decode_pc850(bytes: &[u8]) -> String {
    bytes.iter().map(|b| match b {
        0x20..=0x7E => *b as char,
        0xA0 => 'á', 0x82 => 'é', 0xA1 => 'í', 0xA2 => 'ó', 0xA3 => 'ú',
        0xB5 => 'Á', 0x90 => 'É', 0xD6 => 'Í', 0xE0 => 'Ó', 0xE9 => 'Ú',
        0xA4 => 'ñ', 0xA5 => 'Ñ', 0x81 => 'ü', 0x9A => 'Ü',
        0xA8 => '¿', 0xAD => '¡',
        _ => '?',
    }).collect()
}

struct Dc(*mut c_void);
impl Drop for Dc { fn drop(&mut self) { unsafe { DeleteDC(self.0); } } }
#[repr(C)]
struct BitmapInfoHeader {
    size: u32, width: i32, height: i32, planes: u16, bit_count: u16, compression: u32,
    size_image: u32, x_ppm: i32, y_ppm: i32, clr_used: u32, clr_important: u32,
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: *mut c_void) -> *mut c_void;
    fn DeleteDC(hdc: *mut c_void) -> i32;
    fn CreateDIBSection(hdc: *mut c_void, info: *mut BitmapInfoHeader, usage: u32, bits: *mut *mut c_void, section: *mut c_void, offset: u32) -> *mut c_void;
    fn CreateFontW(height: i32, width: i32, escape: i32, orient: i32, weight: i32, italic: u32, underline: u32, strike: u32, charset: u32, out: u32, clip: u32, quality: u32, pitch: u32, face: *const u16) -> *mut c_void;
    fn SelectObject(hdc: *mut c_void, object: *mut c_void) -> *mut c_void;
    fn DeleteObject(object: *mut c_void) -> i32;
    fn SetBkMode(hdc: *mut c_void, mode: i32) -> i32;
    fn SetBkColor(hdc: *mut c_void, color: u32) -> u32;
    fn SetTextColor(hdc: *mut c_void, color: u32) -> u32;
    fn TextOutW(hdc: *mut c_void, x: i32, y: i32, text: *const u16, len: i32) -> i32;
}
