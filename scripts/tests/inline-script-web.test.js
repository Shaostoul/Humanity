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
//     runs a string as code (eval, new Function): the policy refuses all three.
//  3. Every data-on-* handler, in a page or in a script's markup, names an event safe-markup.js
//     handles and parses in its language; in a script, every value put into one goes through
//     hosArg() (a quote in a name then stays inside its string).
//  4. Every page that has data-on-* handlers, or loads a script that writes them, loads
//     /shared/safe-markup.js in its <head>.
//  5. The site's policy itself: no 'unsafe-inline', 'unsafe-eval' or outside host in script-src,
//     in every server block of the nginx config.
//  6. safe-markup.js's language: what it runs and what it refuses.
//
// SEEN RED (2026-11-20): before the move, check 1 listed 25 inline <script> blocks and 497 on*=
// attributes in 15 pages, check 2 listed 135 on*= attributes in 25 scripts and both new Function
// calls (chat-onboarding.js, calculator-app.js), and check 5 failed on all three server blocks.
// HOS_WEB_DIR points checks 1 to 4 at another copy of web/ (used to see each one red).

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const SM = require(path.join(ROOT, 'web', 'shared', 'safe-markup.js'));

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
function decodeEntities(s) {
  return s.replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&apos;/g, "'")
    .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');
}

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

