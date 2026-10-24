// The website is ready for translation (the ELI5 mandate, docs/PRIORITIES.md TIER 3;
// operator 2026-10-22: "let's only focus on English for now but, make sure we prep for
// all the other languages"). The native twin is tests/i18n_lint.rs.
//
// Run: node --test scripts/tests/i18n-web.test.js
//
// What it proves:
//  1. web/shared/i18n-en.js is what scripts/gen-i18n.js writes from data/i18n/en.json.
//  2. THE RATCHET: each web file writes no more words into the page than its budget in
//     scripts/tests/fixtures/i18n-web-budget.txt (scripts/i18n-web.js says what counts),
//     and no fewer either: a word moved into en.json is locked in so it cannot be spent
//     on a new one. A file not listed has a budget of zero. To lock a win in:
//        I18N_RATCHET_WRITE=1 node --test scripts/tests/i18n-web.test.js
//     (=init rewrites every budget, raising included: installing the ratchet, or an
//     operator decision recorded in the commit, never to turn red green.)
//  3. Every key a page asks for (data-i18n*, i18n.t / tn / tMarked / fill / hint) is in
//     en.json: a typo fails here instead of showing a person the key.
//  4. Every static data-i18n element and data-i18n-<attr> attribute holds exactly what
//     en.json says (scripts/i18n-web.js --html-write rewrites them).
//  5. i18n.js falls back to English for a key a language has not translated, and to the
//     key for one nobody has; it fills {{placeholders}}, picks one/other for counts, and
//     builds glossary marks as [data-term] spans from text, so a translation can never
//     put markup into the page.
//  6. The scanner itself: it finds words in markup, sinks and attributes, and leaves
//     keys, ids, classes, URLs and compared values alone (each check able to fail).
//
// HOS_WEB_DIR points the web checks at another copy of web/ (used to see each one red).

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const W = require(path.join(ROOT, 'scripts', 'i18n-web.js'));
const GEN = require(path.join(ROOT, 'scripts', 'gen-i18n.js'));
const BUDGET = path.join(__dirname, 'fixtures', 'i18n-web-budget.txt');

const readBudget = () => {
  const m = {};
  if (!fs.existsSync(BUDGET)) return m;
  for (const line of fs.readFileSync(BUDGET, 'utf8').split(/\r?\n/)) {
    const l = line.trim();
    if (!l || l.startsWith('#')) continue;
    const sp = l.indexOf(' ');
    m[l.slice(sp + 1).trim()] = parseInt(l.slice(0, sp), 10);
  }
  return m;
};

const writeBudget = (m) => {
  const entries = Object.entries(m).filter(([, n]) => n > 0).sort((a, b) => a[0].localeCompare(b[0]));
  const total = entries.reduce((s, [, n]) => s + n, 0);
  const head =
    '# The website i18n RATCHET\'s budgets: per file under web/, how many words it still\n' +
    '# writes into the page instead of asking data/i18n/en.json by key. Checked by\n' +
    '# scripts/tests/i18n-web.test.js (its header says how to lower these). The numbers only\n' +
    '# go DOWN; a file not listed has a budget of zero.\n' +
    '# Total: ' + total + '\n';
  fs.writeFileSync(BUDGET, head + entries.map(([f, n]) => n + ' ' + f).join('\n') + '\n');
};

test('web/shared/i18n-en.js is generated from data/i18n/en.json and current', () => {
  const want = GEN.build(fs.readFileSync(GEN.EN_PATH, 'utf8'));
  const have = fs.readFileSync(GEN.OUT_PATH, 'utf8').replace(/\r\n/g, '\n');
  assert.strictEqual(have, want, 'web/shared/i18n-en.js is out of date: run node scripts/gen-i18n.js');
});

