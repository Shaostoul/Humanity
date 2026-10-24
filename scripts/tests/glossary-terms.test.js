// The glossary on every page (the ELI5 mandate, docs/PRIORITIES.md TIER 3, 2026-10-23): a word a
// newcomer may not know is marked where it is written, and the mark opens its entry in
// data/glossary.json, on both apps. This proves every mark names an entry that exists, and that
// every mark is drawn somewhere a person can rest the pointer on it.
//
// Run: node --test scripts/tests/glossary-terms.test.js
//      GLOSSARY_REPORT=1 node --test scripts/tests/glossary-terms.test.js   (marks per page)
//
// The marks it reads:
//  - [[term]] and [[shown|term]] in every data/i18n/<language>.json. The desktop app draws them
//    through widgets::glossary_words (t_marked keeps the marks, t strips them); the website through
//    data-i18n, i18n.fill and i18n.html, which build each mark as a <span data-term>.
//  - data-term="term" written straight into a web page or script;
//  - a term named in Rust: widgets::definition_text(ui, shown, "term").
// Lookup is case-insensitive, as both glossary readers are (src/gui/glossary.rs, glossary.js).

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const W = require(path.join(ROOT, 'scripts', 'i18n-web.js'));

const glossary = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'glossary.json'), 'utf8'));
const TERMS = new Set(Object.keys(glossary.terms).map((k) => k.toLowerCase()));
const EN = W.flatten(JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8')));
const MARKED = new Set(Object.keys(EN).filter((k) => EN[k].includes('[[')));
// The two readers themselves: their doc comments show data-term examples, not marks on a page.
const OVERLAY_DOCS = /[\\/]shared[\\/](glossary|i18n)\.js$/;

function walk(dir, ext, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, ext, out);
    else if (ext.test(e.name)) out.push(p);
  }
  return out;
}

/** Every [[...]] mark in a string: [{shown, term}]. */
function marks(s) {
  return W.segments(s).filter((x) => x.term !== undefined);
}

/** The problems with the marks in one flat language table. */
function badMarks(file, flat) {
  const bad = [];
  for (const [key, val] of Object.entries(flat)) {
    for (const m of marks(val)) {
      if (!TERMS.has(m.term.toLowerCase().trim())) {
        bad.push(file + ' ' + key + ': [[' + (m.shown === m.term ? m.term : m.shown + '|' + m.term) + ']]');
      }
    }
    // A half-written mark shows its brackets to a person.
    if ((val.match(/\[\[/g) || []).length !== (val.match(/\]\]/g) || []).length) bad.push(file + ' ' + key + ': unbalanced [[ ]]');
  }
  return bad;
}

