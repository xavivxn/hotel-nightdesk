import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
import { readFileSync, writeFileSync } from 'node:fs';
import ts from 'typescript';
registerHooks({ load(url, context, next) {
  if (url.endsWith('.ts')) return { format: 'module', shortCircuit: true, source: ts.transpileModule(readFileSync(new URL(url), 'utf8'), { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText };
  return next(url, context);
}});
const { buildDailyPdf } = await import('../src/lib/daily-pdf.ts');
const report = { date:'2026-01-15', generated_at:'2026-01-16T09:00:00-03:00', cutoff_at:'2026-01-16T00:00:00-03:00', timezone:'Hora local de recepción (UTC-03:00)', occupied_rooms:23, closed_total_cents:80000, adjustments_total_cents:4000,
  accounts:[{stay_id:1,room_number:'01',check_in_at:'2026-01-15T12:00:00-03:00',check_out_at:'2026-01-15T14:00:00-03:00',closed_on_day:true,open_at_cutoff:false,total_cents:80000}],
  adjustments:Array.from({length:60},(_,i)=>({id:i+1,stay_id:1,kind:'surcharge',description:'Agua fría, café y gaseosa (botella) \\ descripción larga '.repeat(3),amount_cents:i===0?-1000:5000,created_at:'2026-01-15T13:00:00-03:00'})) };
// Fixture totals must reconcile with the deliberately long detail.
report.adjustments_total_cents = report.adjustments.reduce((n,c)=>n+c.amount_cents,0);
report.occupied_rooms = 1;
const bytes = buildDailyPdf(report);
const text = new TextDecoder().decode(bytes);
assert.ok(text.startsWith('%PDF-1.4')); assert.ok(text.endsWith('%%EOF\n'));
assert.ok(text.includes('80.000 Gs.')); assert.ok(text.includes('\\363'));
const xref = Number(text.match(/startxref\n(\d+)/)[1]); assert.equal(text.slice(xref,xref+4),'xref');
const offsets = [...text.slice(xref).matchAll(/(\d{10}) 00000 n/g)].map(m=>Number(m[1]));
offsets.forEach((offset,i)=>assert.ok(text.slice(offset).startsWith(`${i+1} 0 obj`)));
for (const match of text.matchAll(/\/Length (\d+) >>\nstream\n([\s\S]*?)endstream/g)) assert.equal(match[2].length, Number(match[1]));
assert.ok(Number(text.match(/\/Count (\d+)/)[1]) > 1);
const empty = buildDailyPdf({...report,accounts:[],adjustments:[],occupied_rooms:0,closed_total_cents:0,adjustments_total_cents:0});
assert.ok(new TextDecoder().decode(empty).includes('Sin movimientos'));
if (process.argv[2]) writeFileSync(process.argv[2], bytes);
console.log('OK: PDF multipágina, importes, acentos, escapes, offsets, streams y día vacío.');
