// User text never reaches innerHTML unescaped (2026-11-20, the website's script policy,
// docs/PRIORITIES.md item 6b, after an outside review flagged plain-JS XSS risk).
//
// Run: node --test scripts/tests/inner-html-web.test.js
//
// The site's Content-Security-Policy now refuses inline script, so markup injected into a page can
// no longer run code; it can still draw a fake button, a fake link or a fake form. So every value
// written into markup (innerHTML =, innerHTML +=, outerHTML =, insertAdjacentHTML) goes through
// the house escape, hosEsc (web/shared/safe-markup.js), or a page's own esc/escapeHtml/escHtml
// helper that does the same; better still, the code sets textContent.
//
// What it proves:
//  1. THE RATCHET: per script under web/, the number of markup writes holding a value that is not
//     plainly safe (escaped, a literal, a number, a hosIcon picture, a word from en.json) is at
//     most its budget in scripts/tests/fixtures/inner-html-web-budget.txt, and no fewer (lock a
//     win in with INNER_HTML_RATCHET_WRITE=1, which only lowers). A script not listed has a budget
//     of zero, so new code escapes from the start. INNER_HTML_REPORT=<file> lists a file's writes.
//     The clean-up is large (on 2026-11-20, 241 of the 486 markup writes in 56 of 67 files hold
//     such a value, most of them markup built further up from values escaped there, which this
//     reader cannot follow), so it is a ratchet, not a ban.
//  2. Every page helper the ratchet counts as escaping does escape & < > and ", the characters
//     that end text or a double-quoted attribute.
//  3. The reader itself: what it counts and what it lets through.
//
// SEEN RED (2026-11-20): with no budget file, check 1 listed all 56 files; check 2 named 18 page
// helpers as they were: shell.js's esc() (only & and <), nine that read textContent back as
// markup (quotes left alone), equip_esc, log_esc, quest_esc and three more with no quotes,
// vault_escAttr (no < or >), and timestampPill.js's fallback, which escaped nothing. Each now
// escapes all five characters.

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const BUDGET = path.join(__dirname, 'fixtures', 'inner-html-web-budget.txt');
const VENDOR = /[\\/]vendor[\\/]|qrcode\.js$/;

// Calls whose result is safe to write into markup: the escapes, and producers of known markup.
const ESCAPES = ['hosEsc', 'esc', 'escHtml', 'escapeHtml', 'equip_esc', 'log_esc', 'quest_esc', 'vault_esc', 'vault_escAttr'];
const PRODUCERS = ['hosIcon', 'hosArg', 'hosNum', 'i18n.html', 'Number', 'parseInt', 'parseFloat', 'Math.round', 'Math.floor', 'Math.ceil', 'Math.min', 'Math.max', 'Math.abs'];

