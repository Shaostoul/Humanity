#!/usr/bin/env node
// The crate cards (docs/crates/, 2026-10-31): one page for each crate the crate split plans
// (docs/design/crate-split.md), so an agent working inside one crate reads one page instead of
// catching up on the whole repo (operator, 2026-10-30: "Sounds like something we gotta do").
//
// A card is hand-written except for ONE block between marker comments, which this script writes:
// the paths the crate owns and the gate each compiles under, its size, the crates it may use and
// the crates that use it, the references that still point the wrong way (in code and in tests),
// and its de facto interface: the items other crates reference, most first. Every list and number
// comes from scripts/crate-layers.json (the plan's one source) and scripts/crate-graph.js's
// `analyse` (the ratchet's own counting, so a `use a::{x, y}` group counts once and crate-root
// functions such as crate::data_dir() are not counted), never typed by hand. Text outside the
// markers is never touched. docs/crates/README.md's index table is a generated block too.
//
//   node scripts/crate-cards.js            rewrite the generated blocks (`just code-map` and `just brief` run it)
//   node scripts/crate-cards.js --check    the lint (in `just lints`): every crate in the plan has a card, no card
//                                          names a crate the plan does not, each card has the six headings in
//                                          order, one generated block under the second, words in every other
//                                          section, and requests in the mailbox's one-line form. It never
//                                          compares the numbers: every code edit moves them.
//   node scripts/crate-cards.js --new <crate>   a skeleton card for a crate the plan just gained, filled in

'use strict';
const fs = require('fs');
const path = require('path');

const REPO = path.resolve(__dirname, '..');
const SRC = path.join(REPO, 'src');
const CARDS = path.join(REPO, 'docs', 'crates');
const PLAN_PATH = path.join(__dirname, 'crate-layers.json');

const START = '<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->';
const START_RE = /^<!-- generated: crate-cards\.js\b/;
const END = '<!-- end generated -->';
const BLOCK_RE = /^<!-- generated: crate-cards\.js[^\r\n]*\r?\n[\s\S]*?^<!-- end generated -->[ \t]*$/m;
const HEADINGS = ['## What it is for', '## At a glance (generated)', '## Rules', '## How to test it', '## Open work', '## Requests from other crates'];
const GENERATED_IN = HEADINGS[1];
const REQUESTS = HEADINGS[5];
const INTERFACE_MAX = 15;
// One request, one line: `- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`
const REQUEST_RE = /^- (\d{4}-\d{2}-\d{2}), from (humanity-[a-z]+) \(([^)]+)\): (.+?) Why: (.+)\.$/;
const TO_WRITE = '(to write)';

const short = (name) => name.replace(/^humanity-/, '');
const fmt = (n) => n.toLocaleString('en-US');
const sum = (xs) => xs.reduce((a, b) => a + b, 0);
const byCount = (m) => [...m.entries()].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1));
const counts = (m) => byCount(m).map(([k, n]) => `${short(k)} ${fmt(n)}`).join(', ');

// ---------------------------------------------------------------------------------------------
// The checks (pure, tested in scripts/tests/crate-cards.test.js).

