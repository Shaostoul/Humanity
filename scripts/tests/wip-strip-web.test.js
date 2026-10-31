// The unfinished-software warning on the website (2026-10-30, docs/design/app-bottom-strip.md,
// rung 1): web/shared/shell.js builds one amber line fixed to the bottom of every page until
// version 1.0, the mirror of the desktop app's src/gui/bottom_strip.rs. The words are the dated
// finding's (docs/reference/findings/2026-10-30-wip-warning-and-liability.md), kept in
// data/i18n/en.json under wip.*.
//
// Run: node --test scripts/tests/wip-strip-web.test.js   (listed in `just rig-tests`)
// HOS_WEB_DIR points it at another copy of web/ (how each test below was seen red).
//
// What it proves:
//  1. The version rule is the desktop app's: shell.js's hosWipShown, run on the same list of
//     versions the Rust test reads (scripts/tests/fixtures/wip-versions.json).
//  2. The strip's English is en.json's, word for word: the line with "warranty" as a glossary term,
//     the link, its hint and the strip's label, so the website cannot drift from the app.
//  3. What this means leads to a page the Library ships (/library#unfinished-software).
//  4. It sits under nothing and over the footer: fixed at the bottom, the footer's panel standing
//     on it (--hos-wip-h), a spacer at the end of the page, its colours tokens both themes define.
//  5. Every page with the site's menu loads shell.js, so every page carries the strip.
//  6. Loading shell.js (run whole in node:vm over a stand-in page) builds the strip, sets
//     --hos-wip-h to its measured height, and adds the spacer only once the page has been read,
//     as the last thing on it.
//  7. On a 1.0.0 site it builds none of that.
//
// Red first, 2026-10-30, each break made in a copy of web/ and this test run on it with
// HOS_WEB_DIR (each passing again on the real web/):
//  1: hosWipShown returning `Number(major) < 1` without the pre-release half: "1.0.0-rc.1 is
//     unfinished, so the warning shows".
//  2: the line's markup saying "lose work" for "lose your work": "the line's English is en.json's".
//  3: the link's address `/library#unfinished`: the strict equal on the address failed.
//  4: .site-footer back at `bottom: 0`: "the footer stands on the strip".
//  5: web/pages/notes.html loading another script in shell.js's place: the page list failed.
//  6: the spacer added at once instead of after DOMContentLoaded: "the spacer waits for the page,
//     so it lands last" (test 4) and test 6 red with it.
//  7: wipStrip without its hosWipShown check: "no strip on 1.0.0".

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const I18N = require(path.join(ROOT, 'scripts', 'i18n-web.js'));

const shell = fs.readFileSync(path.join(WEB, 'shared', 'shell.js'), 'utf8').replace(/\r\n/g, '\n');
const en = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8'));
const versions = JSON.parse(fs.readFileSync(path.join(__dirname, 'fixtures', 'wip-versions.json'), 'utf8'));

/** The source of one top-level function in the shell, from `function <name>(` to its closing brace. */
function functionSource(name) {
  const start = shell.indexOf('function ' + name + '(');
  assert.ok(start >= 0, 'shell.js has no function ' + name);
  const end = shell.indexOf('\n  }\n', start);
  assert.ok(end > start, 'could not find the end of ' + name);
  return shell.slice(start, end + 4);
}

/** The strip's markup, as the shell writes it into holder.innerHTML (its string pieces joined). */
function stripMarkup() {
  const at = shell.indexOf('(function wipStrip()');
  assert.ok(at >= 0, 'shell.js has no wipStrip block');
  const from = shell.indexOf('holder.innerHTML =', at);
  const to = shell.indexOf(';\n', from);
  const expr = shell.slice(from + 'holder.innerHTML ='.length, to);
  return vm.runInNewContext(expr, {});
}

