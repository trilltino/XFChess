import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { root, readJson, writeJson } from './lib.mjs';

export function metric(covered, total) {
  return { covered, total, percent: total ? Number((covered * 100 / total).toFixed(4)) : null };
}
export function rustSummary(report, packages) {
  const sums = {};
  for (const data of report.data) for (const file of data.files) {
    const filename = file.filename.replaceAll('\\', '/');
    const owner = packages.find((pkg) => filename.startsWith(`${pkg.dir}/src/`));
    if (!owner) continue;
    const row = sums[owner.name] ??= { lines: {covered: 0, total: 0}, branches: {covered: 0, total: 0} };
    for (const key of ['lines', 'branches']) {
      row[key].covered += file.summary[key].covered;
      row[key].total += file.summary[key].count;
    }
  }
  return Object.fromEntries(Object.entries(sums).map(([name, values]) => [name,
    Object.fromEntries(Object.entries(values).map(([key, value]) => [key, metric(value.covered, value.total)]))]));
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(import.meta.filename)) {
  const [kind, input, group, output] = process.argv.slice(2);
  if (!['rust', 'js'].includes(kind) || !input || !group || !output) throw new Error('Usage: collect.mjs rust|js INPUT GROUP OUTPUT');
  let packages;
  if (kind === 'rust') {
    const result = spawnSync('cargo', ['metadata', '--no-deps', '--locked', '--format-version', '1'], {cwd: root, encoding: 'utf8'});
    if (result.status !== 0) throw new Error(result.stderr || result.error);
    const metadata = JSON.parse(result.stdout);
    const owners = metadata.packages.map((p) => ({name: p.name, dir: path.dirname(p.manifest_path).replaceAll('\\', '/')})).sort((a,b) => b.dir.length - a.dir.length);
    packages = rustSummary(readJson(input), owners);
  } else {
    const total = readJson(input).total;
    packages = { [group]: Object.fromEntries(['lines', 'branches', 'functions', 'statements'].map((k) => [k, metric(total[k].covered, total[k].total)])) };
  }
  const entries = Object.fromEntries(Object.entries(packages).map(([name, row]) => [`${kind}:${name}:${kind === 'rust' ? process.platform + ':' : ''}${group}`, {state: 'measured', ...row}]));
  if (!Object.keys(entries).length) throw new Error(`No package coverage found in ${input}`);
  writeJson(output, {schema_version: 1, group, packages: entries});
}
