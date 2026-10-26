import { spawnSync } from 'node:child_process';
import { root, readJson } from './lib.mjs';

const ref = process.env.COVERAGE_BASE_REF;
if (ref && !/^[a-f0-9]{40}$/.test(ref)) throw new Error('Invalid base commit');
const specs = [
  ['docs/coverage/instruction-evidence.json', false],
  ['docs/coverage/ipc-evidence.json', false],
  ['docs/coverage/backend-routes.json', true],
  ['docs/coverage/bridge-routes.json', true],
];
for (const [file, routes] of specs) {
  const current = readJson(file);
  if (!ref) continue;
  const exists = spawnSync('git', ['cat-file','-e',`${ref}:${file}`], {cwd:root});
  if (exists.status !== 0) continue; // First introduction of the matrix.
  const result = spawnSync('git', ['show',`${ref}:${file}`], {cwd:root, encoding:'utf8'});
  if (result.status !== 0) throw new Error(`Cannot read base ${file}`);
  const keyed = (value) => routes ? Object.fromEntries(value.map((row) => [`${row.method} ${row.path}`, row])) : value;
  const baseline = keyed(JSON.parse(result.stdout));
  for (const [id, row] of Object.entries(keyed(current))) {
    const cells = Object.keys(row).filter((key) => Array.isArray(row[key]));
    if (!baseline[id]) {
      if (!row.success?.length || !cells.some((cell) => cell !== 'success' && row[cell].length)) throw new Error(`New entry point needs success and negative evidence: ${id}`);
    } else {
      for (const cell of cells) if (baseline[id][cell]?.length && !row[cell].length) throw new Error(`Reviewed coverage cell removed: ${id}.${cell}`);
    }
  }
}
console.log('Surface evidence ratchet passed');