test('RATCHET: no web file writes more words into the page than its budget, nor fewer', () => {
  const now = W.counts();
  const budget = readBudget();
  const mode = process.env.I18N_RATCHET_WRITE || '';
  if (mode === 'init') { writeBudget(now); return; }
  const over = [];
  const under = [];
  for (const [f, n] of Object.entries(now)) {
    const b = budget[f] || 0;
    if (n > b) {
      const words = W.literalsOf(path.join(ROOT, f)).slice(0, 30).map((w) => '      line ' + w.line + ' [' + w.kind + '] ' + JSON.stringify(w.text));
      over.push('  ' + f + ': ' + n + ' words, budget ' + b + ' (+' + (n - b) + ')\n' + words.join('\n'));
    } else if (n < b) under.push([f, n, b]);
  }
  const gone = Object.keys(budget).filter((f) => !(f in now));
  if (mode === '1') {
    const lowered = { ...budget };
    for (const [f, n] of under) lowered[f] = n;
    for (const f of gone) delete lowered[f];
    writeBudget(lowered);
    under.length = 0;
    gone.length = 0;
  }
  assert.strictEqual(over.length, 0,
    '\n\nWORDS WRITTEN INTO THE PAGE, past the file\'s budget:\n' + over.join('\n') + '\n\n' +
    'Put the words in data/i18n/en.json under the page\'s key. In HTML: data-i18n="page.key" on\n' +
    'the element (keep the English inside it; node scripts/i18n-web.js --html-write fills it from\n' +
    'en.json), data-i18n-title / -placeholder / -aria-label / -tip for attributes. In script:\n' +
    'i18n.t("page.key"). A brand name or code sample: translate="no" on the element, or\n' +
    '`i18n-exempt: <reason>` on the script line. Do not raise the budget.\n');
  const msg = under.map(([f, n, b]) => '  ' + f + ': ' + n + ' words, budget ' + b + ': lower it to ' + n)
    .concat(gone.map((f) => '  ' + f + ': the file is gone; remove its line'));
  assert.strictEqual(msg.length, 0,
    '\n\nRATCHET CLICK (words moved out; lock the win in so it cannot be spent):\n' + msg.join('\n') +
    '\nRun: I18N_RATCHET_WRITE=1 node --test scripts/tests/i18n-web.test.js\n');
});

test('every key a web page asks for is in data/i18n/en.json', () => {
  const en = W.loadEn();
  const missing = [];
  for (const f of W.webFiles()) {
    const src = fs.readFileSync(f, 'utf8');
    if (!/data-i18n|i18n\s*\./.test(src)) continue;
    for (const k of W.keysUsed(src, f.endsWith('.html'))) {
      if (k.call === 'tn') {
        if (!((k.key + '.one') in en && (k.key + '.other') in en)) missing.push(W.relWeb(f) + ':' + k.line + ' tn(' + k.key + ') needs .one and .other');
      } else if (!(k.key in en)) missing.push(W.relWeb(f) + ':' + k.line + ' ' + k.call + ' ' + k.key);
    }
  }
  assert.deepStrictEqual(missing, [], 'keys en.json does not have (a person would see the key):\n' + missing.join('\n'));
});

test('static HTML holds exactly what en.json says for every data-i18n key', () => {
  const en = W.loadEn();
  const bad = [];
  for (const f of W.webFiles().filter((p) => p.endsWith('.html'))) {
    const src = fs.readFileSync(f, 'utf8').replace(/\r\n/g, '\n');
    if (!src.includes('data-i18n')) continue;
    for (const p of W.htmlSync(src, en, false).problems) {
      bad.push(W.relWeb(f) + ':' + p.line + ' ' + p.key + '\n   en.json: ' + JSON.stringify(p.want) + '\n   page:    ' + JSON.stringify(String(p.have).replace(/\s+/g, ' ').trim()));
    }
  }
  assert.deepStrictEqual(bad, [], 'static HTML differs from en.json (run node scripts/i18n-web.js --html-write):\n' + bad.join('\n'));
});

// ── i18n.js in a stand-in browser ─────────────────────────────────────────