/** A card's `## ` headings with their line numbers, passing over fenced code. */
function headingsOf(lines) {
  const out = [];
  let fence = false;
  lines.forEach((l, i) => {
    if (/^\s*```/.test(l)) fence = !fence;
    else if (!fence && /^## /.test(l)) out.push({ text: l.trimEnd(), line: i });
  });
  return out;
}

/** The lines of one `## ` section (after its heading, before the next). */
function sectionOf(lines, heads, heading) {
  const k = heads.findIndex((h) => h.text === heading);
  if (k < 0) return [];
  return lines.slice(heads[k].line + 1, k + 1 < heads.length ? heads[k + 1].line : lines.length);
}

/** The problems with one card's text (an empty list when it is sound). `names`: the plan's crates. */
function checkCard(name, text, names) {
  const problems = [];
  const lines = text.split(/\r?\n/);
  const first = lines.find((l) => l.trim());
  if ((first || '').trimEnd() !== `# ${name}`) problems.push(`its first line should be "# ${name}"`);
  const heads = headingsOf(lines);
  const got = heads.map((h) => h.text);
  const missing = HEADINGS.filter((h) => !got.includes(h));
  const extra = got.filter((h) => !HEADINGS.includes(h));
  if (missing.length) problems.push(`missing heading(s): ${missing.map((h) => `"${h}"`).join(', ')}`);
  if (extra.length) problems.push(`heading(s) a card does not have (use ### inside a section): ${extra.map((h) => `"${h}"`).join(', ')}`);
  if (!missing.length && !extra.length && got.join('\n') !== HEADINGS.join('\n')) {
    problems.push(`the headings are out of order or repeated: they go ${HEADINGS.map((h) => `"${h}"`).join(', ')}`);
  }
  const starts = lines.map((l, i) => (START_RE.test(l) ? i : -1)).filter((i) => i >= 0);
  const ends = lines.map((l, i) => (l.trim() === END ? i : -1)).filter((i) => i >= 0);
  if (starts.length !== 1 || ends.length !== 1) {
    problems.push(`it should hold exactly one generated block (found ${starts.length} start and ${ends.length} end markers)`);
  } else if (ends[0] < starts[0]) {
    problems.push('its generated block ends before it starts');
  } else {
    const k = heads.findIndex((h) => h.text === GENERATED_IN);
    const after = k >= 0 && k + 1 < heads.length ? heads[k + 1].line : lines.length;
    if (k < 0 || starts[0] < heads[k].line || ends[0] > after) problems.push(`its generated block belongs under "${GENERATED_IN}"`);
  }
  for (const h of HEADINGS) {
    if (h === GENERATED_IN || !got.includes(h)) continue;
    if (!sectionOf(lines, heads, h).some((l) => l.trim())) problems.push(`"${h}" is empty`);
  }
  if (text.includes(TO_WRITE)) problems.push(`it still holds the skeleton's "${TO_WRITE}" placeholders`);
  for (const l of sectionOf(lines, heads, REQUESTS)) {
    if (!/^- /.test(l)) continue;
    const m = REQUEST_RE.exec(l.trimEnd());
    if (!m) problems.push(`a request is not in the mailbox's form ("- YYYY-MM-DD, from humanity-<crate> (<who>): <what is needed>. Why: <why>."): ${l.slice(0, 80)}`);
    else if (!names.includes(m[2]) || m[2] === name) problems.push(`a request comes from "${m[2]}", which is not another crate in the plan`);
  }
  return problems;
}

/** The requests waiting in one card's mailbox. */
function requestsOf(text) {
  const lines = text.split(/\r?\n/);
  return sectionOf(lines, headingsOf(lines), REQUESTS).filter((l) => REQUEST_RE.test(l.trimEnd()));
}

/** The problems with the index (docs/crates/README.md). */
function checkReadme(text, names) {
  const problems = [];
  const starts = text.split(/\r?\n/).filter((l) => START_RE.test(l)).length;
  if (starts !== 1 || !BLOCK_RE.test(text)) problems.push('it should hold exactly one generated block (the index table)');
  for (const n of names) if (!text.includes(`(${n}.md)`)) problems.push(`it does not link ${n}.md (run: node scripts/crate-cards.js)`);
  return problems;
}

/** Every problem with a folder of cards against the plan: missing cards, stray cards, each card's own. */
function checkAll(dir, plan) {
  const names = plan.crates.map((c) => c.name);
  const label = path.relative(REPO, dir).replace(/\\/g, '/') || '.';
  const problems = [];
  const present = fs.existsSync(dir) ? fs.readdirSync(dir).filter((f) => f.endsWith('.md')) : [];
  for (const n of names) {
    if (!present.includes(`${n}.md`)) problems.push(`${label}/${n}.md is missing: every crate in scripts/crate-layers.json has a card (start one: node scripts/crate-cards.js --new ${n})`);
  }
  for (const f of present.sort()) {
    const text = fs.readFileSync(path.join(dir, f), 'utf8');
    if (f === 'README.md') {
      for (const q of checkReadme(text, names)) problems.push(`${label}/README.md: ${q}`);
      continue;
    }
    const n = f.replace(/\.md$/, '');
    if (!names.includes(n)) {
      problems.push(`${label}/${f} is a card for a crate the plan does not name (scripts/crate-layers.json): rename it to the crate's name, or delete it`);
      continue;
    }
    for (const q of checkCard(n, text, names)) problems.push(`${label}/${f}: ${q}`);
  }
  if (!present.includes('README.md')) problems.push(`${label}/README.md, the index, is missing`);
  return problems;
}

// ---------------------------------------------------------------------------------------------
// The generated blocks.

/** Swap the block between the markers for `body` (lines), in the file's own line endings; null if
 * the text has no block. Only the block's characters change. */
function replaceBlock(text, body) {
  if (!BLOCK_RE.test(text)) return null;
  const eol = text.includes('\r\n') ? '\r\n' : '\n';
  return text.replace(BLOCK_RE, () => [START, ...body, END].join(eol));
}

/** An item in the de facto interface: a path as written, up to its first capitalised name (a type,
 * trait or constant), so `ecs::components::Transform::default` and `...::Transform` are one item. */
function itemOf(target) {
  const segs = target.split('::');
  const k = segs.findIndex((s) => /^[A-Z]/.test(s));
  return (k < 0 ? segs : segs.slice(0, k + 1)).join('::');
}

/** How a gate reads on a card. */
function gateLabel(g) {
  if (g.root) return 'the crate root';
  if (g.bin) return 'the binary';
  if (!g.gates) return 'not declared by a `mod` line (check by hand)';
  if (!g.gates.length) return 'no gate (every build)';
  if (g.gates.length === 1 && g.gates[0] === 'test') return 'test builds only';
  return g.gates.map((x) => `\`${x}\``).join(' and ');
}

/** The gates one plan path compiles under: each `mod` line on the way down from lib.rs, and each
 * module file's own `#![cfg]`. */
function pathGates(p, isTestFile, read) {
  if (p === 'lib.rs') return { root: true };
  if (p === 'main.rs') return { bin: true };
  const { declaredGates, innerGates } = require('./crate-graph.js');
  const segs = p.replace(/\.rs$/, '').split('/');
  let parent = read('lib.rs');
  const gates = [];
  for (let i = 0; i < segs.length; i++) {
    const base = segs.slice(0, i + 1).join('/');
    const file = read(`${base}.rs`) !== null ? `${base}.rs` : read(`${base}/mod.rs`) !== null ? `${base}/mod.rs` : null;
    const decl = parent === null ? undefined : declaredGates(parent).get(segs[i]);
    if (!decl) return { gates: file && isTestFile(file) ? ['test'] : null };
    gates.push(...decl);
    if (!file) break;
    parent = read(file);
    gates.push(...innerGates(parent));
  }
  return { gates: [...new Set(gates)] };
}

/** A plan path as a reader finds it under src/: a file, a folder, or both. */
function shownPath(p, isDir, isFile) {
  if (p.endsWith('.rs')) return `\`${p}\``;
  const forms = [isFile(`${p}.rs`) && `\`${p}.rs\``, isDir(p) && `\`${p}/\``].filter(Boolean);
  return forms.length ? forms.join(' + ') : `\`${p}\` (not on disk)`;
}

/** The features each crate's non-test `#[cfg]` attributes name, counted: Map(feature -> n). */
function featureGates(files) {
  const out = new Map();
  for (const f of files) {
    if (f.test) continue;
    for (const m of f.code.matchAll(/#!?\[\s*cfg\s*\(/g)) {
      let i = m.index + m[0].length, depth = 1;
      while (i < f.raw.length && depth) { if (f.raw[i] === '(') depth++; else if (f.raw[i] === ')') depth--; i++; }
      const pred = f.raw.slice(m.index, i);
      for (const x of new Set([...pred.matchAll(/feature\s*=\s*"([^"]+)"/g)].map((y) => y[1]))) out.set(x, (out.get(x) || 0) + 1);
    }
  }
  return out;
}

/** The `just test-lib` line that runs this crate's lib tests and no other's: each of its paths as a
 * name filter (`relay/core` -> `relay::core::`), and a `--skip` for each other crate's module whose
 * name holds one of them, since libtest filters match anywhere in a test's name (`engine::` alone
 * would run the GUI's `engine::protected::` tests too). `overlap`: other crates' modules a skip
 * cannot drop without dropping this crate's own (none today). lib.rs and main.rs give no filter. */
function testLine(c, a, names) {
  const name = (mod) => mod.replace(/\//g, '::') + '::';
  const mine = [...new Set(a.files.filter((f) => f.crate === c.name).map((f) => name(f.mod)))];
  // A filter names a module of this crate (a plan path that is a `#[path]` file, such as
  // save_load_body_tests, is `save_load::tests` to rustc), and none is inside another.
  const filters = [];
  for (const x of c.paths.filter((p) => !/^(lib|main)\.rs$/.test(p)).map((p) => name(p.replace(/\.rs$/, '')))) {
    if (mine.some((m) => m.includes(x)) && !filters.some((g) => x.includes(g))) filters.push(x);
  }
  const hits = [...new Set(a.files.filter((f) => f.crate !== c.name).map((f) => name(f.mod)))].filter((m) => filters.some((x) => m.includes(x)));
  const least = hits.filter((m) => !hits.some((o) => o !== m && m.includes(o))).sort();
  const skips = least.filter((s) => !mine.some((m) => m.includes(s)));
  // Then the line is checked against every lib test's real name (`names`: name -> crate), and a
  // test of another crate it would still run (one in an inline `mod x { }`, which no file name
  // shows) gets a skip of its own module, when that drops none of this crate's.
  const own = [...names].filter(([, k]) => k === c.name).map(([n]) => n);
  const runs = (n) => filters.some((x) => n.includes(x)) && !skips.some((s) => n.includes(s));
  for (const [n, k] of names) {
    if (k === c.name || !runs(n)) continue;
    const s = n.slice(0, n.lastIndexOf('::') + 2);
    if (!own.some((o) => o.includes(s))) skips.push(s);
  }
  const extra = [...names].filter(([n, k]) => k !== c.name && runs(n)).map(([n]) => n);
  const missed = own.filter((n) => !runs(n));
  return { line: `just test-lib ${filters.join(' ')}${skips.map((s) => ` --skip ${s}`).join('')}`, tests: own.length, extra, missed, empty: !filters.length, skips: skips.length };
}

/** Every lib test's full name, as libtest prints it (`module::path::inline_mod::fn_name`), with its
 * crate: from each file's module path, the inline `mod x { }` blocks around the `#[test]`, and the
 * function after it. A `macro_rules!` whose body holds a `#[test]` makes one test per use in its
 * file, named by the use's first argument (`page_snapshot!(snapshot_calendar, ...)` in
 * gui/ui_snapshots.rs). main.rs is the binary's, not the library's. Map(name -> crate). */
function testNames(a) {
  const TEST_ATTR = /#\[\s*(?:tokio::)?test\s*(?:\([^\]]*\))?\s*\]/;
  const out = new Map();
  for (const f of a.files) {
    if (f.rel === 'main.rs') continue;
    const base = f.rel === 'lib.rs' ? [] : f.mod.split('/');
    // The test-making macros this file defines.
    const makers = new Set();
    for (const d of f.all.matchAll(/macro_rules!\s*([A-Za-z_][A-Za-z0-9_]*)\s*\{/g)) {
      let i = d.index + d[0].length, depth = 1;
      while (i < f.all.length && depth) { if (f.all[i] === '{') depth++; else if (f.all[i] === '}') depth--; i++; }
      if (TEST_ATTR.test(f.all.slice(d.index, i))) makers.add(d[1]);
    }
    const stack = [];
    let depth = 0;
    // `#[test]`, `#[tokio::test]` and `#[tokio::test(flavor = "multi_thread")]` (its string blanked),
    // and `maker!(name, ...)`.
    for (const m of f.all.matchAll(/\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{|#\[\s*(?:tokio::)?test\s*(?:\([^\]]*\))?\s*\]|\b([A-Za-z_][A-Za-z0-9_]*)!\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)|[{}]/g)) {
      if (m[0] === '{') depth++;
      else if (m[0] === '}') { depth--; while (stack.length && stack[stack.length - 1].depth > depth) stack.pop(); }
      else if (m[1]) { depth++; stack.push({ name: m[1], depth }); }
      else if (m[2]) { if (makers.has(m[2])) out.set([...base, ...stack.map((s) => s.name), m[3]].join('::'), f.crate); }
      else {
        const fn = /\bfn\s+([A-Za-z_][A-Za-z0-9_]*)/.exec(f.all.slice(m.index + m[0].length, m.index + m[0].length + 600));
        if (fn) out.set([...base, ...stack.map((s) => s.name), fn[1]].join('::'), f.crate);
      }
    }
  }
  return out;
}

/** The line budgets the size ratchet holds (tests/file_size_ratchet.rs, in `just lints`):
 * Map("src/x.rs" -> budget), read from its `("src/...", 1_234)` rows. */
function lineBudgets() {
  const out = new Map();
  let text = '';
  try { text = fs.readFileSync(path.join(REPO, 'tests', 'file_size_ratchet.rs'), 'utf8'); } catch { return out; }
  for (const m of text.matchAll(/\(\s*"(src\/[^"]+\.rs)"\s*,\s*([\d_]+)\s*\)/g)) out.set(m[1], Number(m[2].replace(/_/g, '')));
  return out;
}

/** Everything a card's block says, for every crate in the plan (`a`: analyse({ tests: true })). */
function facts(plan, a) {
  const budgets = lineBudgets();
  const testIndex = testNames(a);
  const names = plan.crates.map((c) => c.name);
  const read = (rel) => { try { return fs.readFileSync(path.join(SRC, rel), 'utf8'); } catch { return null; } };
  const isDir = (rel) => { try { return fs.statSync(path.join(SRC, rel)).isDirectory(); } catch { return false; } };
  const isFile = (rel) => { try { return fs.statSync(path.join(SRC, rel)).isFile(); } catch { return false; } };
  const out = new Map();
  for (const c of plan.crates) {
    const files = a.files.filter((f) => f.crate === c.name);
    const tests = files.filter((f) => f.test);
    out.set(c.name, {
      crate: c,
      lines: sum(files.map((f) => f.lines)),
      files: files.length,
      testLines: sum(tests.map((f) => f.lines)),
      testFiles: tests.length,
      paths: c.paths.map((p) => ({ shown: shownPath(p, isDir, isFile), gate: gateLabel(pathGates(p, a.isTestFile, read)) })),
      features: featureGates(files),
      run: testLine(c, a, testIndex),
      // The ratchet counts a file's lines as Rust's `lines()` does (no last empty line); so does this.
      budgeted: files.filter((f) => budgets.has(`src/${f.rel}`)).map((f) => ({ rel: f.rel, now: f.raw.split(/\r?\n/).length - (/\n$/.test(f.raw) ? 1 : 0), budget: budgets.get(`src/${f.rel}`) })),
      mayUse: c.uses,
      mayAlso: [...a.may.get(c.name)].filter((n) => !c.uses.includes(n)),
      mayBeUsedBy: names.filter((n) => a.may.get(n).has(c.name)),
      usesToday: new Map(),
      usedToday: new Map(),
      iface: new Map(),
      wrong: new Map(),
      wrongTests: new Map(),
    });
  }
  for (const [k, m] of a.crossing) {
    const [from, to] = k.split(' -> ');
    const n = sum([...m.values()]);
    out.get(from).usesToday.set(to, n);
    out.get(to).usedToday.set(from, n);
    for (const [target, c] of m) {
      const item = itemOf(target);
      const e = out.get(to).iface.get(item) || { n: 0, from: new Map() };
      e.n += c;
      e.from.set(from, (e.from.get(from) || 0) + c);
      out.get(to).iface.set(item, e);
    }
  }
  for (const [edges, key] of [[a.edges, 'wrong'], [a.testEdges, 'wrongTests']]) {
    for (const [k, m] of edges) {
      const [from, to] = k.split(' -> ');
      out.get(from)[key].set(to, sum([...m.values()]));
    }
  }
  return out;
}

/** The generated block of one card, as lines. */
function cardBlock(f) {
  const L = [];
  L.push('- **Owns** (from `scripts/crate-layers.json`; paths under `src/`), by the gate each compiles under:');
  const groups = new Map();
  for (const p of f.paths) groups.set(p.gate, [...(groups.get(p.gate) || []), p.shown]);
  for (const [gate, ps] of groups) L.push(`  - ${gate}: ${ps.join(', ')}`);
  L.push(`- **Size:** ${fmt(f.lines)} lines in ${fmt(f.files)} files, ${fmt(f.testLines)} of them in ${fmt(f.testFiles)} test files.`);
  if (f.budgeted.length) {
    const rows = [...f.budgeted].sort((x, y) => y.now / y.budget - x.now / x.budget);
    L.push(`- **Line budgets** (\`tests/file_size_ratchet.rs\`, in \`just lints\`; lines now of the budget, closest first): ${rows.map((r) => `\`${r.rel}\` ${fmt(r.now)} of ${fmt(r.budget)}`).join(', ')}.`);
  }
  L.push(`- **Gates inside its files** (\`#[cfg]\` attributes in non-test code, by the feature they name): ${f.features.size ? byCount(f.features).map(([x, n]) => `\`${x}\` ${fmt(n)}`).join(', ') : 'none'}.`);
  L.push(`- **May use** (the plan): ${f.mayUse.length ? f.mayUse.map(short).join(', ') : 'nothing; it is the bottom of the stack'}${f.mayAlso.length ? `; through those, also ${f.mayAlso.map(short).join(', ')}` : ''}. **Uses today** (references in non-test code): ${f.usesToday.size ? counts(f.usesToday) : 'nothing'}.`);
  L.push(`- **May be used by** (the plan): ${f.mayBeUsedBy.length ? f.mayBeUsedBy.map(short).join(', ') : 'nothing; it is the top of the stack'}. **Used today by:** ${f.usedToday.size ? counts(f.usedToday) : 'nothing'}.`);
  const wrong = sum([...f.wrong.values()]);
  const wrongTests = sum([...f.wrongTests.values()]);
  L.push(`- **Wrong-way references:** ${fmt(wrong)} in code${wrong ? ` (${counts(f.wrong)}: \`node scripts/check-crate-layers.js\` fails on them)` : ' (`node scripts/check-crate-layers.js` keeps it there)'}; ` +
    `${fmt(wrongTests)} in test code${wrongTests ? `, to ${counts(f.wrongTests)}: each moves with its test before this crate is cut (docs/design/crate-split.md, rung 0, "Not counted")` : ''}.`);
  const exact = !f.run.extra.length && !f.run.missed.length;
  const why = f.run.skips ? " (a libtest filter matches anywhere in a test's name, so each `--skip` drops another crate's module whose name holds a filter)" : '';
  L.push(`- **Its lib tests:** ${fmt(f.run.tests)} tests${f.run.empty ? '.' : `, and ${exact ? 'this runs exactly them, checked against every test\'s name' : 'this runs them'}${why}:`}`);
  if (!f.run.empty) L.push('', '```', f.run.line, '```');
  const few = (xs) => `${xs.slice(0, 3).map((x) => `\`${x}\``).join(', ')}${xs.length > 3 ? ` and ${fmt(xs.length - 3)} more` : ''}`;
  if (f.run.missed.length) L.push('', `It misses ${fmt(f.run.missed.length)} of them (${few(f.run.missed)}): no filter reaches them without reaching other crates' tests.`);
  if (f.run.extra.length) L.push('', `It also runs ${fmt(f.run.extra.length)} other crates' tests (${few(f.run.extra)}): a skip that drops them would drop this crate's own.`);
  L.push('');
  const items = byCount(new Map([...f.iface].map(([k, e]) => [k, e.n])));
  if (!items.length) {
    L.push('**Interface:** no other crate references anything in it.');
  } else {
    L.push('**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name):', '');
    L.push('| Item | References | From |', '|---|---:|---|');
    for (const [item, n] of items.slice(0, INTERFACE_MAX)) L.push(`| \`${item}\` | ${fmt(n)} | ${counts(f.iface.get(item).from)} |`);
    const rest = items.slice(INTERFACE_MAX);
    L.push('', rest.length ? `And ${fmt(rest.length)} more items, ${fmt(sum(rest.map(([, n]) => n)))} references (\`node scripts/crate-cards.js --interface ${f.crate.name}\` lists them all).` : `All ${fmt(items.length)} items shown.`);
  }
  return L;
}

/** The first sentence of a card's "What it is for", for the index. */
function purposeOf(text) {
  const lines = text.split(/\r?\n/);
  const words = sectionOf(lines, headingsOf(lines), HEADINGS[0]).map((l) => l.trim()).filter(Boolean).join(' ');
  const m = words.match(/^(.*?[.!?])(\s|$)/);
  return (m ? m[1] : words).replace(/\|/g, '/');
}

/** The index's generated block, as lines. */
function readmeBlock(plan, all, texts) {
  const L = ['| Card | Lines | May use | What it is for | Requests |', '|---|---:|---|---|---:|'];
  for (const c of plan.crates) {
    const t = texts.get(c.name);
    const f = all.get(c.name);
    L.push(`| [${c.name}](${c.name}.md) | ${fmt(f.lines)} | ${c.uses.length ? c.uses.map(short).join(', ') : 'nothing'} | ${t ? purposeOf(t) : `(no card yet: \`node scripts/crate-cards.js --new ${c.name}\`)`} | ${t ? requestsOf(t).length : 0} |`);
  }
  return L;
}

/** A skeleton card for a crate the plan just gained. */
function skeleton(name) {
  return [
    `# ${name}`, '',
    HEADINGS[0], '', `${TO_WRITE} Two or three plain sentences: what this crate holds, and why it is a crate of its own.`, '',
    HEADINGS[1], '', START, END, '',
    HEADINGS[2], '', `${TO_WRITE} The rules that bind work in it, each with its source (a CLAUDE.md section or a design doc).`, '',
    HEADINGS[3], '', `${TO_WRITE} The exact recipes, test files, filters and rigs that cover it.`, '',
    HEADINGS[4], '', `${TO_WRITE} From docs/PRIORITIES.md, the open entries of docs/BUGS.md and docs/design/crate-split.md, each with its source.`, '',
    HEADINGS[5], '',
    'This crate\'s mailbox. A session working in another crate that needs something here (an item made',
    '`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work',
    'deletes it in the same commit, so the commit keeps the record. One line per request, in this form',
    '(`node scripts/crate-cards.js --check` holds it):', '',
    '`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`', '',
    'None yet.', '',
  ].join('\n');
}

function refresh(plan) {
  const { analyse } = require('./crate-graph.js');
  const all = facts(plan, analyse({ tests: true }));
  const texts = new Map();
  let changed = 0;
  const problems = [];
  for (const c of plan.crates) {
    const file = path.join(CARDS, `${c.name}.md`);
    if (!fs.existsSync(file)) { problems.push(`no card for ${c.name} (start one: node scripts/crate-cards.js --new ${c.name})`); continue; }
    const text = fs.readFileSync(file, 'utf8');
    const next = replaceBlock(text, cardBlock(all.get(c.name)));
    if (next === null) { problems.push(`docs/crates/${c.name}.md has no generated block to refresh`); texts.set(c.name, text); continue; }
    if (next !== text) { fs.writeFileSync(file, next); changed++; }
    texts.set(c.name, next);
  }
  const index = path.join(CARDS, 'README.md');
  if (fs.existsSync(index)) {
    const text = fs.readFileSync(index, 'utf8');
    const next = replaceBlock(text, readmeBlock(plan, all, texts));
    if (next === null) problems.push('docs/crates/README.md has no generated block to refresh');
    else if (next !== text) { fs.writeFileSync(index, next); changed++; }
  } else problems.push('docs/crates/README.md, the index, is missing');
  const waiting = plan.crates.map((c) => [c.name, texts.has(c.name) ? requestsOf(texts.get(c.name)).length : 0]).filter(([, n]) => n);
  return { all, changed, problems, waiting, files: plan.crates.length + 1 };
}

function main() {
  const plan = JSON.parse(fs.readFileSync(PLAN_PATH, 'utf8'));
  const args = process.argv.slice(2);
  if (args.includes('--check')) {
    const problems = checkAll(CARDS, plan);
    for (const p of problems) console.log(`crate cards: FAILED: ${p}`);
    if (problems.length) process.exit(1);
    console.log(`>> lint ok: crate cards (${plan.crates.length} cards, each with its headings and one generated block)`);
    return;
  }
  if (args.includes('--new')) {
    const name = args[args.indexOf('--new') + 1];
    if (!plan.crates.some((c) => c.name === name)) { console.error(`crate cards: "${name}" is not a crate in scripts/crate-layers.json`); process.exit(1); }
    const file = path.join(CARDS, `${name}.md`);
    if (fs.existsSync(file)) { console.error(`crate cards: docs/crates/${name}.md already exists`); process.exit(1); }
    fs.mkdirSync(CARDS, { recursive: true });
    fs.writeFileSync(file, skeleton(name));
    console.log(`crate cards: wrote docs/crates/${name}.md; write its "${TO_WRITE}" parts (the check fails until they are gone)`);
  }
  if (args.includes('--interface')) {
    const name = args[args.indexOf('--interface') + 1];
    const { analyse } = require('./crate-graph.js');
    const f = facts(plan, analyse({ tests: true })).get(name);
    if (!f) { console.error(`crate cards: "${name}" is not a crate in scripts/crate-layers.json`); process.exit(1); }
    for (const [item, e] of [...f.iface].sort((x, y) => y[1].n - x[1].n || (x[0] < y[0] ? -1 : 1))) console.log(`${String(e.n).padStart(5)}  ${item.padEnd(70)} ${counts(e.from)}`);
    return;
  }
  const { changed, problems, waiting, files } = refresh(plan);
  for (const p of problems) console.log(`crate cards: ${p}`);
  console.log(`crate cards: docs/crates/ refreshed (${changed} of ${files} files changed); requests waiting: ${waiting.length ? waiting.map(([n, k]) => `${n} ${k}`).join(', ') : 'none'}`);
  if (problems.length) process.exit(1);
}

if (require.main === module) main();

module.exports = { checkCard, checkReadme, checkAll, requestsOf, replaceBlock, itemOf, gateLabel, skeleton, testNames, HEADINGS, START, END };
