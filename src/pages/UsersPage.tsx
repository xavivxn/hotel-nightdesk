import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { Plus, Power, ShieldCheck, Trash2, UserRound } from "lucide-react";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Field, Input, PasswordInput, Select, reportInputIssue } from "@/components/ui/Field";
import { FIELD_EMPTY, requireTrimmed } from "@/lib/format";
import type { ManagedUser, SessionUser } from "@/lib/types";
import { cn } from "@/lib/utils";

function roleLabel(role: SessionUser["role"]) {
  return role === "admin" ? "Administración" : "Recepción";
}

function isSelf(user: ManagedUser, current: SessionUser) {
  return user.id === current.id || user.username === current.username.toLowerCase();
}

type Pending = { user: ManagedUser; action: "deactivate" | "delete" };

export function UsersPage({ user }: { user: SessionUser }) {
  const [users, setUsers] = useState<ManagedUser[]>([]);
  const [roleFilter, setRoleFilter] = useState<"all" | SessionUser["role"]>("all");
  const [status, setStatus] = useState<"all" | "active" | "inactive">("all");
  const [formOpen, setFormOpen] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [role, setRole] = useState<SessionUser["role"]>("recepcion");
  const [pending, setPending] = useState<Pending | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const usernameRef = useRef<HTMLInputElement>(null);

  async function load() {
    setUsers(await api.listUsers());
  }

  useEffect(() => {
    void load().catch((e) => setError(String(e)));
    function onCatalog() {
      void load().catch((e) => setError(String(e)));
    }
    window.addEventListener("sync:catalog-updated", onCatalog);
    return () => window.removeEventListener("sync:catalog-updated", onCatalog);
  }, []);

  const filtered = useMemo(() => {
    return users.filter((item) => {
      if (roleFilter !== "all" && item.role !== roleFilter) return false;
      if (status === "active" && !item.active) return false;
      if (status === "inactive" && item.active) return false;
      return true;
    });
  }, [roleFilter, status, users]);

  const activeCount = users.filter((item) => item.active).length;
  const receptionCount = users.filter((item) => item.role === "recepcion" && item.active).length;

  function openCreate() {
    setError(null);
    setNotice(null);
    setUsername("");
    setPassword("");
    setShowPassword(false);
    setRole("recepcion");
    setFormOpen(true);
  }

  async function refreshList() {
    setStatus("all");
    await load();
  }

  async function handleCreate(e: FormEvent) {
    e.preventDefault();
    const nextUser = requireTrimmed(username);
    if (!nextUser) {
      reportInputIssue(usernameRef.current, FIELD_EMPTY);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const created = await api.createUser({ username: nextUser, password, role });
      await refreshList();
      setFormOpen(false);
      setNotice(`Usuario ${created.username} creado como ${roleLabel(created.role).toLowerCase()}.`);
    } catch (err) {
      setError(String(err));
    } finally {
      setPassword("");
      setBusy(false);
    }
  }

  async function applyActive(target: ManagedUser, active: boolean) {
    setBusy(true);
    setError(null);
    setNotice(null);
    setPending(null);
    setUsers((current) =>
      current.map((item) => (item.id === target.id ? { ...item, active } : item)),
    );
    setStatus("all");
    try {
      await api.setUserActive(target.id, active, target.version);
      await load();
      setNotice(
        active
          ? `${target.username} volvió a estar activo.`
          : `${target.username} quedó desactivado. El nombre sigue reservado.`,
      );
    } catch (err) {
      await load().catch(() => undefined);
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function applyDelete(target: ManagedUser) {
    setBusy(true);
    setError(null);
    setNotice(null);
    setPending(null);
    setUsers((current) => current.filter((item) => item.id !== target.id));
    setStatus("all");
    try {
      await api.deleteUser(target.id, target.version);
      await load();
      setNotice(`${target.username} se eliminó. Ese nombre ya se puede volver a usar.`);
    } catch (err) {
      await load().catch(() => undefined);
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="px-6 py-6 lg:px-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="page-kicker">Administración</p>
          <h1 className="page-title">Usuarios</h1>
          <p className="page-description">Cuentas de acceso a este equipo. Desactivar corta el ingreso; eliminar borra la cuenta.</p>
        </div>
        <Button onClick={openCreate}>
          <Plus size={17} /> Nuevo usuario
        </Button>
      </header>

      <div className="mt-6 grid gap-3 sm:grid-cols-3">
        <div className="card rounded-lg p-4">
          <p className="page-kicker">Total</p>
          <p className="mt-1 font-mono text-2xl tabular-nums">{users.length}</p>
          <p className="mt-1 text-xs text-[var(--muted)]">Cuentas registradas</p>
        </div>
        <div className="card rounded-lg p-4">
          <p className="page-kicker">Activos</p>
          <p className="mt-1 font-mono text-2xl tabular-nums text-[var(--ok)]">{activeCount}</p>
          <p className="mt-1 text-xs text-[var(--muted)]">Pueden iniciar sesión</p>
        </div>
        <div className="card rounded-lg p-4">
          <p className="page-kicker">Recepción</p>
          <p className="mt-1 font-mono text-2xl tabular-nums">{receptionCount}</p>
          <p className="mt-1 text-xs text-[var(--muted)]">Turno de mostrador</p>
        </div>
      </div>

      <section className="card mt-6 rounded-lg p-4">
        <div className="flex flex-wrap items-center gap-3">
          <Select className="w-auto min-w-[150px]" value={roleFilter} onChange={(e) => setRoleFilter(e.target.value as "all" | SessionUser["role"])}>
            <option value="all">Todos los roles</option>
            <option value="recepcion">Recepción</option>
            <option value="admin">Administración</option>
          </Select>
          <Select className="w-auto min-w-[150px]" value={status} onChange={(e) => setStatus(e.target.value as "all" | "active" | "inactive")}>
            <option value="all">Todos</option>
            <option value="active">Activos</option>
            <option value="inactive">Desactivados</option>
          </Select>
        </div>
        <div className="mt-4 divide-y divide-[var(--line)]">
          {filtered.map((item) => {
            const mine = isSelf(item, user);
            return (
              <div key={item.id} className="flex flex-wrap items-center justify-between gap-3 py-3 first:pt-1 last:pb-1 hover:bg-[var(--surface-2)]">
                <div className="flex min-w-0 items-center gap-3">
                  <div className={cn("grid h-10 w-10 shrink-0 place-items-center rounded-lg", item.active ? "bg-[var(--accent-soft)] text-[var(--accent)]" : "bg-[var(--surface-2)] text-[var(--muted)]")}>
                    <UserRound size={18} />
                  </div>
                  <div className="min-w-0">
                    <p className={cn("truncate font-mono font-semibold", !item.active && "text-[var(--muted)]")}>
                      {item.username}
                      {mine ? <span className="ml-2 font-sans text-xs font-medium text-[var(--muted)]">Vos</span> : null}
                    </p>
                    <p className="text-xs text-[var(--muted)]">{roleLabel(item.role)} · {item.active ? "Activo" : "Desactivado"}</p>
                  </div>
                </div>
                {mine ? null : (
                  <div className="flex flex-wrap items-center gap-2">
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={busy}
                      onClick={() => {
                        setError(null);
                        setNotice(null);
                        if (item.active) setPending({ user: item, action: "deactivate" });
                        else void applyActive(item, true);
                      }}
                    >
                      <Power size={15} /> {item.active ? "Desactivar" : "Activar"}
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      disabled={busy}
                      onClick={() => {
                        setError(null);
                        setNotice(null);
                        setPending({ user: item, action: "delete" });
                      }}
                    >
                      <Trash2 size={15} /> Eliminar
                    </Button>
                  </div>
                )}
              </div>
            );
          })}
          {filtered.length === 0 ? <p className="py-8 text-center text-sm text-[var(--muted)]">No hay cuentas para esos filtros.</p> : null}
        </div>
      </section>

      <div className="mt-4 rounded-lg border border-[var(--line)] bg-[var(--surface-2)] p-4">
        <div className="flex items-center gap-2 font-semibold">
          <ShieldCheck size={17} className="text-[var(--accent)]" /> Permisos
        </div>
        <p className="mt-2 text-sm text-[var(--muted)]">
          Este menú es solo para administración. Desactivar aplica baja lógica y reserva el nombre. Eliminar borra la cuenta y libera el usuario.
        </p>
      </div>

      {error ? <p className="login-error mt-4 text-sm text-[var(--danger)]">{error}</p> : null}
      {notice ? <p className="mt-4 text-sm text-[var(--muted)]">{notice}</p> : null}

      <Dialog
        open={formOpen}
        title="Nuevo usuario"
        subtitle="Va a poder entrar a este equipo con su usuario y contraseña."
        onClose={() => !busy && setFormOpen(false)}
      >
        <form className="space-y-4" onSubmit={(e) => void handleCreate(e)}>
          <Field label="Usuario">
            <Input
              ref={usernameRef}
              required
              autoComplete="off"
              autoFocus
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </Field>
          <Field label="Contraseña">
            <PasswordInput
              required
              show={showPassword}
              onToggle={() => setShowPassword((v) => !v)}
              autoComplete="new-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          <Field label="Rol">
            <Select value={role} onChange={(e) => setRole(e.target.value as SessionUser["role"])}>
              <option value="recepcion">Recepción</option>
              <option value="admin">Administración</option>
            </Select>
          </Field>
          {error ? <p className="login-error text-sm text-[var(--danger)]">{error}</p> : null}
          <div className="flex justify-end gap-2 pt-2">
            <Button type="button" variant="secondary" disabled={busy} onClick={() => setFormOpen(false)}>Cancelar</Button>
            <Button type="submit" disabled={busy}>{busy ? "Creando…" : "Crear usuario"}</Button>
          </div>
        </form>
      </Dialog>

      <Dialog
        open={pending?.action === "deactivate"}
        title="Desactivar usuario"
        subtitle={pending ? `${pending.user.username} no va a poder entrar. El nombre queda reservado.` : undefined}
        onClose={() => !busy && setPending(null)}
      >
        <div className="space-y-4">
          <div className="space-y-2 text-sm text-[var(--muted)]">
            <p>Después de confirmar:</p>
            <ul className="list-disc space-y-1 pl-5">
              <li>No va a poder iniciar sesión en este equipo.</li>
              <li>Si tiene una sesión abierta, se cierra ahora.</li>
              <li>El nombre queda reservado; se puede reactivar después.</li>
            </ul>
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <Button type="button" variant="secondary" disabled={busy} onClick={() => setPending(null)}>Cancelar</Button>
            <Button
              type="button"
              variant="danger"
              disabled={busy || !pending}
              onClick={() => pending && void applyActive(pending.user, false)}
            >
              <Power size={15} /> {busy ? "Desactivando…" : "Desactivar"}
            </Button>
          </div>
        </div>
      </Dialog>

      <Dialog
        open={pending?.action === "delete"}
        title="Eliminar usuario"
        subtitle={pending ? `Vas a borrar la cuenta de ${pending.user.username}.` : undefined}
        onClose={() => !busy && setPending(null)}
      >
        <div className="space-y-4">
          <div className="space-y-2 text-sm text-[var(--ink)]">
            <p>Esta acción no se puede deshacer:</p>
            <ul className="list-disc space-y-1 pl-5 text-[var(--muted)]">
              <li>La cuenta desaparece de este equipo.</li>
              <li>Si tiene una sesión abierta, se cierra ahora.</li>
              <li>El nombre queda libre: se puede crear otra cuenta igual.</li>
            </ul>
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <Button type="button" variant="secondary" disabled={busy} onClick={() => setPending(null)}>Cancelar</Button>
            <Button
              type="button"
              variant="danger"
              disabled={busy || !pending}
              onClick={() => pending && void applyDelete(pending.user)}
            >
              <Trash2 size={15} /> {busy ? "Eliminando…" : "Eliminar"}
            </Button>
          </div>
        </div>
      </Dialog>
    </div>
  );
}
