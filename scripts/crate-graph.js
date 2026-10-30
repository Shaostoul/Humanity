#!/usr/bin/env node
// The crate's module graph and the references that point the wrong way under the planned crate
// split (docs/design/crate-split.md). Read-only. Run: node scripts/crate-graph.js [--edges] [--crates]
//   default   every top-level module: lines, feature gate, what it reaches, the cycles, the leaves
//   --edges   each reference a crate may not make under scripts/crate-layers.json (the plan's one
//             source), by edge, with the items it names: the rung-0 work list
//   --crates  each planned crate's lines, and what it may use
// Written 2026-10-30 to plan the split from data rather than from a guess; --edges read a
// hard-coded five-layer list until the plan grew to ten crates the same day.
if (process.argv.includes("--edges") || process.argv.includes("--crates")) {
  const fs = require('fs');
  const path = require('path');
  const ROOT = 'C:/Humanity/src';
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
  const lines = new Map();
  const edges = new Map(); // "from -> to" -> Map("module: item" -> count)
  let unowned = new Set();
  for (const f of files) {
    const rel = path.relative(ROOT, f).replace(/\\/g, '/');
    if (rel === 'build.rs') continue;
    // The file's module path: a/b.rs -> a/b, a/b/mod.rs -> a/b.
    const mod = rel.replace(/\/mod\.rs$/, '').replace(/\.rs$/, '');
    const from = crateOf(mod);
    if (!from) { unowned.add(mod.split('/')[0]); continue; }
    const raw = fs.readFileSync(f, 'utf8');
    lines.set(from, (lines.get(from) || 0) + raw.split('\n').length);
    if (/_tests?\.rs$|\/tests?\//.test(rel)) continue;
    // Non-test code only: comments out, and everything from the first inline test module on.
    let src = raw.replace(/\/\/.*$/gm, '');
    const ti = src.search(/#\[cfg\(test\)\]\s*mod\s/);
    if (ti >= 0) src = src.slice(0, ti);
    // `super::` resolves against this file's module: a/b/c.rs's super is a/b (mod.rs's is a's).
    const selfPath = mod.split('/');
    const parentOf = (n) => selfPath.slice(0, Math.max(0, selfPath.length - n));
    const refs = [];
    for (const r of src.matchAll(/crate::([a-z_0-9]+(?:::[A-Za-z_0-9]+)*)/g)) refs.push(r[1].split('::'));
    for (const g of src.matchAll(/use\s+crate::\{([^}]*)\}/g)) {
      for (const part of g[1].split(',')) {
        const p = part.trim().replace(/\s+as\s+\w+$/, '');
        if (p) refs.push(p.split('::'));
      }
    }
    for (const r of src.matchAll(/((?:super::)+)([A-Za-z_0-9]+(?:::[A-Za-z_0-9]+)*)/g)) {
      const ups = r[1].length / 'super::'.length;
      refs.push([...parentOf(ups), ...r[2].split('::')]);
    }
    for (const segs of refs) {
      const lower = [];
      for (const s of segs) { if (!/^[a-z_0-9]+$/.test(s)) break; lower.push(s); }
      const to = crateOf(lower.join('/'));
      if (!to || to === from || may.get(from).has(to)) continue;
      const k = `${from} -> ${to}`;
      const m = edges.get(k) || new Map();
      const item = `${selfPath[0]}: ${segs.join('::')}`;
      m.set(item, (m.get(item) || 0) + 1);
      edges.set(k, m);
    }
  }
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
} else {
  const fs = require('fs');
  const path = require('path');
  const ROOT = 'C:/Humanity/src';
  
  // 1. Top-level modules and their gates, from lib.rs.
  const lib = fs.readFileSync(path.join(ROOT, 'lib.rs'), 'utf8').split(/\r?\n/);
  const mods = new Map();
  for (let i = 0; i < lib.length; i++) {
    const m = lib[i].match(/^\s*(?:pub(?:\(crate\))?\s+)?mod\s+([a-z_0-9]+)\s*;/);
    if (!m) continue;
    const gates = [];
    for (let j = i - 1; j >= 0 && j >= i - 6; j--) {
      const g = lib[j].match(/#\[cfg\((.*)\)\]/);
      if (g) gates.unshift(g[1]);
      else if (lib[j].trim() && !lib[j].trim().startsWith('//') && !lib[j].trim().startsWith('#[')) break;
    }
    mods.set(m[1], { name: m[1], gate: gates.join(' & ') || 'always', files: [], lines: 0, out: new Map() });
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
