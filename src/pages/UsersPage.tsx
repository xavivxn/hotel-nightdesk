import { useState } from "react";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/Button";
import { Input, PasswordInput, Select } from "@/components/ui/Field";
import type { SessionUser } from "@/lib/types";

export function UsersPage() {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [role, setRole] = useState<SessionUser["role"]>("recepcion");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  return <div className="px-6 py-6 lg:px-8"><p className="page-kicker">Administración</p><h1 className="page-title">Crear usuario</h1><form className="card mt-6 max-w-md space-y-4 rounded-lg p-5" onSubmit={async e => {
    e.preventDefault(); setBusy(true); setNotice("");
    try { const user = await api.createUser({ username, password, role }); setNotice(`Usuario ${user.username} creado como ${user.role}.`); setUsername(""); }
    catch(e) { setNotice(String(e)); } finally { setPassword(""); setBusy(false); }
  }}><label className="block">Usuario<Input required autoComplete="off" value={username} onChange={e => setUsername(e.target.value)} /></label><label className="block">Contraseña<PasswordInput required show={showPassword} onToggle={() => setShowPassword(v => !v)} autoComplete="new-password" value={password} onChange={e => setPassword(e.target.value)} /></label><label className="block">Rol<Select value={role} onChange={e => setRole(e.target.value as SessionUser["role"])}><option value="recepcion">Recepción</option><option value="admin">Administración</option></Select></label><Button type="submit" disabled={busy}>Crear usuario</Button><p role="status" className="text-sm">{notice}</p></form></div>;
}
