#!/usr/bin/env node
// The crate's module graph and the upward references under the proposed crate layering
// (docs/design/crate-split.md). Read-only. Run: node scripts/crate-graph.js [--edges]
//   default  every top-level module: lines, feature gate, what it reaches, the cycles, the leaves
//   --edges  each reference that points UP the layering, by edge, with the items it names
// Written 2026-10-30 to plan the split from data rather than from a guess.
if (process.argv.includes("--edges")) {
  const fs = require('fs');
  const path = require('path');
  const ROOT = 'C:/Humanity/src';
  const layer = {};
  const L = [
    'input audio utilities physics debug platform turning reactions lod_registry boot_timing credits scenic_views showroom mods player_look cosmetics test_temp test_log test_clock auto_unlock',
    'embedded_data ecs hot_reload storage cosmos dev_travel',
    'systems machines ship persistence station surface_walk surface_move',
    'relay how_to release_update renderer terrain assets',
    'engine gui net config save_load media games software web_reader updater wasm_entry',
  ];
  L.forEach((s, i) => s.split(/\s+/).forEach((m) => (layer[m] = i)));
  const SAME3 = new Set(['relay', 'how_to', 'release_update']); // the server group; renderer/terrain/assets the other
  function walk(dir, out) {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) walk(p, out);
      else if (e.name.endsWith('.rs')) out.push(p);
    }
  }
  const files = [];
  walk(ROOT, files);
  const edges = new Map(); // "from->to" -> Map(item -> count)
  for (const f of files) {
    const rel = path.relative(ROOT, f).replace(/\\/g, '/');
    if (rel === 'lib.rs' || rel === 'main.rs' || rel === 'build.rs') continue;
    if (/_tests?\.rs$|\/tests?\//.test(rel)) continue;
    const from = rel.split('/')[0].replace(/\.rs$/, '');
    if (!(from in layer)) continue;
    const src = fs.readFileSync(f, 'utf8').replace(/\/\/.*$/gm, '');
    const refs = [...src.matchAll(/crate::([a-z_0-9]+)((?:::[A-Za-z_0-9]+)*)/g)].map((r) => [r[1], r[1] + r[2]]);
    for (const g of src.matchAll(/use\s+crate::\{([^}]*)\}/g)) {
      for (const part of g[1].split(',')) {
        const p = part.trim().replace(/\s+as\s+\w+$/, '');
        if (p) refs.push([p.split('::')[0], p]);
      }
    }
    for (const [to, item] of refs) {
      if (!(to in layer) || to === from) continue;
      const up = layer[to] > layer[from] || (layer[to] === 3 && layer[from] === 3 && SAME3.has(to) !== SAME3.has(from));
      if (!up) continue;
      const k = `${from} -> ${to}`;
      const m = edges.get(k) || new Map();
      m.set(item, (m.get(item) || 0) + 1);
      edges.set(k, m);
    }
  }
  const rows = [...edges.entries()].map(([k, m]) => [k, [...m.values()].reduce((a, b) => a + b, 0), m]).sort((a, b) => b[1] - a[1]);
  let total = 0;
  for (const [k, n, m] of rows) {
    total += n;
    const items = [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8).map(([i, c]) => `${i}(${c})`).join(' ');
    console.log(`${String(n).padStart(4)}  ${k.padEnd(30)} ${items}`);
  }
  console.log(`\n${rows.length} upward edges, ${total} references in non-test code`);
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
