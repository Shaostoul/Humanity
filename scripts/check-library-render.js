#!/usr/bin/env node
// Runs the REAL web Library renderer over the REAL shipped documents and looks
// at what comes out.
//
// WHY THIS EXISTS
// Every other Library gate reads the source: are the links resolvable, are the
// dashes ASCII, is the shipped copy current. None of them had ever looked at
// the rendered result, and the repo's dominant defect class is exactly a check
// whose evidence is the setup code rather than what a reader sees. Two real
// defects were sitting in shipped documents when this script was first run:
//
//   1. An underscore inside a word opened emphasis. `what_soil_is.md`,
//      `silverdale_wa`, `poison_hemlock`, `game_join`: the reader lost the
//      underscores and got italics from the second one to the next. 82 spans
//      across 23 shipped documents. The NATIVE reader showed every one of them
//      correctly, so this was a web-only divergence that could only be found by
//      opening both clients side by side, which nobody does.
//
//   2. One table never rendered at all. It was indented inside a checklist
//      item, where the renderer cannot see it, so the page showed its separator
//      row as literal pipes and dashes. The fix was to the document, not the
//      renderer: teaching the renderer to read indented tables would have put
//      every indented code block in 107 documents at risk for one occurrence.
//
// PROVING IT RED. Revert either guard in web/shared/markdown.js and this script
// reports the exact count above. That was done before trusting it.
//
// The native reader is checked separately by the unit tests in
// src/gui/widgets/markdown.rs. This script is the web half.
//
//   node scripts/check-library-render.js
//   node scripts/check-library-render.js --verbose

const fs = require('fs');
const path = require('path');

const DIR = 'data/library';
const VERBOSE = process.argv.includes('--verbose');

// markdown.js is a browser script that hangs itself off window. escapeHtml()
// round-trips through a real element's textContent/innerHTML, so the stub has
// to actually escape rather than merely exist; without that every call returns
// undefined and the first render throws, which looks exactly like a renderer
// bug and is not one.
function makeEl() {
  let text = '';
  return {
    set textContent(v) { text = String(v); },
    get textContent() { return text; },
    get innerHTML() {
      return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    },
    setAttribute() {}, appendChild() {}, style: {},
  };
}

const win = {};
new Function('window', 'document', fs.readFileSync('web/shared/markdown.js', 'utf8'))(win, {
  getElementById: () => null,
  createElement: makeEl,
  head: { appendChild() {} },
});

const md = win.hosMarkdown;
if (!md || typeof md.render !== 'function') {
  console.log('web/shared/markdown.js did not expose window.hosMarkdown.render');
  process.exit(1);
}

const problems = [];

// ---------------------------------------------------------------- constructs

function construct(name, source, mustContain, mustNotContain) {
  const html = md.render(source);
  const missing = mustContain.filter(s => !html.includes(s));
  const leaked = (mustNotContain || []).filter(s => html.includes(s));
  if (missing.length || leaked.length) {
    problems.push(name +
      (missing.length ? '; missing ' + JSON.stringify(missing) : '') +
      (leaked.length ? '; leaked ' + JSON.stringify(leaked) : '') +
      '; got: ' + html.replace(/\s+/g, ' ').slice(0, 160));
  }
}

construct('a pipe table becomes a real table',
  '| Species | Eats |\n|---|---|\n| Otter | Crab |\n',
  ['<table', '<th', '<td', 'Otter'], ['|---|']);

// The docs tree hard-wraps at 72 columns, so emphasis crosses line breaks
// constantly: 532 such spans across 34 shipped documents. A line-by-line
// renderer shows every one of them as literal asterisks.
construct('bold that opens on one line and closes on the next',
  'Use it carefully. **Rule of thumb, not a sourced\nclaim:** ask what it does.\n',
  ['<strong>', 'Rule of thumb, not a sourced claim:'], ['**']);

construct('a wrapped list item stays one item',
  '- **Wash after gardening.** Soil and grease\n  shield organisms from alcohol.\n',
  ['<li>', 'shield organisms from alcohol'], ['**']);

// Defect 1. Every cross-reference between guides has an underscored filename.
construct('an underscored link target survives',
  'See [What Soil Is](what_soil_is.md) first.\n',
  ['what_soil_is.md', 'What Soil Is'], ['<em>', '](']);

// The Library teaches from data whose ids are snake_case. Mangling them
// silently teaches the reader the wrong id.
construct('a snake_case identifier in prose is left alone',
  'The record id is poison_hemlock and the locale is silverdale_wa.\n',
  ['poison_hemlock', 'silverdale_wa'], ['<em>']);

construct('underscores still emphasise at a word boundary',
  'This word is _emphasised_ here.\n', ['<em>emphasised</em>']);

construct('a heading renders', '## Where energy enters\n',
  ['<h2', 'Where energy enters']);

// The /library#doc/heading deep-link grammar depends on this exact shape, and
// the native reader implements the same one.
const slugs = md.headings('# A\n## B and C\n## B and C\n## B and C\n').map(h => h.slug).join(',');
if (slugs !== 'a,b-and-c,b-and-c-1,b-and-c-2') {
  problems.push('heading slug grammar changed: got ' + slugs +
    ', expected a,b-and-c,b-and-c-1,b-and-c-2. The native reader in ' +
    'src/gui/widgets/markdown.rs must match, and /library#doc/heading links break.');
}

// ---------------------------------------------------- the shipped documents

let docs = 0, tables = 0, withTables = 0;
if (!fs.existsSync(DIR)) {
  console.log('No ' + DIR + '; run scripts/build-library.js first.');
  process.exit(0);
}

for (const f of fs.readdirSync(DIR).filter(f => f.endsWith('.md'))) {
  const text = fs.readFileSync(path.join(DIR, f), 'utf8');
  const html = md.render(text);
  docs++;

  // A table that did not render leaves its separator row on the page as
  // literal pipes and dashes.
  const leaked = (html.match(/\|\s*-{3,}/g) || []).length;
  if (leaked) {
    problems.push(f + ': a table did not render; ' + leaked +
      ' separator fragments are visible to the reader as raw text. The usual ' +
      'cause is a table indented inside a list item. Lift it out of the list.');
  }
  if (/^\|/m.test(text)) {
    withTables++;
    tables += (html.match(/<table/g) || []).length;
  }

  // An identifier the reader is meant to be able to type back must survive.
  // Checked against the SOURCE so the expectation is what the writer wrote.
  const lines = text.split(/\r?\n/);
  let fence = false;
  const mangled = [];
  for (const line of lines) {
    if (/^\s*(```|~~~)/.test(line)) { fence = !fence; continue; }
    if (fence || /^    /.test(line)) continue;
    const bare = line.replace(/`[^`]*`/g, '');
    for (const id of bare.match(/[A-Za-z0-9]+_[A-Za-z0-9_]+/g) || []) {
      if (!md.render(line).includes(id)) mangled.push(id);
    }
  }
  if (mangled.length) {
    problems.push(f + ': ' + mangled.length + ' snake_case identifier(s) do not ' +
      'survive rendering, e.g. ' + mangled.slice(0, 3).join(', ') +
      '. An underscore inside a word must not open emphasis.');
  }
}

if (problems.length) {
  console.log('');
  console.log('Problems:');
  for (const p of problems) console.log('  ' + p);
  console.log('');
} else if (VERBOSE) {
  console.log('  ' + withTables + ' documents contain a pipe table, ' + tables + ' rendered');
}

console.log('Rendered ' + docs + ' Library documents through the web reader. Problems: ' +
  problems.length);
process.exit(problems.length ? 1 : 0);
