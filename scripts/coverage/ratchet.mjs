import path from 'node:path';
import { readJson } from './lib.mjs';

export function decreases(baseline, current) {
  const errors = [];
  for (const [key, actual] of Object.entries(current.packages)) {
    const previous = baseline.packages[key];
    if (!previous || previous.state !== 'measured') continue;
    for (const name of ['lines', 'branches', 'functions', 'statements']) {
      const before = previous[name], after = actual[name];
      if (!before || before.percent === null) continue;
      if (!after || after.percent === null || after.covered * before.total < before.covered * after.total) {
        errors.push(`${key} ${name}: ${before.percent}% -> ${after?.percent ?? 'missing'}%`);
      }
    }
  }
  return errors;
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(import.meta.filename)) {
  const inputs = process.argv.slice(2).filter((arg) => !arg.startsWith('--'));
  if (inputs.length < 2) throw new Error('Usage: ratchet.mjs BASELINE REPORT... [--allow-decrease]');
  const baseline = readJson(inputs[0]);
  const current = {packages: {}};
  for (const input of inputs.slice(1)) Object.assign(current.packages, readJson(input).packages);
  if (!Object.keys(current.packages).length) throw new Error('No measured packages supplied');
  const errors = decreases(baseline, current);
  if (errors.length && !process.argv.includes('--allow-decrease')) throw new Error(`Coverage decreased:\n${errors.join('\n')}`);
  for (const error of errors) console.warn(`Explicit coverage override: ${error}`);
  console.log(`Coverage ratchet checked ${Object.keys(current.packages).length} package/configuration rows`);
}
