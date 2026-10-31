#!/usr/bin/env node
// The crate's module graph and the references that point the wrong way under the planned crate
// split (docs/design/crate-split.md). Read-only. Run: node scripts/crate-graph.js [--edges] [--crates]
//   default   every top-level module: lines, feature gate, what it reaches, the cycles, the leaves
//   --edges   each reference a crate may not make under scripts/crate-layers.json (the plan's one
//             source), by edge, with the items it names: the rung-0 work list
//   --crates  each planned crate's lines, and what it may use
// Written 2026-10-30 to plan the split from data rather than from a guess; --edges read a
// hard-coded five-layer list until the plan grew to ten crates the same day.
//
// WHAT COUNTS AS TEST CODE (2026-10-31, rung 0). The first reading cut each file at its first
// `#[cfg(test)] mod`, assuming tests sit at the end. Two kinds of file broke that: one that
// declares a test FILE near its top (`#[cfg(test)] mod sowing_tests;` on line 22 of
// systems/farming/mod.rs hid the 2,800 lines after it, eleven wrong-way references among them),
// and one whose test module carries another gate (`#[cfg(all(test, feature = "native"))]`, which
// the cut did not see, so its tests counted as code). Now the file is read as Rust: comments and
// the insides of string and character literals blanked (a path in a log message is not a
// reference), then every item gated to tests alone removed wherever it sits, by its braces; and a
// file a test-only `mod x;` declares (with its `#[path]`) is a test file, as is everything under it.

/** The source with comments and the insides of string and char literals turned to spaces
 * (newlines kept), so every offset still points at the same place in the original. */
function blankLiterals(src) {
  const out = src.split('');
  const n = src.length;
  const blank = (a, b) => { for (let k = a; k < b; k++) if (out[k] !== '\n' && out[k] !== '\r') out[k] = ' '; };
  let i = 0;
  while (i < n) {
    const c = src[i], d = src[i + 1];
    if (c === '/' && d === '/') {
      const e = src.indexOf('\n', i);
      const end = e < 0 ? n : e;
      blank(i, end);
      i = end;
      continue;
    }
    if (c === '/' && d === '*') {
      let depth = 1, j = i + 2;
      while (j < n && depth) {
        if (src[j] === '/' && src[j + 1] === '*') { depth++; j += 2; }
        else if (src[j] === '*' && src[j + 1] === '/') { depth--; j += 2; }
        else j++;
      }
      blank(i, j);
      i = j;
      continue;
    }
    // A raw string, r"..." or r#"..."# (and br"..."): the closing quote is the one followed by as
    // many hashes as opened it. Not when the r ends an identifier.
    if ((c === 'r' || (c === 'b' && d === 'r')) && !/[A-Za-z0-9_]/.test(src[i - 1] || '')) {
      let j = c === 'b' ? i + 2 : i + 1;
      let hashes = 0;
      while (src[j] === '#') { hashes++; j++; }
      if (src[j] === '"') {
        const close = '"' + '#'.repeat(hashes);
        const e = src.indexOf(close, j + 1);
        const end = e < 0 ? n : e;
        blank(j + 1, end);
        i = e < 0 ? n : end + close.length;
        continue;
      }
    }
    if (c === '"') {
      let j = i + 1;
      while (j < n && src[j] !== '"') { if (src[j] === '\\') j++; j++; }
      blank(i + 1, j);
      i = j + 1;
      continue;
    }
    // A char literal ('x', '\n', '\u{2014}', '\''); a lifetime ('a, 'static) has no closing quote.
    if (c === "'") {
      const m = /^'(?:\\(?:u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|.)|[^\\'\r\n])'/.exec(src.slice(i, i + 12));
      if (m) {
        blank(i + 1, i + m[0].length - 1);
        i += m[0].length;
        continue;
      }
    }
    i++;
  }
  return out.join('');
}

