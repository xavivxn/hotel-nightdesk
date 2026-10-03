const formatter = new Intl.DateTimeFormat("sv-SE", { timeZone: "America/Asuncion", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23" });
export function asuncionInput(date = new Date()) { return formatter.format(date).replace(" ", "T").slice(0, 16); }
/** Interpret an explicit wall-clock input in the business timezone, independent of this PC. */
export function asuncionInstant(input: string) {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(input)) throw new Error("Ingresá fecha y hora.");
  const wall = Date.parse(`${input}:00Z`);
  let instant = wall;
  for (let i = 0; i < 3; i++) {
    const actualWall = Date.parse(`${formatter.format(new Date(instant)).replace(" ", "T")}Z`);
    instant += wall - actualWall;
  }
  if (asuncionInput(new Date(instant)) !== input) throw new Error("La hora no existe en America/Asuncion.");
  return new Date(instant).toISOString();
}
