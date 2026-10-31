#!/usr/bin/env node
// The crate split's ratchet (docs/design/crate-split.md, rung 0; 2026-10-30).
//
// The split is decided, and before any crate exists the references that point the wrong way under
// the plan (scripts/crate-layers.json) were moved, a few edges per commit (rung 0, 157 of them, done
// 2026-10-31). This check keeps that from going backwards: each file's wrong-way edges ("src/x.rs:
// humanity-gui -> humanity-engine") have a budget in crate-layers.json's "budgets", and the check
// FAILS when a file goes over its budget or a file not listed reaches the wrong way at all. When a
// file comes in under its budget it says so, so the budget is lowered in the same commit (a ratchet
// only works if it is tightened). When every budget is empty the crates can be cut along these
// lines. Held by FILE since 2026-10-31 (it was by edge): a budget kept for a file another session is
// editing cannot be spent by a new reference elsewhere on the same edge.
//
// The reading is scripts/crate-graph.js's (`analyse`): every `crate::` and `super::` path in
// non-test code, mapped to a crate by the plan's most specific path, and a path naming one of the
// crate root's own items (`crate::data_dir()`) into the crate that owns lib.rs.
//
// TEST CODE TOO (2026-10-31, before rung 1). A test that reaches up the stack cannot compile once
// its crate is cut, so test code is held the same way: crate-layers.json's "testBudgets", by file.
// A test that reaches up moves to the crate it reaches, or reaches through the crate under it
// (docs/design/crate-split.md).
//
//   node scripts/check-crate-layers.js            the check (in `just lints`)
//   node scripts/check-crate-layers.js --write    set every budget, code and tests, to today's count

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

/** The key a test budget is judged by: "src/<file>: <from> -> <to>". */
const SEP = ': ';

/** Per-file counts (analyse's codeByFile or testByFile, files src/-relative) as judge's flat counts. */
function flatByFile(byFile) {
  const out = {};
  for (const [rel, m] of byFile) for (const [edge, n] of m) out[`src/${rel}${SEP}${edge}`] = n;
  return out;
}

/** crate-layers.json's "budgets" or "testBudgets" ({ "src/<file>": { edge: n } }) as judge's flat
 * budgets (pure, tested). */
function flatBudgets(budgets) {
  const out = {};
  for (const [file, m] of Object.entries(budgets || {})) for (const [edge, n] of Object.entries(m)) out[`${file}${SEP}${edge}`] = n;
  return out;
}

/** judge's flat counts back to the file-by-file shape crate-layers.json keeps (pure, tested). */
function nestByFile(flat) {
  const out = {};
  for (const [k, n] of Object.entries(flat).sort((a, b) => a[0].localeCompare(b[0]))) {
    const i = k.indexOf(SEP);
    const file = k.slice(0, i);
    (out[file] = out[file] || {})[k.slice(i + SEP.length)] = n;
  }
  return out;
}

function counts() {
  const { edges, codeByFile, testByFile, unowned } = analyse({ tests: true });
  return { code: flatByFile(codeByFile), edges, tests: flatByFile(testByFile), unowned };
}

function main() {
  const raw = fs.readFileSync(PLAN_PATH, 'utf8');
  const plan = JSON.parse(raw);
  const { code, edges, tests, unowned } = counts();
  if (process.argv.includes('--write')) {
    plan.budgets = nestByFile(code);
    plan.testBudgets = nestByFile(tests);
    fs.writeFileSync(PLAN_PATH, JSON.stringify(plan, null, 2) + '\n');
    console.log(`crate layers: budgets written for ${Object.keys(plan.budgets).length} files in code and ${Object.keys(plan.testBudgets).length} in test code`);
    return;
  }
  const { over, fresh, under } = judge(code, flatBudgets(plan.budgets));
  const t = judge(tests, flatBudgets(plan.testBudgets));
  const total = Object.values(code).reduce((a, b) => a + b, 0);
  const testTotal = Object.values(tests).reduce((a, b) => a + b, 0);
  let failed = false;
  if (unowned.size) {
    failed = true;
    console.log(`crate layers: FAILED: top-level modules no crate claims (add them to scripts/crate-layers.json): ${[...unowned].join(', ')}`);
  }
  // A flat key is "src/<file>: <edge>"; the items named on its edge, for the message.
  const items = (key) => [...(edges.get(key.slice(key.indexOf(SEP) + SEP.length)) || new Map()).entries()].sort((a, b) => b[1] - a[1]).slice(0, 6).map(([i, c]) => `${i} (${c})`).join('; ');
  for (const o of over) {
    failed = true;
    console.log(`crate layers: FAILED: code in ${o.edge} has ${o.n} wrong-way references, its budget is ${o.budget}. New ones: move the item to the lower crate, or take it as an argument. On that edge: ${items(o.edge)}`);
  }
  for (const f of fresh) {
    failed = true;
    console.log(`crate layers: FAILED: code in ${f.edge} reaches the wrong way (${f.n}): move the item to the lower crate, or take it as an argument. On that edge: ${items(f.edge)}`);
  }
  for (const u of under) {
    console.log(`crate layers: code in ${u.edge} is down to ${u.n} (budget ${u.budget}): lower its budget in scripts/crate-layers.json (node scripts/check-crate-layers.js --write)`);
  }
  const how = 'move the test to a test file of the crate it reaches, or reach through the crate under it (node scripts/crate-graph.js --tests lists each one)';
  for (const o of t.over) {
    failed = true;
    console.log(`crate layers: FAILED: test code in ${o.edge} has ${o.n} wrong-way references, its budget is ${o.budget}: ${how}`);
  }
  for (const f of t.fresh) {
    failed = true;
    console.log(`crate layers: FAILED: test code in ${f.edge} reaches the wrong way (${f.n}): ${how}`);
  }
  for (const u of t.under) {
    console.log(`crate layers: test code in ${u.edge} is down to ${u.n} (budget ${u.budget}): lower its budget in scripts/crate-layers.json (node scripts/check-crate-layers.js --write)`);
  }
  if (failed) process.exit(1);
  const files = (flat) => new Set(Object.keys(flat).map((k) => k.slice(0, k.indexOf(SEP)))).size;
  console.log(`>> lint ok: crate layers (${total} wrong-way references in code in ${files(code)} files, ${testTotal} in test code in ${files(tests)} files, each held to its file's budget; none new)`);
}

if (require.main === module) main();

module.exports = { judge, flatByFile, flatBudgets, nestByFile };
