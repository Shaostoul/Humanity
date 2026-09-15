#!/usr/bin/env node
// Refuses an em or en dash in any document the Library ships.
//
// WHY THIS EXISTS
// "No em dashes anywhere, docs included" is a standing house rule, and the only
// thing enforcing it was `cargo test --test emdash_lint`, which scans `src/gui/`
// and nothing else. So the rule held in Rust string literals and drifted
// everywhere a reader would actually see it: on 2026-09-15 eight shipped Library
// documents carried 31 between them, including the Humanity Accord.
//
// This checks what users read: the generated `data/library/` tree, which is what
// both clients load. Fix the SOURCE under docs/ and re-run
// `node scripts/build-library.js`; editing the generated copy is pointless
// because the next build overwrites it.
//
// THE EXEMPTION, AND WHY IT IS NARROW
// A document marked `verbatim` in data/library/catalog.json is a transcription
// of somebody else's text, and its punctuation is not ours to tidy. The US
// Constitution is the case that forced this: its dashes are the 1787 parchment's
// own ("Oath or Affirmation:-", and the run of clauses in Article III), and
// normalising them would falsify the very thing the NARA rebuild was for. The
// flag is per-document and lives in the catalogue, so the reason travels with
// the entry instead of sitting in an allowlist here.
//
//   node scripts/check-library-dashes.js
//   node scripts/check-library-dashes.js --quiet

const fs = require('fs');
const path = require('path');

const LIB = 'data/library';
const MANIFEST = path.join(LIB, 'index.json');
const QUIET = process.argv.includes('--quiet');

if (!fs.existsSync(MANIFEST)) {
  console.log('No ' + MANIFEST + '; run scripts/build-library.js first.');
  process.exit(0);
}

const idx = JSON.parse(fs.readFileSync(MANIFEST, 'utf8'));
let checked = 0, exempt = 0;
const offenders = [];

for (const cat of idx.categories || []) {
  for (const doc of cat.docs || []) {
    if (doc.verbatim) { exempt++; continue; }
    const file = path.join(LIB, doc.file);
    if (!fs.existsSync(file)) continue;
    checked++;
    const text = fs.readFileSync(file, 'utf8');
    const lines = text.split(/\r?\n/);
    const hits = [];
    lines.forEach((line, i) => {
      if (/[–—]/.test(line)) {
        hits.push({ line: i + 1, text: line.trim().slice(0, 90) });
      }
    });
    if (hits.length) offenders.push({ title: doc.title, file: doc.file, hits });
  }
}

if (!QUIET && offenders.length) {
  console.log('Em or en dashes in shipped Library documents:');
  for (const o of offenders) {
    console.log('  ' + o.title + '  (' + o.file + ', ' + o.hits.length + ')');
    for (const h of o.hits.slice(0, 3)) console.log('      line ' + h.line + ': ' + h.text);
    if (o.hits.length > 3) console.log('      ... and ' + (o.hits.length - 3) + ' more');
  }
  console.log('');
  console.log('Fix the SOURCE file under docs/, not the copy in data/library/,');
  console.log('then re-run scripts/build-library.js. A comma usually carries a');
  console.log('parenthetical break; a range like 1-3 wants a plain hyphen.');
  console.log('If the document is a verbatim transcription of somebody else\'s');
  console.log('text, mark it `"verbatim": true` in data/library/catalog.json');
  console.log('instead, and say in the catalogue why.');
  console.log('');
}

const total = offenders.reduce((n, o) => n + o.hits.length, 0);
console.log(
  'Checked ' + checked + ' shipped Library documents (' + exempt +
  ' exempt as verbatim). Documents with em or en dashes: ' + offenders.length +
  (total ? ', ' + total + ' occurrence(s)' : '')
);
process.exit(offenders.length ? 1 : 0);
