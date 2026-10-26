import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
export const relative = (file) => path.relative(root, file).replaceAll('\\', '/');
export const readJson = (file) => JSON.parse(fs.readFileSync(path.resolve(root, file), 'utf8'));
export function writeJson(file, value) {
  write(file, `${JSON.stringify(value, null, 2)}\n`);
}
export function write(file, value) {
  const target = path.resolve(root, file);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, value);
}
export function filesUnder(dir, extension) {
  if (!fs.existsSync(path.resolve(root, dir))) return [];
  return fs.readdirSync(path.resolve(root, dir), { withFileTypes: true }).flatMap((entry) => {
    const file = `${dir}/${entry.name}`;
    if (['target', 'node_modules', '.git', 'dist', 'coverage'].includes(entry.name)) return [];
    return entry.isDirectory() ? filesUnder(file, extension) : file.endsWith(extension) ? [file] : [];
  }).sort();
}
export function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited ${result.status}`);
}
export function checkedDocument(file, text) {
  if (process.argv.includes('--check')) {
    if (!fs.existsSync(path.resolve(root, file)) || fs.readFileSync(path.resolve(root, file), 'utf8').replaceAll('\r\n', '\n') !== text) {
      throw new Error(`${file} is stale; regenerate the coverage matrices`);
    }
  } else write(file, text);
}

// Inventory test functions, retaining bodies for reference validation. This is
// source inventory, never evidence that an assertion executed or killed a mutant.
export function rustTests(dirs) {
  const tests = new Map();
  for (const dir of dirs) for (const file of filesUnder(dir, '.rs')) {
    const source = fs.readFileSync(path.resolve(root, file), 'utf8');
    const functions = [...source.matchAll(/\b(?:async\s+)?fn\s+(\w+)\s*(?:<[^\n]*?>)?\s*\(/g)];
    for (let i = 0; i < functions.length; i++) {
      const match = functions[i];
      const preceding = source.slice(i ? functions[i - 1].index : 0, match.index);
      if (!/#\[(?:tokio::)?test(?:\([^\]]*\))?\]/.test(preceding)) continue;
      const ref = `${file}::${match[1]}`;
      tests.set(ref, source.slice(match.index, functions[i + 1]?.index ?? source.length));
    }
  }
  return tests;
}
