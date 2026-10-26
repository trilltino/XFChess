import fs from 'node:fs';
import path from 'node:path';
import { root, readJson, writeJson, checkedDocument, filesUnder, rustTests } from './lib.mjs';

const source = fs.readFileSync(path.join(root, 'tauri/src/main.rs'), 'utf8');
const block = source.split('tauri::generate_handler![')[1]?.split('])')[0];
if (!block) throw new Error('Cannot find native invoke handler');
const names = [...block.replace(/#\[cfg\([^\n]+\)\]/g, '').matchAll(/\b([a-z]\w*(?:::\w+)*)\s*,/g)].map((m) => m[1]).sort();
const manifest = 'docs/coverage/ipc-evidence.json';
if (process.argv.includes('--bootstrap')) {
  if (fs.existsSync(path.join(root, manifest))) throw new Error('IPC evidence already exists');
  writeJson(manifest, Object.fromEntries(names.map((name) => [name, {success: [], failure: []}])));
}
const evidence = readJson(manifest);
const tests = rustTests(['tauri/src', 'tauri/tests']);
const native = filesUnder('tauri/src', '.rs').map((file) => fs.readFileSync(path.join(root, file), 'utf8')).join('\n');
for (const name of names) {
  if (!evidence[name]) throw new Error(`New IPC command ${name} needs test evidence`);
  for (const cell of ['success', 'failure']) {
    if (!Array.isArray(evidence[name][cell])) throw new Error(`${name}.${cell} must be a list of tests`);
    for (const ref of evidence[name][cell]) if (!tests.has(ref)) throw new Error(`Unknown native IPC test: ${ref}`);
  }
  if (!native.includes(`fn ${name.split('::').at(-1)}(`)) throw new Error(`Missing command definition ${name}`);
}
if (Object.keys(evidence).some((name) => !names.includes(name))) throw new Error('IPC command removed; review evidence');
checkedDocument('docs/coverage/ipc.md', `# Native IPC matrix\n\nGenerated from the actual \`invoke_handler\` command list, including tournament-admin gated commands. Evidence means a native command test; frontend invoke mocks do not prove native behavior.\n\n| Command | Success test | Failure/authorization test |\n|---|---|---|\n${names.map((name) => `| \`${name}\` | ${evidence[name].success.join('<br>') || 'gap'} | ${evidence[name].failure.join('<br>') || 'gap'} |`).join('\n')}\n`);
