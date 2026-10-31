// The crate split's ratchet (scripts/check-crate-layers.js): over a budget or a new wrong-way edge
// fails; under a budget asks for the budget to come down. Run: node --test scripts/tests/
const test = require('node:test');
const assert = require('node:assert');
const { judge } = require('../check-crate-layers.js');

test('an edge over its budget, or a new edge, fails', () => {
  const r = judge({ 'a -> b': 5, 'c -> d': 1 }, { 'a -> b': 4 });
  assert.deepStrictEqual(r.over, [{ edge: 'a -> b', n: 5, budget: 4 }]);
  assert.deepStrictEqual(r.fresh, [{ edge: 'c -> d', n: 1 }]);
});

test('an edge under its budget, or gone, asks for the budget to come down', () => {
  const r = judge({ 'a -> b': 3 }, { 'a -> b': 4, 'e -> f': 2 });
  assert.strictEqual(r.over.length + r.fresh.length, 0);
  assert.deepStrictEqual(r.under, [{ edge: 'a -> b', n: 3, budget: 4 }, { edge: 'e -> f', n: 0, budget: 2 }]);
});

test('at budget, nothing to say', () => {
  const r = judge({ 'a -> b': 4 }, { 'a -> b': 4 });
  assert.deepStrictEqual(r, { over: [], fresh: [], under: [] });
});

// The reading the ratchet counts with (scripts/crate-graph.js, 2026-10-31): test code is every item
// gated to tests alone, wherever it sits, and a path in a comment or a string is not a reference.
const { blankLiterals, cfgIsTestOnly, nonTestCode, declaredFiles } = require('../crate-graph.js');

test('comments and literals are blanked in place: same length, same lines, no paths left', () => {
  const src = 'let a = "crate::x"; // crate::y\r\n/* crate::w */ use crate::z; let c = \'"\'; let r = r#"a "crate::v" b"#;\n';
  const out = blankLiterals(src);
  assert.strictEqual(out.length, src.length);
  assert.strictEqual(out.split('\n').length, src.split('\n').length);
  for (const gone of ['crate::x', 'crate::y', 'crate::w', 'crate::v']) assert.ok(!out.includes(gone), gone);
  assert.ok(out.includes('use crate::z;'));
  // A lifetime is not a char literal: the code after it is read.
  assert.ok(blankLiterals("fn f<'a>(x: &'a str) { crate::k(); }").includes('crate::k'));
});

test('only a test-only cfg gates test code', () => {
  assert.ok(cfgIsTestOnly('test'));
  assert.ok(cfgIsTestOnly('all(test,feature="native")'));
  assert.ok(cfgIsTestOnly('all(feature="native",test)'));
  assert.ok(!cfgIsTestOnly('any(test,feature="native")'));
  assert.ok(!cfgIsTestOnly('not(test)'));
  assert.ok(!cfgIsTestOnly('feature="native"'));
});

test('a test item in the middle of a file goes, and the code after it stays', () => {
  const src = [
    'use crate::a;',
    '#[cfg(test)]',
    'mod sowing_tests;',
    '#[cfg(test)]',
    '#[path = "falls_tests.rs"]',
    'mod tests;',
    '#[cfg(all(test, feature = "native"))]',
    'mod native_tests { use crate::engine::x; fn f() { let c = \'{\'; } }',
    'fn after() { crate::b(); }',
    '#[cfg(test)]',
    'mod t {',
    '    use crate::c;',
    '}',
    '#[cfg(not(test))]',
    'fn live() { crate::d(); }',
  ].join('\n');
  const { code, testMods } = nonTestCode(src);
  assert.strictEqual(code.length, src.length);
  for (const kept of ['crate::a', 'crate::b', 'crate::d', 'fn after', 'fn live']) assert.ok(code.includes(kept), kept);
  for (const gone of ['crate::engine', 'crate::c', 'sowing_tests']) assert.ok(!code.includes(gone), gone);
  assert.deepStrictEqual(testMods, [{ name: 'sowing_tests', path: null }, { name: 'tests', path: 'falls_tests.rs' }]);
});

test('a declared test module is found where rustc finds it', () => {
  assert.deepStrictEqual(declaredFiles('systems/farming/mod.rs', { name: 'heat_tests', path: null }),
    ['systems/farming/heat_tests.rs', 'systems/farming/heat_tests/mod.rs']);
  assert.deepStrictEqual(declaredFiles('gui/pages/maps.rs', { name: 'tests', path: null }),
    ['gui/pages/maps/tests.rs', 'gui/pages/maps/tests/mod.rs']);
  assert.deepStrictEqual(declaredFiles('systems/day_energy.rs', { name: 'tests', path: 'day_energy_tests.rs' }),
    ['systems/day_energy_tests.rs']);
  assert.deepStrictEqual(declaredFiles('lib.rs', { name: 'test_clock', path: null }), ['test_clock.rs', 'test_clock/mod.rs']);
});
