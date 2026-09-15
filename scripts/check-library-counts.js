#!/usr/bin/env node
// Refuses a shipped Library document that states a count of locale records
// which the locale data no longer supports.
//
// WHY THIS EXISTS
// A guide that teaches from a dataset naturally quotes its shape: "122 plants,
// fungi, mammals, birds, fish and shellfish", "99 native, 9 introduced and 14
// invasive", "the split is 11 yes, 62 conditional, 49 no". Every one of those
// was true the day it was written. Then three sea star records were added and
// all of them were false, in two documents, without anything breaking.
//
// It was caught by a writer who happened to re-derive the numbers while
// working on a third guide. That is not a process. The dataset is supposed to
// grow, so this will happen every time it does.
//
// The honest alternative to this check would be for the guides to stop quoting
// counts, but the counts are good teaching: "eleven of 125 are a plain yes"
// lands where "most are conditional" does not. So the numbers stay and this
// keeps them true.
//
// WHAT IT CHECKS. For every locale, it computes the real totals and then looks
// through the shipped documents for the specific sentence shapes that assert
// them. A number in one of those shapes that matches no locale is a failure.
// Prose that does not use one of these shapes is not inspected, so this is a
// floor and not a guarantee; add a shape when a guide invents a new one.
//
// PROVEN RED: run it against the documents as they stood before the sea stars
// landed and it reports five stale counts across two files.
//
//   node scripts/check-library-counts.js

const fs = require('fs');
const path = require('path');

const LOCALES = 'data/locales';
const DOCS = 'data/library';

if (!fs.existsSync(LOCALES) || !fs.existsSync(DOCS)) {
  console.log('No locales or no built Library; nothing to check.');
  process.exit(0);
}

// Every total any document is allowed to assert, per locale.
const totals = { total: new Set(), native: new Set(), introduced: new Set(), invasive: new Set(),
                 yes: new Set(), conditional: new Set(), no: new Set(), noToxicity: new Set() };
let locales = 0;
for (const id of fs.readdirSync(LOCALES)) {
  const f = path.join(LOCALES, id, 'species.json');
  if (!fs.existsSync(f)) continue;
  locales++;
  const sp = JSON.parse(fs.readFileSync(f, 'utf8')).species || [];
  const count = (k, v) => sp.filter(s => s[k] === v).length;
  totals.total.add(sp.length);
  totals.native.add(count('status', 'native'));
  totals.introduced.add(count('status', 'introduced'));
  totals.invasive.add(count('status', 'invasive'));
  totals.yes.add(count('edible', 'yes'));
  totals.conditional.add(count('edible', 'conditional'));
  totals.no.add(count('edible', 'no'));
  totals.noToxicity.add(sp.filter(s => !s.toxic).length);
}

const WORDS = { fifty: 50, sixty: 60, seventy: 70, eighty: 80, ninety: 90, forty: 40, thirty: 30 };
const TENS = { one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7, eight: 8, nine: 9 };
function spelled(word) {
  const w = word.toLowerCase();
  if (WORDS[w] !== undefined) return WORDS[w];
  const m = w.match(/^(fifty|sixty|seventy|eighty|ninety|forty|thirty)-(one|two|three|four|five|six|seven|eight|nine)$/);
  return m ? WORDS[m[1]] + TENS[m[2]] : null;
}

// Each shape names which total it asserts and how to pull the number(s) out.
const SHAPES = [
  { key: 'total', re: /(\d{2,4}) plants, fungi, mammals, birds, fish/g },
  { key: 'total', re: /(\d{2,4}) records carries a lookalike/g },
  { key: 'total', re: /out of (\d{2,4}) records/g },
  { key: 'total', re: /of the (\d{2,4}) (?:Silverdale|locale) records/g },
  { key: 'total', re: /(\d{2,4}) species records/g },
  { key: 'native', re: /(\d{2,4}) native, \d+ introduced/g },
  { key: 'introduced', re: /\d+ native, (\d{2,4}) introduced/g },
  { key: 'invasive', re: /introduced and (\d{2,4}) invasive/g },
  { key: 'yes', re: /split is (\d{2,4}) yes/g },
  { key: 'conditional', re: /yes, (\d{2,4}) conditional/g },
  { key: 'no', re: /conditional, (\d{2,4}) no\b/g },
  { key: 'noToxicity', re: /([A-Za-z-]+) of the \d{2,4} (?:Silverdale|locale) records/g, word: true },
];

const problems = [];
let docs = 0, asserts = 0;

for (const f of fs.readdirSync(DOCS).filter(f => f.endsWith('.md'))) {
  // Every document hard-wraps at 72 columns, so any of these sentences can be
  // split across a line break. Collapse whitespace before matching, or the
  // check silently passes exactly the claims that happen to wrap, which is
  // most of them.
  const text = fs.readFileSync(path.join(DOCS, f), 'utf8').replace(/\s+/g, ' ');
  docs++;
  for (const shape of SHAPES) {
    shape.re.lastIndex = 0;
    let m;
    while ((m = shape.re.exec(text)) !== null) {
      const n = shape.word ? spelled(m[1]) : Number(m[1]);
      if (n === null) continue;           // a word this script cannot read is not a claim it can judge
      asserts++;
      if (totals[shape.key].has(n)) continue;
      problems.push(
        f + ': claims ' + n + ' for ' + shape.key + ', but no locale has that. ' +
        'Live value' + (totals[shape.key].size > 1 ? 's' : '') + ': ' +
        [...totals[shape.key]].join(', ') + '. Context: ' + JSON.stringify(m[0])
      );
    }
  }
}

if (problems.length) {
  console.log('');
  console.log('Stale dataset counts in shipped documents:');
  for (const p of problems) console.log('  ' + p);
  console.log('');
  console.log('  The data grew and the prose did not. Update the sentence, or');
  console.log('  stop quoting a count there.');
  console.log('');
}
console.log('Checked ' + asserts + ' dataset-count claims across ' + docs +
  ' shipped documents and ' + locales + ' locale(s). Stale: ' + problems.length);
process.exit(problems.length ? 1 : 0);
