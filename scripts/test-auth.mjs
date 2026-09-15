import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import { readFileSync } from 'node:fs';
import ts from 'typescript';

registerHooks({
  resolve(specifier, context, next) {
    try { return next(specifier, context); }
    catch (error) { if (specifier.startsWith('.') && !specifier.endsWith('.ts')) return next(specifier + '.ts', context); throw error; }
  },
  load(url, context, next) {
    if (url.endsWith('.ts')) return { format: 'module', shortCircuit: true, source: ts.transpileModule(readFileSync(new URL(url), 'utf8'), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText };
    return next(url, context);
  },
});

const storage = new Map();
globalThis.localStorage = { getItem: k => storage.get(k) ?? null, setItem: (k, v) => storage.set(k, v) };
const { mockInvoke: call } = await import('../src/lib/mock.ts');
await assert.rejects(call('list_board'), /SESSION_EXPIRED/);
assert.equal(await call('auth_setup_required'), true);
await call('auth_setup', { payload: { username: 'admin', password: 'Prueba-segura-123' } });
await assert.rejects(call('auth_setup', { payload: { username: 'otro', password: 'Prueba-segura-123' } }), /FORBIDDEN/);
const admin = await call('auth_login', { payload: { username: 'admin', password: 'Prueba-segura-123' } });
await call('auth_create_user', { session_token: admin.token, payload: { username: 'recepcion', password: 'Prueba-segura-456', role: 'recepcion' } });
const reception = await call('auth_login', { payload: { username: 'recepcion', password: 'Prueba-segura-456' } });
const args = { session_token: reception.token };
for (const command of ['save_room', 'save_rate_plan', 'save_product', 'set_product_active', 'add_charge', 'delete_charge', 'save_settings', 'print_test', 'auth_create_user']) {
  await assert.rejects(call(command, args), /FORBIDDEN/, command);
}
assert.equal((await call('list_board', args)).length, 23);
const product = await call('save_product', { session_token: admin.token, payload: { name: 'Prueba', category: 'bebidas', price_cents: 15000, active: true } });
assert.equal(product.price_cents, 15000);
const originalNow = Date.now;
Date.now = () => originalNow() + 28801000;
await assert.rejects(call('list_board', args), /SESSION_EXPIRED/);
Date.now = originalNow;
await call('auth_logout', { session_token: admin.token });
await assert.rejects(call('save_product', { session_token: admin.token }), /SESSION_EXPIRED/);
for (let i = 0; i < 5; i++) await assert.rejects(call('auth_login', { payload: { username: 'admin', password: 'incorrecta' } }), /INVALID_CREDENTIALS/);
await assert.rejects(call('auth_login', { payload: { username: 'admin', password: 'Prueba-segura-123' } }), /RATE_LIMITED/);
console.log('OK: setup único, login, permisos, acceso operativo, expiración, logout y bloqueo de intentos (mock).');
