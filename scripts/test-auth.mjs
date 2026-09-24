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

async function rejectsCode(promise, code) {
  try {
    await promise;
    assert.fail(`esperaba código ${code}`);
  } catch (error) {
    assert.equal(error.code, code, error.message);
  }
}

await rejectsCode(call('list_board'), 'session_expired');
assert.equal(await call('auth_setup_required'), true);
await call('auth_setup', { payload: { username: 'admin', password: 'Prueba-segura-123' } });
await rejectsCode(call('auth_setup', { payload: { username: 'otro', password: 'Prueba-segura-123' } }), 'forbidden');
const admin = await call('auth_login', { payload: { username: 'admin', password: 'Prueba-segura-123' } });
await call('auth_create_user', { session_token: admin.token, payload: { username: 'recepcion', password: 'Prueba-segura-456', role: 'recepcion' } });
const reception = await call('auth_login', { payload: { username: 'recepcion', password: 'Prueba-segura-456' } });
const args = { session_token: reception.token };
for (const command of ['save_room', 'save_rate_plan', 'save_product', 'set_product_active', 'add_charge', 'save_settings', 'print_test', 'auth_create_user']) {
  await rejectsCode(call(command, args), 'forbidden');
}
assert.equal((await call('list_board', args)).length, 23);
assert.equal((await call('contract_info', args)).contract_version, 1);
const product = await call('save_product', { session_token: admin.token, payload: { name: 'Prueba', category: 'bebidas', price_cents: 15000, active: true } });
assert.equal(product.price_cents, 15000);
const stay = await call('check_in', { session_token: reception.token, payload: { room_id: 1, guest_name: 'Huésped demo', rate_plan_id: 1, expected_hours: 3 } });
const manualCharge = await call('add_charge', { session_token: admin.token, payload: { stay_id: stay.id, kind: 'surcharge', description: 'Prueba de permiso', amount_cents: 1000 } });
await rejectsCode(call('delete_charge', { session_token: reception.token, charge_id: manualCharge.id }), 'forbidden');
assert.ok((await call('get_stay_detail', { session_token: reception.token, stay_id: stay.id }))[2].some(c => c.id === manualCharge.id));
await call('delete_charge', { session_token: admin.token, charge_id: manualCharge.id });
assert.ok(!(await call('get_stay_detail', { session_token: reception.token, stay_id: stay.id }))[2].some(c => c.id === manualCharge.id));
const charge = await call('add_product_charge', { session_token: reception.token, payload: { stay_id: stay.id, product_id: product.id } });
assert.equal(charge.description, 'Prueba');
const preview = await call('preview_bill', { session_token: reception.token, stay_id: stay.id });
const closed = await call('check_out', { session_token: reception.token, payload: { stay_id: stay.id, print: false } });
assert.equal(closed.stay.status, 'closed');
assert.equal(closed.bill.total_cents, preview.total_cents);
assert.equal(closed.bill.applied_kind, preview.applied_kind);
try {
  await call('check_in', { session_token: reception.token, payload: { room_id: 1, guest_name: 'Otro', rate_plan_id: 1 } });
  assert.fail('esperaba conflicto de check-in');
} catch (error) {
  assert.equal(error.code, 'validation');
  assert.equal(String(error), 'La habitación necesita limpieza antes del check-in');
  assert.notEqual(String(error), '[object Object]');
}
const secondStay = await call('check_in', { session_token: reception.token, payload: { room_id: 2, guest_name: 'Segundo', rate_plan_id: 1, expected_hours: 3 } });
try {
  await call('check_in', { session_token: reception.token, payload: { room_id: 2, guest_name: 'Duplicado', rate_plan_id: 1 } });
  assert.fail('esperaba habitación ocupada');
} catch (error) {
  assert.equal(error.code, 'conflict');
  assert.equal(String(error), 'La habitación ya está ocupada');
}
const today = new Date();
const day = `${today.getFullYear()}-${String(today.getMonth()+1).padStart(2,'0')}-${String(today.getDate()).padStart(2,'0')}`;
// El corte del informe debe ser posterior al ingreso aun si ambas llamadas caen en el mismo milisegundo.
await new Promise(resolve => setTimeout(resolve, 10));
const report = await call('daily_report', {...args, date:day});
const history = await call('list_history', {...args, date:day});
assert.equal(report.closed_total_cents, history.reduce((sum,row)=>sum+row.total_cents,0));
assert.ok(report.accounts.some(a=>a.stay_id === secondStay.id && a.open_at_cutoff));
await rejectsCode(call('daily_report', {...args,date:'2026-02-30'}),'validation');
await rejectsCode(call('list_printers', args),'forbidden');
const originalNow = Date.now;
Date.now = () => originalNow() + 28801000;
await rejectsCode(call('list_board', args), 'session_expired');
Date.now = originalNow;
await call('auth_logout', { session_token: admin.token });
await rejectsCode(call('save_product', { session_token: admin.token }), 'session_expired');
for (let i = 0; i < 5; i++) await rejectsCode(call('auth_login', { payload: { username: 'admin', password: 'incorrecta' } }), 'invalid_credentials');
await rejectsCode(call('auth_login', { payload: { username: 'admin', password: 'Prueba-segura-123' } }), 'rate_limited');
console.log('OK: setup único, login, permisos, acceso operativo, expiración, logout y bloqueo de intentos (mock).');