/** Whether a `cfg(...)` predicate (spaces removed) holds only in test builds: `test`, or an
 * `all(...)` with `test` among its terms. `any(test, ...)` and `not(test)` are not. */
function cfgIsTestOnly(expr) {
  if (expr === 'test') return true;
  if (!expr.startsWith('all(') || !expr.endsWith(')')) return false;
  const terms = [];
  let depth = 0, cur = '';
  for (const ch of expr.slice(4, -1)) {
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (ch === ',' && depth === 0) { terms.push(cur); cur = ''; } else cur += ch;
  }
  terms.push(cur);
  return terms.includes('test');
}

/** A file's non-test code, as `blankLiterals` gives it with every item gated to tests alone
 * (`#[cfg(test)]`, `#[cfg(all(test, ...))]`: a module, a function, a `use`) blanked too, and the
 * test-only module FILES it declares: `[{ name, path }]`, `path` from a `#[path = "..."]` or null. */
function nonTestCode(raw) {
  const code = blankLiterals(raw).split('');
  const text = code.join('');
  const testMods = [];
  const re = /#\[\s*cfg\s*\(/g;
  let m;
  while ((m = re.exec(text))) {
    // The predicate: the balanced parentheses after `cfg(`.
    let i = m.index + m[0].length, depth = 1;
    const start = i;
    while (i < text.length && depth) { if (text[i] === '(') depth++; else if (text[i] === ')') depth--; i++; }
    if (!cfgIsTestOnly(text.slice(start, i - 1).replace(/\s+/g, ''))) continue;
    // The item it gates ends at its first `;` outside braces, or at the brace that closes its body
    // (attributes between hold neither: their strings are blanked).
    let j = text.indexOf(']', i) + 1, braces = 0, end = text.length;
    for (; j < text.length; j++) {
      const ch = text[j];
      if (ch === '{') braces++;
      else if (ch === '}') { braces--; if (braces === 0) { end = j + 1; break; } }
      else if (ch === ';' && braces === 0) { end = j + 1; break; }
    }
    const item = text.slice(m.index, end);
    const decl = /^#\[[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_0-9]+)\s*;$/.exec(item.trim());
    if (decl) {
      const p = /#\[\s*path\s*=\s*"([^"]+)"\s*\]/.exec(raw.slice(m.index, end));
      testMods.push({ name: decl[1], path: p ? p[1] : null });
    }
    for (let k = m.index; k < end; k++) if (code[k] !== '\n' && code[k] !== '\r') code[k] = ' ';
    re.lastIndex = end;
  }
  // `all` is the whole file as `blankLiterals` gives it, test items included (the crate cards
  // read test code from the difference, scripts/crate-cards.js).
  return { code: code.join(''), testMods, all: text };
}

/** Only the test-gated items of a file: where `all` (literals blanked) and `code` (test items
 * blanked too) differ, `all`'s text; everywhere else spaces, newlines kept. */
function testOnlyCode(all, code) {
  let out = '';
  for (let k = 0; k < all.length; k++) {
    const ch = all[k];
    out += ch === code[k] ? (ch === '\n' || ch === '\r' ? ch : ' ') : ch;
  }
  return out;
}

/** The `cfg(...)` predicates on each file module a source declares (`mod name;`, with or without
 * `pub`): Map(name -> [predicate, ...]), from the attributes on its line and the lines right above
 * it (blank lines, comments, doc comments and other attributes between are passed over). Shared
 * by the default report below and the crate cards (scripts/crate-cards.js). */
