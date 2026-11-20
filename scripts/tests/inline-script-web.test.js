// No inline script on the website (2026-11-20, the website's script policy, docs/PRIORITIES.md
// item 6b). The site's Content-Security-Policy (scripts/nginx/humanity.conf) no longer allows
// 'unsafe-inline' in script-src, so a browser refuses every <script> block written in a page and
// every onclick="..." attribute, in the page or in markup a script builds. What a button does is
// written as data-on-click="..." and run by web/shared/safe-markup.js, which parses a small call
// language and never evaluates code.
//
// Run: node --test scripts/tests/inline-script-web.test.js
//
// What it proves:
//  1. No HTML page under web/ has a <script> without src (JSON-LD data blocks excepted), an on*=
//     attribute, or a javascript: address.
//  2. No script under web/ writes an on*= attribute into markup, sets one with setAttribute, or
//     runs a string as code (eval, new Function): the policy refuses all three. Nor does a script
//     write a data-on-* attribute itself (BUG-331): markup built in script gets its handlers from
//     hosOn.attr(type, text), whose token is the only thing safe-markup.js runs from such markup.
//  3. Every handler, data-on-* in a page or hosOn.attr in a script, names an event safe-markup.js
//     handles and parses in its language; in a script, every value put into one goes through
//     hosArg() (a quote in a name then stays inside its string) or hosNum().
//  4. Every page that has data-on-* handlers, or loads a script that builds them, loads
//     /shared/safe-markup.js as the FIRST script of its <head> (it records the page's static
//     handlers as the parser makes them, so it must run before anything else can make markup).
//  5. The site's policy itself: no 'unsafe-inline', 'unsafe-eval' or outside host in script-src,
//     in every server block of the nginx config.
//  6. safe-markup.js's language: what it runs and what it refuses.
//  7. No handler starts a path at a name the browser's window already has (document, window,
//     console, Math...): safe-markup.js refuses those at run time, so such a handler is broken.
//
// SEEN RED (2026-11-20): before the move, check 1 listed 25 inline <script> blocks and 497 on*=
// attributes in 15 pages, check 2 listed 135 on*= attributes in 25 scripts and both new Function
// calls (chat-onboarding.js, calculator-app.js), and check 5 failed on all three server blocks.
// SEEN RED (2026-11-20, BUG-331) against web/ at 28d1c3596: check 2 listed 181 data-on-*
// attributes written in strings on 172 lines of 31 scripts; check 3 refused 12 static handlers that
// start at document or sessionStorage; check 4 named chat/index.html (emoji.js and
// index-preflight.js ran first) and pages/files.html (files-frame.js first); check 7 named the
// three that start at console, Math and String.
// HOS_WEB_DIR points checks 1 to 4 and 7 at another copy of web/ (used to see each one red).

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const SM = require(path.join(ROOT, 'web', 'shared', 'safe-markup.js'));
const { pageHandlers, attrHandlers, stripComments, decodeEntities } = require('./handlers-web.js');

// Third-party files kept as published (each also checked by hand when it was added).
const VENDOR = /[\\/]vendor[\\/]|qrcode\.js$/;

function walk(dir, ext, out = []) {
  for (const f of fs.readdirSync(dir)) {
    const p = path.join(dir, f);
    const st = fs.statSync(p);
    if (st.isDirectory()) walk(p, ext, out);
    else if (p.endsWith(ext)) out.push(p);
  }
  return out;
}
const rel = (p) => path.relative(WEB, p).replace(/\\/g, '/');
const lineOf = (src, idx) => src.slice(0, idx).split('\n').length;

const pages = walk(WEB, '.html');
const scripts = walk(WEB, '.js').filter((p) => !VENDOR.test(p));

