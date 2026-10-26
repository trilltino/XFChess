import fs from 'node:fs';
import path from 'node:path';
import { root, filesUnder, readJson, writeJson, checkedDocument } from './lib.mjs';

const found = [];
for (const dir of ['backend', 'libs', 'programs', 'src', 'tests', 'tauri', 'tools']) for (const file of filesUnder(dir, '.rs')) {
  const source = fs.readFileSync(path.join(root, file), 'utf8');
  for (const match of source.matchAll(/#\[ignore(?:\s*=\s*"([^"]*)")?\]/g)) {
    const name = source.slice(match.index + match[0].length).match(/\b(?:async\s+)?fn\s+(\w+)/)?.[1];
    if (!name || !match[1]?.trim()) throw new Error(`${file}: ignored test needs a reason`);
    found.push({id: `${file}::${name}`, reason: match[1]});
  }
}
found.sort((a,b) => a.id.localeCompare(b.id, 'en'));
const manifest = 'docs/coverage/ignored-tests.json';
if (process.argv.includes('--bootstrap')) {
  if (fs.existsSync(path.join(root, manifest))) throw new Error('Ignore audit already exists');
  writeJson(manifest, found.map((row) => ({...row,
    owner: row.id.startsWith('backend/') ? 'backend maintainers' : row.id.includes('braid-') ? 'netcode maintainers' : row.id.startsWith('programs/') ? 'contract maintainers' : 'client maintainers',
    disposition: /fixture|Only invoked/i.test(row.reason) ? 'subprocess-fixture' : /unbounded/.test(row.reason) ? 'manual-unbounded' : /unsupported/.test(row.reason) ? 'quarantine' : /local L1/.test(row.reason) ? 'manual-local-stack' : /braid-http checkout/.test(row.reason) ? 'manual-upstream' : /bundled native/.test(row.reason) ? 'nightly-offline' : 'nightly-live',
    prerequisites: row.reason,
  })));
}
const audit = readJson(manifest);
const ids = new Set();
for (const row of audit) {
  if (ids.has(row.id)) throw new Error(`Duplicate ignored test ${row.id}`);
  ids.add(row.id);
  if (!row.owner?.trim() || !row.prerequisites?.trim()) throw new Error(`Ignore lacks owner/prerequisites: ${row.id}`);
  if (!['subprocess-fixture', 'manual-unbounded', 'quarantine', 'manual-local-stack', 'manual-upstream', 'nightly-offline', 'nightly-live'].includes(row.disposition)) throw new Error(`Invalid ignore disposition: ${row.id}`);
  if (row.disposition === 'quarantine' && !row.review_by) throw new Error(`Quarantine needs a review date: ${row.id}`);
  const current = found.find((entry) => entry.id === row.id);
  if (!current || current.reason !== row.reason) throw new Error(`Stale ignore audit: ${row.id}`);
}
for (const row of found) if (!ids.has(row.id)) throw new Error(`Unaudited ignored test: ${row.id}`);
checkedDocument('docs/coverage/ignored-tests.md', `# Ignored-test audit\n\n${audit.length} ignored tests. Owners below are component ownership roles; no individual assignment is implied. Fixtures must never be run by a blanket \`--ignored\` job. Live runs need explicit secrets and funding.\n\n| Test | Reason / prerequisites | Owner | Disposition |\n|---|---|---|---|\n${audit.map((row) => `| \`${row.id}\` | ${row.reason.replaceAll('|', '\\|')} | ${row.owner} | ${row.disposition}${row.review_by ? ` (review ${row.review_by})` : ''} |`).join('\n')}\n`);
