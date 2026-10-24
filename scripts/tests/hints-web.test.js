// A plain-language hint on every control of the website (the ELI5 mandate, docs/PRIORITIES.md
// TIER 3, 2026-10-22): what each button, box and link does, in words a person new to computers
// understands, shown by web/shared/shell.js on hover and on keyboard focus through the person's
// hint mode (Settings > Appearance > Page Hints). The native twin is native_hint_coverage_only_grows
// in tests/i18n_lint.rs.
//
// Run: node --test scripts/tests/hints-web.test.js
//
// What it proves:
//  1. THE RATCHET: per page (an HTML page with the scripts named after it, the chat, the front
//     door, or a shared file), the number of controls with no hint is at most its budget in
//     scripts/tests/fixtures/hint-web-budget.txt, and no fewer (lock a win in with
//     I18N_RATCHET_WRITE=1; =init rewrites every budget, an operator decision only).
//     HINT_REPORT=1 prints hinted / total per page.
//  2. shell.js shows the hint by the hint mode: Full and Hover show data-tip-desc, Off shows none
//     (only an icon-only control's name); a control with no hint shows no made-up sentence.
//  3. The scanner itself finds controls and their hints (each check able to fail).
//
// HOS_WEB_DIR points the checks at another copy of web/ (used to see each one red).

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const W = require(path.join(ROOT, 'scripts', 'i18n-web.js'));
const BUDGET = path.join(__dirname, 'fixtures', 'hint-web-budget.txt');

function readBudget() {
  const m = {};
  if (!fs.existsSync(BUDGET)) return m;
  for (const line of fs.readFileSync(BUDGET, 'utf8').split(/\r?\n/)) {
    const l = line.trim();
    if (!l || l.startsWith('#')) continue;
    const sp = l.indexOf(' ');
    m[l.slice(sp + 1).trim()] = parseInt(l.slice(0, sp), 10);
  }
  return m;
}

function writeBudget(m, counts) {
  const total = Object.values(counts).reduce((s, c) => s + c.total, 0);
  const missing = Object.values(m).reduce((s, n) => s + n, 0);
  const head =
    '# The website HINT ratchet\'s budgets: per page, how many controls still have no plain-language\n' +
    '# hint (data-tip-desc / data-i18n-tip). Checked by scripts/tests/hints-web.test.js; the numbers\n' +
    '# only go DOWN, and a page not listed has a budget of zero.\n' +
    '# Controls without a hint: ' + missing + ' of ' + total + '\n';
  const rows = Object.entries(m).filter(([, n]) => n > 0).sort((a, b) => a[0].localeCompare(b[0])).map(([p, n]) => n + ' ' + p);
  fs.writeFileSync(BUDGET, head + rows.join('\n') + '\n');
}

test('RATCHET: no web page has more controls without a hint than its budget, nor fewer', () => {
  const now = W.hintCounts();
  if (process.env.HINT_REPORT) {
    for (const [p, c] of Object.entries(now).sort()) console.log(String(c.total - c.missing).padStart(4) + ' of ' + String(c.total).padStart(4) + '  ' + p);
  }
  const missing = Object.fromEntries(Object.entries(now).map(([p, c]) => [p, c.missing]));
  const mode = process.env.I18N_RATCHET_WRITE || '';
  if (mode === 'init') { writeBudget(missing, now); return; }
  const budget = readBudget();
  const over = [];
  const under = [];
  for (const [p, n] of Object.entries(missing)) {
    const b = budget[p] || 0;
    if (n > b) over.push('  ' + p + ': ' + n + ' controls without a hint, budget ' + b + ' (+' + (n - b) + ')');
    else if (n < b) under.push([p, n, b]);
  }
  const gone = Object.keys(budget).filter((p) => !(p in missing));
  if (mode === '1') {
    const lowered = { ...budget };
    for (const [p, n] of under) lowered[p] = n;
    for (const p of gone) delete lowered[p];
    writeBudget(lowered, now);
    under.length = 0;
    gone.length = 0;
  }
  assert.strictEqual(over.length, 0,
    '\n\nCONTROLS WITHOUT A PLAIN-LANGUAGE HINT, past the page\'s budget:\n' + over.join('\n') + '\n\n' +
    'Give each one data-i18n-tip="page.thing_hint" with its English in data-tip-desc (just i18n\n' +
    'fills it from data/i18n/en.json), or i18n.hint(el, "page.thing_hint") for one made in script.\n' +
    'Do not raise the budget.\n');
  const msg = under.map(([p, n, b]) => '  ' + p + ': ' + n + ' without a hint, budget ' + b + ': lower it to ' + n)
    .concat(gone.map((p) => '  ' + p + ': no controls left; remove its line'));
  assert.strictEqual(msg.length, 0, '\n\nRATCHET CLICK (hints added; lock the win in):\n' + msg.join('\n') +
    '\nRun: I18N_RATCHET_WRITE=1 node --test scripts/tests/hints-web.test.js\n');
});