test('1. the website shows the warning for exactly the versions the desktop app does', () => {
  const ctx = {};
  vm.runInNewContext(functionSource('hosWipShown') + '\nthis.hosWipShown = hosWipShown;', ctx);
  assert.ok(versions.shown.length >= 5 && versions.hidden.length >= 3, 'the shared version list is nearly empty');
  for (const v of versions.shown) assert.strictEqual(ctx.hosWipShown(v), true, JSON.stringify(v) + ' is unfinished, so the warning shows');
  for (const v of versions.hidden) assert.strictEqual(ctx.hosWipShown(v), false, JSON.stringify(v) + ' is a finished release, so the warning hides');
  // The version it is given is the one the update checker already knows the site to be.
  assert.match(shell, /window\.__HOS_WEB_VERSION = CURRENT_VERSION;/, 'the shell hands its version to the strip');
});

test('2. the strip says what en.json says, word for word', () => {
  const html = stripMarkup();
  const wip = en.wip;
  assert.ok(wip && wip.line && wip.more && wip.more_hint && wip.label && wip.version, 'en.json has the wip.* words');
  const between = (re) => { const m = re.exec(html); assert.ok(m, 'the strip has ' + re); return m[1]; };
  assert.strictEqual(between(/data-i18n="wip\.line">([\s\S]*?)<\/span><a /), I18N.renderMarked(wip.line), "the line's English is en.json's, warranty as a glossary term");
  assert.match(wip.line, /\[\[warranty\]\]/, 'the line marks warranty for the glossary');
  assert.strictEqual(between(/data-i18n="wip\.more"[^>]*>([^<]*)<\/a>/), I18N.renderMarked(wip.more), "the link's words are en.json's");
  assert.strictEqual(I18N.decodeEntities(between(/data-tip-desc="([^"]*)"/)), wip.more_hint, "the link's hint is en.json's");
  assert.strictEqual(between(/\saria-label="([^"]*)"/), wip.label, "the strip's label is en.json's");
  assert.match(html, /data-i18n-aria-label="wip\.label"/);
  assert.match(html, /data-i18n-tip="wip\.more_hint"/);
  // The version line fills its number into en.json's words once i18n.js is here.
  assert.match(wip.version, /\{\{version\}\}/);
  assert.match(shell, /window\.i18n\.t\('wip\.version', \{ version: version \}\)/, 'the version shows in en.json\'s words');
  // The glossary must have the term the line marks, or the mark is a dead end.
  const glossary = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'glossary.json'), 'utf8'));
  assert.ok(glossary.terms.warranty, 'data/glossary.json defines warranty');
});

test('3. What this means opens a page the Library ships', () => {
  const html = stripMarkup();
  const href = /class="hos-wip-more" href="([^"]+)"/.exec(html);
  assert.ok(href, 'the link has an address');
  assert.strictEqual(href[1], '/library#unfinished-software');
  const index = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'library', 'index.json'), 'utf8'));
  const slugOf = (f) => f.replace(/\.md$/i, '').replace(/_/g, '-').toLowerCase(); // scripts/build-library.js
  const slugs = new Set();
  const walk = (o) => {
    if (Array.isArray(o)) return o.forEach(walk);
    if (o && typeof o === 'object') {
      if (typeof o.file === 'string') slugs.add(slugOf(o.file));
      Object.values(o).forEach(walk);
    }
  };
  walk(index);
  assert.ok(slugs.has('unfinished-software'), 'data/library/index.json lists unfinished_software.md (node scripts/build-library.js)');
  // The desktop app opens the same page (src/gui/bottom_strip.rs DETAILS_SLUG).
  const rust = fs.readFileSync(path.join(ROOT, 'src', 'gui', 'bottom_strip.rs'), 'utf8');
  assert.match(rust, /pub const DETAILS_SLUG: &str = "unfinished-software";/, 'both apps open the same page');
});