/** A document that holds only what was built in it, enough for i18n.apply. */
function miniDoc() {
  class El {
    constructor(tag) { this.tagName = tag.toUpperCase(); this.attrs = {}; this.childNodes = []; this.ownerDocument = doc; }
    setAttribute(k, v) { this.attrs[k] = String(v); }
    getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; }
    hasAttribute(k) { return k in this.attrs; }
    appendChild(c) { this.childNodes.push(c); c.parentNode = this; return c; }
    removeChild(c) { this.childNodes = this.childNodes.filter((x) => x !== c); return c; }
    get firstChild() { return this.childNodes[0] || null; }
    set textContent(v) { this.childNodes = [{ nodeType: 3, text: String(v) }]; }
    get textContent() { return this.childNodes.map((c) => (c.nodeType === 3 ? c.text : c.textContent)).join(''); }
    get innerHTML() { throw new Error('innerHTML is never read or written by i18n.js'); }
    set innerHTML(_) { throw new Error('innerHTML is never read or written by i18n.js'); }
    matches(sel) { const m = /^\[([\w-]+)\]$/.exec(sel); return !!m && this.hasAttribute(m[1]); }
    querySelectorAll(sel) {
      const out = [];
      const visit = (e) => { for (const c of e.childNodes) if (c instanceof El) { if (c.matches(sel)) out.push(c); visit(c); } };
      visit(this);
      return out;
    }
  }
  const doc = {
    createElement: (t) => new El(t),
    createTextNode: (s) => ({ nodeType: 3, text: String(s) }),
  };
  doc.documentElement = new El('html');
  doc.body = new El('body');
  doc.documentElement.appendChild(doc.body);
  doc.querySelectorAll = (sel) => doc.documentElement.querySelectorAll(sel);
  doc.matches = () => false;
  return { doc, El };
}

function loadI18n(langFiles) {
  const { doc } = miniDoc();
  const store = new Map();
  const events = [];
  const window = {
    HOS_I18N_EN: langFiles.en,
    dispatchEvent: (e) => events.push(e.detail),
  };
  const ctx = {
    window,
    document: doc,
    localStorage: { getItem: (k) => (store.has(k) ? store.get(k) : null), setItem: (k, v) => store.set(k, v) },
    fetch: async (url) => {
      const code = /\/data\/i18n\/(\w+)\.json$/.exec(url)[1];
      if (!langFiles[code]) return { ok: false };
      return { ok: true, json: async () => langFiles[code] };
    },
    CustomEvent: class { constructor(type, init) { this.type = type; this.detail = init && init.detail; } },
    module: undefined,
  };
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(W.WEB, 'shared', 'i18n.js'), 'utf8'), ctx);
  return { i18n: window.i18n, doc, events, store };
}

test('i18n.js: a language that lacks a key shows the English, and a key nobody has shows the key', async () => {
  const { i18n } = loadI18n({
    en: { meta: { code: 'en' }, nav: { home: 'Home', files: 'Files' }, only: { english: 'Only in English' } },
    es: { meta: { code: 'es' }, nav: { home: 'Inicio' } },
  });
  assert.strictEqual(i18n.t('nav.home'), 'Home', 'English is there at once, before any load');
  await i18n.load('es');
  assert.strictEqual(i18n.getLanguage(), 'es');
  assert.strictEqual(i18n.t('nav.home'), 'Inicio');
  assert.strictEqual(i18n.t('only.english'), 'Only in English', 'an untranslated key falls back to English');
  assert.strictEqual(i18n.t('no.such.key'), 'no.such.key');
  await i18n.load('xx');
  assert.strictEqual(i18n.getLanguage(), 'en', 'a language with no file falls back to English');
  assert.strictEqual(i18n.t('nav.files'), 'Files');
});