function declaredGates(text) {
  const lines = text.split(/\r?\n/);
  const out = new Map();
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(/^\s*((?:#\[[^\]]*\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_0-9]+)\s*;/);
    if (!m) continue;
    const gates = [];
    for (let j = i - 1; j >= 0; j--) {
      const t = lines[j].trim();
      const g = t.match(/^#\[cfg\((.*)\)\]/);
      if (g) gates.unshift(g[1]);
      else if (t && !t.startsWith('//') && !t.startsWith('#[')) break;
    }
    for (const a of m[1].matchAll(/#\[cfg\((.*?)\)\]/g)) gates.push(a[1]);
    out.set(m[2], gates);
  }
  return out;
}

/** A module file's own gates: its `#![cfg(...)]` inner attributes (release_update.rs has one). */
function innerGates(text) {
  return [...text.matchAll(/^#!\[cfg\((.*)\)\]/gm)].map((m) => m[1]);
}

/** The files a test-only `mod` declaration in `rel` (a path under src/, `/`-separated) can name:
 * `#[path]` is relative to the declaring file's folder; otherwise a mod.rs's children sit beside
 * it and an `a/b.rs`'s in `a/b/`, as `x.rs` or `x/mod.rs`. */
function declaredFiles(rel, decl) {
  const dir = rel.includes('/') ? rel.slice(0, rel.lastIndexOf('/')) : '';
  const join = (a, b) => (a ? `${a}/${b}` : b);
  if (decl.path) return [join(dir, decl.path).replace(/\/\.\//g, '/')];
  const own = /(^|\/)(mod|lib|main)\.rs$/.test(rel) ? dir : rel.replace(/\.rs$/, '');
  return [join(own, `${decl.name}.rs`), join(own, `${decl.name}/mod.rs`)];
}

/** The plan's reading of src/: lines per crate, the wrong-way edges ("from -> to" -> Map("module: item" -> count)),
 * and the top-level modules no crate claims. Shared with scripts/check-crate-layers.js (the ratchet).
 * Also, for the crate cards (scripts/crate-cards.js, 2026-10-31): `crossing`, EVERY reference from one
 * crate into another, allowed or not ("from -> to" -> Map(the path as written after `crate::` -> count)),
 * counted exactly as the edges are; `files` ({ rel, crate, lines, test, raw, code }); and the plan's
 * lookups (`crateOf`, `may`, `isTestFile`).
 * `{ tests: true }` also reads the TEST code in the same pass (test files, and the test-gated items of
 * the others) into `testEdges`, the wrong-way references rung 1 must move first
 * (docs/design/crate-split.md, rung 0's "Not counted"). There a `super::` counts only in a test file:
 * inside an inline `mod tests { }` it names the file's own module, which the file's path alone cannot
 * tell from its parent, and that module is the code under test, in the same crate. */
function analyse(opts = {}) {
  const withTests = !!opts.tests;
  const fs = require('fs');
  const path = require('path');
  const ROOT = path.join(__dirname, '..', 'src');
  const PLAN = JSON.parse(fs.readFileSync(path.join(__dirname, 'crate-layers.json'), 'utf8'));
  // path -> crate, and each crate's transitive set of crates it may use.
  const owner = new Map();
  for (const c of PLAN.crates) for (const p of c.paths) owner.set(p.replace(/\.rs$/, ''), c.name);
  const direct = new Map(PLAN.crates.map((c) => [c.name, c.uses]));
  const may = new Map();
  const reach = (n, seen = new Set()) => {
    for (const u of direct.get(n) || []) if (!seen.has(u)) { seen.add(u); reach(u, seen); }
    return seen;
  };
  for (const c of PLAN.crates) may.set(c.name, reach(c.name));
  // The crate that owns a module path ("engine/protected/x"): its most specific listed prefix.
  const crateOf = (mod) => {
    const parts = mod.split('/');
    for (let k = parts.length; k > 0; k--) {
      const hit = owner.get(parts.slice(0, k).join('/'));
      if (hit) return hit;
    }
    return null;
  };
  function walk(dir, out) {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) walk(p, out);
      else if (e.name.endsWith('.rs')) out.push(p);
    }
  }
  const files = [];
  walk(ROOT, files);
  // Every file read once, its non-test code worked out, and the test-only module files it declares
  // noted: a declared test file is test code, and so is every file under its folder.
  const parsed = [];
  const testFiles = new Set();
  const testDirs = [];
  // A file a `#[path = "..."] mod x;` names belongs to the module that declares it, wherever it
  // sits (2026-10-31, the crate cards): engine/household.rs declares engine/household_tests.rs as
  // its `tests`, so that file is `engine::household::tests`, the GUI's, not a file of the engine
  // folder's. file -> { by: the declaring file, name }.
  const declaredBy = new Map();
  for (const f of files) {
    const rel = path.relative(ROOT, f).replace(/\\/g, '/');
    if (rel === 'build.rs') continue;
    const raw = fs.readFileSync(f, 'utf8');
    const { code, testMods, all } = nonTestCode(raw);
    for (const t of testMods) {
      for (const p of declaredFiles(rel, t)) {
        testFiles.add(p);
        testDirs.push(p.replace(/\/mod\.rs$/, '').replace(/\.rs$/, '') + '/');
      }
    }
    for (const d of pathDecls(raw, all)) {
      const target = declaredFiles(rel, d)[0];
      if (!declaredBy.has(target)) declaredBy.set(target, { by: rel, name: d.name });
    }
    parsed.push({ rel, raw, code, all });
  }
  const isTestFile = (rel) =>
    /_tests?\.rs$|(^|\/)tests?(\.rs$|\/)/.test(rel) || testFiles.has(rel) || testDirs.some((d) => rel.startsWith(d));
  // A file's module path: a/b.rs -> a/b, a/b/mod.rs -> a/b, and a `#[path]` file its declarer's
  // path and the name it was declared by (lib.rs's children sit at the top).
  const modulePath = (rel, depth = 0) => {
    const d = declaredBy.get(rel);
    if (d && depth < 16) {
      const base = /^(lib|main)\.rs$/.test(d.by) ? '' : modulePath(d.by, depth + 1);
      return base ? `${base}/${d.name}` : d.name;
    }
    return rel.replace(/\/mod\.rs$/, '').replace(/\.rs$/, '');
  };
  const lines = new Map();
  const edges = new Map(); // "from -> to" -> Map("module: item" -> count)
  const crossing = new Map(); // "from -> to" -> Map("path::as::written" -> count), allowed ones too
  const testEdges = new Map(); // as `edges`, in test code (only with { tests: true })
  const fileList = [];
  let unowned = new Set();
  for (const { rel, raw, code, all } of parsed) {
    const mod = modulePath(rel);
    const from = crateOf(mod);
    if (!from) { unowned.add(mod.split('/')[0]); continue; }
    const n = raw.split('\n').length;
    lines.set(from, (lines.get(from) || 0) + n);
    const test = isTestFile(rel);
    fileList.push({ rel, mod, crate: from, lines: n, test, raw, code, all });
    // `super::` resolves against this file's module: a/b/c.rs's super is a/b (mod.rs's is a's).
    const selfPath = mod.split('/');
    const parentOf = (n) => selfPath.slice(0, Math.max(0, selfPath.length - n));
    // One reading of the code (never a test file's), and with { tests: true } one of the tests.
    const readings = [];
    if (!test) readings.push({ src: code, supers: true, into: edges, cross: crossing });
    if (withTests) readings.push({ src: test ? all : testOnlyCode(all, code), supers: test, into: testEdges, cross: null });
    for (const { src, supers, into, cross } of readings) {
      const refs = [];
      for (const r of src.matchAll(/crate::([a-z_0-9]+(?:::[A-Za-z_0-9]+)*)/g)) refs.push(r[1].split('::'));
      for (const g of src.matchAll(/use\s+crate::\{([^}]*)\}/g)) {
        for (const part of g[1].split(',')) {
          const p = part.trim().replace(/\s+as\s+\w+$/, '');
          if (p) refs.push(p.split('::'));
        }
      }
      if (supers) {
        for (const r of src.matchAll(/((?:super::)+)([A-Za-z_0-9]+(?:::[A-Za-z_0-9]+)*)/g)) {
          const ups = r[1].length / 'super::'.length;
          refs.push([...parentOf(ups), ...r[2].split('::')]);
        }
      }
      for (const segs of refs) {
        const lower = [];
        for (const s of segs) { if (!/^[a-z_0-9]+$/.test(s)) break; lower.push(s); }
        const to = crateOf(lower.join('/'));
        if (!to || to === from) continue;
        const k = `${from} -> ${to}`;
        if (cross) {
          const c = cross.get(k) || new Map();
          const target = segs.join('::');
          c.set(target, (c.get(target) || 0) + 1);
          cross.set(k, c);
        }
        if (may.get(from).has(to)) continue;
        const m = into.get(k) || new Map();
        const item = `${selfPath[0]}: ${segs.join('::')}`;
        m.set(item, (m.get(item) || 0) + 1);
        into.set(k, m);
      }
    }
  }
  return { PLAN, lines, edges, unowned, crossing, testEdges, files: fileList, crateOf, may, isTestFile };
}

/** Every `#[path = "..."] mod name;` in a file's code (comments passed over; `all` is the file as
 * `blankLiterals` gives it, so the path itself is read from `raw` at the same place). */
function pathDecls(raw, all) {
  const out = [];
  const re = /#\[\s*path\s*=\s*"[^"]*"\s*\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_0-9]+)\s*;/g;
  for (const m of all.matchAll(re)) {
    const p = /#\[\s*path\s*=\s*"([^"]+)"/.exec(raw.slice(m.index, m.index + m[0].length));
    if (p) out.push({ name: m[1], path: p[1] });
  }
  return out;
}

if (require.main === module && (process.argv.includes("--edges") || process.argv.includes("--crates"))) {
  const { PLAN, lines, edges, unowned } = analyse();
  if (process.argv.includes("--crates")) {
    for (const c of PLAN.crates) {
      console.log(`${String(lines.get(c.name) || 0).padStart(7)}  ${c.name.padEnd(20)} uses ${c.uses.map((u) => u.replace('humanity-', '')).join(', ') || '(nothing)'}`);
    }
  }
  if (process.argv.includes("--edges")) {
    const rows = [...edges.entries()].map(([k, m]) => [k, [...m.values()].reduce((a, b) => a + b, 0), m]).sort((a, b) => b[1] - a[1]);
    let total = 0;
    for (const [k, n, m] of rows) {
      total += n;
      const items = [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8).map(([i, c]) => `${i}(${c})`).join('  ');
      console.log(`${String(n).padStart(4)}  ${k.padEnd(40)} ${items}`);
    }
    console.log(`\n${rows.length} wrong-way edges, ${total} references in non-test code (plan: scripts/crate-layers.json)`);
  }
  if (unowned.size) console.log(`\nNOT IN THE PLAN (add them to scripts/crate-layers.json): ${[...unowned].join(', ')}`);
} else if (require.main === module) {
  const fs = require('fs');
  const path = require('path');
  const ROOT = path.join(__dirname, '..', 'src');
  
  // 1. Top-level modules and their gates, from lib.rs (the reader the crate cards share).
  const mods = new Map();
  for (const [name, gates] of declaredGates(fs.readFileSync(path.join(ROOT, 'lib.rs'), 'utf8'))) {
    mods.set(name, { name, gate: gates.join(' & ') || 'always', files: [], lines: 0, out: new Map() });
  }
  
  // 2. Files of each module.
  function walk(dir, out) {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) walk(p, out);
      else if (e.name.endsWith('.rs')) out.push(p);
    }
  }
  for (const m of mods.values()) {
    const f = path.join(ROOT, m.name + '.rs');
    if (fs.existsSync(f)) m.files.push(f);
    const d = path.join(ROOT, m.name);
    if (fs.existsSync(d) && fs.statSync(d).isDirectory()) walk(d, m.files);
  }
  
  // 3. References: crate::X anywhere, and `use crate::{X, Y::...}` groups.
  for (const m of mods.values()) {
    for (const f of m.files) {
      const src = fs.readFileSync(f, 'utf8');
      m.lines += src.split('\n').length;
      const isTest = /_tests?\.rs$|[\\/]tests?[\\/]/.test(f);
      const hit = (to) => {
        if (to === m.name || !mods.has(to)) return;
        const e = m.out.get(to) || { refs: 0, files: new Set(), testOnly: true };
        e.refs++;
        e.files.add(f);
        if (!isTest) e.testOnly = false;
        m.out.set(to, e);
      };
      for (const r of src.matchAll(/crate::([a-z_0-9]+)/g)) hit(r[1]);
      for (const g of src.matchAll(/use\s+crate::\{([^}]*)\}/g)) {
        for (const part of g[1].split(',')) {
          const head = part.trim().split('::')[0].trim();
          if (head) hit(head);
        }
      }
    }
  }
  
  // 4. Strongly connected components (Tarjan), on non-test edges.
  const names = [...mods.keys()];
  let index = 0;
  const idx = new Map(), low = new Map(), on = new Set(), stack = [], sccs = [];
  function strong(v) {
    idx.set(v, index); low.set(v, index); index++; stack.push(v); on.add(v);
    for (const [w, e] of mods.get(v).out) {
      if (e.testOnly) continue;
      if (!idx.has(w)) { strong(w); low.set(v, Math.min(low.get(v), low.get(w))); }
      else if (on.has(w)) low.set(v, Math.min(low.get(v), idx.get(w)));
    }
    if (low.get(v) === idx.get(v)) {
      const c = [];
      let w;
      do { w = stack.pop(); on.delete(w); c.push(w); } while (w !== v);
      sccs.push(c);
    }
  }
  for (const v of names) if (!idx.has(v)) strong(v);
  
  // 5. Report.
  const byLines = [...mods.values()].sort((a, b) => b.lines - a.lines);
  const total = byLines.reduce((s, m) => s + m.lines, 0);
  console.log(`MODULES (${mods.size}), ${total} lines in their files (lib.rs itself not included)`);
  for (const m of byLines) {
    const deps = [...m.out.entries()].filter(([, e]) => !e.testOnly).sort((a, b) => b[1].refs - a[1].refs);
    console.log(`${String(m.lines).padStart(7)}  ${m.name.padEnd(16)} [${m.gate}]  -> ${deps.map(([t, e]) => `${t}(${e.refs})`).join(' ')}`);
  }
  console.log('\nCYCLES (modules that reach each other, directly or round a loop; non-test references):');
  for (const c of sccs.filter((c) => c.length > 1).sort((a, b) => b.length - a.length)) {
    const lines = c.reduce((s, n) => s + mods.get(n).lines, 0);
    console.log(`  ${c.length} modules, ${lines} lines: ${c.sort().join(', ')}`);
  }
  const leaves = byLines.filter((m) => [...m.out.values()].every((e) => e.testOnly)).map((m) => `${m.name}(${m.lines})`);
  console.log('\nLEAVES (reach no other top-level module): ' + leaves.join(', '));
  // Who depends on each module (fan-in), non-test.
  const fanIn = new Map(names.map((n) => [n, []]));
  for (const m of mods.values()) for (const [t, e] of m.out) if (!e.testOnly) fanIn.get(t).push(m.name);
  console.log('\nFAN-IN (used by):');
  for (const m of byLines.slice(0, 20)) console.log(`  ${m.name.padEnd(16)} <- ${fanIn.get(m.name).length}: ${fanIn.get(m.name).join(' ')}`);

}

module.exports = { analyse, blankLiterals, cfgIsTestOnly, nonTestCode, testOnlyCode, declaredFiles, declaredGates, innerGates, pathDecls };
