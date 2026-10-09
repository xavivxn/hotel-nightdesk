import type { AnalyticsSummary } from "./types";
import { analyticsTime } from "./analytics-metrics";

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
const modeNames: Record<string, string> = { hourly: "Por hora", overnight: "Noche / dormida", unclassified: "Sin clasificar" };
const weekNames = ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"];
const decimal = (n: number) => n.toLocaleString("es-PY", { maximumFractionDigits: 1 });

/** Vector overview: prints without a browser, network, chart library or raster images. */
function insightPage(report: AnalyticsSummary): string {
  let stream = "";
  const text = (x: number, y: number, value: string, size = 9, bold = false) => {
    stream += `0.12 0.16 0.17 rg BT /${bold ? "F2" : "F3"} ${size} Tf ${x} ${y} Td (${literal(value)}) Tj ET\n`;
  };
  const rect = (x: number, y: number, width: number, height: number, color: string) => {
    if (width > 0) stream += `${color} rg ${x} ${y} ${width.toFixed(2)} ${height} re f\n`;
  };
  const accent = "0.10 0.40 0.36", neutral = "0.85 0.88 0.87";
  text(40, 754, "PRODUCTOS MÁS VENDIDOS", 12, true);
  text(40, 738, "Primeros 5 por unidades · Cuentas cerradas", 9);
  const products = report.product_sales.slice(0, 5);
  const max = Math.max(1, ...products.map(row => row.units));
  products.forEach((row, index) => {
    const y = 709 - index * 39;
    const name = row.description.length > 30 ? `${row.description.slice(0, 27)}...` : row.description;
    text(40, y, name, 9, true);
    text(40, y - 13, `${row.units} unidades | ${money(row.revenue_cents)}`, 8);
    rect(40, y - 23, 255, 5, neutral);
    rect(40, y - 23, row.units / max * 255, 5, accent);
  });
  if (!products.length) text(40, 710, "Sin ventas de productos identificados.");
  const purchase = report.product_purchase;
  text(325, 754, "ESTADÍAS CON PRODUCTOS", 12, true);
  text(325, 707, purchase.rate_percent === null ? "Sin datos" : `${decimal(purchase.rate_percent)} %`, 29, true);
  text(325, 682, `${purchase.purchasing_accounts} de ${purchase.eligible_accounts} cuentas evaluables`, 9);
  rect(325, 657, 230, 13, neutral);
  rect(325, 657, (purchase.rate_percent ?? 0) / 100 * 230, 13, accent);
  text(325, 642, "Con productos / Sin productos", 8);
  text(325, 612, "Productos por cuenta compradora", 9);
  text(325, 590, purchase.average_purchase_cents === null ? "Sin datos" : money(purchase.average_purchase_cents), 17, true);
  text(325, 567, `${purchase.incomplete_accounts} cuentas con cobertura incompleta`, 9);
  text(40, 505, report.product_tracking_since ? `Registro de productos desde ${analyticsTime(report.product_tracking_since).date}.` : "Cobertura de productos todavía no disponible.", 9);
  text(40, 491, "Los consumos históricos sin identificar no se estiman. Importes antes de descuentos e impuestos.", 8);

  text(40, 463, "APORTE POR HORA Y DORMIDAS", 12, true);
  const colors = [accent, "0.18 0.23 0.25", neutral];
  report.by_stay_mode.forEach((mode, index) => {
    rect(40 + index * 172, 438, 8, 8, colors[index]); text(53 + index * 172, 438, modeNames[mode.mode], 9);
  });
  (["closed_accounts", "lodging_cents"] as const).forEach((metric, index) => {
    const y = 405 - index * 50;
    text(40, y + 16, metric === "closed_accounts" ? "Cuentas cerradas" : "Importe de alojamiento", 9, true);
    const total = report.by_stay_mode.reduce((sum, row) => sum + row[metric], 0);
    rect(40, y, 515, 11, neutral);
    let x = 40;
    report.by_stay_mode.forEach((row, colorIndex) => {
      const width = total ? row[metric] / total * 515 : 0;
      rect(x, y, width, 11, colors[colorIndex]); x += width;
    });
    text(40, y - 13, report.by_stay_mode.map(row => `${modeNames[row.mode]}: ${total ? decimal(row[metric] / total * 100) + " %" : "sin datos"}`).join("   |   "), 8);
  });
  text(40, 324, "Alojamiento y tiempo adicional, antes de descuentos e impuestos.", 8);

  if (report.daily.length >= 7) {
    text(40, 292, "DÍAS Y HORARIOS DE MAYOR MOVIMIENTO", 12, true);
    text(40, 277, "Promedio de entradas por bloque completo de dos horas · Asunción", 9);
    const peak = Math.max(1, ...report.check_in_heatmap.map(cell => cell.average ?? 0));
    for (let column = 0; column < 12; column++) text(88 + column * 39, 254, `${String(column * 2).padStart(2, "0")}:00`, 7);
    for (let day = 0; day < 7; day++) text(40, 231 - day * 21, weekNames[day], 9);
    for (const cell of report.check_in_heatmap) {
      const intensity = cell.average === null ? 0 : cell.average / peak;
      const color = `${0.96 - intensity * 0.40} ${0.97 - intensity * 0.20} ${0.97 - intensity * 0.22}`;
      const x = 83 + cell.hour / 2 * 39, y = 226 - cell.weekday * 21;
      rect(x, y, 36, 19, color);
      text(x + 5, y + 6, cell.average === null ? "-" : decimal(cell.average), 8);
    }
    text(40, 81, "Mayor intensidad = más entradas. - = sin bloques observados. Se excluye el bloque en curso.", 8);
    text(40, 67, "El detalle completo de productos, modalidades y bloques figura en las páginas siguientes.", 8);
  } else {
    text(40, 292, "ENTRADAS POR HORA", 12, true);
    const peak = Math.max(1, ...report.check_in_hours.map(hour => hour.count));
    report.check_in_hours.forEach(hour => {
      const x = 40 + hour.hour * 21;
      rect(x, 114, 15, hour.count / peak * 135, accent);
      text(x, 100, String(hour.hour).padStart(2,"0"), 7);
      if (hour.count) text(x, 119 + hour.count / peak * 135, String(hour.count), 7);
    });
    text(40, 77, "Hora de Asunción. Para períodos de siete días o más se muestra el mapa semanal.", 8);
  }
  return stream;
}

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
  add(`Generado: ${new Date(report.generated_at).toLocaleString("es-PY", { timeZone: "America/Asuncion" })} (Asunción)`);
  add("Informe interno. Importes de cuentas cerradas; no es comprobante fiscal ni registra cobros.");
  add(); add("INDICADORES");
  add(`Ingresos de cuentas cerradas: ${money(report.total_revenue_cents)}`);
  add(`Cuentas cerradas: ${report.closed_accounts} | Ticket promedio: ${money(report.average_ticket_cents)}`);
  add(`Alojamiento: ${money(report.lodging_cents)} | Cargos adicionales: ${money(report.extras_cents)}`);
  add(`Descuentos: ${money(report.discount_cents)} | Impuestos: ${money(report.tax_cents)}`);
  add(`Duración promedio de estadía cerrada: ${report.average_stay_minutes === null ? "Sin datos" : `${report.average_stay_minutes} min`}`);
  add(`Llegadas previstas: ${report.reservation_arrivals} | Canceladas: ${report.reservation_cancellations} | No se presentaron: ${report.no_shows}`);
  add("Las reservas se agrupan por fecha de llegada prevista, no de cancelación.");
  add(); add("PRODUCTOS MÁS VENDIDOS (POR UNIDADES)");
  add(report.product_tracking_since ? `Registro confiable desde: ${analyticsTime(report.product_tracking_since).date}` : "Cobertura todavía no disponible.");
  for (const row of report.product_sales) add(`${row.description}: ${row.units} unidades | ${money(row.revenue_cents)}`);
  if (!report.product_sales.length) add("Sin productos identificados en cuentas cerradas del período.");
  add("Importes de venta antes de descuentos e impuestos. Excluye anulaciones y recargos manuales.");
  add(); add("ESTADÍAS QUE COMPRAN PRODUCTOS");
  const purchase = report.product_purchase;
  add(`${purchase.rate_percent === null ? "Sin datos" : decimal(purchase.rate_percent) + " %"} | ${purchase.purchasing_accounts} de ${purchase.eligible_accounts} cuentas evaluables`);
  add(`Importe promedio de productos por cuenta compradora: ${purchase.average_purchase_cents === null ? "Sin datos" : money(purchase.average_purchase_cents)}`);
  add(`${purchase.incomplete_accounts} cuentas con cobertura incompleta; no se estiman compras antiguas.`);
  add(); add("APORTE POR MODALIDAD DE CIERRE");
  for (const mode of report.by_stay_mode) add(`${modeNames[mode.mode]}: ${mode.closed_accounts} cuentas | Alojamiento ${money(mode.lodging_cents)} | Promedio ${mode.average_lodging_cents === null ? "Sin datos" : money(mode.average_lodging_cents)}`);
  add("Alojamiento y tiempo adicional antes de descuentos e impuestos; conversiones incluidas en dormidas.");
  if (report.daily.length >= 7) {
    add(); add("MOVIMIENTO POR DÍA Y BLOQUE (ASUNCIÓN)");
    for (const cell of report.check_in_heatmap) add(`${weekNames[cell.weekday]} ${String(cell.hour).padStart(2,"0")}:00-${String(cell.hour + 2).padStart(2,"0")}:00 | ${cell.count} entradas | ${cell.observed_blocks} bloques | Promedio ${cell.average === null ? "Sin datos" : decimal(cell.average)}`);
    add("Solo bloques completos observados. No incluye el bloque en curso ni bloques futuros.");
  }
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
  add(); add("DETALLE DE CARGOS ADICIONALES (POR IMPORTE)");
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
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
  ];
  const children: string[] = [];
  const streams = [insightPage(report), ...pages.map(page => {
    let stream = "0.13 0.16 0.17 rg BT /F1 9 Tf 40 755 Td 15 TL\n";
    for (const line of page) stream += `(${literal(line)}) Tj T*\n`;
    return stream + "ET\n";
  })];
  streams.forEach((content, index) => {
    const pageId = objects.length + 1;
    const streamId = pageId + 1;
    children.push(`${pageId} 0 R`);
    objects.push(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R >> >> /Contents ${streamId} 0 R >>`);
    let stream = `0.10 0.40 0.36 rg 40 780 515 3 re f\n0.13 0.16 0.17 rg\nBT /F2 18 Tf 40 805 Td (${literal("Análisis del negocio")}) Tj ET\n`;
    stream += content;
    stream += `BT /F1 9 Tf 40 42 Td (${literal(`${report.from} a ${report.to} | ${roomType || "Todas"} | Página ${index + 1} de ${streams.length}`)}) Tj ET\n`;
    objects.push(`<< /Length ${stream.length} >>\nstream\n${stream}endstream`);
  });
  objects[1] = `<< /Type /Pages /Kids [${children.join(" ")}] /Count ${streams.length} >>`;
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