test('i18n.js: placeholders, counts, and glossary marks built from text only', () => {
  const { i18n, doc } = loadI18n({
    en: {
      x: { by: 'Listed by {{name}}', items: { one: '{{n}} item', other: '{{n}} items' } },
      y: { relay: 'Your words go to a [[relay]] and its [[peers|peer]].' },
      z: { evil: 'Hi <img src=x onerror=alert(1)> [[<b>bold</b>|relay]]' },
    },
  });
  assert.strictEqual(i18n.t('x.by', { name: 'Ann' }), 'Listed by Ann');
  assert.strictEqual(i18n.t('x.by'), 'Listed by {{name}}', 'a missing value stays visible');
  assert.strictEqual(i18n.tn('x.items', 1), '1 item');
  assert.strictEqual(i18n.tn('x.items', 4), '4 items');
  assert.strictEqual(i18n.t('y.relay'), 'Your words go to a relay and its peers.');

  const p = doc.createElement('p');
  p.setAttribute('data-i18n', 'y.relay');
  doc.body.appendChild(p);
  const evil = doc.createElement('p');
  evil.setAttribute('data-i18n', 'z.evil');
  doc.body.appendChild(evil);
  const btn = doc.createElement('button');
  btn.setAttribute('data-i18n-tip', 'x.by');
  btn.setAttribute('data-i18n-aria-label', 'y.relay');
  doc.body.appendChild(btn);
  i18n.apply(doc);
  const terms = p.childNodes.filter((c) => c.tagName === 'SPAN');
  assert.deepStrictEqual(terms.map((s) => [s.getAttribute('data-term'), s.textContent]), [['relay', 'relay'], ['peer', 'peers']]);
  assert.strictEqual(p.textContent, 'Your words go to a relay and its peers.');
  assert.ok(evil.childNodes.every((c) => c.nodeType === 3 || c.tagName === 'SPAN'), 'only text and term spans');
  assert.strictEqual(evil.textContent, 'Hi <img src=x onerror=alert(1)> <b>bold</b>', 'markup in a translation stays text');
  assert.strictEqual(btn.getAttribute('data-tip-desc'), 'Listed by {{name}}');
  assert.strictEqual(btn.getAttribute('aria-label'), 'Your words go to a relay and its peers.', 'attributes get the plain words');
  i18n.hint(btn, 'y.relay');
  assert.strictEqual(btn.getAttribute('data-tip-desc'), 'Your words go to a relay and its peers.');
  assert.strictEqual(btn.getAttribute('data-i18n-tip'), 'y.relay');
});

// The glossary on every page (2026-10-23): a page that builds its markup as a string gets the
// marks as [data-term] spans with every word escaped, and a value put into {{name}} (a listing
// title, a person's name) is never read as a mark, whichever way the words reach the page.
test('i18n.js: html() builds escaped words with term spans, and a value is never a mark', () => {
  const { i18n, doc } = loadI18n({
    en: {
      m: { note: 'Sent [[end-to-end encrypted|e2ee]] to {{name}} & co.', plain: 'About "{{title}}"' },
      z: { evil: '<img src=x onerror=alert(1)> [[<b>x</b>|relay]]' },
    },
  });
  assert.strictEqual(
    i18n.html('m.note', { name: '<b>Ann</b>' }),
    'Sent <span data-term="e2ee">end-to-end encrypted</span> to &lt;b&gt;Ann&lt;/b&gt; &amp; co.',
  );
  assert.strictEqual(i18n.html('z.evil'), '&lt;img src=x onerror=alert(1)&gt; <span data-term="relay">&lt;b&gt;x&lt;/b&gt;</span>');
  assert.strictEqual(i18n.html('m.plain', { title: '[[relay]]' }), 'About &quot;[[relay]]&quot;', 'a value stays words');
  const p = doc.createElement('p');
  i18n.fill(p, 'm.plain', { title: '[[relay]]' });
  assert.ok(p.childNodes.every((c) => c.nodeType === 3), 'fill: a value is not built into a term span');
  assert.strictEqual(p.textContent, 'About "[[relay]]"');
  assert.strictEqual(i18n.html('no.such.key'), 'no.such.key');
});

// ── The scanner's own checks ──────────────────────────────────────────────

test('scanner: words in HTML text and attributes count; data-i18n, translate=no and code do not', () => {
  const html = [
    '<h1>Hello there</h1>',
    '<h2 data-i18n="a.b">Translated</h2>',
    '<button title="Clears it" data-i18n-title="a.c">C</button>',
    '<button aria-label="Close the window">x</button>',
    '<span translate="no">HumanityOS</span>',
    '<code>let x = 1;</code>',
    '<input placeholder="Search notes">',
    '<style>.a { color: red; }</style>',
    '<script>el.textContent = "Saved";</script>',
  ].join('\n');
  const found = W.markupWords(html, 1, { scripts: true }).map((f) => f.text);
  assert.deepStrictEqual(found, ['Hello there', 'Close the window', 'Search notes', 'Saved']);
});

