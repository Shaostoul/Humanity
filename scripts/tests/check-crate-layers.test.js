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
