import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { root, run } from './lib.mjs';

const groups = {
  application: ['-p', 'xfchess', '-p', 'backend', '-p', 'xfchess-rules', '-p', 'xfchess-stockfish'],
  contract: ['-p', 'xfchess-game'],
  workspace: ['--workspace', ...['xfchess', 'backend', 'xfchess-game', 'xfchess-rules', 'xfchess-stockfish', 'xfchess-tauri'].flatMap((p) => ['--exclude', p])],
  proxy: ['-p', 'braid-iroh', '--features', 'proxy'],
  desktop: ['-p', 'xfchess-tauri', '--features', 'production,wallet,tournament-admin'],
  engines: ['-p', 'xfchess-rules', '-p', 'xfchess-stockfish'],
};
const group = process.argv[2];
if (!(group in groups)) throw new Error(`Choose a group: ${Object.keys(groups).join(', ')}`);
if (group === 'workspace') {
  const metadata = spawnSync('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1'], {cwd: root, encoding: 'utf8'});
  if (metadata.status !== 0) throw new Error(metadata.stderr || metadata.error);
  const excluded = new Set(['xfchess', 'backend', 'xfchess-game', 'xfchess-rules', 'xfchess-stockfish', 'xfchess-tauri']);
  groups.workspace = JSON.parse(metadata.stdout).packages.filter((p) => !excluded.has(p.name)).flatMap((p) => ['-p', p.name]);
}
if (group === 'contract' && !fs.existsSync(path.join(root, 'target/deploy/xfchess_game.so'))) throw new Error('Build SBF first: missing target/deploy/xfchess_game.so');
const patterns = fs.readFileSync(path.join(root, 'coverage-exclusions.toml'), 'utf8').split('[[exclude]]').slice(1).flatMap((entry) => {
  const pattern = entry.match(/^pattern = '([^']+)'$/m)?.[1];
  const reason = entry.match(/^reason = '([^']+)'$/m)?.[1];
  if (!pattern || !reason) throw new Error('Each coverage exclusion needs a pattern and reason');
  const scope = entry.match(/^groups = \[([^\]]+)\]$/m)?.[1];
  if (scope && ![...scope.matchAll(/'([^']+)'/g)].some((m) => m[1] === group)) return [];
  return [process.platform === 'win32' ? pattern.replaceAll('/', String.raw`[/\\]`) : pattern];
});
if (!patterns.length) throw new Error('No coverage exclusions parsed');
const out = path.join(root, 'target/coverage', group);
fs.mkdirSync(out, { recursive: true });
const env = {
  ...process.env,
  CARGO_TARGET_DIR: path.join(root, 'target/coverage-build', group),
  CARGO_PROFILE_DEV_DEBUG: '2', CARGO_PROFILE_DEV_STRIP: 'none', CARGO_PROFILE_DEV_OPT_LEVEL: '0',
  CARGO_PROFILE_TEST_DEBUG: '2', CARGO_PROFILE_TEST_STRIP: 'none', CARGO_PROFILE_TEST_OPT_LEVEL: '0',
  XFCHESS_COVERAGE_DIR: path.join(out, 'surfaces'),
};
fs.mkdirSync(env.XFCHESS_COVERAGE_DIR, { recursive: true });
const ignore = ['--ignore-filename-regex', patterns.join('|')];
const selection = [...groups[group]];
selection.push('--profile', 'coverage');
if (group === 'application') selection.push('--features', 'backend/coverage');
const branch = process.argv.includes('--branch') ? ['--branch'] : [];
run('cargo', ['llvm-cov', '--locked', ...selection, ...branch, ...ignore, '--json', '--output-path', path.join(out, 'llvm.json'), ...(group === 'contract' ? ['--', '--test-threads=1'] : [])], { env });
for (const [format, file] of [['--lcov', 'lcov.info'], ['--html', 'html']]) {
  run('cargo', ['llvm-cov', 'report', ...selection, ...branch, ...ignore, format, format === '--html' ? '--output-dir' : '--output-path', path.join(out, file)], { env });
}
