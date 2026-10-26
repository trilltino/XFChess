import fs from 'node:fs';
import path from 'node:path';
import { root, readJson, writeJson, checkedDocument, rustTests } from './lib.mjs';

const surface = process.argv[2];
if (!['backend', 'bridge'].includes(surface)) throw new Error('Usage: routes.mjs backend|bridge [--runtime FILE] [--hits DIRECTORY] [--bootstrap] [--check]');
const option = (name) => { const at = process.argv.indexOf(name); return at < 0 ? null : process.argv[at + 1]; };
const snapshot = `docs/coverage/${surface}-routes.json`;
const runtime = option('--runtime');
if (process.argv.includes('--bootstrap')) {
  if (!runtime || fs.existsSync(path.join(root, snapshot))) throw new Error('Bootstrap needs runtime output and a missing snapshot');
  const rows = readJson(runtime);
  writeJson(snapshot, rows.map((row) => ({...row, success: [], failure: [], legacy_gap: true})));
}
const rows = readJson(snapshot);
const key = (row) => `${row.method} ${row.path}`;
if (runtime) {
  const actual = readJson(runtime).map(key).sort();
  const recorded = rows.map(key).sort();
  if (JSON.stringify(actual) !== JSON.stringify(recorded)) throw new Error(`Runtime ${surface} router differs from matrix. Add success/failure evidence for new routes; review removals.\nNew: ${actual.filter((id) => !recorded.includes(id)).join(', ')}\nRemoved: ${recorded.filter((id) => !actual.includes(id)).join(', ')}`);
}
const tests = rustTests(surface === 'backend' ? ['backend/tests', 'backend/src'] : ['tauri/src', 'tauri/tests']);
const ids = new Set();
for (const row of rows) {
  if (ids.has(key(row))) throw new Error(`Duplicate route ${key(row)}`);
  ids.add(key(row));
  for (const cell of ['success', 'failure']) {
    if (!Array.isArray(row[cell])) throw new Error(`Missing ${cell} cell for ${key(row)}`);
    for (const ref of row[cell]) if (!tests.has(ref)) throw new Error(`Unknown test reference ${ref}`);
  }
  if (!row.legacy_gap && (!row.success.length || !row.failure.length)) throw new Error(`New route needs both success and failure tests: ${key(row)}`);
}
const hitsDir = option('--hits');
const hits = [];
if (hitsDir) {
  const dir = path.resolve(root, hitsDir);
  for (const file of fs.readdirSync(dir).filter((name) => /^hits-.*\.jsonl$/.test(name))) {
    hits.push(...fs.readFileSync(path.join(dir, file), 'utf8').trim().split('\n').filter(Boolean).map((line) => JSON.parse(line)));
  }
}
// Observed 2xx/4xx traffic is useful evidence for review, not automatic proof of
// a successful business operation or a correctly authorized rejection.
if (hitsDir) writeJson(`target/coverage/${surface}-observed.json`, rows.map((row) => ({
  method: row.method, path: row.path,
  observations: hits.filter((hit) => key(hit) === key(row)),
})));
checkedDocument(`docs/coverage/${surface}-routes.md`, `# ${surface === 'backend' ? 'Backend HTTP' : 'Tauri bridge HTTP'} matrix\n\nRuntime Axum inventory, including mounted aliases and explicit methods. \`ANY\` denotes a nested service. Implicit HEAD fallback is not a separate registration. Reviewed evidence is separate from observed statuses; a 2xx alone does not prove behavior.\n\n| Method | Matched path | Success test | Failure/authorization test |\n|---|---|---|---|\n${rows.map((row) => `| ${row.method} | \`${row.path}\` | ${row.success.map((ref) => `\`${ref}\``).join('<br>') || 'gap'} | ${row.failure.map((ref) => `\`${ref}\``).join('<br>') || 'gap'} |`).join('\n')}\n`);