// ── 1. pages ─────────────────────────────────────────────────────────────
function pageProblems(src) {
  const out = [];
  const re = /<script\b([^>]*)>/gi;
  let m;
  while ((m = re.exec(src))) {
    if (/\bsrc\s*=/i.test(m[1])) continue;
    if (/type\s*=\s*["']?application\/(ld\+)?json/i.test(m[1])) continue;
    out.push(lineOf(src, m.index) + ': an inline <script> block (move it into a file)');
  }
  const tags = /<[a-zA-Z][^<>]*>/g;
  while ((m = tags.exec(src))) {
    const a = /\son([a-z]+)\s*=/i.exec(m[0]);
    if (a) out.push(lineOf(src, m.index) + ': on' + a[1] + '= (write data-on-' + a[1] + '="..." instead)');
    if (/\s(href|src|action|formaction)\s*=\s*["']?\s*javascript:/i.test(m[0])) out.push(lineOf(src, m.index) + ': a javascript: address');
  }
  return out;
}

test('no page has an inline <script>, an on*= attribute or a javascript: address', () => {
  const bad = [];
  for (const p of pages) for (const b of pageProblems(fs.readFileSync(p, 'utf8'))) bad.push(rel(p) + ':' + b);
  assert.deepStrictEqual(bad, [], '\nINLINE SCRIPT the site\'s policy refuses:\n' + bad.join('\n'));
});

// ── 2. scripts ───────────────────────────────────────────────────────────
// An on*= attribute inside a string a script writes into markup: on, a lower-case event name,
// "=" and a quote (plain, or escaped for the string it sits in).
const ON_ATTR = /\son[a-z]{3,}=(?:["'`]|\\["'])/g;
function scriptProblems(src) {
  const out = [];
  let m;
  ON_ATTR.lastIndex = 0;
  while ((m = ON_ATTR.exec(src))) out.push(lineOf(src, m.index) + ': ' + m[0].trim() + ' in markup (write data-on-...)');
  const res = [
    [/setAttribute\(\s*['"]on[a-z]/g, 'setAttribute of an on* attribute'],
    [/\bnew\s+Function\s*\(/g, 'new Function (a string run as code)'],
    [/(^|[^.\w$])eval\s*\(/g, 'eval (a string run as code)'],
    [/setTimeout\(\s*['"`]|setInterval\(\s*['"`]/g, 'a timer given a string of code'],
    // script-src is 'self': a module imported from another site is refused (pq-identity.js
    // imported noble from esm.sh until 2026-11-20, so its PQ calls failed live).
    [/\bimport\s*\(\s*['"`]https?:|https?:\/\/(esm\.sh|unpkg\.com|cdn\.skypack\.dev|cdnjs\.cloudflare\.com|ga\.jspm\.io)\b/g, 'a script from another site (the policy runs only this site\'s)'],
  ];
  for (const [re, what] of res) {
    re.lastIndex = 0;
    while ((m = re.exec(src))) out.push(lineOf(src, m.index) + ': ' + what);
  }
  // A data-on-* attribute written by the script itself (BUG-331): safe-markup.js ignores it, since
  // markup a script builds may hold someone else's text, and runs only hosOn.attr's tokens.
  const code = stripComments(src);
  const dataOn = [
    [/data-on-[a-z]+\s*=/g, 'a data-on-* attribute written in a string (write hosOn.attr(type, text))'],
    [/setAttribute(?:NS)?\(\s*(?:[^,()]+,\s*)?['"`]data-on-/g, 'setAttribute of a data-on-* attribute (write hosOn.attr(type, text) into the markup)'],
    [/\.dataset\.on[A-Z]|dataset\[\s*['"`]on[A-Z]/g, 'a data-on-* attribute set through dataset'],
  ];
  for (const [re, what] of dataOn) {
    re.lastIndex = 0;
    while ((m = re.exec(code))) out.push(lineOf(src, m.index) + ': ' + what);
  }
  return out;
}

test('no script writes an on*= attribute, sets one, or runs a string as code', () => {
  const bad = [];
  for (const p of scripts) {
    if (/safe-markup\.js$/.test(p)) continue; // its header shows the old form it replaces
    for (const b of scriptProblems(fs.readFileSync(p, 'utf8'))) bad.push(rel(p) + ':' + b);
  }
  assert.deepStrictEqual(bad, [], '\nINLINE SCRIPT in a script\'s markup:\n' + bad.join('\n'));
});

// ── 3. handlers ──────────────────────────────────────────────────────────
// Static handlers are data-on-* in a page; markup built in script takes hosOn.attr(type, text),
// read by scripts/tests/handlers-web.js (hosArg values stand as "H", hosNum values as 0). A line
// marked `// handler-exempt` (a builder whose every caller passes a constant handler) is skipped.

function handlerProblems(where, list) {
  const out = [];
  for (const h of list) {
    if (!SM.TYPES.includes(h.type)) out.push(where + ':' + h.line + ': data-on-' + h.type + ' is not an event safe-markup.js handles');
    if (h.bad) { out.push(where + ':' + h.line + ': ' + h.bad); continue; }
    if (h.text === null) continue;
    try { SM.parse(h.text); } catch (e) { out.push(where + ':' + h.line + ': ' + e.message); }
  }
  return out;
}

test('every handler parses, names a handled event, and takes data only through hosArg() or hosNum()', () => {
  const bad = [];
  for (const p of pages) bad.push(...handlerProblems(rel(p), pageHandlers(fs.readFileSync(p, 'utf8'))));
  for (const p of scripts) {
    if (/safe-markup\.js$/.test(p)) continue;
    bad.push(...handlerProblems(rel(p), attrHandlers(fs.readFileSync(p, 'utf8'))));
  }
  assert.deepStrictEqual(bad, [], '\nHANDLERS that will not run:\n' + bad.join('\n'));
});

// ── 4. safe-markup.js loaded where it is needed, first ──────────────────
function scriptSrcs(src) {
  const out = [];
  const re = /<script\b[^>]*\bsrc\s*=\s*["']?([^"' >]+)/gi;
  let m;
  while ((m = re.exec(src))) out.push({ src: m[1].split('?')[0], at: m.index });
  return out;
}
function fileForSrc(src) {
  if (!src.startsWith('/')) return null;
  const p = path.join(WEB, src.slice(1));
  return fs.existsSync(p) ? p : null;
}
/** Where the page's first script tag starts (a JSON-LD data block is not a script). */
function firstScriptAt(src) {
  const m = /<script\b(?![^>]*type\s*=\s*["']?application\/(ld\+)?json)/i.exec(src);
  return m ? m.index : -1;
}
/** Why a page needs safe-markup.js, or null. */
function needsSafeMarkup(src) {
  if (/\sdata-on-[a-z]+\s*=/.test(src)) return 'its own markup';
  for (const s of scriptSrcs(src)) {
    const f = fileForSrc(s.src);
    if (f && !/safe-markup\.js$/.test(f) && /data-on-[a-z]+=|\bhosOn\.attr\s*\(/.test(fs.readFileSync(f, 'utf8'))) return s.src;
  }
  return null;
}
function loadOrderProblems(name, src) {
  const needs = needsSafeMarkup(src);
  if (!needs) return [];
  const sm = scriptSrcs(src).find((s) => s.src === '/shared/safe-markup.js');
  const head = src.search(/<\/head>/i);
  if (!sm) return [name + ': needs /shared/safe-markup.js (for ' + needs + ') and does not load it'];
  if (head >= 0 && sm.at > head) return [name + ': loads /shared/safe-markup.js after </head>; load it in the <head>'];
  if (firstScriptAt(src) !== sm.at) return [name + ': /shared/safe-markup.js is not the first script (it must see every element the parser makes)'];
  return [];
}

test('every page with handlers loads /shared/safe-markup.js as the first script of its <head>', () => {
  const bad = [];
  for (const p of pages) bad.push(...loadOrderProblems(rel(p), fs.readFileSync(p, 'utf8')));
  assert.deepStrictEqual(bad, [], '\n' + bad.join('\n'));
});

// ── 7. no handler starts at a browser name ──────────────────────────────
// safe-markup.js refuses a path that starts at a name the window had when it loaded (it is the
// first script, so those are the browser's own: document, location, console, Math...). The
// browser's names: scripts/tests/fixtures/browser-window-names.txt.
const BROWSER_NAMES = new Set(fs.readFileSync(path.join(__dirname, 'fixtures', 'browser-window-names.txt'), 'utf8')
  .split('\n').filter((l) => !/^\s*#/.test(l)).join(' ').split(/\s+/).filter(Boolean));
function builtinRootProblems(where, list) {
  const out = [];
  for (const h of list) {
    if (h.bad || h.text === null) continue;
    let calls;
    try { calls = SM.parse(h.text); } catch (e) { continue; } // check 3 reports it
    for (const g of SM.globalsOf(calls)) {
      if (BROWSER_NAMES.has(g) || /^on[a-z]+$/.test(g)) out.push(where + ':' + h.line + ': starts at ' + g + ', the browser\'s (move it into a named page function): ' + h.text);
    }
  }
  return out;
}
const HANDLER_ROOT_SKIP = /safe-markup\.js$/;

test('no handler starts a path at a name the browser provides', () => {
  const bad = [];
  for (const p of pages) bad.push(...builtinRootProblems(rel(p), pageHandlers(fs.readFileSync(p, 'utf8'))));
  for (const p of scripts) if (!HANDLER_ROOT_SKIP.test(p)) bad.push(...builtinRootProblems(rel(p), attrHandlers(fs.readFileSync(p, 'utf8'))));
  assert.deepStrictEqual(bad, [], '\n' + bad.join('\n'));
});

// ── 5. the policy ────────────────────────────────────────────────────────
test('the site\'s Content-Security-Policy allows no inline script, no eval and no outside script host', () => {
  const conf = fs.readFileSync(path.join(ROOT, 'scripts', 'nginx', 'humanity.conf'), 'utf8');
  const csps = conf.split('\n').filter((l) => /add_header\s+Content-Security-Policy/.test(l));
  assert.ok(csps.length >= 3, 'the main server block, /chat and / each set the policy (found ' + csps.length + ')');
  for (const line of csps) {
    const m = /script-src([^;]*);/.exec(line);
    assert.ok(m, 'a script-src in: ' + line.trim().slice(0, 120));
    const srcList = m[1].trim().split(/\s+/);
    assert.deepStrictEqual(srcList, ["'self'"], "script-src is 'self' alone, found: " + m[1].trim());
  }
});

// The emoji are this site's own since 2026-11-20 (OpenMoji, web/shared/emoji.js), so no directive
// names cdn.jsdelivr.net, and img-src keeps the live server's breadth (`https:`): the chat's
// click-to-load cards show a picture from another site after the reader presses Load (BUG-326).
// SEEN RED (2026-11-20) against the config before this change: "img-src allows https: ... found:
// 'self' data: blob: https://cdn.jsdelivr.net https://*.arweave.net ..." on all three lines.
test('every server block sends the same policy, names no emoji host, and lets a reader load a picture', () => {
  const conf = fs.readFileSync(path.join(ROOT, 'scripts', 'nginx', 'humanity.conf'), 'utf8');
  const csps = conf.split('\n')
    .map((l) => /add_header\s+Content-Security-Policy\s+"([^"]+)"/.exec(l))
    .filter(Boolean)
    .map((m) => m[1]);
  assert.ok(csps.length >= 3, 'found ' + csps.length + ' policy lines');
  // nginx drops every inherited add_header in a location that declares its own, so /chat and /
  // repeat the policy: one edited without the others is a page with a different policy.
  assert.strictEqual(new Set(csps).size, 1, 'the policy lines differ:\n' + csps.join('\n'));
  for (const p of csps) {
    const img = /img-src([^;]*);/.exec(p);
    assert.ok(img && img[1].trim().split(/\s+/).includes('https:'), 'img-src allows https: (a picture the reader asks for), found: ' + (img ? img[1].trim() : 'none'));
    assert.ok(!/cdn\.jsdelivr\.net/.test(p), 'no directive names cdn.jsdelivr.net (the emoji are hosted here): ' + p);
  }
});

/** Does a service worker's fetch handler leave another site's request to the browser before it
 *  answers anything itself? (A fetch the worker makes comes under connect-src, not img-src.) */
function swPassesOtherSites(src) {
  const at = src.indexOf("addEventListener('fetch'");
  const pass = src.search(/new URL\([^)]*\)\.origin\s*!==\s*self\.location\.origin\)\s*return;/);
  const answer = src.indexOf('respondWith', at);
  return at >= 0 && pass > at && (answer < 0 || pass < answer);
}

// BUG-326, its second half: the worker answered every request, another site's picture included, so
// with img-src open the picture still failed under connect-src. SEEN RED 2026-11-20 against
// sw.js before the fix (git show 9f1b35967:web/shared/sw.js): false.
test('the service worker leaves another site\'s picture to the browser and the page\'s img-src', () => {
  assert.ok(swPassesOtherSites(fs.readFileSync(path.join(WEB, 'shared', 'sw.js'), 'utf8')),
    'web/shared/sw.js returns early for a request to another origin, before any respondWith');
  assert.ok(!swPassesOtherSites("self.addEventListener('fetch', e => { e.respondWith(fetch(e.request)); });"), 'the check can fail');
});

// ── 6. the language ──────────────────────────────────────────────────────
test('safe-markup.js runs calls with literal and path arguments, as an inline handler did', () => {
  const seen = [];
  const win = {
    showSection: (a, b) => seen.push(['showSection', a, b]),
    savePref: (k, v) => seen.push(['savePref', k, v]),
    HosTasks: { open(id) { seen.push(['open', id, this === win.HosTasks]); } },
  };
  const el = { value: 'typed', checked: true, dataset: { id: '7' } };
  const ev = { stopPropagation() { seen.push(['stop']); }, key: 'Enter' };
  const text = "event.stopPropagation(); showSection('a b', -2.5); savePref(\"k\", this.checked); HosTasks.open(this.dataset.id)";
  const calls = SM.parse(text);
  assert.strictEqual(calls.length, 4);
  assert.deepStrictEqual(SM.globalsOf(calls).sort(), ['HosTasks', 'savePref', 'showSection']);
  // The module evaluates against the global object it was loaded under; put the stand-ins there.
  Object.assign(globalThis, win);
  try {
    // A name is reachable only once a handler the page vouched for names it (BUG-331).
    assert.throws(() => SM.run('showSection(1)', el, ev), ReferenceError, 'not yet vouched for');
    assert.match(SM.attr('click', text), /^data-on-click="h[0-9a-f]{32}"$/, 'hosOn.attr writes a token, never the text');
    SM.run(text, el, ev);
  } finally {
    for (const k of Object.keys(win)) delete globalThis[k];
  }
  assert.deepStrictEqual(seen, [['stop'], ['showSection', 'a b', -2.5], ['savePref', 'k', true], ['open', '7', true]]);
});

test('safe-markup.js refuses anything that is not a call, and hosArg keeps a quote inside its string', () => {
  for (const bad of [
    "x = 1", "if (a) b()", "a() + b()", "fn('unclosed)", "this.value", "eval('1')",
    "Function('x')()", "a.constructor.constructor('x')()", "a()b()", "setTimeout(f, 1)", "fn(() => 1)",
    // BUG-331: no path from a written value, and none through the names that reach the window,
    // the document, storage, or a way to run code or write markup.
    "go(''.concat('x'))", "go([1].map(f))", "go(this.ownerDocument)", "go(event.view)", "go(event.composedPath())",
    "location.assign('x')", "go(localStorage.getItem('k'))", "go(f.bind(null))", "go(p.then(f))",
    "Object.assign(this, {innerHTML: 'x'})", "go(this.innerHTML)", "this.insertAdjacentHTML('beforeend', 'x')",
    "go(document.cookie)", "window.open('x')", "go(this.getRootNode())", "go(this.contentWindow)",
  ]) {
    assert.throws(() => SM.parse(bad), SyntaxError, 'refused: ' + bad);
  }
  // A hostile name, put in with hosArg, decodes back to ONE string argument and nothing more.
  const name = "a'); deleteAll('\" <b>&";
  const attr = 'openNote(' + SM.hosArg(name) + ', ' + SM.hosNum(5) + ', ' + SM.hosArg(null) + ')';
  assert.ok(!/["<>]/.test(attr), 'escaped for a double-quoted attribute: ' + attr);
  const calls = SM.parse(decodeEntities(attr));
  assert.strictEqual(calls.length, 1, 'one call');
  assert.deepStrictEqual(calls[0].args.map((a) => a.v), [name, 5, 'null'], 'hosArg writes String(v), hosNum a number');
  assert.strictEqual(SM.hosEsc('<a href="x">\'&'), '&lt;a href=&quot;x&quot;&gt;&#39;&amp;');
  // hosOn.attr reads the same text the attribute held: one token per text, a fresh one per text.
  const a1 = SM.attr('click', 'openNote(' + SM.hosArg(name) + ')');
  assert.strictEqual(SM.attr('click', 'openNote(' + SM.hosArg(name) + ')'), a1, 'the same text keeps its token');
  assert.notStrictEqual(SM.attr('click', 'openNote(' + SM.hosArg('b') + ')'), a1);
  assert.throws(() => SM.attr('clik', 'go()'), TypeError, 'an event it does not handle');
  assert.throws(() => SM.attr('click', 'go('), SyntaxError, 'a mistake shows where the markup is built');
  // set() refuses markup, handlers and addresses, and the window, document and storage as targets.
  for (const [t, k] of [[{}, 'innerHTML'], [{}, 'outerHTML'], [{}, 'onclick'], [{}, 'href'], [{}, 'srcdoc'], [{}, '__proto__'], [globalThis, 'x']]) {
    assert.throws(() => SM.set(t, k, 'x'), TypeError, 'set refuses ' + k);
  }
});

test('the checks above each fail on what they are for', () => {
  assert.strictEqual(pageProblems('<script>alert(1)</script>').length, 1);
  assert.strictEqual(pageProblems('<script type="application/ld+json">{}</script>').length, 0);
  assert.strictEqual(pageProblems('<button onclick="go()">x</button>').length, 1);
  assert.strictEqual(pageProblems('<a href="javascript:go()">x</a>').length, 1);
  assert.strictEqual(pageProblems('<button data-on-click="go()">x</button>').length, 0);
  assert.strictEqual(scriptProblems("el.innerHTML = '<b onclick=\"go()\">';").length, 1);
  assert.strictEqual(scriptProblems("el.innerHTML = '<b onclick=\\'go()\\'>';").length, 1);
  assert.strictEqual(scriptProblems("b.setAttribute('onclick', 'go()');").length, 1);
  assert.strictEqual(scriptProblems("new Function('return 1')").length, 1);
  assert.strictEqual(scriptProblems("btn.onclick = function () {};").length, 0);
  assert.strictEqual(scriptProblems("const m = await import(`${'https://esm.sh/@noble/post-quantum'}/ml-dsa`);").length, 1);
  assert.strictEqual(scriptProblems("const m = await import('https://example.org/x.js');").length, 1);
  assert.strictEqual(scriptProblems("const m = await import('/shared/vendor/noble-pq.bundle.js');").length, 0);
  // BUG-331: a data-on-* attribute a script writes itself, however it writes it.
  assert.strictEqual(scriptProblems("x = `<b data-on-click=\"open(${hosArg(id)})\">`;").length, 1);
  assert.strictEqual(scriptProblems("x = '<b data-on-click=\"go()\">';").length, 1);
  assert.strictEqual(scriptProblems("b.setAttribute('data-on-click', 'go()');").length, 1);
  assert.strictEqual(scriptProblems("b.dataset.onClick = 'go()';").length, 1);
  assert.strictEqual(scriptProblems("// a comment may show data-on-click=\"go()\"\nx = 1;").length, 0);
  assert.strictEqual(scriptProblems("x = '<b ' + hosOn.attr('click', 'go()') + '>';").length, 0);
  const h = (s) => attrHandlers(s).map((x) => x.bad);
  assert.deepStrictEqual(h("x = `<b ${hosOn.attr('click', `open(${hosArg(id)})`)}>`;"), [null]);
  assert.deepStrictEqual(h("x = '<b ' + hosOn.attr('click', 'open(' + hosArg(n.id) + ', 1)') + '>';"), [null]);
  assert.deepStrictEqual(h("x = `<b ${hosOn.attr('click', `open('${id}')`)}>`;"), ['a ${...} value not written with hosArg() or hosNum(): ${id}']);
  assert.deepStrictEqual(h("x = '<b ' + hosOn.attr('click', 'open(\\'' + id + '\\')') + '>';").map((b) => /without hosArg/.test(b)), [true]);
  assert.deepStrictEqual(h("x = `<b ${hosOn.attr('click', `open(${hosNum(i)}, ${hosArg(k)})`)}>`;"), [null]);
  assert.deepStrictEqual(h("x = '<b ' + hosOn.attr('click', h) + '>'; // handler-exempt"), [null], 'a marked builder line is not judged');
  assert.deepStrictEqual(attrHandlers("x = '<b ' + hosOn.attr('click', 'open(' + hosArg(i) + ')') + '>';")[0].text, 'open("H")');
  assert.deepStrictEqual(handlerProblems('t', [{ type: 'clik', text: 'go()', line: 1 }]).length, 1);
  assert.deepStrictEqual(handlerProblems('t', [{ type: 'click', text: 'go(', line: 1 }]).length, 1);
  // Check 4: safe-markup.js present, in the head, and first.
  const page = (head) => '<html><head>' + head + '</head><body><b data-on-click="go()">x</b></body></html>';
  assert.strictEqual(loadOrderProblems('p', page('<script src="/shared/safe-markup.js"></script><script src="/x.js"></script>')).length, 0);
  assert.strictEqual(loadOrderProblems('p', page('<script src="/x.js"></script><script src="/shared/safe-markup.js"></script>')).length, 1);
  assert.strictEqual(loadOrderProblems('p', page('<script type="application/ld+json">{}</script><script src="/shared/safe-markup.js"></script>')).length, 0);
  assert.strictEqual(loadOrderProblems('p', page('')).length, 1);
  // Check 7: a handler that starts at the browser's names.
  assert.strictEqual(builtinRootProblems('t', [{ text: "console.warn('x')", line: 1 }]).length, 1);
  assert.strictEqual(builtinRootProblems('t', [{ text: "hosOn.byId('x').click(); go(Math)", line: 1 }]).length, 1);
  assert.strictEqual(builtinRootProblems('t', [{ text: "hosOn.byId('x').click()", line: 1 }]).length, 0);
});
