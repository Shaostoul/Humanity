#!/usr/bin/env node
// Every source path named in schemas/ must exist.
//
// WHY THIS EXISTS
// schemas/ is what an agent or a contributor reads to learn what a row IS, and
// most of the files explain themselves by pointing at the code that consumes
// them ("Combat system -- src/systems/combat/ ..."). On 2026-09-15, SIXTEEN of
// the thirty schema files still pointed at `native/src/`, `server/src/` or
// `crates/`, none of which have existed since the unified-binary merge in
// v0.90.0.
//
// CLAUDE.md carries a standing warning about exactly this: an agent that reads
// a stale path writes its edits to a dead location, and it has happened. A
// schema is a high-traffic document for precisely the readers most likely to
// act on a path without checking it.
//
// Four paths survived the mechanical rewrite because the modules themselves had
// moved: `systems/economy.rs` and `systems/inventory.rs` became directories,
// client-side `systems/trading.rs` never existed (trading is relay-side), and
// there is no `construction/csg.rs`. Those were repointed by hand at what is
// actually on disk, which is the part a blind substitution would have got wrong.
//
// PROVEN RED: point any schema at a module that does not exist and this names
// the file and the path.
//
//   node scripts/check-schema-paths.js
//   node scripts/check-schema-paths.js --docs   # also REPORT on docs/, never fail

const fs = require('fs');
const path = require('path');

const DEAD_PREFIXES = ['native/src/', 'server/src/', 'crates/'];
const ALSO_DOCS = process.argv.includes('--docs');

// A path in prose may name a file, a directory, or a module whose file and
// directory forms are interchangeable. Resolve generously; the point is to
// catch paths that resolve to NOTHING.
function exists(p) {
  const c = p.replace(/[.,;:)\]]+$/, '');
  return [c, c + '.rs', c.replace(/\/$/, '') + '.rs', c.replace(/\/$/, '')]
    .some(x => fs.existsSync(x));
}

function scan(file) {
  const text = fs.readFileSync(file, 'utf8');
  const out = [];
  for (const dead of DEAD_PREFIXES) {
    if (text.includes(dead)) {
      out.push({ file, path: dead, why: 'a path prefix that has not existed since the unified-binary merge' });
    }
  }
  for (const m of text.match(/\bsrc\/[A-Za-z0-9_\/.-]*/g) || []) {
    if (!exists(m)) out.push({ file, path: m, why: 'resolves to nothing on disk' });
  }
  return out;
}

function filesIn(dir, filter) {
  const out = [];
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { out.push(...filesIn(p, filter)); continue; }
    if (filter(p)) out.push(p);
  }
  return out;
}

const problems = [];
let checked = 0, paths = 0;

for (const f of filesIn('schemas', () => true)) {
  checked++;
  const hits = scan(f);
  paths += (fs.readFileSync(f, 'utf8').match(/\bsrc\/[A-Za-z0-9_\/.-]*/g) || []).length;
  problems.push(...hits);
}

if (problems.length) {
  console.log('');
  console.log('Schema files naming a source path that does not exist:');
  for (const p of problems) console.log('  ' + p.file + ': ' + p.path + '  (' + p.why + ')');
  console.log('');
  console.log('  A schema is what somebody reads to learn what a row is. A stale');
  console.log('  path there sends them to edit a file that is not there.');
  console.log('');
}

console.log('Checked ' + paths + ' source paths across ' + checked +
  ' schema files. Unresolved: ' + problems.length);

// docs/ is a different problem and must NOT fail the build, because a doc that
// MENTIONS a dead path is usually doing the right thing: STATUS.md saying those
// directories "no longer exist", 02-ARCHITECTURE.md warning you about agents
// that reference them, SOP.md recording the migration. All 17 live docs that
// match were read by hand on 2026-09-15 and every one was correct.
//
// So this lists mentions, says what they usually are, and screens out the two
// false positives that look like paths and are not: crates.io URLs, and the
// wooden crates bobbing in the ocean design doc.
if (ALSO_DOCS) {
  const FALSE_POSITIVE = /crates\.io|crates\/debris/;
  const docFiles = filesIn('docs', p => p.endsWith('.md') && !p.includes('history'));
  const byFile = new Map();
  for (const f of docFiles) {
    const text = fs.readFileSync(f, 'utf8');
    let n = 0;
    for (const line of text.split(/\r?\n/)) {
      if (FALSE_POSITIVE.test(line)) continue;
      if (DEAD_PREFIXES.some(d => line.includes(d))) n++;
    }
    if (n) byFile.set(f, n);
  }
  console.log('');
  console.log('docs/ mentions of a pre-v0.90 path (listed, never fails; history/ skipped):');
  if (!byFile.size) {
    console.log('  none');
  } else {
    for (const [f, n] of [...byFile].sort((a, b) => b[1] - a[1])) {
      console.log('  ' + String(n).padStart(3) + '  ' + f);
    }
    console.log('');
    console.log('  A mention here is usually CORRECT: a doc recording that those');
    console.log('  directories are gone, or warning you about agents that still');
    console.log('  reference them. Read the line before treating it as rot. Every');
    console.log('  one of these was hand-checked on 2026-09-15 and was correct.');
  }
}

process.exit(problems.length ? 1 : 0);