// ── shell.js's hover hint in a stand-in page ──────────────────────────────

/** Run shell.js's tooltip code against a stand-in document and report what the
 *  hint shows when the pointer rests on `el`, under hint mode `mode`. */
/** Does a stand-in element match one simple selector of the kinds shell.js uses? */
function matchesSimple(el, sel) {
  sel = sel.trim();
  let m = /^\[([\w-]+)(?:="([^"]*)")?\]$/.exec(sel);
  if (m) return el.hasAttribute(m[1]) && (m[2] === undefined || el.getAttribute(m[1]) === m[2]);
  m = /^(\w+):not\(\[type="hidden"\]\)$/.exec(sel);
  if (m) return el.tagName === m[1].toUpperCase() && el.getAttribute('type') !== 'hidden';
  if (/^\w+$/.test(sel)) return el.tagName === sel.toUpperCase();
  return false;
}

function tipFor(mode, attrs, text, tagName = 'SPAN', viaChild = false) {
  const shell = fs.readFileSync(path.join(W.WEB, 'shared', 'shell.js'), 'utf8');
  const start = shell.indexOf('function defaultTooltipDescription');
  const end = shell.indexOf('setTimeout(initRichTooltips, 0);');
  assert.ok(start > 0 && end > start, 'shell.js still holds the hint code between defaultTooltipDescription and its start');
  const code = shell.slice(start, end) + '\ninitRichTooltips();';
  const listeners = {};
  let tipEl = null;
  const el = {
    attrs: { ...attrs },
    getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; },
    setAttribute(k, v) { this.attrs[k] = String(v); },
    removeAttribute(k) { delete this.attrs[k]; },
    hasAttribute(k) { return k in this.attrs; },
    textContent: text,
    tagName,
    closest(sel) {
      if (sel === '.hub-nav' || sel === '[data-tip-above]') return null;
      return sel.split(',').some((one) => matchesSimple(this, one)) ? this : null;
    },
    getBoundingClientRect() { return { left: 10, top: 10, right: 50, bottom: 30, width: 40, height: 20 }; },
  };
  const document = {
    documentElement: { getAttribute: (k) => (k === 'data-hints' ? mode : null), dataset: { hints: mode } },
    createElement() {
      tipEl = { style: {}, id: '', innerHTML: '', offsetWidth: 200, offsetHeight: 40, setAttribute() {}, removeAttribute() {}, getAttribute() { return null; } };
      return tipEl;
    },
    body: { appendChild() {} },
    querySelectorAll() { return []; },
    addEventListener(type, fn) { listeners[type] = fn; },
  };
  const ctx = { window: { innerWidth: 1000, innerHeight: 800 }, document, MutationObserver: class { observe() {} }, console };
  ctx.window.__HOS_RICH_TOOLTIPS__ = false;
  vm.createContext(ctx);
  vm.runInContext(code, ctx);
  // viaChild: the pointer is on something INSIDE el (a paragraph in a dialog), not on el itself.
  const child = { closest: (sel) => (sel === '.hub-nav' || sel === '[data-tip-above]' || sel === '[data-no-tip]' ? null : el.closest(sel)) };
  listeners.mouseover({ target: viaChild ? child : el, clientX: 20, clientY: 20 });
  return { shown: tipEl.style.display === 'block', html: tipEl.innerHTML };
}

