import { fail } from "./errors";
import type { CreateUserPayload, LoginPayload, SessionInfo, SessionUser } from "./types";
import { ADMIN_COMMANDS } from "./permissions";

// Browser demonstration only. Production credentials are verified by Rust/Argon2id.
type User = SessionUser & { salt: string; hash: string; active: boolean };
type AuthDb = { users: User[]; attempts: Record<string, { failures: number; blocked: number }> };
const KEY = "nightdesk.mock.auth.v1";
const sessions = new Map<string, SessionInfo>();
function load(): AuthDb { return JSON.parse(localStorage.getItem(KEY) ?? '{"users":[],"attempts":{}}'); }
function save(db: AuthDb) { localStorage.setItem(KEY, JSON.stringify(db)); }
async function hash(password: string, salt: string) {
  const key = await crypto.subtle.importKey("raw", new TextEncoder().encode(password), "PBKDF2", false, ["deriveBits"]);
  const bits = await crypto.subtle.deriveBits({ name: "PBKDF2", hash: "SHA-256", salt: new TextEncoder().encode(salt), iterations: 210000 }, key, 256);
  return Array.from(new Uint8Array(bits), b => b.toString(16).padStart(2, "0")).join("");
}
export function requireMockSession(args: Record<string, unknown>, admin = false) {
  const session = sessions.get(String(args.session_token));
  const user = session && load().users.find(u => u.id === session.user.id && u.active);
  if (!session || !user || session.expires_at <= Date.now() / 1000) {
    sessions.delete(String(args.session_token));
    fail("session_expired", "La sesión terminó. Volvé a ingresar");
  }
  if (admin && user.role !== "admin") fail("forbidden", "Esta operación requiere administración");
  return { ...session, user: { id: user.id, username: user.username, role: user.role } };
}
export async function mockAuth(name: string, args: Record<string, unknown>): Promise<unknown> {
  if (!name.startsWith("auth_")) { requireMockSession(args, ADMIN_COMMANDS.has(name)); return; }
  const db = load();
  if (name === "auth_setup_required") return db.users.length === 0;
  if (name === "auth_session") return requireMockSession(args);
  if (name === "auth_logout") { sessions.delete(String(args.session_token)); return null; }
  if (name === "auth_setup" || name === "auth_create_user") {
    if (name === "auth_setup") {
      if (db.users.length) fail("forbidden", "La administración ya está configurada");
      const previous = JSON.parse(localStorage.getItem("nightdesk.mock.v4") ?? "{}").settings?.pin_hash;
      if (previous && previous !== `mock:${args.legacy_pin ?? ""}`) fail("validation", "Ingresá el PIN anterior para configurar el administrador");
    } else requireMockSession(args, true);
    const payload = args.payload as CreateUserPayload;
    const username = payload.username.trim().toLowerCase();
    if (!username || username.length > 64) fail("validation", "Ingresá un usuario de 1 a 64 caracteres");
    const length = new TextEncoder().encode(payload.password).length;
    if (length < 1 || length > 128) fail("validation", "La contraseña debe tener entre 1 y 128 bytes");
    const role = name === "auth_setup" ? "admin" : payload.role;
    if (role !== "admin" && role !== "recepcion") fail("validation", "Rol inválido");
    if (db.users.some(u => u.username === username)) fail("conflict", "Ese usuario ya existe");
    const salt = crypto.randomUUID();
    const passwordHash = await hash(payload.password, salt);
    const latest = load();
    if (name === "auth_setup" && latest.users.length) fail("forbidden", "La administración ya está configurada");
    if (latest.users.some(u => u.username === username)) fail("conflict", "Ese usuario ya existe");
    const user: SessionUser = { id: Math.max(0, ...latest.users.map(u => u.id)) + 1, username, role };
    latest.users.push({ ...user, salt, hash: passwordHash, active: true }); save(latest); return user;
  }
  if (name === "auth_login") {
    const payload = args.payload as LoginPayload;
    const username = payload.username.trim().toLowerCase();
    const attempt = db.attempts[username] ?? { failures: 0, blocked: 0 };
    if (attempt.blocked > Date.now()) fail("rate_limited", "Demasiados intentos. Esperá 5 minutos");
    const user = db.users.find(u => u.username === username && u.active);
    if (!user || await hash(payload.password, user.salt) !== user.hash) {
      const failures = attempt.blocked ? 1 : attempt.failures + 1;
      db.attempts[username] = { failures, blocked: failures >= 5 ? Date.now() + 300000 : 0 }; save(db);
      fail("invalid_credentials", "Usuario o contraseña incorrectos");
    }
    delete db.attempts[username]; save(db);
    const session: SessionInfo = { token: crypto.randomUUID(), user: { id: user.id, username: user.username, role: user.role }, expires_at: Math.floor(Date.now() / 1000) + 28800 };
    sessions.set(session.token, session); return session;
  }
  fail("validation", "Comando de acceso no implementado");
}
