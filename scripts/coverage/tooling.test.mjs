import { test } from 'node:test';
import assert from 'node:assert/strict';
import { rustSummary } from './collect.mjs';
import { decreases } from './ratchet.mjs';

test('attributes files to the deepest package and excludes test/dependency sources', () => {
  const f = (filename, covered, count) => ({filename, summary: {lines: {covered, count}, branches: {covered: 0, count: 0}}});
  const result = rustSummary({data:[{files:[f('C:\\repo\\src\\lib.rs', 3, 10), f('C:\\repo\\libs\\rules\\src\\lib.rs', 8, 10), f('C:\\repo\\tests\\test.rs', 100, 100), f('C:\\registry\\src\\lib.rs', 100, 100)]}]}, [{name:'rules', dir:'C:/repo/libs/rules'}, {name:'app', dir:'C:/repo'}]);
  assert.equal(result.app.lines.percent, 30);
  assert.equal(result.rules.lines.percent, 80);
  assert.equal(result.app.branches.percent, null, 'unmeasured branches must not become 100%');
});

test('ratchet catches denominator growth and lost branch data without rounded-percent masking', () => {
  const row = (covered, total) => ({covered, total, percent: Number((covered*100/total).toFixed(2))});
  const baseline = {packages:{a:{state:'measured', lines:row(9000, 10000), branches:row(2,4)}}};
  assert.equal(decreases(baseline, {packages:{a:{state:'measured', lines:row(9000,10001)}}}).length, 2);
  assert.deepEqual(decreases(baseline, {packages:{a:{state:'measured', lines:row(99,100), branches:row(3,4)}}}), []);
});