test('scanner: words in script sinks and markup strings count; keys, ids, classes, URLs and compared values do not', () => {
  const js = [
    "el.textContent = 'All changes saved';",
    "el.title = cond ? 'On' : 'Off';",
    "list.innerHTML = '<li class=\"row active\">' + esc(name) + ' items</li>';",
    "alert('Too short');",
    "el.setAttribute('aria-label', 'Close');",
    "el.setAttribute('class', 'big button');",
    "el.textContent = i18n.t('notes.saved');",
    "if (mode === 'Some Mode') {}",
    "el.innerHTML = hosIcon('lock', 12) + ' Encrypted';",
    "fetch('/api/thing');",
    "const btn = { label: 'Save note', id: 'save_btn' };",
    "el.textContent = 'x'; // one letter is not a word",
    "toast('Copied'); // i18n-exempt: test of the escape hatch",
    "const t = `<button title=\"Send it\">${label} now</button>`;",
  ].join('\n');
  const found = W.jsLiterals(js).map((f) => f.text);
  assert.deepStrictEqual(found, ['All changes saved', 'On', 'Off', 'items', 'Too short', 'Close', ' Encrypted', 'Save note', 'Send it', 'now']);
});

test('scanner: isWords tells words from code', () => {
  for (const w of ['Save', 'Hello there', 'Loading…', '{} items', '日本']) assert.ok(W.isWords(w), w);
  for (const c of ['nav.home', 'chat_input', '/api/send', 'https://x.org', '#fff', '1px solid', 'display:none', 'color: red; margin: 0', '{}', 'x', '12px']) assert.ok(!W.isWords(c), c);
});

test('htmlSync: checks and rewrites data-i18n content and attributes from en.json', () => {
  const en = { a: 'Your [[relay]] & you', b: 'Clears the display' };
  const src = '<p data-i18n="a">Old words</p><button data-i18n-tip="b" data-tip-desc="old">C</button><p data-i18n="missing">x</p>';
  const { problems, out } = W.htmlSync(src, en, true);
  assert.strictEqual(problems.length, 3);
  assert.ok(out.includes('<p data-i18n="a">Your <span data-term="relay">relay</span> &amp; you</p>'), out);
  assert.ok(out.includes('data-tip-desc="Clears the display"'), out);
  assert.strictEqual(W.htmlSync(out, en, false).problems.length, 1, 'after a rewrite only the missing key remains');
});

// Found 2026-10-23 on calendar.html: i18n.t('calendar.month_' + n) was read as the key
// "calendar.month_", which does not exist. A key built at run time is not a literal key.
test('keysUsed: a key built at run time is not read as a literal key', () => {
  const keys = W.keysUsed("i18n.t('calendar.month_' + n); i18n.t('calendar.title'); i18n.hint(el, 'a.b_hint');", false).map((k) => k.key);
  assert.deepStrictEqual(keys, ['calendar.title', 'a.b_hint']);
});

// Found 2026-10-23 on tasks.html: an input with a placeholder AND a hint to rewrite came out with
// one of the two written, and on the second run a stray `">` after the tag. Red before the fix
// (each attribute was a separate edit of the same original tag), and a `$` in the words was read
// as a replacement pattern.
test('htmlSync: two attributes of one tag, and a $ in the words, rewrite cleanly in one pass', () => {
  const en = { p: 'Search tasks...', h: 'Costs $5 or $&, shown as typed.' };
  const src = '<input id="q" data-i18n-placeholder="p" placeholder="old" data-i18n-tip="h" data-tip-desc="old">';
  const once = W.htmlSync(src, en, true).out;
  assert.strictEqual(once, '<input id="q" data-i18n-placeholder="p" placeholder="Search tasks..." data-i18n-tip="h" data-tip-desc="Costs $5 or $&amp;, shown as typed.">');
  assert.strictEqual(W.htmlSync(once, en, false).problems.length, 0, 'one pass is enough');
  assert.strictEqual(W.htmlSync(once, en, true).out, once, 'a second pass changes nothing');
  const missing = W.htmlSync('<button data-i18n-tip="h" data-i18n-aria-label="p">x</button>', en, true).out;
  assert.strictEqual(missing, '<button data-i18n-tip="h" data-i18n-aria-label="p" aria-label="Search tasks..." data-tip-desc="Costs $5 or $&amp;, shown as typed.">x</button>');
});