test('shell.js shows the plain-language hint in Full and Hover, and none in Off', () => {
  const attrs = { 'aria-label': 'Clear', 'data-tip-desc': 'Empties the display so you can start again.' };
  for (const mode of ['full', 'hover']) {
    const t = tipFor(mode, attrs, 'C');
    assert.ok(t.shown && t.html.includes('Empties the display'), mode + ': ' + t.html);
  }
  const off = tipFor('off', attrs, 'C');
  assert.ok(!off.html.includes('Empties the display'), 'off hides the hint: ' + off.html);
});

// Found 2026-10-23 by the Tasks conversion: a text box or a clickable card carrying only its hint
// (no title, no aria-label, no words of its own) showed nothing. Red before the fix.
test('shell.js shows the hint of a text box and of a clickable card that carry only their hint', () => {
  const box = tipFor('full', { 'data-tip-desc': 'Type a few words to find a task.', placeholder: 'Search tasks...' }, '', 'INPUT');
  assert.ok(box.shown && box.html.includes('Type a few words') && box.html.includes('Search tasks...'), box.html);
  const card = tipFor('full', { 'data-tip-desc': 'Opens this task.', role: 'button' }, '', 'DIV');
  assert.ok(card.shown && card.html.includes('Opens this task.'), card.html);
  const bare = tipFor('full', { 'data-tip-desc': 'Opens this task.' }, '', 'DIV');
  assert.ok(bare.shown, 'an element with only data-tip-desc shows it: ' + bare.html);
});

// Found 2026-10-23 on the Market: a dialog's backdrop hint ('press outside to close') showed while
// the pointer was on empty space INSIDE the dialog, because the dialog sits inside the backdrop.
test('shell.js shows the hint of a backdrop on the backdrop only, not over the dialog it holds', () => {
  const attrs = { id: 'create-modal', class: 'modal-overlay', onclick: 'x', 'data-tip-desc': 'Pressing outside the box closes it.' };
  assert.ok(tipFor('full', attrs, 'Form text', 'DIV').shown, 'on the backdrop itself the hint shows');
  assert.ok(!tipFor('full', attrs, 'Form text', 'DIV', true).shown, 'inside the dialog it does not');
  const card = { class: 'task-card', role: 'button', 'data-tip-desc': 'Opens this task.' };
  assert.ok(tipFor('full', card, 'A task', 'DIV', true).shown, 'a clickable card still shows its hint over its own words');
});

test('shell.js never makes up a sentence for a control with no hint', () => {
  const t = tipFor('full', { 'aria-label': 'Frobnicate' }, '');
  assert.ok(!/Tap or click to use this control/.test(t.html), t.html);
});

// ── The scanner's own checks ──────────────────────────────────────────────

test('scanner: controls in HTML, and which have hints', () => {
  const html = [
    '<button data-tip-desc="Saves it">Save</button>',
    '<button>Bare</button>',
    '<a href="/x">Link</a>',
    '<a name="anchor">not a control</a>',
    '<input type="hidden" name="h">',
    '<input type="text" data-i18n-tip="a.b">',
    '<div data-no-tip><button>Explains itself</button></div>',
    '<div role="button" tabindex="0">Fake button</div>',
    '<span onclick="go()">Click me</span>',
  ].join('\n');
  const cs = W.markupControls(html).map((c) => [c.line, c.name, c.hinted]);
  assert.deepStrictEqual(cs, [[1, 'button', true], [2, 'button', false], [3, 'a', false], [6, 'input', true], [7, 'button', true], [8, 'div', false], [9, 'span', false]]);
});

test('scanner: controls a script makes, and which have hints', () => {
  const js = [
    "const a = document.createElement('button');",
    "a.dataset.tipDesc = 'Saves it';",
    "const b = document.createElement('button');",
    "const c = document.createElement('input');",
    "i18n.hint(c, 'x.c_hint');",
    "const d = document.createElement('div');",
    "list.innerHTML = '<button class=\"x\">' + name + '</button><button data-i18n-tip=\"x.y\">Y</button>';",
  ].join('\n');
  const cs = W.jsControls(js).map((c) => [c.line, c.name, c.hinted]);
  assert.deepStrictEqual(cs, [[1, 'button', true], [3, 'button', false], [4, 'input', true], [7, 'button', false], [7, 'button', true]]);
});
