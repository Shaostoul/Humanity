// The Library dash budget (scripts/check-library-dashes.js): a handful per document, never two in
// one paragraph, quotations and code left exactly as written. Run: node --test scripts/tests/
const test = require('node:test');
const assert = require('node:assert');
const { spend, overspent, HANDFUL } = require('../check-library-dashes.js');

const D = '—'; // em dash
const N = '–'; // en dash

test('two dashes in one paragraph are the tell', () => {
  const s = spend(`One idea ${D} and another ${D} in the same paragraph.\n`);
  assert.strictEqual(s.total, 2);
  assert.strictEqual(s.paragraphs.length, 1);
  assert.strictEqual(s.paragraphs[0].line, 1);
  assert.ok(overspent(s));
});

test('a handful spread over paragraphs is fine; one more is not', () => {
  const para = (k) => `Paragraph ${k} has one ${D} dash.\n\n`;
  const ok = spend(Array.from({ length: HANDFUL }, (_, k) => para(k)).join(''));
  assert.strictEqual(ok.total, HANDFUL);
  assert.ok(!overspent(ok));
  const over = spend(Array.from({ length: HANDFUL + 1 }, (_, k) => para(k)).join(''));
  assert.ok(overspent(over));
});

test('list items are paragraphs of their own', () => {
  const s = spend(`- first ${D} item\n- second ${D} item\n`);
  assert.strictEqual(s.total, 2);
  assert.strictEqual(s.paragraphs.length, 0, 'one each, not two in one');
});

test('a quotation keeps its dashes, even across lines', () => {
  const s = spend(
    `- The licence (2026): "This applies to the terms only ${D} it does not\n` +
      `  affect the rest ${D} of the code." and our note.\n` +
      `- “Publicly” means: “at a place ${D} open” ${N} our note\n`
  );
  assert.strictEqual(s.total, 1, 'only the dash outside every quotation counts');
  assert.ok(!overspent(s));
});

test('blockquotes and code are left alone', () => {
  const s = spend(
    `> A quoted passage ${D} with ${D} dashes.\n\n` +
      'Some `code ' + D + ' with ' + D + ' dashes` here.\n\n' +
      '```\nlet x = "a ' + D + ' b ' + D + ' c";\n```\n'
  );
  assert.strictEqual(s.total, 0);
});

test('headings count, as blocks of their own', () => {
  const s = spend(`# Title ${D} subtitle\nBody ${D} text.\n`);
  assert.strictEqual(s.total, 2);
  assert.strictEqual(s.paragraphs.length, 0);
});

test('hyphens were never the issue', () => {
  const s = spend('A well-known range, 1-3, and a compound-word phrase.\n');
  assert.strictEqual(s.total, 0);
});