// ── 3. data-on-* handlers ────────────────────────────────────────────────
/** Each data-on-* attribute in a page: {type, text, line}. */
function pageHandlers(src) {
  const out = [];
  const re = /\sdata-on-([a-z]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/g;
  let m;
  while ((m = re.exec(src))) out.push({ type: m[1], text: decodeEntities(m[2] !== undefined ? m[2] : m[3]), line: lineOf(src, m.index) });
  return out;
}

/**
 * Each data-on-* attribute written in a script's strings: {type, text, line, bad}. The value runs
 * from the attribute's opening quote to its closing one. Inside it every interpolation, `${...}`
 * or a string broken by `' + ... + '`, must be hosArg(...) or hosNum(...): `bad` names the first
 * that is not. A line marked `// handler-exempt` (a builder whose every caller passes a constant
 * handler, never data) is skipped.
 */
function scriptHandlers(src) {
  const out = [];
  const re = /data-on-([a-z]+)=(\\?["'])/g;
  let m;
  while ((m = re.exec(src))) {
    const q = m[2].slice(-1);
    const escaped = m[2].length === 2;
    const start = m.index + m[0].length;
    // Find the closing quote of the attribute, skipping over hosArg(...) and ${...} groups.
    let i = start;
    let seg = '';
    let bad = null;
    let closed = false;
    while (i < src.length) {
      if (src.startsWith('hosArg(', i) || src.startsWith('hosNum(', i)) {
        let depth = 0;
        let j = i + 6;
        for (; j < src.length; j++) {
          if (src[j] === '(') depth++;
          else if (src[j] === ')') { depth--; if (depth === 0) break; }
        }
        seg += 'H';
        i = j + 1;
        continue;
      }
      if (escaped ? src.startsWith('\\' + q, i) : src[i] === q) { closed = true; break; }
      if (src[i] === '\n') break;
      seg += src[i];
      i++;
    }
    if (!closed) bad = bad || 'the attribute does not close on its line';
    // The allowed interpolations: ${H} and a string broken around H.
    const rest = seg.replace(/\$\{\s*H\s*\}/g, '0').replace(/(['"`])\s*\+\s*H\s*\+\s*\1/g, '0');
    if (/\$\{/.test(rest)) bad = bad || 'a ${...} value not written with hosArg() or hosNum()';
    else if (/['"`]\s*\+|\+\s*['"`]/.test(rest)) bad = bad || 'a value joined into the string without hosArg() or hosNum()';
    const text = decodeEntities(rest.replace(/\\(['"\\])/g, '$1'));
    const eolAt = src.indexOf('\n', m.index);
    const lineText = src.slice(src.lastIndexOf('\n', m.index) + 1, eolAt < 0 ? src.length : eolAt);
    if (/\/\/ handler-exempt\b/.test(lineText)) continue;
    out.push({ type: m[1], text, line: lineOf(src, m.index), bad });
  }
  return out;
}

function handlerProblems(where, list) {
  const out = [];
  for (const h of list) {
    if (!SM.TYPES.includes(h.type)) out.push(where + ':' + h.line + ': data-on-' + h.type + ' is not an event safe-markup.js handles');
    if (h.bad) { out.push(where + ':' + h.line + ': ' + h.bad); continue; }
    try { SM.parse(h.text); } catch (e) { out.push(where + ':' + h.line + ': ' + e.message); }
  }
  return out;
}

test('every data-on-* handler parses, names a handled event, and takes data only through hosArg()', () => {
  const bad = [];
  for (const p of pages) bad.push(...handlerProblems(rel(p), pageHandlers(fs.readFileSync(p, 'utf8'))));
  for (const p of scripts) {
    if (/safe-markup\.js$/.test(p)) continue;
    bad.push(...handlerProblems(rel(p), scriptHandlers(fs.readFileSync(p, 'utf8'))));
  }
  assert.deepStrictEqual(bad, [], '\nDATA-ON HANDLERS that will not run:\n' + bad.join('\n'));
});

// ── 4. safe-markup.js loaded where it is needed ─────────────────────────
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

test('every page with data-on-* handlers loads /shared/safe-markup.js in its <head>', () => {
  const bad = [];
  for (const p of pages) {
    const src = fs.readFileSync(p, 'utf8');
    const srcs = scriptSrcs(src);
    let needs = /\sdata-on-[a-z]+\s*=/.test(src) ? 'its own markup' : null;
    for (const s of srcs) {
      if (needs) break;
      const f = fileForSrc(s.src);
      if (f && !/safe-markup\.js$/.test(f) && /data-on-[a-z]+=/.test(fs.readFileSync(f, 'utf8'))) needs = s.src;
    }
    if (!needs) continue;
    const sm = srcs.find((s) => s.src === '/shared/safe-markup.js');
    const head = src.search(/<\/head>/i);
    if (!sm) bad.push(rel(p) + ': needs /shared/safe-markup.js (for ' + needs + ') and does not load it');
    else if (head >= 0 && sm.at > head) bad.push(rel(p) + ': loads /shared/safe-markup.js after </head>; load it in the <head>');
  }
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
  const calls = SM.parse("event.stopPropagation(); showSection('a b', -2.5); savePref(\"k\", this.checked); HosTasks.open(this.dataset.id)");
  assert.strictEqual(calls.length, 4);
  assert.deepStrictEqual(SM.globalsOf(calls).sort(), ['HosTasks', 'savePref', 'showSection']);
  // The module evaluates against the global object it was loaded under; put the stand-ins there.
  Object.assign(globalThis, win);
  try {
    SM.run("event.stopPropagation(); showSection('a b', -2.5); savePref(\"k\", this.checked); HosTasks.open(this.dataset.id)", el, ev);
  } finally {
    for (const k of Object.keys(win)) delete globalThis[k];
  }
  assert.deepStrictEqual(seen, [['stop'], ['showSection', 'a b', -2.5], ['savePref', 'k', true], ['open', '7', true]]);
});

test('safe-markup.js refuses anything that is not a call, and hosArg keeps a quote inside its string', () => {
  for (const bad of [
    "x = 1", "if (a) b()", "a() + b()", "fn('unclosed)", "this.value", "eval('1')",
    "Function('x')()", "a.constructor.constructor('x')()", "a()b()", "setTimeout(f, 1)", "fn(() => 1)",
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
  const h = (s) => scriptHandlers(s).map((x) => x.bad);
  assert.deepStrictEqual(h("x = `<b data-on-click=\"open(${hosArg(id)})\">`;"), [null]);
  assert.deepStrictEqual(h("x = '<b data-on-click=\"open(' + hosArg(n.id) + ', 1)\">';"), [null]);
  assert.deepStrictEqual(h("x = `<b data-on-click=\"open('${id}')\">`;"), ['a ${...} value not written with hosArg() or hosNum()']);
  assert.deepStrictEqual(h("x = '<b data-on-click=\"open(\\'' + id + '\\')\">';"), ['a value joined into the string without hosArg() or hosNum()']);
  assert.deepStrictEqual(h("x = `<b data-on-click=\"open(${hosNum(i)}, ${hosArg(k)})\">`;"), [null]);
  assert.deepStrictEqual(h("x = '<b data-on-click=\"' + h + '\">'; // handler-exempt"), [], 'a marked builder line is skipped');
  assert.deepStrictEqual(handlerProblems('t', [{ type: 'clik', text: 'go()', line: 1 }]).length, 1);
  assert.deepStrictEqual(handlerProblems('t', [{ type: 'click', text: 'go(', line: 1 }]).length, 1);
});
