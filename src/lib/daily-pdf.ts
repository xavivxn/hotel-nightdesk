import type { DailyReport } from "./types";

// PDF 1.4, embedded layout and standard fonts: works offline without a print driver.
// WinAnsi octal escapes preserve Spanish accents and keep byte offsets deterministic.
function literal(text: string) {
  return text.normalize("NFC").split("").map(c => {
    const n = c.charCodeAt(0);
    if (c === "(" || c === ")" || c === "\\") return `\\${c}`;
    if (n >= 32 && n < 127) return c;
    if (n >= 160 && n <= 255) return `\\${n.toString(8).padStart(3, "0")}`;
    return "?";
  }).join("");
}
const money = (n: number) => `${n.toLocaleString("es-PY")} Gs.`;
const stamp = (s: string) => {
  const d = new Date(s);
  return `${String(d.getDate()).padStart(2, "0")}/${String(d.getMonth() + 1).padStart(2, "0")} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
};

export function buildDailyPdf(report: DailyReport): Uint8Array {
  const lines: string[] = [];
  const boundaries = new Set<number>();
  const add = (s = "") => {
    const clean = s.replace(/[\r\n\t]/g, " ");
    if (!clean) { lines.push(""); return; }
    let rest = clean;
    while (rest.length > 88) {
      const space = rest.lastIndexOf(" ", 88);
      const cut = space > 0 ? space : 88;
      lines.push(rest.slice(0, cut)); rest = rest.slice(cut).trimStart();
    }
    lines.push(rest);
  };
  add(report.timezone);
  add(`Generado: ${stamp(report.generated_at)} | Corte: ${stamp(report.cutoff_at)}`);
  add("Documento interno. Importes de cuentas; no representa cobros ni factura fiscal.");
  add(); add("RESUMEN");
  add(`Habitaciones con ocupación durante el día: ${report.occupied_rooms}`);
  add(`Estadías del día: ${report.accounts.length}`);
  add(`Cuentas cerradas: ${report.accounts.filter(a => a.closed_on_day).length}`);
  add(`Cuentas abiertas al corte: ${report.accounts.filter(a => a.open_at_cutoff).length}`);
  add(`TOTAL DE CUENTAS CERRADAS: ${money(report.closed_total_cents)}`);
  add(`Consumos/recargos y descuentos registrados en el día: ${money(report.adjustments_total_cents)}`);
  add("Este último subtotal es informativo; no se suma al total de cuentas cerradas.");
  add(); add("ESTADÍAS Y CUENTAS");
  add("Cuenta    Hab.     Entrada       Salida        Estado al corte       Total cerrado");
  add("-".repeat(88));
  if (!report.accounts.length) add("Sin estadías para la fecha seleccionada.");
  for (const a of report.accounts) {
    const status = a.closed_on_day ? "Cerrada" : a.open_at_cutoff ? "Abierta" : "Finalizada";
    add(`${String(a.stay_id).padEnd(9)} ${a.room_number.slice(0, 7).padEnd(8)} ${stamp(a.check_in_at)}   ${a.closed_on_day && a.check_out_at ? stamp(a.check_out_at) : "     -     "}   ${status.padEnd(15)} ${a.total_cents === null ? "-" : money(a.total_cents)}`);
  }
  add(); add("CONSUMOS, RECARGOS Y DESCUENTOS DEL DÍA");
  add("Los registros de consumo y recargo comparten categoría en los datos existentes.");
  if (!report.adjustments.length) add("Sin movimientos para la fecha seleccionada.");
  for (const c of report.adjustments) {
    boundaries.add(lines.length);
    add(`#${c.id} | Cuenta ${c.stay_id} | ${stamp(c.created_at)} | ${money(c.amount_cents)}`);
    add(`  ${c.description}`);
    boundaries.add(lines.length);
  }
  add(); add("Las cuentas abiertas no se valorizan con tarifas actuales en informes históricos.");
  const pages: string[][] = [];
  for (let i = 0; i < lines.length;) {
    let end = Math.min(i + 44, lines.length);
    if (end < lines.length) {
      const boundary = [...boundaries].filter(n => n > i && n <= end).pop();
      if (boundary !== undefined) end = boundary;
    }
    pages.push(lines.slice(i, end)); i = end;
  }
  const objects: string[] = ["<< /Type /Catalog /Pages 2 0 R >>", "", "<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>", "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"];
  const kids: string[] = [];
  pages.forEach((page, index) => {
    const pageId = objects.length + 1, streamId = pageId + 1;
    kids.push(`${pageId} 0 R`);
    objects.push(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents ${streamId} 0 R >>`);
    let stream = `0.06 0.46 0.43 rg 40 780 515 3 re f\n0.08 0.10 0.12 rg\nBT /F2 18 Tf 40 805 Td (${literal("Resumen diario")}) Tj ET\nBT /F1 10 Tf 385 805 Td (${literal(report.date)}) Tj ET\n`;
    stream += "BT /F1 9 Tf 40 755 Td 15 TL\n";
    for (const line of page) stream += `(${literal(line)}) Tj T*\n`;
    stream += `ET\nBT /F1 9 Tf 40 42 Td (${literal(`Informe local | ${report.date} | Página ${index + 1} de ${pages.length}`)}) Tj ET\n`;
    objects.push(`<< /Length ${stream.length} >>\nstream\n${stream}endstream`);
  });
  objects[1] = `<< /Type /Pages /Kids [${kids.join(" ")}] /Count ${pages.length} >>`;
  let pdf = "%PDF-1.4\n";
  const offsets = [0];
  objects.forEach((o, i) => { offsets.push(pdf.length); pdf += `${i + 1} 0 obj\n${o}\nendobj\n`; });
  const xref = pdf.length;
  pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (const offset of offsets.slice(1)) pdf += `${String(offset).padStart(10, "0")} 00000 n \n`;
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return new TextEncoder().encode(pdf);
}
