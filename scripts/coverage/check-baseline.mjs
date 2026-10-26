import { spawnSync } from 'node:child_process';
import { root, readJson } from './lib.mjs';
import { decreases } from './ratchet.mjs';

const report = readJson(process.argv[2]);
const proposed = readJson('docs/coverage-baseline.json');
let baseline = proposed;
const ref = process.env.COVERAGE_BASE_REF;
if (ref) {
  if (!/^[a-f0-9]{40}$/.test(ref)) throw new Error('Invalid baseline commit');
  const exists = spawnSync('git', ['cat-file', '-e', `${ref}:docs/coverage-baseline.json`], {cwd: root});
  if (exists.status === 0) {
    const result = spawnSync('git', ['show', `${ref}:docs/coverage-baseline.json`], {cwd: root, encoding: 'utf8'});
    if (result.status !== 0) throw new Error('Cannot read base coverage baseline');
    baseline = JSON.parse(result.stdout);
  }
}
const errors = [...decreases(baseline, report)];
// Reject attempts to lower the committed floors in the same PR as a regression.
for (const key of Object.keys(report.packages)) {
  const before = baseline.packages[key];
  if (before?.state === 'measured' && proposed.packages[key]?.state !== 'measured') errors.push(`Measured floor removed: ${key}`);
}
errors.push(...decreases(baseline, {packages: Object.fromEntries(Object.keys(report.packages).filter((key) => proposed.packages[key]).map((key) => [key, proposed.packages[key]]))}));
for (const key of Object.keys(baseline.packages)) {
  if (key.endsWith(`:${report.group}`) && baseline.packages[key].state === 'measured' && !report.packages[key] && key.includes(`:${process.platform}:`)) errors.push(`Measured package missing from report: ${key}`);
}
const missing = Object.keys(report.packages).filter((key) => baseline.packages[key]?.state !== 'measured');
for (const key of missing) console.log(`Informational first measurement; commit this floor after review: ${key}`);
if (errors.length && process.env.COVERAGE_ALLOW_DECREASE !== 'true') throw new Error(errors.join('\n'));
for (const error of errors) console.warn(`coverage-allow-decrease override: ${error}`);
console.log(`Checked ${Object.keys(report.packages).length} measured package/configuration rows`);
