import type { AnalyticsSummary } from "./types";

function literal(value: string) {
  return value.normalize("NFC").split("").map(char => {
    const code = char.charCodeAt(0);
    if (char === "(" || char === ")" || char === "\\") return `\\${char}`;
    if (code >= 32 && code < 127) return char;
    if (code >= 160 && code <= 255) return `\\${code.toString(8).padStart(3, "0")}`;
    return "?";
  }).join("");
}

const money = (amount: number) => `${amount.toLocaleString("es-PY")} Gs.`;
const statusLabel: Record<string, string> = {
  available: "Libres", occupied: "Ocupadas", dirty: "Por limpiar",
  reserved: "Reservadas", blocked: "Bloqueadas",
};

export function buildAnalyticsPdf(report: AnalyticsSummary, roomType?: string): Uint8Array {
  const lines: string[] = [];
  const add = (line = "") => {
    const clean = line.replace(/[\r\n\t]/g, " ");
    if (!clean) { lines.push(""); return; }
    let rest = clean;
    while (rest.length > 86) {
      const at = rest.lastIndexOf(" ", 86);
      const cut = at > 0 ? at : 86;
      lines.push(rest.slice(0, cut));
      rest = rest.slice(cut).trimStart();
    }
    lines.push(rest);
  };

  add(`Período: ${report.from} al ${report.to} | Habitaciones: ${roomType || "Todas"}`);
  add(`Generado: ${new Date(report.generated_at).toLocaleString("es-PY")}`);
  add("Informe interno. Importes de cuentas cerradas; no es comprobante fiscal ni registra cobros.");
  add(); add("INDICADORES");
  add(`Ingresos de cuentas cerradas: ${money(report.total_revenue_cents)}`);
  add(`Cuentas cerradas: ${report.closed_accounts} | Ticket promedio: ${money(report.average_ticket_cents)}`);
  add(`Alojamiento: ${money(report.lodging_cents)} | Cargos adicionales: ${money(report.extras_cents)}`);
  add(`Descuentos: ${money(report.discount_cents)} | Impuestos: ${money(report.tax_cents)}`);
  add(`Duración promedio de estadía cerrada: ${report.average_stay_minutes === null ? "Sin datos" : `${report.average_stay_minutes} min`}`);
  add(`Llegadas previstas: ${report.reservation_arrivals} | Canceladas: ${report.reservation_cancellations} | No se presentaron: ${report.no_shows}`);
  add("Las reservas se agrupan por fecha de llegada prevista, no de cancelación.");
  add(); add("ESTADO ACTUAL DE HABITACIONES (NO CORRESPONDE AL PERÍODO)");
  for (const status of report.current_rooms) add(`${statusLabel[status.status] ?? status.status}: ${status.count}`);
  add(); add("EVOLUCIÓN DIARIA");
  add("Fecha          Cuentas      Entradas       Total cerrado");
  for (const day of report.daily) {
    add(`${day.date.padEnd(14)} ${String(day.closed_accounts).padEnd(12)} ${String(day.check_ins).padEnd(14)} ${money(day.revenue_cents)}`);
  }
  add(); add("RESERVAS POR LLEGADA PREVISTA");
  add("Fecha          Previstas       Canceladas      No se presentaron");
  for (const day of report.daily) {
    add(`${day.date.padEnd(14)} ${String(day.reservation_arrivals).padEnd(15)} ${String(day.reservation_cancellations).padEnd(16)} ${day.no_shows}`);
  }
  add(); add("ENTRADAS POR HORA");
  for (const item of report.check_in_hours) add(`${String(item.hour).padStart(2, "0")}:00 | ${item.count}`);
  add(); add("POR TIPO DE HABITACIÓN");
  for (const row of report.by_room_type) add(`${row.room_type}: ${row.closed_accounts} cuentas | ${money(row.revenue_cents)}`);
  add(); add("HABITACIONES CON MÁS INGRESOS");
  for (const row of report.by_room.slice(0, 15)) add(`${row.room_number} (${row.room_type}): ${row.closed_accounts} cuentas | ${money(row.revenue_cents)}`);
  add(); add("CARGOS ADICIONALES MÁS FRECUENTES");
  add("Se agrupan por descripción; pueden incluir consumos y recargos manuales.");
  for (const row of report.top_extras) add(`${row.description}: ${row.count} | ${money(row.revenue_cents)}`);
  if (!report.top_extras.length) add("Sin cargos adicionales en el período.");

  const pages: string[][] = [];
  for (let index = 0; index < lines.length; index += 43) pages.push(lines.slice(index, index + 43));
  const objects: string[] = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "",
    "<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>",
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>",
  ];
  const children: string[] = [];
  pages.forEach((page, index) => {
    const pageId = objects.length + 1;
    const streamId = pageId + 1;
    children.push(`${pageId} 0 R`);
    objects.push(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents ${streamId} 0 R >>`);
    let stream = `0.51 0.20 0.26 rg 40 780 515 3 re f\n0.13 0.09 0.12 rg\nBT /F2 18 Tf 40 805 Td (${literal("Análisis del negocio")}) Tj ET\n`;
    stream += "BT /F1 9 Tf 40 755 Td 15 TL\n";
    for (const line of page) stream += `(${literal(line)}) Tj T*\n`;
    stream += `ET\nBT /F1 9 Tf 40 42 Td (${literal(`${report.from} a ${report.to} | Página ${index + 1} de ${pages.length}`)}) Tj ET\n`;
    objects.push(`<< /Length ${stream.length} >>\nstream\n${stream}endstream`);
  });
  objects[1] = `<< /Type /Pages /Kids [${children.join(" ")}] /Count ${pages.length} >>`;
  let pdf = "%PDF-1.4\n";
  const offsets: number[] = [0];
  objects.forEach((object, index) => {
    offsets.push(pdf.length);
    pdf += `${index + 1} 0 obj\n${object}\nendobj\n`;
  });
  const xref = pdf.length;
  pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (const offset of offsets.slice(1)) pdf += `${String(offset).padStart(10, "0")} 00000 n \n`;
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return new TextEncoder().encode(pdf);
}