/** Rust key reads in a source: [{key, marked}] (t_marked keeps marks; t, t_dyn, tf, tn strip them). */
function rustReads(src) {
  const out = [];
  const re = /\b(t_marked|t_dyn|tf|tn|t)\(\s*"([a-z0-9_.]+)"/g;
  let m;
  while ((m = re.exec(src)) !== null) out.push({ key: m[2], marked: m[1] === 't_marked', line: src.slice(0, m.index).split('\n').length });
  return out;
}

/** Web key reads that strip marks: i18n.t / i18n.tn in script, and the hint and label attributes. */
function webPlainReads(file) {
  const src = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
  const out = [];
  for (const u of W.keysUsed(src, file.endsWith('.html'))) {
    if (u.call === 't' || u.call === 'tn') out.push({ key: u.key, line: u.line, how: 'i18n.' + u.call });
  }
  const attr = /data-i18n-(title|placeholder|aria-label|alt|tip|tip-title|content)\s*=\s*["']([^"'{}]+)["']/g;
  let m;
  while ((m = attr.exec(src)) !== null) out.push({ key: m[2], line: src.slice(0, m.index).split('\n').length, how: 'data-i18n-' + m[1] });
  return out;
}

/** Web reads that draw marks: data-i18n="key", i18n.fill(el, key), i18n.html(key). */
function webMarkedReads(file) {
  const src = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
  const out = [];
  const attr = /data-i18n\s*=\s*["']([^"'{}]+)["']/g;
  let m;
  while ((m = attr.exec(src)) !== null) out.push(m[1]);
  for (const u of W.keysUsed(src, file.endsWith('.html'))) if (u.call === 'fill' || u.call === 'html' || u.call === 'tMarked') out.push(u.key);
  return out;
}

test('every [[term]] mark in every language file names a glossary entry', () => {
  const bad = [];
  for (const f of fs.readdirSync(path.join(ROOT, 'data', 'i18n')).filter((n) => n.endsWith('.json'))) {
    bad.push(...badMarks(f, W.flatten(JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', f), 'utf8')))));
  }
  assert.deepStrictEqual(bad, [], 'marks whose term data/glossary.json lacks (add the entry, or fix the mark):\n' + bad.join('\n'));
});

test('every data-term on the website names a glossary entry', () => {
  const bad = [];
  for (const f of W.webFiles().filter((x) => !OVERLAY_DOCS.test(x))) {
    const src = fs.readFileSync(f, 'utf8');
    const re = /data-term\s*=\s*["']([^"'{}$+]+)["']/g;
    let m;
    while ((m = re.exec(src)) !== null) {
      if (!TERMS.has(m[1].toLowerCase().trim())) bad.push(W.relWeb(f) + ': data-term="' + m[1] + '"');
    }
  }
  assert.deepStrictEqual(bad, [], bad.join('\n'));
});

test('every glossary term the desktop code names exists', () => {
  const bad = [];
  for (const f of walk(path.join(ROOT, 'src', 'gui'), /\.rs$/)) {
    const src = fs.readFileSync(f, 'utf8');
    const re = /definition_text\([^,]+,\s*[^,]+,\s*"([^"]+)"\)/g;
    let m;
    while ((m = re.exec(src)) !== null) {
      if (!TERMS.has(m[1].toLowerCase())) bad.push(path.relative(ROOT, f).replace(/\\/g, '/') + ': "' + m[1] + '"');
    }
  }
  assert.deepStrictEqual(bad, [], bad.join('\n'));
});

// A mark is only worth writing where it shows. t("key") on the desktop and i18n.t('key') or a
// hint attribute on the website give the plain words, so a marked key read that way loses its
// glossary link without anyone noticing. Read it with t_marked + widgets::glossary_words (desktop)
// or data-i18n / i18n.fill / i18n.html (website); a hint that needs the same words takes a key of
// its own without marks.
test('a key that marks glossary terms is drawn with its marks on both apps', () => {
  const bad = [];
  for (const f of walk(path.join(ROOT, 'src'), /\.rs$/)) {
    const rel = path.relative(ROOT, f).replace(/\\/g, '/');
    if (rel === 'src/gui/i18n.rs') continue; // the reader itself and its tests
    for (const r of rustReads(fs.readFileSync(f, 'utf8'))) {
      if (!r.marked && MARKED.has(r.key)) bad.push(rel + ':' + r.line + ' reads "' + r.key + '" without its marks (use t_marked + widgets::glossary_words)');
    }
  }
  for (const f of W.webFiles()) {
    for (const r of webPlainReads(f)) {
      if (MARKED.has(r.key)) bad.push(W.relWeb(f) + ':' + r.line + ' reads "' + r.key + '" with ' + r.how + ', which drops its marks (use data-i18n, i18n.fill or i18n.html)');
    }
  }
  assert.deepStrictEqual(bad, [], bad.join('\n'));
});

test('the glossary checks can fail', () => {
  assert.deepStrictEqual(marks('a [[relay]] and [[peers|peer]]').map((x) => x.term), ['relay', 'peer']);
  assert.ok(TERMS.has('relay'), 'the glossary holds relay');
  assert.deepStrictEqual(badMarks('x.json', { 'a.b': 'see [[relay]]' }), []);
  assert.deepStrictEqual(badMarks('x.json', { 'a.b': 'see [[zzz-not-a-term]]' }), ['x.json a.b: [[zzz-not-a-term]]']);
  assert.deepStrictEqual(badMarks('x.json', { 'a.b': 'see [[relay' }), ['x.json a.b: unbalanced [[ ]]']);
  const reads = rustReads('t("a.b") t_marked("c.d") tf("e.f", &[])');
  assert.deepStrictEqual(reads.map((r) => [r.key, r.marked]), [['a.b', false], ['c.d', true], ['e.f', false]]);
});

test('marks per page (GLOSSARY_REPORT=1 prints them)', () => {
  const native = {};
  for (const f of walk(path.join(ROOT, 'src', 'gui', 'pages'), /\.rs$/)) {
    const src = fs.readFileSync(f, 'utf8');
    let n = 0;
    for (const r of rustReads(src)) if (r.marked && MARKED.has(r.key)) n += marks(EN[r.key]).length;
    n += (src.match(/definition_text\(/g) || []).length;
    if (n) native[path.basename(f, '.rs')] = n;
  }
  const htmlPages = new Set(W.webFiles().filter((f) => /web[\\/]pages[\\/][^\\/]+\.html$/.test(f)).map((f) => path.basename(f, '.html')));
  const web = {};
  for (const f of W.webFiles().filter((x) => !OVERLAY_DOCS.test(x))) {
    const page = W.webPageOf(W.relWeb(f), htmlPages);
    let n = 0;
    for (const k of webMarkedReads(f)) if (MARKED.has(k)) n += marks(EN[k]).length;
    n += (fs.readFileSync(f, 'utf8').match(/data-term\s*=\s*["'][^"'{}$+]+["']/g) || []).length;
    if (n) web[page] = (web[page] || 0) + n;
  }
  if (process.env.GLOSSARY_REPORT) {
    console.log('desktop pages with glossary marks: ' + JSON.stringify(native, null, 1));
    console.log('web pages with glossary marks: ' + JSON.stringify(web, null, 1));
  }
  assert.ok(Object.keys(native).length > 0 && Object.keys(web).length > 0);
});
