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
// THE REAL BOUNDARIES (2026-11-01, rung 1). Once a crate is cut (crates/<name>/, the plan's `dir`),
// the compiler holds its code to the dependencies its Cargo.toml lists, so the plan must agree with the
// manifests, or a crate could be given a dependency the plan forbids while every reading here stayed
// green. So the check also reads the workspace: every member of the root Cargo.toml's [workspace] is a
// cut crate of the plan (or the root package), every cut crate is a member, and a member's dependency
// on another member (normal or dev) is one the plan lets it make. References by a cut crate's name
// (`humanity_core::storage::data_dir()`) are read like `crate::` paths into it (crate-graph.js), and
// a cut crate in whose folder nothing was read fails the check (a reading of nothing).
//
// A CRATE WITH A WORKSPACE OF ITS OWN (2026-11-09, the AI engine: docs/design/embeddable-ai.md, 7.1).
// A plan entry with `"workspace": "own"` is a program the app must never depend on, built only by its
// own recipe: so it must NOT be a member of the root workspace, the root's [workspace] `exclude` must
// list its dir (or cargo would refuse it as a stray package inside the workspace), its Cargo.toml must
// hold a [workspace] table of its own, and no crate of the plan may list it in `uses`. Its own
// dependencies on the plan's crates are held to the plan like any member's.
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

/** The key a budget is judged by: "<file from the repo root>: <from> -> <to>" ("src/x.rs", "crates/humanity-core/src/x.rs"). */
const SEP = ': ';

/** Per-file counts (analyse's codeByFile or testByFile, files repo-relative) as judge's flat counts. */
function flatByFile(byFile) {
  const out = {};
  for (const [file, m] of byFile) for (const [edge, n] of m) out[`${file}${SEP}${edge}`] = n;
  return out;
}

/** A manifest's dependency tables (pure, tested): { members: [..] or null, exclude: [..] or null, deps: [{ table, name, path }] }:
 * `members` and `exclude` from [workspace], and each dependency given by `path = "..."` in [dependencies],
 * [dev-dependencies], [build-dependencies] and their `target.'...'.` forms (the only way one member
 * names another), in an inline table or in its own `[dependencies.x]` table. */
function manifestDeps(toml) {
  const lines = String(toml).replace(/\r\n/g, '\n').split('\n');
  let table = '';
  let members = null;
  let exclude = null;
  const deps = [];
  const tables = new Map();
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    const h = /^\s*\[([^\]]+)\]\s*(?:#.*)?$/.exec(l);
    if (h) { table = h[1].trim(); continue; }
    if (table === 'workspace') {
      const m = /^\s*(members|exclude)\s*=\s*\[(.*)$/.exec(l);
      if (m) {
        let body = m[2];
        while (!body.includes(']') && i + 1 < lines.length) body += lines[++i];
        const list = [...body.slice(0, body.indexOf(']')).matchAll(/"([^"]+)"/g)].map((x) => x[1]);
        if (m[1] === 'members') members = list;
        else exclude = list;
      }
      continue;
    }
    // A dependency's own table: [dependencies.x], [target.'cfg(..)'.dev-dependencies.x].
    const own = /(?:^|\.)(dependencies|dev-dependencies|build-dependencies)\.([A-Za-z0-9_-]+)$/.exec(table);
    if (own) {
      const p = /^\s*path\s*=\s*"([^"]+)"/.exec(l);
      const pkg = /^\s*package\s*=\s*"([^"]+)"/.exec(l);
      const e = tables.get(table) || { table: own[1], name: own[2], path: null };
      if (p) e.path = p[1];
      if (pkg) e.name = pkg[1];
      tables.set(table, e);
      continue;
    }
    const kind = /(?:^|\.)(dependencies|dev-dependencies|build-dependencies)$/.exec(table);
    if (!kind) continue;
    const d = /^\s*([A-Za-z0-9_-]+)\s*=\s*\{(.*)\}\s*(?:#.*)?$/.exec(l);
    if (!d) continue;
    const p = /\bpath\s*=\s*"([^"]+)"/.exec(d[2]);
    const pkg = /\bpackage\s*=\s*"([^"]+)"/.exec(d[2]);
    if (p) deps.push({ table: kind[1], name: pkg ? pkg[1] : d[1], path: p[1] });
  }
  for (const e of tables.values()) if (e.path) deps.push(e);
  return { members, exclude, deps };
}

/** The plan against the workspace on disk (pure over its inputs, tested): the problems, as lines.
 * `plan`: crate-layers.json; `rootToml`: the root Cargo.toml's text; `readToml(dir)`: a member's
 * Cargo.toml text (null if missing); `rootCrate`: the plan crate at the top of the root package. */
