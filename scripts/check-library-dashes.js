#!/usr/bin/env node
// Holds the em and en dashes in the documents the Library ships to the house budget.
//
// THE RULE (operator, 2026-06-13, revised 2026-09-18; CLAUDE.md, Writing style)
// Em dashes are used sparingly, because people take a page full of them for AI
// writing and leave. "We just don't want to use a bunch." A handful across a long
// document is fine; two in one paragraph is the tell. The rule began as a flat ban
// and became a budget for one reason: a ban corrupts quoted text (the US
// Constitution is full of dashes, and changing punctuation inside a quotation is
// falsifying it). Records stay as written too.
//
// WHAT THIS CHECKS, AND WHAT IT LEAVES ALONE
// It reads what users read, the generated `data/library/` tree both clients load.
// In each document it ignores:
//   - quoted text: a "..." or a curly-quoted span, and a `>` blockquote line;
//   - code: a backtick span and a fenced block;
//   - a whole document marked `verbatim` (a transcription of somebody else's text,
//     such as the Constitution) or `record` (a session history or decision log,
//     kept as written; the catalogue marks the Project History shelf so);
// and fails on what is left when
//   - one paragraph (or one list item) holds two or more dashes, or
//   - the document holds more than HANDFUL of them.
// Until 2026-10-30 it refused every dash, the rule as it stood before 2026-09-18,
// so `just preflight` failed on history records and on statute quotations the
// rules say to keep exactly.
//
// Fix the SOURCE under docs/ and re-run `node scripts/build-library.js`; editing
// the generated copy is pointless because the next build overwrites it.
//
//   node scripts/check-library-dashes.js
//   node scripts/check-library-dashes.js --quiet

const fs = require('fs');
const path = require('path');

/** A handful: the most dashes one document may carry outside quotations and code. */
const HANDFUL = 5;
const DASH = /[–—]/g;

/**
 * The dashes a document spends against the budget (pure, tested).
 * Returns { total, paragraphs: [{ line, count, text }] } where `paragraphs` lists every
 * paragraph or list item holding two or more, and `total` counts all of them.
 */
function spend(text) {
  const lines = text.split(/\r?\n/);
  // Blocks: paragraphs split at blank lines and at list items, fenced code dropped, blockquote
  // lines dropped. Each block keeps the line it starts on.
  const blocks = [];
  let cur = null;
  let fenced = false;
  const close = () => {
    if (cur) blocks.push(cur);
    cur = null;
  };
  lines.forEach((raw, i) => {
    const line = raw.trimEnd();
    if (/^\s*(```|~~~)/.test(line)) {
      fenced = !fenced;
      close();
      return;
    }
    if (fenced) return;
    if (!line.trim() || /^\s*>/.test(line)) {
      close();
      return;
    }
    if (/^\s*#/.test(line)) {
      // A heading is a block of its own.
      close();
      blocks.push({ line: i + 1, text: line.trim() });
      return;
    }
    if (/^\s*([-*+]|\d+[.)])\s+/.test(line)) close();
    if (!cur) cur = { line: i + 1, text: '' };
    cur.text += (cur.text ? ' ' : '') + line.trim();
  });
  close();
  let total = 0;
  const paragraphs = [];
  for (const b of blocks) {
    const bare = b.text
      .replace(/`[^`]*`/g, '')
      .replace(/"[^"]*"/g, '')
      .replace(/“[^”]*”/g, '');
    const count = (bare.match(DASH) || []).length;
    total += count;
    if (count >= 2) paragraphs.push({ line: b.line, count, text: b.text.slice(0, 90) });
  }
  return { total, paragraphs };
}

/** Over the budget? A paragraph with two or more, or more than HANDFUL in the document. */
const overspent = (s) => s.paragraphs.length > 0 || s.total > HANDFUL;

function main() {
  const LIB = 'data/library';
  const MANIFEST = path.join(LIB, 'index.json');
  const QUIET = process.argv.includes('--quiet');
  if (!fs.existsSync(MANIFEST)) {
    console.log('No ' + MANIFEST + '; run scripts/build-library.js first.');
    process.exit(0);
  }
  const idx = JSON.parse(fs.readFileSync(MANIFEST, 'utf8'));
  let checked = 0, verbatim = 0, records = 0;
  const offenders = [];
  for (const cat of idx.categories || []) {
    for (const doc of cat.docs || []) {
      if (doc.verbatim) { verbatim++; continue; }
      if (doc.record) { records++; continue; }
      const file = path.join(LIB, doc.file);
      if (!fs.existsSync(file)) continue;
      checked++;
      const s = spend(fs.readFileSync(file, 'utf8'));
      if (overspent(s)) offenders.push({ title: doc.title, file: doc.file, s });
    }
  }
  if (!QUIET && offenders.length) {
    console.log('Library documents over the dash budget (a handful per document, never two in one paragraph):');
    for (const o of offenders) {
      console.log('  ' + o.title + '  (' + o.file + ': ' + o.s.total + ' outside quotations and code)');
      for (const p of o.s.paragraphs.slice(0, 3)) console.log('      line ' + p.line + ', ' + p.count + ' in one paragraph: ' + p.text);
      if (o.s.paragraphs.length > 3) console.log('      ... and ' + (o.s.paragraphs.length - 3) + ' more paragraphs');
    }
    console.log('');
    console.log('Fix the SOURCE file under docs/, not the copy in data/library/, then re-run');
    console.log('scripts/build-library.js. A comma, a colon, a full stop or parentheses usually');
    console.log('carry the break; a range like 1-3 wants a plain hyphen. Never change a quotation:');
    console.log('quoted text is already left out of the count. A transcription of somebody else\'s');
    console.log('text is marked `"verbatim": true` in data/library/catalog.json, with the reason.');
    console.log('');
  }
  console.log(
    'Checked ' + checked + ' shipped Library documents (' + verbatim + ' verbatim and ' + records +
      ' records left as written). Over the dash budget: ' + offenders.length
  );
  process.exit(offenders.length ? 1 : 0);
}

if (require.main === module) main();

module.exports = { spend, overspent, HANDFUL };
