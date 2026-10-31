#!/usr/bin/env node
// The crate split's ratchet (docs/design/crate-split.md, rung 0; 2026-10-30).
//
// The split is decided, and before any crate exists the 155 references that point the wrong way
// under the plan (scripts/crate-layers.json) are being moved, a few edges per commit. This check
// keeps that from going backwards while it happens: each wrong-way edge ("humanity-gui ->
// humanity-engine") has a budget in crate-layers.json's "budgets", today's count, and the check
// FAILS when an edge goes over its budget or a new wrong-way edge appears. When an edge comes in
// under its budget it says so, so the budget is lowered in the same commit (a ratchet only works
// if it is tightened). When every budget reaches zero the crates can be cut along these lines.
//
// The reading is scripts/crate-graph.js's (`analyse`): every `crate::` and `super::` path in
// non-test code, mapped to a crate by the plan's most specific path.
//
//   node scripts/check-crate-layers.js            the check (in `just lints`)
//   node scripts/check-crate-layers.js --write    set every budget to today's count

const fs = require('fs');
const path = require('path');
const { analyse } = require('./crate-graph.js');

const PLAN_PATH = path.join(__dirname, 'crate-layers.json');

/** Compare edge counts with budgets (pure, tested): { over, fresh, under }. */
function judge(counts, budgets) {
  const over = [], fresh = [], under = [];
  for (const [edge, n] of Object.entries(counts)) {
    if (!(edge in budgets)) fresh.push({ edge, n });
    else if (n > budgets[edge]) over.push({ edge, n, budget: budgets[edge] });
    else if (n < budgets[edge]) under.push({ edge, n, budget: budgets[edge] });
  }
  for (const [edge, budget] of Object.entries(budgets)) {
    if (!(edge in counts) && budget > 0) under.push({ edge, n: 0, budget });
  }
  return { over, fresh, under };
}

function counts() {
  const { edges, unowned } = analyse();
  const out = {};
  for (const [k, m] of edges) out[k] = [...m.values()].reduce((a, b) => a + b, 0);
  return { counts: out, edges, unowned };
}

function main() {
  const raw = fs.readFileSync(PLAN_PATH, 'utf8');
  const plan = JSON.parse(raw);
  const { counts: now, edges, unowned } = counts();
  if (process.argv.includes('--write')) {
    plan.budgets = Object.fromEntries(Object.entries(now).sort((a, b) => b[1] - a[1]));
    fs.writeFileSync(PLAN_PATH, JSON.stringify(plan, null, 2) + '\n');
    console.log(`crate layers: budgets written for ${Object.keys(now).length} wrong-way edges`);
    return;
  }
  const { over, fresh, under } = judge(now, plan.budgets || {});
  const total = Object.values(now).reduce((a, b) => a + b, 0);
  let failed = false;
  if (unowned.size) {
    failed = true;
    console.log(`crate layers: FAILED: top-level modules no crate claims (add them to scripts/crate-layers.json): ${[...unowned].join(', ')}`);
  }
  const items = (edge) => [...(edges.get(edge) || new Map()).entries()].sort((a, b) => b[1] - a[1]).slice(0, 6).map(([i, c]) => `${i} (${c})`).join('; ');
  for (const o of over) {
    failed = true;
    console.log(`crate layers: FAILED: ${o.edge} has ${o.n} wrong-way references, its budget is ${o.budget}. New ones: move the item to the lower crate, or take it as an argument. ${items(o.edge)}`);
  }
  for (const f of fresh) {
    failed = true;
    console.log(`crate layers: FAILED: a NEW wrong-way edge, ${f.edge} (${f.n}): ${items(f.edge)}`);
  }
  for (const u of under) {
    console.log(`crate layers: ${u.edge} is down to ${u.n} (budget ${u.budget}): lower its budget in scripts/crate-layers.json (node scripts/check-crate-layers.js --write)`);
  }
  if (failed) process.exit(1);
  console.log(`>> lint ok: crate layers (${total} wrong-way references in ${Object.keys(now).length} edges, none new)`);
}

if (require.main === module) main();

module.exports = { judge };