function boundaryProblems(plan, rootToml, readToml, rootCrate = 'humanity-engine') {
  const out = [];
  const { members, exclude, deps: rootDeps } = manifestDeps(rootToml);
  const cut = plan.crates.filter((c) => c.dir);
  const own = (c) => c.workspace === 'own';
  const norm = (p) => p.replace(/\\/g, '/').replace(/^\.\//, '').replace(/\/$/, '');
  for (const c of plan.crates) if (c.dir && c.paths && c.paths.length) out.push(`${c.name} is cut (dir ${c.dir}) but still lists paths under src/: its files are in its dir now`);
  for (const c of plan.crates) if (c.workspace !== undefined && !own(c)) out.push(`${c.name}'s "workspace" is ${JSON.stringify(c.workspace)}: the only value is "own"`);
  if (!members) {
    if (cut.length) out.push(`the plan has cut crates (${cut.map((c) => c.name).join(', ')}) but the root Cargo.toml has no [workspace] members`);
    return out;
  }
  const memberDirs = members.map(norm).filter((m) => m !== '.' && m !== '');
  const excluded = (exclude || []).map(norm);
  for (const c of cut) {
    if (own(c)) {
      if (memberDirs.includes(norm(c.dir))) out.push(`${c.name} has a workspace of its own ("workspace": "own") but is a member of the root Cargo.toml's [workspace]`);
      if (!excluded.includes(norm(c.dir))) out.push(`${c.name} has a workspace of its own but the root Cargo.toml's [workspace] exclude does not list ${c.dir}`);
    } else if (!memberDirs.includes(norm(c.dir))) {
      out.push(`${c.name}'s dir, ${c.dir}, is not a member of the root Cargo.toml's [workspace]`);
    }
  }
  for (const m of memberDirs) if (!cut.some((c) => norm(c.dir) === m)) out.push(`the workspace member ${m} is not a crate the plan cut (give its plan entry "dir": "${m}")`);
  // Nothing may depend on a crate with a workspace of its own (the AI engine: the app never depends on it).
  for (const c of cut.filter(own)) {
    for (const x of plan.crates) if (x.uses.includes(c.name)) out.push(`${x.name} may use ${c.name} in the plan, but ${c.name} has a workspace of its own: nothing may depend on it`);
  }
  // Who may depend on whom: the plan's `uses` and what they use (a crate may reach through).
  const direct = new Map(plan.crates.map((x) => [x.name, x.uses]));
  const reach = (n, seen = new Set()) => { for (const u of direct.get(n) || []) if (!seen.has(u)) { seen.add(u); reach(u, seen); } return seen; };
  const names = new Set(plan.crates.map((x) => x.name));
  const check = (from, deps) => {
    for (const d of deps) {
      if (!names.has(d.name) || d.name === from) continue; // not one of ours (vendor/rav1d)
      if (!reach(from).has(d.name)) out.push(`${from}'s Cargo.toml depends on ${d.name} ([${d.table}]), which the plan does not let ${from} use`);
    }
  };
  check(rootCrate, rootDeps);
  for (const c of cut) {
    const text = readToml(c.dir);
    if (text == null) { out.push(`${c.name}: no Cargo.toml in ${c.dir}`); continue; }
    const name = /^\s*name\s*=\s*"([^"]+)"/m.exec(String(text));
    if (!name || name[1] !== c.name) out.push(`${c.dir}/Cargo.toml names its package "${name ? name[1] : '?'}", the plan "${c.name}"`);
    if (own(c) && !/^\s*\[workspace\]\s*(?:#.*)?$/m.test(String(text))) out.push(`${c.dir}/Cargo.toml has no [workspace] table of its own, which a crate with "workspace": "own" needs`);
    check(c.name, manifestDeps(text).deps);
  }
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
  const { edges, codeByFile, testByFile, unowned, files, cut } = analyse({ tests: true });
  return { code: flatByFile(codeByFile), edges, tests: flatByFile(testByFile), unowned, files, cut };
}

function main() {
  const raw = fs.readFileSync(PLAN_PATH, 'utf8');
  const plan = JSON.parse(raw);
  const { code, edges, tests, unowned, files: read, cut } = counts();
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
  const repo = path.join(__dirname, '..');
  const readToml = (dir) => { try { return fs.readFileSync(path.join(repo, dir, 'Cargo.toml'), 'utf8'); } catch { return null; } };
  for (const p of boundaryProblems(plan, fs.readFileSync(path.join(repo, 'Cargo.toml'), 'utf8'), readToml)) {
    failed = true;
    console.log(`crate layers: FAILED: the plan and the workspace disagree: ${p}`);
  }
  // A cut crate whose folder the reading found nothing in is a reading of nothing.
  const counted = (name) => read.filter((f) => f.crate === name).length;
  for (const c of cut) {
    if (!counted(c.name)) { failed = true; console.log(`crate layers: FAILED: no .rs file read under ${c.dir}/src: the counting would pass on nothing`); }
  }
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
  const cutSummary = cut.map((c) => `${c.name.replace('humanity-', '')} (${counted(c.name)} files)`).join(', ');
  console.log(`>> lint ok: crate layers (${total} wrong-way references in code in ${files(code)} files, ${testTotal} in test code in ${files(tests)} files, each held to its file's budget; none new; the cut crates ${cutSummary || 'none'} agree with the workspace)`);
}

if (require.main === module) main();

module.exports = { judge, flatByFile, flatBudgets, nestByFile, manifestDeps, boundaryProblems };
