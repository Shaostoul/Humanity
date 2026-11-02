// scripts/check-run-sources.js: every row of the run catalogs names a source the findings
// documents define (spline routing rung 0, 2026-11-02). Each failure is seen on a fixture here, and
// the shipped catalogs pass. Run: node --test scripts/tests/run-sources.test.js (in `just rig-tests`).
'use strict';
const test = require('node:test');
const assert = require('node:assert');
const path = require('path');
const fs = require('fs');
const { findingNumbers, parseRon, checkSources } = require('../check-run-sources.js');

const ROOT = path.resolve(__dirname, '..', '..');
const findings = { R: new Set([1, 3, 13]), D: new Set([1]) };

const check = (ron) => checkSources('fixture.ron', parseRon(ron, 'fixture.ron'), findings);

test('a row citing findings the documents define, or a reasoned Ours, passes', () => {
  const r = check('#![enable(implicit_some)]\n// a comment\n(rows: [(id: "a", min: 1.5, source: [R(13), D(1)]), (id: "b", source: [Ours("the maker\'s figure, not read")])])');
  assert.deepStrictEqual(r.problems, []);
  assert.strictEqual(r.rows, 2);
});

test('a row with no source fails', () => {
  const r = check('(rows: [(id: "a", min: 1.5)])');
  assert.strictEqual(r.problems.length, 1);
  assert.match(r.problems[0], /row `a` has no source/);
});

test('a row with an empty source fails', () => {
  const r = check('(rows: [(id: "a", source: [])])');
  assert.match(r.problems.join('\n'), /row `a` has an empty source/);
});

test('a source naming a finding the document does not define fails', () => {
  const r = check('(rows: [(id: "a", source: [R(45)]), (id: "b", source: [D(2)])])');
  assert.strictEqual(r.problems.length, 2, r.problems.join('\n'));
  assert.match(r.problems[0], /cites R45, which docs\/reference\/findings\/2026-11-01-pipe-and-cable-routing-rules\.md does not define/);
  assert.match(r.problems[1], /cites D2/);
});

test('an Ours with no real reason fails', () => {
  const r = check('(rows: [(id: "a", source: [Ours("ours")])])');
  assert.match(r.problems.join('\n'), /does not say why/);
});

test('rows inside rows are checked: a size, and a bend radius with no id', () => {
  const r = check('(materials: [(id: "m", source: [R(1)], sizes: [(id: "s", source: [R(3)], bend_radii: [(method: Free, min_m: 0.1, as_read: "4 in")])])])');
  assert.strictEqual(r.problems.length, 1, r.problems.join('\n'));
  assert.match(r.problems[0], /bend radius has no source/);
});

test('vocabulary rows (id and meaning only) and mapping rows (no id) need no source', () => {
  const r = check('(tags: [(id: "armour", meaning: "Metal armour.")], kinds: [(kind: "rigid_copper", material: "m")], profiles: [])');
  assert.deepStrictEqual(r.problems, []);
  assert.strictEqual(r.rows, 0);
});

test('a malformed file does not read', () => {
  assert.throws(() => parseRon('(rows: [(id: "a", source: [R(1)]]', 'broken.ron'), /broken\.ron:1/);
});

test('the reader keeps enum payloads, Some, negative numbers and escaped quotes', () => {
  const v = parseRon('(k: support_spacing(horizontal_m: 1.37), o: Some(Ours("a \\"q\\" b")), n: -3, t: (0.0, 0.025), u: Above(height_m: None))', 'x');
  assert.strictEqual(v.fields.get('k').name, 'support_spacing');
  assert.strictEqual(v.fields.get('k').fields.get('horizontal_m').v, 1.37);
  assert.strictEqual(v.fields.get('o').items[0].items[0].v, 'a "q" b');
  assert.strictEqual(v.fields.get('n').v, -3);
  assert.strictEqual(v.fields.get('t').items[1].v, 0.025);
});

test('the findings documents define their findings as numbered bold paragraphs', () => {
  const routing = findingNumbers(fs.readFileSync(path.join(ROOT, 'docs/reference/findings/2026-11-01-pipe-and-cable-routing-rules.md'), 'utf8'));
  const doors = findingNumbers(fs.readFileSync(path.join(ROOT, 'docs/reference/findings/2026-11-01-manual-door-releases.md'), 'utf8'));
  assert.strictEqual(routing.size, 44);
  assert.strictEqual(doors.size, 19);
  assert.ok(routing.has(13) && !routing.has(45));
});

test('the shipped catalogs pass', () => {
  const { execFileSync } = require('child_process');
  const out = execFileSync(process.execPath, [path.join(ROOT, 'scripts', 'check-run-sources.js')], { encoding: 'utf8' });
  assert.match(out, /lint ok: run sources/);
});