function walk(dir, out = []) {
  for (const f of fs.readdirSync(dir)) {
    const p = path.join(dir, f);
    if (fs.statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.js') && !VENDOR.test(p)) out.push(p);
  }
  return out;
}
const rel = (p) => 'web/' + path.relative(WEB, p).replace(/\\/g, '/');

// ── A small reader of JavaScript expressions ─────────────────────────────
const REGEX_BEFORE = '(,=:[!&|?{};+-*%<>~^';
/** Index just past the string, template or regex literal starting at src[i]. */
function skipLiteral(src, i) {
  const q = src[i];
  if (q === '`') {
    i++;
    while (i < src.length && src[i] !== '`') {
      if (src[i] === '\\') { i += 2; continue; }
      if (src[i] === '$' && src[i + 1] === '{') { i = skipBalanced(src, i + 1); continue; }
      i++;
    }
    return i + 1;
  }
  if (q === '/') {
    let cls = false;
    i++;
    while (i < src.length && src[i] !== '\n') {
      if (src[i] === '\\') { i += 2; continue; }
      if (src[i] === '[') cls = true; else if (src[i] === ']') cls = false;
      else if (src[i] === '/' && !cls) break;
      i++;
    }
    i++;
    while (/[a-z]/i.test(src[i] || '')) i++;
    return i;
  }
  i++;
  while (i < src.length && src[i] !== q && src[i] !== '\n') { if (src[i] === '\\') i++; i++; }
  return i + 1;
}
function prevSignificant(src, i) {
  let j = i - 1;
  while (j >= 0 && /\s/.test(src[j])) j--;
  return j >= 0 ? src[j] : '';
}
function isRegexStart(src, i) {
  if (src[i] !== '/' || src[i + 1] === '/' || src[i + 1] === '*') return false;
  const p = prevSignificant(src, i);
  return p === '' || REGEX_BEFORE.includes(p) || /\b(return|typeof|in|of)$/.test(src.slice(Math.max(0, i - 8), i).trimEnd());
}
/** From an opening bracket at src[i], the index just past its match. */
function skipBalanced(src, i) {
  const open = src[i];
  const close = open === '(' ? ')' : open === '[' ? ']' : '}';
  let depth = 0;
  for (; i < src.length; i++) {
    const c = src[i];
    if (c === '"' || c === "'" || c === '`' || isRegexStart(src, i)) { i = skipLiteral(src, i) - 1; continue; }
    if (c === '/' && src[i + 1] === '/') { while (i < src.length && src[i] !== '\n') i++; continue; }
    if (c === '/' && src[i + 1] === '*') { i = src.indexOf('*/', i + 2) + 1; continue; }
    if (c === open) depth++;
    else if (c === close) { depth--; if (depth === 0) return i + 1; }
  }
  return src.length;
}
/**
 * The expression starting at src[i]: up to a ";" or a "," or ")" at depth 0 (when `inCall`), or a
 * line end that does not continue the expression.
 */
function readExpr(src, i, inCall) {
  const start = i;
  for (; i < src.length; i++) {
    const c = src[i];
    if (c === '"' || c === "'" || c === '`' || isRegexStart(src, i)) { i = skipLiteral(src, i) - 1; continue; }
    if (c === '/' && src[i + 1] === '/') { while (i < src.length && src[i] !== '\n') i++; i--; continue; }
    if (c === '/' && src[i + 1] === '*') { i = src.indexOf('*/', i + 2) + 1; continue; }
    if (c === '(' || c === '[' || c === '{') { i = skipBalanced(src, i) - 1; continue; }
    if (c === ';' || c === '}' || (inCall && (c === ')' || c === ','))) break;
    if (c === ')' || c === ']') break;
    if (c === '\n') {
      const before = src.slice(start, i).trimEnd();
      const after = src.slice(i).trimStart();
      if (!/[+\-*/%(\[{,?:=|&!]$/.test(before) && !/^[+.?:)\]}|&,]/.test(after)) break;
    }
  }
  return src.slice(start, i).trim();
}

/** Split an expression at its top-level occurrences of the characters in `seps`. */
function splitTop(e, sep) {
  const parts = [];
  let last = 0;
  for (let i = 0; i < e.length; i++) {
    const c = e[i];
    if (c === '"' || c === "'" || c === '`' || isRegexStart(e, i)) { i = skipLiteral(e, i) - 1; continue; }
    if (c === '(' || c === '[' || c === '{') { i = skipBalanced(e, i) - 1; continue; }
    if (e.startsWith(sep, i) && !(sep === '+' && (e[i + 1] === '+' || e[i + 1] === '='))) { parts.push(e.slice(last, i)); last = i + sep.length; i += sep.length - 1; }
  }
  parts.push(e.slice(last));
  return parts.map((p) => p.trim());
}
function wholeCall(e, names) {
  for (const n of names) {
    if (!e.startsWith(n + '(')) continue;
    if (skipBalanced(e, n.length) === e.length) return n;
  }
  return null;
}

/** The values in `e` that are not plainly safe to write into markup. */
function unsafeValues(e, depth = 0) {
  e = e.trim();
  if (!e || depth > 12) return [];
  while (e.startsWith('(') && skipBalanced(e, 0) === e.length) e = e.slice(1, -1).trim();
  // a ? b : c: the branches are what gets written
  const q = splitTop(e, '?');
  if (q.length >= 2) {
    const rest = q.slice(1).join('?');
    const branches = splitTop(rest, ':');
    if (branches.length >= 2) return [].concat(...branches.map((b) => unsafeValues(b, depth + 1)));
  }
  for (const op of ['||', '&&', '??']) {
    const parts = splitTop(e, op);
    if (parts.length > 1) return [].concat(...parts.map((p) => unsafeValues(p, depth + 1)));
  }
  const plus = splitTop(e, '+');
  if (plus.length > 1) return [].concat(...plus.map((p) => unsafeValues(p, depth + 1)));
  if (/^(['"])[\s\S]*\1$/.test(e) && skipLiteral(e, 0) === e.length) return [];
  if (e[0] === '`' && skipLiteral(e, 0) === e.length) {
    const out = [];
    for (let i = 1; i < e.length - 1; i++) {
      if (e[i] === '\\') { i++; continue; }
      if (e[i] === '$' && e[i + 1] === '{') {
        const end = skipBalanced(e, i + 1);
        out.push(...unsafeValues(e.slice(i + 2, end - 1), depth + 1));
        i = end - 1;
      }
    }
    return out;
  }
  if (/^-?\d+(\.\d+)?$/.test(e) || /^(true|false|null|undefined)$/.test(e)) return [];
  if (wholeCall(e, ESCAPES) || wholeCall(e, PRODUCERS)) return [];
  // A word from en.json, with no values put into it (i18n.html escapes the ones it is given).
  for (const t of ['i18n.t', 't', 'T']) {
    if (wholeCall(e, [t]) && splitTop(e.slice(t.length + 1, -1), ',').length === 1) return [];
  }
  if (/\.(length|toFixed\([^)]*\)|toLocaleString\([^)]*\))$/.test(e)) return [];
  return [e.length > 80 ? e.slice(0, 77) + '...' : e];
}

/** Each markup write in a script: {line, expr, unsafe}. */
function markupWrites(src) {
  const out = [];
  const re = /\.(innerHTML|outerHTML)\s*(\+?=)(?!=)|\.insertAdjacentHTML\s*\(/g;
  let m;
  while ((m = re.exec(src))) {
    let at = m.index + m[0].length;
    let expr;
    if (m[0].startsWith('.insertAdjacentHTML')) {
      const first = readExpr(src, at, true);
      at = src.indexOf(',', at + first.length) + 1;
      expr = readExpr(src, at, true);
    } else {
      expr = readExpr(src, at, false);
    }
    out.push({ line: src.slice(0, m.index).split('\n').length, expr, unsafe: unsafeValues(expr) });
  }
  return out;
}

function counts() {
  const out = {};
  for (const f of walk(WEB)) {
    const n = markupWrites(fs.readFileSync(f, 'utf8')).filter((w) => w.unsafe.length).length;
    if (n) out[rel(f)] = n;
  }
  return out;
}
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
function writeBudget(m) {
  const files = Object.keys(m).filter((k) => m[k] > 0).sort();
  const total = files.reduce((s, k) => s + m[k], 0);
  const head = [
    '# The innerHTML RATCHET\'s budgets: per script under web/, how many markup writes (innerHTML,',
    '# outerHTML, insertAdjacentHTML) hold a value scripts/tests/inner-html-web.test.js cannot see is',
    '# escaped. Its header says how to lower these. The numbers only go DOWN; a file not listed has a',
    '# budget of zero.',
    '# Total: ' + total + ' in ' + files.length + ' files',
  ];
  fs.writeFileSync(BUDGET, head.concat(files.map((k) => m[k] + ' ' + k)).join('\n') + '\n');
}

test('RATCHET: no script writes more unescaped values into markup than its budget, nor fewer', () => {
  const now = counts();
  const budget = readBudget();
  if (process.env.INNER_HTML_REPORT) {
    const f = path.join(ROOT, process.env.INNER_HTML_REPORT);
    for (const w of markupWrites(fs.readFileSync(f, 'utf8'))) if (w.unsafe.length) console.log(process.env.INNER_HTML_REPORT + ':' + w.line + '  ' + w.unsafe.join(' | '));
  }
  if (process.env.INNER_HTML_RATCHET_WRITE === 'init') { writeBudget(now); return; }
  const over = [], under = [];
  for (const [f, n] of Object.entries(now)) if (n > (budget[f] || 0)) over.push(f + ': ' + n + ', budget ' + (budget[f] || 0));
  for (const [f, b] of Object.entries(budget)) if ((now[f] || 0) < b) under.push(f + ': ' + (now[f] || 0) + ', budget ' + b);
  if (process.env.INNER_HTML_RATCHET_WRITE === '1' && under.length && !over.length) {
    const m = { ...budget };
    for (const f of Object.keys(m)) m[f] = Math.min(m[f], now[f] || 0);
    writeBudget(m);
    return;
  }
  assert.deepStrictEqual(over, [], '\nMARKUP WRITES with a value not plainly escaped, past the file\'s budget\n' +
    '(INNER_HTML_REPORT=<file> lists them). Escape each value with hosEsc() or the page\'s esc(), or set\n' +
    'textContent; never raise the budget:\n' + over.join('\n'));
  assert.deepStrictEqual(under, [], '\nFewer than the budget: lock the win in with INNER_HTML_RATCHET_WRITE=1:\n' + under.join('\n'));
});

// ── 2. the page helpers ──────────────────────────────────────────────────
/** Each definition of a helper named in ESCAPES: {file, line, name, body}. */
function helperDefinitions() {
  const out = [];
  for (const f of walk(WEB)) {
    const src = fs.readFileSync(f, 'utf8');
    const re = new RegExp('(?:function\\s+(' + ESCAPES.join('|') + ')\\s*\\(|(?:const|let|var)\\s+(' + ESCAPES.join('|') + ')\\s*=)', 'g');
    let m;
    while ((m = re.exec(src))) {
      const brace = src.indexOf('{', m.index);
      const arrow = src.indexOf('=>', m.index);
      const lineEnd = src.indexOf('\n', m.index);
      // An arrow with an expression body ends at its line; a block body at its brace.
      const body = arrow > 0 && arrow < lineEnd && (brace < 0 || brace > arrow + 3 || src[arrow + 2] !== ' ' || src[arrow + 3] !== '{')
        ? src.slice(m.index, lineEnd)
        : src.slice(m.index, skipBalanced(src, brace));
      out.push({ file: rel(f), line: src.slice(0, m.index).split('\n').length, name: m[1] || m[2], body });
    }
  }
  return out;
}
function escapesEnough(body) {
  if (/\bhosEsc\(/.test(body)) return true;
  // Text set as textContent and read back as innerHTML comes back with & < > escaped (the
  // browser's serializer does that), but quotes left alone: so it must add &quot; itself.
  if (/\.textContent\s*=/.test(body) && /\.innerHTML\b/.test(body)) return body.includes('&quot;');
  return ['&amp;', '&lt;', '&gt;', '&quot;'].every((ent) => body.includes(ent));
}

test('every page helper counted as an escape escapes & < > and "', () => {
  const defs = helperDefinitions();
  assert.ok(defs.length >= 20, 'the helpers are found (' + defs.length + ')');
  const weak = defs.filter((d) => !escapesEnough(d.body)).map((d) => d.file + ':' + d.line + ' ' + d.name);
  assert.deepStrictEqual(weak, [], '\nThese leave a character that ends text or an attribute; make each escape & < > " (and \'), or call hosEsc:\n' + weak.join('\n'));
});

// ── 3. the reader ────────────────────────────────────────────────────────
test('the reader counts a raw value and lets escaped, literal and numeric ones through', () => {
  const u = (s) => markupWrites(s).map((w) => w.unsafe);
  assert.deepStrictEqual(u("el.innerHTML = '<b>' + name + '</b>';"), [['name']]);
  assert.deepStrictEqual(u('el.innerHTML = `<b>${msg.text}</b>`;'), [['msg.text']]);
  assert.deepStrictEqual(u("el.innerHTML = '<b>' + esc(name) + '</b>';"), [[]]);
  assert.deepStrictEqual(u('el.innerHTML = `<b>${hosEsc(n)}</b> ${items.length} ${hosIcon(\'x\', 12)}`;'), [[]]);
  assert.deepStrictEqual(u("el.innerHTML = ok ? '<i>yes</i>' : '<i>' + why + '</i>';"), [['why']]);
  assert.deepStrictEqual(u("el.insertAdjacentHTML('beforeend', '<p>' + text + '</p>');"), [['text']]);
  assert.deepStrictEqual(u("el.innerHTML = i18n.t('a.b');\nnext();"), [[]]);
  assert.deepStrictEqual(u("el.innerHTML = i18n.t('a.b', { name: who });"), [["i18n.t('a.b', { name: who })"]]);
  assert.deepStrictEqual(u("el.innerHTML = '<p>' + s.replace(/'/g, '') + '</p>';"), [["s.replace(/'/g, '')"]]);
  assert.deepStrictEqual(u("if (el.innerHTML === '') go();"), [], 'a comparison is not a write');
  assert.ok(escapesEnough("function esc(s){return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/\"/g,'&quot;');}"));
  assert.ok(!escapesEnough("function esc(s){return s.replace(/&/g,'&amp;').replace(/</g,'&lt;');}"));
  assert.ok(!escapesEnough('function esc(s){const d=document.createElement("div");d.textContent=s;return d.innerHTML;}'));
  assert.ok(escapesEnough("function esc(s){const d=document.createElement('div');d.textContent=s;return d.innerHTML.replace(/\"/g, '&quot;');}"));
});
