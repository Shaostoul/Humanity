#!/usr/bin/env node
// Refuses a shipped Library copy that no longer matches its source.
//
// WHY THIS EXISTS
// data/library/ is a GENERATED copy of documents that live under docs/. Editing
// a source without re-running scripts/build-library.js ships the old text, and
// nothing looks wrong: the document is present, the links resolve, the search
// index is populated. It just says something other than what the author wrote.
//
// CLAUDE.md records that this drifted silently for months before anyone caught
// it, and says in as many words that there is no CI check for it. There is now.
// It caught a real one the day it was written: a writer was still editing
// firewood.md when the source was staged, so the shipped copy was 42 lines short
// of the final text, missing a whole section on fuel cost comparison.
//
// HOW IT COMPARES
// The build REWRITES cross-document links as it copies (a relative ../admin/X.md
// becomes /library#x), so a byte comparison would report every document with a
// link as stale. Both sides are therefore normalised by replacing every link
// TARGET with a placeholder and keeping the link text, which leaves every real
// content difference visible and every rewritten link invisible.
//
// Pairs source to shipped copy through data/library/index.json rather than by
// filename, because the build renames collisions: README.md exists in four
// source folders and only one of them keeps that name. Comparing by basename
// reports two false positives, which is how the first version of this check
// behaved before it was corrected.
//
//   node scripts/check-library-fresh.js
//   node scripts/check-library-fresh.js --quiet

const fs = require('fs');
const path = require('path');

const LIB = 'data/library';
const CATALOG = path.join(LIB, 'catalog.json');
const MANIFEST = path.join(LIB, 'index.json');
const QUIET = process.argv.includes('--quiet');

if (!fs.existsSync(CATALOG) || !fs.existsSync(MANIFEST)) {
  console.log('No catalogue or manifest; run scripts/build-library.js first.');
  process.exit(0);
}

/** Replace every markdown link target with a placeholder, keeping the text, so
    the build's link rewriting cannot register as a content change. */
function normalise(text) {
  return text
    .replace(/\]\([^)]*\)/g, '](LINK)')
    .replace(/\r\n/g, '\n')
    .trimEnd();
}

const catalog = JSON.parse(fs.readFileSync(CATALOG, 'utf8'));
const manifest = JSON.parse(fs.readFileSync(MANIFEST, 'utf8'));

// The build skips a catalogue entry whose source is missing, so the manifest
// lists exactly the entries that exist, in the same order.
const sources = [];
for (const c of catalog.categories || []) {
  for (const d of c.docs || []) if (fs.existsSync(d.src)) sources.push(d);
}
const shipped = [];
for (const c of manifest.categories || []) {
  for (const d of c.docs || []) shipped.push(d);
}

if (sources.length !== shipped.length) {
  console.error(
    'The catalogue lists ' + sources.length + ' existing sources and the manifest ' +
    shipped.length + ' documents. Run scripts/build-library.js.'
  );
  process.exit(1);
}

const stale = [];
for (let i = 0; i < sources.length; i++) {
  const src = sources[i].src;
  const out = path.join(LIB, shipped[i].file);
  if (!fs.existsSync(out)) { stale.push({ src, out, why: 'shipped copy missing' }); continue; }
  const a = normalise(fs.readFileSync(src, 'utf8'));
  const b = normalise(fs.readFileSync(out, 'utf8'));
  if (a === b) continue;
  const la = a.split('\n').length, lb = b.split('\n').length;
  stale.push({
    src, out,
    why: la === lb ? 'same length, different text' : 'source ' + la + ' lines, shipped ' + lb,
  });
}

if (!QUIET && stale.length) {
  console.log('Shipped Library copies that no longer match their source:');
  for (const s of stale) console.log('  ' + s.out + '  <-  ' + s.src + '  (' + s.why + ')');
  console.log('');
  console.log('Run `node scripts/build-library.js` to resync, then commit the');
  console.log('regenerated data/library/ alongside the source you edited. The');
  console.log('shipped copy is what both clients load, so an unsynced edit is an');
  console.log('edit nobody reads.');
  console.log('');
}
console.log(
  'Compared ' + sources.length + ' shipped Library documents against their sources. Stale: ' +
  stale.length
);
process.exit(stale.length ? 1 : 0);
