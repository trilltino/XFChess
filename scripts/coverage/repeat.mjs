import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { root, writeJson } from './lib.mjs';

const count = Number(process.argv[2] ?? 10);
if (!Number.isInteger(count) || count < 1 || count > 10) throw new Error('Repeat count must be 1..10');
const lanes = [
  ['application', ['-p','xfchess','-p','backend','-p','xfchess-rules','-p','xfchess-stockfish']],
  ['contract', ['-p','xfchess-game','--','--test-threads=1']],
  ['workspace', ['--workspace', ...['xfchess','backend','xfchess-game','xfchess-rules','xfchess-stockfish'].flatMap((p) => ['--exclude', p])]],
  ['proxy', ['-p','braid-iroh','--features','proxy']],
];
const dir = path.join(root, 'target/coverage/flake-audit');
fs.mkdirSync(dir, {recursive:true});
const runs = [];
for (let iteration = 1; iteration <= count; iteration++) {
  for (const [lane, args] of lanes) {
    console.log(`Flake audit: ${iteration}/${count}, ${lane}`);
    const result = spawnSync('cargo', ['test', '--locked', ...args], {cwd:root, encoding:'utf8', maxBuffer:100*1024*1024, timeout:45*60*1000});
    const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;
    fs.writeFileSync(path.join(dir, `${iteration}-${lane}.log`), output);
    const failed = [...output.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map((match) => match[1]);
    runs.push({iteration, lane, exit_code:result.status, error: result.error?.message, failed_tests: failed});
    writeJson('target/coverage/flake-audit/results.json', {requested_runs:count, complete:runs.length === count*lanes.length, runs});
    // Compilation/prerequisite failures cannot identify a flaky test. Fail
    // promptly, preserving the report, instead of repeating a broken build 10x.
    if (result.status !== 0 && !failed.length) throw new Error(`${lane}: build/prerequisite failure; see flake-audit log`);
  }
}
const failures = runs.filter((run) => run.exit_code !== 0);
if (failures.length) throw new Error(`${failures.length} test runs failed; triage results.json by test name`);
