// The website's WCAG 2.1 AA static checks (the accessibility audit,
// docs/design/wcag-audit-2026-10-23.md): every control has an accessible name, a clickable element
// that is not a control can be reached by keyboard, no tabindex above 0, every page has a language
// and a title, no page stops pinch zoom, every picture has an alt. scripts/a11y-web.js holds the
// scanner; `node scripts/a11y-web.js` prints the counts and `--list <file>` the lines.
//
// Run: node --test scripts/tests/a11y-web.test.js
//
// RATCHET: scripts/tests/fixtures/a11y-web-budget.txt says how many problems of each kind each page
// may still have. More fails (fix the new control). FEWER also fails, asking for the click that
// locks the win in: I18N_RATCHET_WRITE=1 node --test scripts/tests/a11y-web.test.js rewrites the
// budgets to today's counts (only ever lower; a rise still fails).
// The pages the accessibility pass converted must stay at zero, whatever their budget says.

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const A = require(path.join(ROOT, 'scripts', 'a11y-web.js'));
const W = require(path.join(ROOT, 'scripts', 'i18n-web.js'));
const BUDGET = path.join(__dirname, 'fixtures', 'a11y-web-budget.txt');

const CONVERTED = ['calculator', 'notes', 'tasks', 'calendar', 'crafting', 'laws', 'wallet', 'donate', 'bugs', 'guilds', 'governance', 'humanity', 'civilization', 'market'];

function readBudget() {
  const out = {};
  if (!fs.existsSync(BUDGET)) return out;
  for (const line of fs.readFileSync(BUDGET, 'utf8').split(/\r?\n/)) {
    const m = /^(\d+)\s+(.+)$/.exec(line.trim());
    if (m) out[m[2]] = parseInt(m[1], 10);
  }
  return out;
}

function writeBudget(counts) {
  const total = Object.values(counts).reduce((a, b) => a + b, 0);
  const lines = [
    '# The website ACCESSIBILITY ratchet\'s budgets: per page and kind of problem, how many the',
    '# static checks (scripts/a11y-web.js) still find. Checked by scripts/tests/a11y-web.test.js;',
    '# the numbers only go DOWN, and a page not listed has a budget of zero.',
    '# Problems: ' + total,
  ];
  for (const k of Object.keys(counts).sort()) lines.push(counts[k] + ' ' + k);
  fs.writeFileSync(BUDGET, lines.join('\n') + '\n');
}

test('RATCHET: no web page has more accessibility problems than its budget, nor fewer', () => {
  const counts = A.counts(W.WEB);
  const budget = readBudget();
  const over = [];
  const under = [];
  for (const k of new Set([...Object.keys(counts), ...Object.keys(budget)])) {
    const n = counts[k] || 0;
    const b = budget[k] || 0;
    if (n > b) over.push('  ' + k + ': ' + n + ', budget ' + b + ' (+' + (n - b) + ')');
    else if (n < b) under.push('  ' + k + ': ' + n + ', budget ' + b + ': lower it to ' + n);
  }
  // =init writes today's counts whatever they are (the first run only); =1 only ever lowers.
  if (process.env.I18N_RATCHET_WRITE === 'init' || (process.env.I18N_RATCHET_WRITE && over.length === 0)) {
    writeBudget(counts);
    return;
  }
  assert.deepStrictEqual(over, [], '\nACCESSIBILITY PROBLEMS past the page\'s budget (node scripts/a11y-web.js --list <file> shows the lines):\n' + over.join('\n') + '\nDo not raise the budget: name the control, give the clickable element role="button" tabindex="0", and so on.');
  assert.deepStrictEqual(under, [], '\nRATCHET CLICK (problems fixed; lock the win in with I18N_RATCHET_WRITE=1):\n' + under.join('\n'));
});

test('the pages the accessibility pass converted have no static accessibility problem', () => {
  const counts = A.counts(W.WEB);
  const bad = Object.entries(counts).filter(([k]) => CONVERTED.some((p) => k.startsWith('page ' + p + ' ')));
  assert.deepStrictEqual(bad, []);
});

test('scanner: each check fails on what it is for, and passes the fixed form', () => {
  const rules = (html) => A.markupProblems(html).map((p) => p.rule);
  assert.deepStrictEqual(rules('<button></button>'), ['name'], 'a button with no words');
  assert.deepStrictEqual(rules('<button>Save</button>'), []);
  assert.deepStrictEqual(rules('<button aria-label="Close">x</button>'), []);
  assert.deepStrictEqual(rules('<select id="s"></select>'), ['name'], 'a list with no label');
  assert.deepStrictEqual(rules('<label for="s">Size</label><select id="s"></select>'), []);
  assert.deepStrictEqual(rules('<input id="q" placeholder="Search">'), [], 'a placeholder is the last-resort name');
  assert.deepStrictEqual(rules('<div onclick="go()">Open</div>'), ['keyboard'], 'a clickable div the keyboard cannot reach');
  assert.deepStrictEqual(rules('<div role="button" tabindex="0" onclick="go()">Open</div>'), []);
  assert.deepStrictEqual(rules('<div role="dialog" onclick="close()"><p>x</p></div>'), [], 'a dialog backdrop');
  assert.deepStrictEqual(rules('<a href="/x" tabindex="2">x</a>'), ['focus-order']);
  assert.deepStrictEqual(rules('<img src="a.png">'), ['img-alt']);
  assert.deepStrictEqual(rules('<img src="a.png" alt="">'), []);
  assert.deepStrictEqual(rules('<meta name="viewport" content="width=device-width, user-scalable=no">'), ['zoom']);
});