test('4. the strip is fixed at the very bottom, the footer stands on it, and the page ends above it', () => {
  const rule = (sel) => {
    const at = shell.indexOf('    ' + sel + ' {\n');
    assert.ok(at >= 0, 'shell.js styles ' + sel);
    return shell.slice(at, shell.indexOf('\n    }', at));
  };
  const strip = rule('.hos-wip-strip');
  assert.match(strip, /position: fixed;/);
  assert.match(strip, /bottom: 0;/);
  assert.match(strip, /background: var\(--warning-bg\);/);
  assert.match(strip, /border-top: 1px solid var\(--warning\);/);
  const z = Number(/z-index: (\d+);/.exec(strip)[1]);
  const footer = rule('.site-footer');
  assert.match(footer, /bottom: var\(--hos-wip-h, 0px\);/, 'the footer stands on the strip');
  const footerZ = Number(/z-index: (\d+);/.exec(footer)[1]);
  assert.ok(z > footerZ, 'the folded footer slides away behind the strip, not over it');
  assert.ok(z < 5500, 'the strip stays under the top menu and its drawers');
  assert.match(shell, /\.hos-wip-spacer \{ display: block; flex: 0 0 auto; height: var\(--hos-wip-h, 0px\); \}/, 'a spacer keeps the end of the page clear');
  assert.match(shell, /document\.documentElement\.style\.setProperty\('--hos-wip-h', h \+ 'px'\)/, 'the strip measures itself');
  assert.match(shell, /document\.readyState === 'loading'\) document\.addEventListener\('DOMContentLoaded', addSpacer\)/, 'the spacer waits for the page, so it lands last');
  // Its colours are tokens, defined for the dark theme (theme.css :root) and the light one.
  const css = fs.readFileSync(path.join(WEB, 'shared', 'theme.css'), 'utf8');
  assert.ok((css.match(/--warning-bg: #[0-9a-f]{6};/g) || []).length >= 2, 'theme.css defines --warning-bg for both themes');
});

// A stand-in that answers anything harmlessly, so shell.js runs whole (the server strip's test,
// scripts/tests/server-strip-web.test.js, runs the shell the same way).
function anything() {
  const fn = function () {};
  return new Proxy(fn, {
    get(_t, prop) {
      if (prop === Symbol.toPrimitive) return () => '';
      if (prop === Symbol.iterator) return function* () {};
      if (prop === 'then') return undefined;
      if (prop === 'length') return 0;
      return anything();
    },
    set: () => true, has: () => true, apply: () => anything(), construct: () => anything(),
  });
}

/** Run shell.js as a page loads it (the page still being read), with the version it carries
 *  replaced by `version`. Returns what it built into the page and set on <html>. */
function loadShell(version) {
  const rootStyle = {};
  const appended = [];
  const markups = [];
  const docListeners = {};
  const el = (tag) => {
    const t = {
      tagName: String(tag).toUpperCase(), style: {}, dataset: {}, className: '',
      getBoundingClientRect: () => ({ height: 26.4, width: 1200, top: 0, left: 0, right: 1200, bottom: 26 }),
      querySelector: () => el('span'), querySelectorAll: () => [],
      appendChild: (c) => { appended.push(c); return c; }, prepend() {}, append() {}, insertBefore: (c) => c,
      insertAdjacentHTML() {}, insertAdjacentElement() {},
      addEventListener() {}, removeEventListener() {}, setAttribute() {}, getAttribute: () => null,
      removeAttribute() {}, hasAttribute: () => false, remove() {}, contains: () => false, closest: () => null,
    };
    return new Proxy(t, {
      get: (o, p) => (p === 'firstChild' ? (o.firstChild = o.firstChild || el('div')) : p in o ? o[p] : anything()),
      set: (o, p, v) => { if (p === 'innerHTML') markups.push(String(v)); o[p] = v; return true; },
    });
  };
  const html = el('html');
  html.style = { setProperty: (k, v) => { rootStyle[k] = v; } };
  const doc = el('#document');
  Object.assign(doc, {
    documentElement: html, body: el('body'), head: el('head'), currentScript: el('script'), readyState: 'loading',
    createElement: (t) => el(t), createTextNode: () => el('#text'),
    // Nothing of the strip's is on the page yet; anything else the shell looks for is.
    getElementById: () => null,
    querySelector: (sel) => (String(sel).includes('hos-wip') ? null : el('div')),
    addEventListener: (t, f) => { (docListeners[t] = docListeners[t] || []).push(f); },
  });
  const store = new Map();
  const ctx = {
    document: doc,
    location: { hostname: 'united-humanity.us', host: 'united-humanity.us', pathname: '/library', href: 'https://united-humanity.us/library', search: '', hash: '' },
    localStorage: { getItem: (k) => (store.has(k) ? store.get(k) : null), setItem: (k, v) => store.set(k, String(v)), removeItem: (k) => store.delete(k) },
    navigator: anything(), matchMedia: () => ({ matches: false, addEventListener() {} }),
    fetch: () => new Promise(() => {}), XMLHttpRequest: class { open() {} send() {} setRequestHeader() {} },
    setTimeout: () => 0, clearTimeout() {}, setInterval: () => 0, clearInterval() {}, requestAnimationFrame: () => 0,
    ResizeObserver: class { observe() {} }, MutationObserver: class { observe() {} disconnect() {} },
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    addEventListener() {}, removeEventListener() {},
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const src = fs.readFileSync(path.join(WEB, 'shared', 'shell.js'), 'utf8').replace(/var CURRENT_VERSION = '[^']*';/, "var CURRENT_VERSION = '" + version + "';");
  vm.runInContext(src, ctx, { filename: 'shell.js' });
  const readDone = () => (docListeners.DOMContentLoaded || []).forEach((f) => f());
  return { rootStyle, appended, markups, readDone };
}

test('6. loading shell.js builds the strip on a 0.x site, sizes the footer by it, and adds the spacer last', () => {
  const page = loadShell('0.1547.0');
  assert.ok(page.markups.some((m) => m.includes('id="hos-wip-strip"')), 'shell.js builds the strip');
  assert.strictEqual(page.rootStyle['--hos-wip-h'], '27px', "--hos-wip-h is the strip's measured height");
  const spacerBefore = page.appended.filter((c) => c && c.className === 'hos-wip-spacer').length;
  assert.strictEqual(spacerBefore, 0, 'the spacer waits until the page has been read');
  page.readDone();
  const spacers = page.appended.filter((c) => c && c.className === 'hos-wip-spacer');
  assert.strictEqual(spacers.length, 1, 'once the page is read, one spacer goes at its end');
  assert.strictEqual(page.appended[page.appended.length - 1], spacers[0], 'the spacer is the last thing on the page');
});

test('7. a finished site (1.0.0) builds no strip, sets no height and adds no spacer', () => {
  const page = loadShell('1.0.0');
  page.readDone();
  assert.ok(!page.markups.some((m) => m.includes('hos-wip-strip')), 'no strip on 1.0.0');
  assert.strictEqual(page.rootStyle['--hos-wip-h'], undefined, 'no strip height on 1.0.0');
  assert.ok(!page.appended.some((c) => c && c.className === 'hos-wip-spacer'), 'no spacer on 1.0.0');
});

test('5. every page with the site menu loads shell.js, so every page carries the strip', () => {
  const pages = [];
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith('.html')) pages.push(p);
    }
  };
  walk(path.join(WEB, 'pages'));
  walk(path.join(WEB, 'chat'));
  walk(path.join(WEB, 'home'));
  assert.ok(pages.length >= 40, 'found the site\'s pages');
  const without = pages
    .filter((p) => !/\/shared\/shell\.js/.test(fs.readFileSync(p, 'utf8')))
    .map((p) => path.relative(WEB, p).split(path.sep).join('/'));
  // mission.html is a forwarder: it holds only a redirect to the Library and draws no page.
  assert.deepStrictEqual(without, ['pages/mission.html']);
});
