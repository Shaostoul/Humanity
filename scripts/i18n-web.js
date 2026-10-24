#!/usr/bin/env node
/**
 * i18n-web.js: the website's side of "ready for translation" (the ELI5 mandate,
 * docs/PRIORITIES.md TIER 3, operator 2026-10-22). The native side is
 * tests/i18n_lint.rs; this is its twin for web/, used by
 * scripts/tests/i18n-web.test.js and by hand.
 *
 * What it knows how to do:
 *
 *  - COUNT the words a web file still writes into the page instead of asking
 *    data/i18n/en.json by key (`literals`): in HTML, text between tags and the
 *    word-carrying attributes (title, aria-label, placeholder, alt,
 *    data-tip-desc, ...) of any element not marked data-i18n / data-i18n-<attr>;
 *    in script, the words assigned to a page sink (textContent, innerHTML,
 *    title, placeholder, ...), passed to one (alert, confirm, setAttribute of a
 *    word attribute, a toast), or written as markup in a string
 *    ('<button>Send</button>'). A ratchet over these counts lives in the test.
 *    Skipped: <script> data blocks, <style>, <svg>, <code>, <pre>, <kbd>, any
 *    element with translate="no" (the HTML standard's own "do not translate"),
 *    and a script line holding `i18n-exempt`.
 *  - LIST the keys a file asks for (`keysUsed`), so the test can fail on a key
 *    en.json lacks.
 *  - KEEP STATIC HTML EQUAL TO en.json (`htmlSync`): a static page keeps its
 *    English in the markup (it reads with or without script), and this checks,
 *    or with write rewrites, the content of every data-i18n element and the
 *    attribute behind every data-i18n-<attr> from en.json, glossary marks as
 *    <span data-term> elements.
 *
 * Usage:
 *   node scripts/i18n-web.js --list web/pages/notes.html   # the words still in a file
 *   node scripts/i18n-web.js --counts                     # per-file counts
 *   node scripts/i18n-web.js --html-check                 # static HTML vs en.json
 *   node scripts/i18n-web.js --html-write                 # rewrite it from en.json
 */
'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const EN_PATH = path.join(ROOT, 'data', 'i18n', 'en.json');

const EXEMPT = 'i18n-exempt';

// ── Words ─────────────────────────────────────────────────────────────────

/** A stylesheet or rule block: a selector, braces, declarations. */
function isCss(s) {
  return /[\w\])*:-]\s*\{[^{}]*:[^{}]*;?\s*\}/.test(s) && !/<[a-zA-Z][\w-]*[\s>]/.test(String(s).slice(0, 40));
}

/** Does a string hold words a person reads? Two letters in a row once
 *  placeholders are out, not a URL, not an identifier, not CSS. */
function isWords(s) {
  let t = String(s)
    .replace(/\$\{[^}]*\}/g, ' ')
    .replace(/\{\{?\w*\}?\}/g, ' ')
    .replace(/&[a-z]+;|&#\d+;|&#x[0-9a-f]+;/gi, ' ')
    .trim();
  if (!t) return false;
  if (/:\/\//.test(t) || /^www\./.test(t) || /^mailto:/.test(t)) return false;
  // A stylesheet ('.a { color: red; }').
  if (isCss(t)) return false;
  // Identifier-like: no spaces, lower-case with a separator (nav.home, chat_input,
  // send-status, /api/x, #id).
  if (!/\s/.test(t) && /^[a-z0-9_.:\/#\-\[\]=*>+~"'()]+$/.test(t) && /[_.:\/#\[\-]/.test(t)) return false;
  // CSS declarations ('color:red;margin:0') and values ('1px solid', 'rgba(...)').
  if (/^([a-z-]+\s*:\s*[^;]+;?\s*)+$/i.test(t) && !/\s[a-z]+\s[a-z]+\s/i.test(t.replace(/:[^;]*/g, ''))) return false;
  if (/^[\d.\s]+(px|em|rem|%|vh|vw|ms|s|deg)?(\s+[\w#().,%-]+)*$/.test(t) && !/[a-z]{3,}\s[a-z]{3,}/i.test(t)) return false;
  return /\p{L}{2}/u.test(t) || /[\u3000-\u9fff]/.test(t);
}

// ── HTML ──────────────────────────────────────────────────────────────────

const VOID = new Set(['area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'param', 'source', 'track', 'wbr']);
const SKIP_TEXT = new Set(['style', 'svg', 'code', 'pre', 'kbd', 'math']);
/** Attributes whose value a person reads. */
const WORD_ATTRS = ['title', 'aria-label', 'placeholder', 'alt', 'data-tip-desc', 'data-tip-title', 'data-tip', 'aria-description', 'label'];
/** data-i18n-<x> -> the attribute it fills. */
const I18N_ATTRS = {
  'data-i18n-title': 'title',
  'data-i18n-placeholder': 'placeholder',
  'data-i18n-aria-label': 'aria-label',
  'data-i18n-alt': 'alt',
  'data-i18n-tip': 'data-tip-desc',
  'data-i18n-tip-title': 'data-tip-title',
  // A <meta name="description" content="..."> (what a search result shows).
  'data-i18n-content': 'content',
};

function decodeEntities(s) {
  return String(s)
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(+d))
    .replace(/&#x([0-9a-f]+);/gi, (_, h) => String.fromCodePoint(parseInt(h, 16)));
}

function escapeHtml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

/** Parse the attributes of a start tag's inside (`a href="x" disabled`). */
function parseAttrs(s) {
  const out = {};
  const re = /([^\s=/>"']+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+)))?/g;
  let m;
  while ((m = re.exec(s)) !== null) {
    const v = m[2] !== undefined ? m[2] : m[3] !== undefined ? m[3] : m[4] !== undefined ? m[4] : '';
    out[m[1].toLowerCase()] = v;
  }
  return out;
}

/**
 * Walk markup and call back for each piece: tags (with the stack of open
 * elements), text runs and script bodies. `src` may be a whole page or a
 * fragment from a script string. Line numbers are 1-based from `baseLine`.
 */
function walkMarkup(src, cb, baseLine = 1) {
  const stack = [];
  let i = 0;
  const n = src.length;
  const lineAt = (pos) => baseLine + (src.slice(0, pos).match(/\n/g) || []).length;
  while (i < n) {
    const lt = src.indexOf('<', i);
    const textEnd = lt < 0 ? n : lt;
    if (textEnd > i) cb({ kind: 'text', text: src.slice(i, textEnd), pos: i, line: lineAt(i), stack });
    if (lt < 0) break;
    if (src.startsWith('<!--', lt)) {
      const e = src.indexOf('-->', lt + 4);
      i = e < 0 ? n : e + 3;
      continue;
    }
    if (src.startsWith('<!', lt) || src.startsWith('<?', lt)) {
      const e = src.indexOf('>', lt);
      i = e < 0 ? n : e + 1;
      continue;
    }
    const close = /^<\/([a-zA-Z][\w-]*)\s*>/.exec(src.slice(lt, lt + 200));
    if (close) {
      const name = close[1].toLowerCase();
      for (let k = stack.length - 1; k >= 0; k--) {
        if (stack[k].name === name) {
          const el = stack[k];
          stack.length = k;
          cb({ kind: 'close', name, el, pos: lt, end: lt + close[0].length, line: lineAt(lt), stack });
          break;
        }
      }
      i = lt + close[0].length;
      continue;
    }
    const open = /^<([a-zA-Z][\w-]*)/.exec(src.slice(lt, lt + 100));
    if (!open) {
      // A bare '<' in text ("a < b"): it is text.
      cb({ kind: 'text', text: '<', pos: lt, line: lineAt(lt), stack });
      i = lt + 1;
      continue;
    }
    // Find the end of the start tag, respecting quotes.
    let j = lt + open[0].length;
    let q = null;
    while (j < n) {
      const c = src[j];
      if (q) {
        if (c === q) q = null;
      } else if (c === '"' || c === "'") {
        q = c;
      } else if (c === '>') {
        break;
      }
      j++;
    }
    const name = open[1].toLowerCase();
    const inner = src.slice(lt + open[0].length, j);
    const attrs = parseAttrs(inner.replace(/\/\s*$/, ''));
    const el = { name, attrs, pos: lt, contentStart: j + 1, line: lineAt(lt) };
    cb({ kind: 'open', name, el, attrs, pos: lt, end: j + 1, line: el.line, stack });
    i = j + 1;
    if (name === 'script' || name === 'style' || name === 'textarea' || name === 'title') {
      const endRe = new RegExp('</' + name + '\\s*>', 'i');
      const rest = src.slice(i);
      const m = endRe.exec(rest);
      const bodyEnd = m ? i + m.index : n;
      const body = src.slice(i, bodyEnd);
      if (name === 'script') cb({ kind: 'script', el, body, pos: i, line: lineAt(i), stack });
      else if (name === 'title' || name === 'textarea') cb({ kind: 'text', text: body, pos: i, line: lineAt(i), stack: stack.concat([el]) });
      el.contentEnd = bodyEnd;
      cb({ kind: 'close', name, el, pos: bodyEnd, end: m ? bodyEnd + m[0].length : n, line: lineAt(bodyEnd), stack });
      i = m ? bodyEnd + m[0].length : n;
      continue;
    }
    if (!VOID.has(name) && !/\/\s*$/.test(inner)) stack.push(el);
  }
}

/** Is a node under an element whose words are not counted? */
function shielded(stack, forText) {
  for (const el of stack) {
    if (el.attrs.translate === 'no') return true;
    if (SKIP_TEXT.has(el.name)) return true;
    if (forText && el.attrs['data-i18n'] !== undefined) return true;
  }
  return false;
}

/** The words a piece of markup writes in: text runs and word attributes. */
function markupWords(src, baseLine = 1, opts = {}) {
  const found = [];
  walkMarkup(src, (ev) => {
    if (ev.kind === 'text') {
      if (shielded(ev.stack, true)) return;
      const text = decodeEntities(ev.text).replace(/\s+/g, ' ').trim();
      if (isWords(text)) found.push({ line: ev.line, text, kind: 'text' });
    } else if (ev.kind === 'open') {
      if (shielded(ev.stack, false) || ev.attrs.translate === 'no') return;
      const covered = new Set(Object.keys(I18N_ATTRS).filter((a) => ev.attrs[a] !== undefined).map((a) => I18N_ATTRS[a]));
      for (const a of WORD_ATTRS) {
        if (ev.attrs[a] === undefined || covered.has(a)) continue;
        const v = decodeEntities(ev.attrs[a]);
        if (isWords(v)) found.push({ line: ev.line, text: v, kind: '@' + a });
      }
      const type = (ev.attrs.type || '').toLowerCase();
      if (ev.name === 'input' && ['button', 'submit', 'reset'].includes(type) && ev.attrs.value && isWords(ev.attrs.value)) {
        found.push({ line: ev.line, text: decodeEntities(ev.attrs.value), kind: '@value' });
      }
      if (ev.name === 'meta' && !covered.has('content') && /^(description|og:description|og:title|twitter:title|twitter:description)$/.test(ev.attrs.name || ev.attrs.property || '') && isWords(ev.attrs.content || '')) {
        found.push({ line: ev.line, text: ev.attrs.content, kind: '@content' });
      }
    } else if (ev.kind === 'script' && opts.scripts) {
      const type = (ev.el.attrs.type || '').toLowerCase();
      if (ev.el.attrs.src || (type && !/javascript|module/.test(type))) return;
      for (const f of jsLiterals(ev.body, ev.line)) found.push(f);
    }
  }, baseLine);
  return found;
}

// ── Script ────────────────────────────────────────────────────────────────

/**
 * Tokenize JavaScript well enough to find strings and the calls and
 * assignments around them. Template literals give a `str` token for their
 * static text (`${...}` becomes '{}') and their expressions are tokenized in
 * place. Regex literals are recognised after an operator or keyword.
 */
function jsTokens(src, baseLine = 1) {
  const toks = [];
  let i = 0;
  let line = baseLine;
  const n = src.length;
  const tmplStack = []; // [brace depth at which each open `${` resumes, its template id]
  let braceDepth = 0;
  let tmplCount = 0;
  let curTmpl = 0;
  const prevSignificant = () => toks[toks.length - 1];
  const regexAllowed = () => {
    const p = prevSignificant();
    if (!p) return true;
    if (p.type === 'punct') return !/^[)\]}]$/.test(p.value);
    if (p.type === 'id') return /^(return|typeof|case|do|else|in|of|new|delete|void|throw|yield|await)$/.test(p.value);
    return false;
  };
  function readTemplate() {
    // i is just past a '`' or a '}' that closes a ${...}.
    const startLine = line;
    let s = '';
    while (i < n) {
      const c = src[i];
      if (c === '\\') { s += src[i + 1] || ''; i += 2; continue; }
      if (c === '`') { i++; toks.push({ type: 'str', value: s, line: startLine, tmpl: curTmpl }); return; }
      if (c === '$' && src[i + 1] === '{') {
        s += '{}';
        toks.push({ type: 'str', value: s, line: startLine, tmpl: curTmpl, partial: true });
        s = '';
        i += 2;
        tmplStack.push([braceDepth, curTmpl]);
        braceDepth++;
        return;
      }
      if (c === '\n') line++;
      s += c;
      i++;
    }
    toks.push({ type: 'str', value: s, line: startLine, tmpl: curTmpl });
  }
  while (i < n) {
    const c = src[i];
    if (c === '\n') { line++; i++; continue; }
    if (/\s/.test(c)) { i++; continue; }
    if (c === '/' && src[i + 1] === '/') { while (i < n && src[i] !== '\n') i++; continue; }
    if (c === '/' && src[i + 1] === '*') {
      const e = src.indexOf('*/', i + 2);
      const end = e < 0 ? n : e + 2;
      line += (src.slice(i, end).match(/\n/g) || []).length;
      i = end;
      continue;
    }
    if (c === '"' || c === "'") {
      const startLine = line;
      let s = '';
      i++;
      while (i < n && src[i] !== c && src[i] !== '\n') {
        if (src[i] === '\\') {
          const e = src[i + 1];
          if (e === 'n') s += '\n';
          else if (e === 'u') {
            const m = /^u\{([0-9a-f]+)\}|^u([0-9a-f]{4})/i.exec(src.slice(i + 1, i + 9));
            if (m) { s += String.fromCodePoint(parseInt(m[1] || m[2], 16)); i += 1 + m[0].length; continue; }
          } else if (e === '\n') { line++; }
          else s += e;
          i += 2;
          continue;
        }
        s += src[i];
        i++;
      }
      i++;
      toks.push({ type: 'str', value: s, line: startLine });
      continue;
    }
    if (c === '`') { i++; curTmpl = ++tmplCount; readTemplate(); continue; }
    if (c === '/' && regexAllowed()) {
      // A regex literal: skip to its unescaped closing slash (classes may hold '/').
      let j = i + 1;
      let inClass = false;
      while (j < n && src[j] !== '\n') {
        if (src[j] === '\\') { j += 2; continue; }
        if (src[j] === '[') inClass = true;
        else if (src[j] === ']') inClass = false;
        else if (src[j] === '/' && !inClass) break;
        j++;
      }
      j++;
      while (j < n && /[a-z]/i.test(src[j])) j++;
      toks.push({ type: 'regex', value: src.slice(i, j), line });
      i = j;
      continue;
    }
    if (/[A-Za-z_$]/.test(c)) {
      let j = i + 1;
      while (j < n && /[\w$]/.test(src[j])) j++;
      toks.push({ type: 'id', value: src.slice(i, j), line });
      i = j;
      continue;
    }
    if (/[0-9]/.test(c)) {
      let j = i + 1;
      while (j < n && /[\w.]/.test(src[j])) j++;
      toks.push({ type: 'num', value: src.slice(i, j), line });
      i = j;
      continue;
    }
    if (c === '{') { braceDepth++; }
    if (c === '}') {
      braceDepth--;
      if (tmplStack.length && tmplStack[tmplStack.length - 1][0] === braceDepth) {
        curTmpl = tmplStack.pop()[1];
        i++;
        readTemplate();
        continue;
      }
    }
    // Multi-char operators that matter: =, +=, ==, ===, =>.
    const three = src.slice(i, i + 3);
    const two = src.slice(i, i + 2);
    if (three === '===' || three === '!==') { toks.push({ type: 'punct', value: three, line }); i += 3; continue; }
    if (['==', '!=', '+=', '=>', '&&', '||', '??', '?.', '<=', '>='].includes(two)) { toks.push({ type: 'punct', value: two, line }); i += 2; continue; }
    toks.push({ type: 'punct', value: c, line });
    i++;
  }
  return toks;
}

/** Properties whose value a person reads. */
const SINK_PROPS = new Set(['textContent', 'innerText', 'innerHTML', 'outerHTML', 'title', 'placeholder', 'alt', 'label', 'tipDesc', 'tipTitle', 'ariaLabel']);
const MARKUP_PROPS = new Set(['innerHTML', 'outerHTML']);
/** Calls whose string arguments a person reads. */
const SINK_CALLS = /^(alert|confirm|prompt|createTextNode|hosToast|showToast|toast|notify|showStatus|setStatus|flash|hosConfirm|holdToConfirm|showError|showNotice|announce)$/;
const ATTR_SETTERS = new Set(['setAttribute']);
const WORD_ATTR_SET = new Set(WORD_ATTRS.concat(['data-tip-desc', 'data-tip-title']));
/** Object keys whose string value is shown ({ label: 'Save' }). */
const UI_KEYS = new Set(['text', 'label', 'title', 'placeholder', 'tooltip', 'tip', 'hint', 'desc', 'description', 'message', 'caption', 'heading', 'subtitle', 'ariaLabel', 'confirmText', 'cancelText', 'empty', 'emptyText']);

function hasMarkup(s) {
  if (isCss(s) && !/^\s*</.test(s)) return false;
  return /<\/?[a-zA-Z][\w-]*[\s>/]/.test(s) || /<[a-zA-Z][\w-]*$/.test(s);
}

/**
 * Strings read in CHAINS, the way the page receives them: a template literal's
 * pieces (split at each ${...}) are one string, and 'a' + x + 'b' joins into
 * "a{}b", so '<a href="' + url + '" title="Go">' reads as one tag. Returns
 * { chains: [{ks, text, line}], chainOf: Map(token index -> chain) }.
 */
function stringChains(toks) {
  // Units first: a plain string, or a whole template (first..last piece).
  const units = [];
  const unitAt = new Map(); // token index of a unit's FIRST token -> unit
  const tmplUnit = new Map();
  toks.forEach((t, k) => {
    if (t.type !== 'str') return;
    if (t.tmpl) {
      let u = tmplUnit.get(t.tmpl);
      if (!u) {
        u = { ks: [], start: k, end: k, text: '' };
        tmplUnit.set(t.tmpl, u);
        units.push(u);
        unitAt.set(k, u);
      }
      u.ks.push(k);
      u.end = k;
      u.text += t.value;
    } else {
      const u = { ks: [k], start: k, end: k, text: t.value };
      units.push(u);
      unitAt.set(k, u);
    }
  });
  // Skip one operand from token j: balanced tokens up to a + or an end at depth 0.
  const skipOperand = (j) => {
    let depth = 0;
    while (j < toks.length) {
      const v = toks[j].value;
      if (toks[j].type === 'punct') {
        if (v === '(' || v === '[' || v === '{') depth++;
        else if (v === ')' || v === ']' || v === '}') { if (depth === 0) return j; depth--; }
        else if (depth === 0 && ['+', ',', ';', ':', '?', '||', '&&', '??', '='].includes(v)) return j;
      }
      j++;
    }
    return j;
  };
  const chainOf = new Map();
  const chains = [];
  for (const u of units) {
    if (chainOf.has(u.start)) continue;
    const chain = { ks: [], text: '', line: toks[u.start].line, start: u.start };
    if (u.start > 0 && toks[u.start - 1].value === '+') chain.text += '{}';
    let cur = u;
    for (;;) {
      chain.ks.push(...cur.ks);
      chain.text += cur.text;
      let j = cur.end + 1;
      let next = null;
      while (toks[j] && toks[j].value === '+') {
        j++;
        if (unitAt.has(j)) { next = unitAt.get(j); break; }
        chain.text += '{}';
        j = skipOperand(j);
      }
      if (!next) break;
      cur = next;
    }
    for (const k of chain.ks) chainOf.set(k, chain);
    chains.push(chain);
  }
  return { chains, chainOf };
}

/** The words a script writes into the page (see the header). */
function jsLiterals(src, baseLine = 1) {
  const toks = jsTokens(src, baseLine);
  const lines = src.split('\n');
  const exemptLine = (ln) => (lines[ln - baseLine] || '').includes(EXEMPT);
  const found = [];
  const counted = new Set();
  const take = (k, t, kind) => {
    if (counted.has(k) || exemptLine(t.line)) return;
    counted.add(k);
    found.push({ line: t.line, text: t.value, kind });
  };
  // 1. Markup inside strings: its text runs and word attributes, read in
  //    chains (see `stringChains`).
  const { chains, chainOf } = stringChains(toks);
  for (const chain of chains) {
    if (exemptLine(chain.line) || !hasMarkup(chain.text)) continue;
    for (const j of chain.ks) counted.add(j);
    for (const w of markupWords(chain.text.replace(/\{\}/g, ' '), chain.line)) found.push({ line: w.line, text: w.text, kind: 'markup' + (w.kind === 'text' ? '' : w.kind) });
  }
  const chainIsCss = (k) => { const c = chainOf.get(k); return !!c && isCss(c.text.replace(/\{\}/g, '0')); };
  // Where a statement or argument ends, from token k (exclusive bound).
  const exprEnd = (k) => {
    let depth = 0;
    for (let j = k; j < toks.length; j++) {
      const v = toks[j].value;
      if (toks[j].type === 'punct') {
        if (v === '(' || v === '[' || v === '{') depth++;
        else if (v === ')' || v === ']' || v === '}') {
          if (depth === 0) return j;
          depth--;
        } else if ((v === ';' || v === ',') && depth === 0) return j;
      }
      // A new statement on the next line at depth 0 (no operator between).
      if (j > k && depth === 0 && toks[j].line > toks[j - 1].line) {
        const prev = toks[j - 1];
        const contOps = ['+', '?', ':', '||', '&&', '??', '(', '=', ',', '.'];
        if (!(prev.type === 'punct' && contOps.includes(prev.value)) && !(toks[j].type === 'punct' && ['+', '?', ':', '||', '&&', '??', '.', ')'].includes(toks[j].value))) return j;
      }
    }
    return toks.length;
  };
  // Calls a word passes THROUGH on its way to the page (escaping, joining).
  const THROUGH = /^(esc|escapeHtml|escHtml|escapeHTML|html|String|concat|join|map|trim|toUpperCase|toLowerCase|padStart|padEnd)$/;
  const COMPARE = new Set(['===', '==', '!==', '!=']);
  /** Is the string at j an argument of a call (inside the range a..j) that is
   *  not a pass-through, such as hosIcon('lock') or classList.add('on')? */
  const insideOtherCall = (a, j) => {
    let depth = 0;
    for (let k = j - 1; k >= a; k--) {
      const v = toks[k].value;
      if (toks[k].type !== 'punct') continue;
      if (v === ')' || v === ']' || v === '}') depth++;
      else if (v === '(' || v === '[' || v === '{') {
        if (depth > 0) { depth--; continue; }
        if (v !== '(') continue;
        const callee = toks[k - 1];
        if (!callee || callee.type !== 'id' || /^(if|for|while|switch|catch|function|return|typeof|await)$/.test(callee.value)) continue;
        if (THROUGH.test(callee.value)) continue;
        return true;
      }
    }
    return false;
  };
  const takeRange = (a, b, kind) => {
    for (let j = a; j < b; j++) {
      const t = toks[j];
      if (t.type !== 'str' || !isWords(t.value) || chainIsCss(j)) continue;
      // A key call inside the range: its first argument is a key, not words.
      if (j >= 2 && toks[j - 1].value === '(' && toks[j - 2].type === 'id' && /^(t|tn|tMarked|hint|has|fill)$/.test(toks[j - 2].value)) continue;
      // Compared, a case label, or an object key: a value the code tests, not words shown.
      if ((toks[j - 1] && COMPARE.has(toks[j - 1].value)) || (toks[j + 1] && COMPARE.has(toks[j + 1].value))) continue;
      if (toks[j - 1] && toks[j - 1].value === 'case') continue;
      if (toks[j + 1] && toks[j + 1].value === ':' && toks[j - 1] && (toks[j - 1].value === '{' || toks[j - 1].value === ',')) continue;
      if (insideOtherCall(a, j)) continue;
      take(j, t, kind);
    }
  };
  for (let k = 0; k < toks.length; k++) {
    const t = toks[k];
    // 2. Assignment sinks: el.textContent = '...'
    if (t.type === 'id' && SINK_PROPS.has(t.value) && k > 0 && toks[k - 1].value === '.' && toks[k + 1] && (toks[k + 1].value === '=' || toks[k + 1].value === '+=')) {
      takeRange(k + 2, exprEnd(k + 2), MARKUP_PROPS.has(t.value) ? 'html' : t.value);
      continue;
    }
    // 3. Call sinks: alert('...'), setAttribute('title', '...')
    if (t.type === 'id' && toks[k + 1] && toks[k + 1].value === '(') {
      if (SINK_CALLS.test(t.value)) {
        takeRange(k + 2, exprEnd(k + 2), t.value + '()');
        continue;
      }
      if (ATTR_SETTERS.has(t.value) && toks[k + 2] && toks[k + 2].type === 'str' && WORD_ATTR_SET.has(toks[k + 2].value) && toks[k + 3] && toks[k + 3].value === ',') {
        takeRange(k + 4, exprEnd(k + 4), '@' + toks[k + 2].value);
        continue;
      }
      if (t.value === 'insertAdjacentHTML') {
        const a = exprEnd(k + 2);
        takeRange(a + 1, exprEnd(a + 1), 'html');
        continue;
      }
    }
    // 4. Object keys that are shown: { label: 'Save' }
    if ((t.type === 'id' || t.type === 'str') && UI_KEYS.has(t.value) && toks[k + 1] && toks[k + 1].value === ':' && toks[k - 1] && (toks[k - 1].value === '{' || toks[k - 1].value === ',')) {
      takeRange(k + 2, exprEnd(k + 2), t.value + ':');
    }
  }
  found.sort((a, b) => a.line - b.line);
  return found;
}

/** Every literal key a script or page asks en.json for. Comments are skipped
 *  (a doc example is not a request). HTML: data-i18n* attributes outside
 *  comments, plus its inline scripts; script: i18n.t / tn / tMarked ('key'),
 *  i18n.fill / hint (el, 'key'), and data-i18n* attributes in markup strings. */
function keysUsed(src, isHtml) {
  const out = [];
  const attrRe = /data-i18n(?:-[a-z-]+)?\s*=\s*["']([^"'{}]+)["']/g;
  const fromScript = (js, base) => {
    const toks = jsTokens(js, base);
    for (let k = 0; k < toks.length; k++) {
      const t = toks[k];
      if (t.type === 'str') {
        let m;
        attrRe.lastIndex = 0;
        while ((m = attrRe.exec(t.value)) !== null) out.push({ line: t.line, key: m[1], call: 'attr' });
        continue;
      }
      if (t.type !== 'id' || t.value !== 'i18n' || !toks[k + 1] || toks[k + 1].value !== '.') continue;
      const fn = toks[k + 2] && toks[k + 2].value;
      if (!/^(t|tn|tMarked|fill|hint|html)$/.test(fn || '') || !toks[k + 3] || toks[k + 3].value !== '(') continue;
      // fill(el, key) and hint(el, key) take the element first; the others the key.
      let a = k + 4;
      if (fn === 'fill' || fn === 'hint') {
        let depth = 0;
        while (a < toks.length) {
          const v = toks[a].value;
          if (v === '(' || v === '[' || v === '{') depth++;
          else if (v === ')' || v === ']' || v === '}') { if (depth === 0) break; depth--; }
          else if (v === ',' && depth === 0) { a++; break; }
          a++;
        }
      }
      // A key built at run time ('calendar.month_' + n) is not a literal key: skip it.
      const built = toks[a + 1] && toks[a + 1].value === '+';
      if (toks[a] && toks[a].type === 'str' && !toks[a].tmpl && !built) out.push({ line: toks[a].line, key: toks[a].value, call: fn });
    }
  };
  if (!isHtml) {
    fromScript(src, 1);
    return out;
  }
  walkMarkup(src, (ev) => {
    if (ev.kind === 'open') {
      for (const [a, v] of Object.entries(ev.attrs)) {
        if ((a === 'data-i18n' || a in I18N_ATTRS) && v) out.push({ line: ev.line, key: v, call: 'attr' });
      }
    } else if (ev.kind === 'script') {
      const type = (ev.el.attrs.type || '').toLowerCase();
      if (!ev.el.attrs.src && (!type || /javascript|module/.test(type))) fromScript(ev.body, ev.line);
    }
  });
  return out;
}

// ── Hints: a plain-language tooltip on every control (phase 2) ─────────────
//
// A CONTROL is an element a person can press, type into or choose from:
// <button>, <a href>, <input> (not hidden), <select>, <textarea>, <summary>, an
// element with a control role (button, link, tab, checkbox, switch, menuitem,
// option, radio, slider, ...), an onclick, or a tabindex of 0 or more. In
// script, the same tags in markup strings, and document.createElement of a
// control tag. It HAS A HINT when it carries data-tip-desc or data-i18n-tip
// (or, made in script, gets one later in the file: el.dataset.tipDesc,
// setAttribute('data-tip-desc' | 'data-i18n-tip', ..), i18n.hint(el, ..)).
// data-no-tip on it or around it marks a control that explains itself (the
// shell then shows no hint); it counts as hinted. web/shared/shell.js shows
// the hint on hover and on keyboard focus, through the person's hint mode.

const CONTROL_TAGS = new Set(['button', 'input', 'select', 'textarea', 'summary']);
const CONTROL_ROLES = new Set(['button', 'link', 'tab', 'checkbox', 'switch', 'menuitem', 'menuitemcheckbox', 'menuitemradio', 'option', 'radio', 'slider', 'spinbutton', 'combobox', 'textbox', 'treeitem']);

function isControl(name, attrs) {
  if (name === 'input' && (attrs.type || '').toLowerCase() === 'hidden') return false;
  if (CONTROL_TAGS.has(name)) return true;
  if (name === 'a' && attrs.href !== undefined) return true;
  if (attrs.role && CONTROL_ROLES.has(attrs.role.toLowerCase())) return true;
  if (attrs.onclick !== undefined) return true;
  if (attrs.contenteditable !== undefined && attrs.contenteditable !== 'false') return true;
  if (attrs.tabindex !== undefined && parseInt(attrs.tabindex, 10) >= 0) return true;
  return false;
}

function hasHintAttr(attrs) {
  return attrs['data-tip-desc'] !== undefined || attrs['data-i18n-tip'] !== undefined || attrs['data-no-tip'] !== undefined;
}

/** The controls in a piece of markup: [{line, name, hinted, label}]. */
function markupControls(src, baseLine = 1, opts = {}) {
  const out = [];
  walkMarkup(src, (ev) => {
    if (ev.kind === 'open') {
      if (!isControl(ev.name, ev.attrs)) return;
      // data-no-tip around it (explains itself), or a Settings row: its
      // description line is the hint, shown by the hint mode as on the desktop
      // app's Settings page (shell.js copies it onto the row's controls).
      const shieldedByNoTip = ev.stack.some((el) => el.attrs['data-no-tip'] !== undefined || /(^|\s)setting-row(\s|$)/.test(el.attrs.class || ''));
      const label = ev.attrs['aria-label'] || ev.attrs.title || ev.attrs['data-tip-title'] || ev.attrs.placeholder || ev.attrs.id || ev.name;
      out.push({ line: ev.line, name: ev.name, hinted: hasHintAttr(ev.attrs) || shieldedByNoTip, label });
    } else if (ev.kind === 'script' && opts.scripts) {
      const type = (ev.el.attrs.type || '').toLowerCase();
      if (ev.el.attrs.src || (type && !/javascript|module/.test(type))) return;
      for (const c of jsControls(ev.body, ev.line)) out.push(c);
    }
  }, baseLine);
  return out;
}

/** The controls a script makes: in markup strings, and by createElement. */
function jsControls(src, baseLine = 1) {
  const toks = jsTokens(src, baseLine);
  const lines = src.split('\n');
  const exemptLine = (ln) => (lines[ln - baseLine] || '').includes('hint-exempt');
  const out = [];
  for (const chain of stringChains(toks).chains) {
    if (exemptLine(chain.line) || !hasMarkup(chain.text)) continue;
    for (const c of markupControls(chain.text.replace(/\{\}/g, ' '), chain.line)) out.push(c);
  }
  for (let k = 0; k + 3 < toks.length; k++) {
    if (toks[k].value !== 'createElement' || toks[k + 1].value !== '(' || toks[k + 2].type !== 'str') continue;
    const tag = toks[k + 2].value.toLowerCase();
    if (!CONTROL_TAGS.has(tag) && tag !== 'a') continue;
    if (exemptLine(toks[k].line)) continue;
    // The variable it is put in: `NAME = document.createElement(...)`.
    let name = null;
    let j = k - 1;
    while (j >= 0 && (toks[j].value === '.' || toks[j].value === 'document' || toks[j].value === 'doc' || toks[j].value === 'ownerDocument')) j--;
    if (j >= 1 && toks[j].value === '=' && toks[j - 1].type === 'id') name = toks[j - 1].value;
    let hinted = false;
    if (name) {
      const rest = toks.slice(k + 3);
      for (let r = 0; r + 4 < rest.length && !hinted; r++) {
        const a = rest[r];
        if (a.type === 'id' && a.value === name && rest[r + 1].value === '.') {
          const m = rest[r + 2].value;
          if (m === 'dataset' && rest[r + 3].value === '.' && /^(tipDesc|i18nTip|noTip)$/.test(rest[r + 4].value)) hinted = true;
          if (m === 'setAttribute' && rest[r + 3].value === '(' && rest[r + 4].type === 'str' && /^data-(tip-desc|i18n-tip|no-tip)$/.test(rest[r + 4].value)) hinted = true;
        }
        if (a.value === 'hint' && r >= 2 && rest[r - 1].value === '.' && rest[r + 1].value === '(' && rest[r + 2].value === name) hinted = true;
      }
    }
    out.push({ line: toks[k].line, name: tag, hinted, label: name || tag });
  }
  out.sort((a, b) => a.line - b.line);
  return out;
}

function controlsOf(file) {
  const src = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
  return file.endsWith('.html') ? markupControls(src, 1, { scripts: true }) : jsControls(src);
}

/** The page a web file belongs to: an HTML page and the scripts named after it
 *  (pages/notes.html + pages/notes-app.js), the chat, the front door, or a
 *  shared file on its own. */
function webPageOf(rel, htmlPages) {
  const m = /^web\/pages\/([^/]+)\.(html|js)$/.exec(rel);
  if (m) {
    const base = m[1];
    if (htmlPages.has(base)) return 'page ' + base;
    // The longest page name the script is named after: files-panes.js -> files.
    let best = null;
    for (const p of htmlPages) if (base.startsWith(p + '-') && (!best || p.length > best.length)) best = p;
    return 'page ' + (best || base);
  }
  if (rel.startsWith('web/chat/')) return 'page chat';
  if (rel.startsWith('web/home/')) return 'page home';
  return rel;
}

function hintCounts(webDir = WEB) {
  const files = webFiles(webDir);
  const htmlPages = new Set(files.map((f) => relWeb(f, webDir)).map((r) => /^web\/pages\/([^/]+)\.html$/.exec(r)).filter(Boolean).map((m) => m[1]));
  const out = {};
  for (const f of files) {
    const cs = controlsOf(f);
    if (!cs.length) continue;
    const page = webPageOf(relWeb(f, webDir), htmlPages);
    if (!out[page]) out[page] = { total: 0, missing: 0 };
    out[page].total += cs.length;
    out[page].missing += cs.filter((c) => !c.hinted).length;
  }
  return out;
}

// ── Files ─────────────────────────────────────────────────────────────────

function walk(dir, out) {
  let entries = [];
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return out; }
  entries.sort((a, b) => a.name.localeCompare(b.name));
  for (const e of entries) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (/\.(html|js)$/.test(e.name)) out.push(p);
  }
  return out;
}

/** The web files the ratchet watches: everything but vendored, minified and generated code. */
function webFiles(webDir = WEB) {
  return walk(webDir, []).filter((p) => {
    const r = p.replace(/\\/g, '/');
    return !r.includes('/vendor/') && !/\.min\.js$/.test(r) && !r.endsWith('/shared/i18n-en.js') && !r.endsWith('/chat/bip39-english.js');
  });
}

function relWeb(p, webDir = WEB) {
  return 'web/' + path.relative(webDir, p).replace(/\\/g, '/');
}

function literalsOf(file) {
  const src = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
  return file.endsWith('.html') ? markupWords(src, 1, { scripts: true }) : jsLiterals(src);
}

function counts(webDir = WEB) {
  const out = {};
  for (const f of webFiles(webDir)) out[relWeb(f, webDir)] = literalsOf(f).length;
  return out;
}

// ── Keeping static HTML equal to en.json ──────────────────────────────────

function flatten(obj, prefix = '', out = {}) {
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? prefix + '.' + k : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) flatten(v, key, out);
    else if (typeof v === 'string') out[key] = v;
  }
  return out;
}

function segments(marked) {
  const out = [];
  let rest = String(marked);
  for (;;) {
    const open = rest.indexOf('[[');
    if (open < 0) break;
    const close = rest.indexOf(']]', open + 2);
    if (close < 0) break;
    const inner = rest.slice(open + 2, close);
    if (open > 0) out.push({ text: rest.slice(0, open) });
    const bar = inner.indexOf('|');
    out.push(bar >= 0 ? { shown: inner.slice(0, bar), term: inner.slice(bar + 1) } : { shown: inner, term: inner });
    rest = rest.slice(close + 2);
  }
  if (rest) out.push({ text: rest });
  return out;
}

function plain(marked) {
  return segments(marked).map((s) => (s.text !== undefined ? s.text : s.shown)).join('');
}

/** The markup a data-i18n element holds for a marked string. */
function renderMarked(marked) {
  return segments(marked)
    .map((s) => (s.text !== undefined ? escapeHtml(s.text) : '<span data-term="' + escapeHtml(s.term) + '">' + escapeHtml(s.shown) + '</span>'))
    .join('');
}

const norm = (s) => String(s).replace(/\s+/g, ' ').trim();

/**
 * Check (and with write, fix) a page's static English against en.json.
 * Returns { problems: [{line, key, want, have}], out: the rewritten page }.
 */
function htmlSync(src, en, write) {
  const problems = [];
  const edits = []; // [start, end, replacement], applied back to front
  walkMarkup(src, (ev) => {
    if (ev.kind === 'open') {
      // Every attribute fix of one tag is made to the SAME copy of the tag and
      // written back once: two separate edits of one tag each started from the
      // original text, and the second undid or garbled the first (found
      // 2026-10-23 on tasks.html, a placeholder and a hint on one input).
      // Replacements are functions, so a `$` in the words is never a pattern.
      let tag = src.slice(ev.pos, ev.end);
      let changed = false;
      for (const [a, target] of Object.entries(I18N_ATTRS)) {
        const key = ev.attrs[a];
        if (key === undefined) continue;
        if (!(key in en)) { problems.push({ line: ev.line, key, want: '(missing from en.json)', have: '' }); continue; }
        const want = plain(en[key]);
        const have = ev.attrs[target];
        if (have === undefined || norm(decodeEntities(have)) !== norm(want)) {
          problems.push({ line: ev.line, key, want, have: have === undefined ? '(no ' + target + ')' : have });
          if (write) {
            const value = '"' + escapeHtml(want) + '"';
            const re = new RegExp('(\\s' + target.replace(/-/g, '\\-') + '\\s*=\\s*)("[^"]*"|\'[^\']*\'|[^\\s"\'>]+)');
            tag = re.test(tag)
              ? tag.replace(re, (_m, lead) => lead + value)
              : tag.replace(/(\s*\/?>)$/, (end) => ' ' + target + '=' + value + end);
            changed = true;
          }
        }
      }
      if (changed) edits.push([ev.pos, ev.end, tag]);
    } else if (ev.kind === 'close' && ev.el.attrs['data-i18n'] !== undefined) {
      const key = ev.el.attrs['data-i18n'];
      const start = ev.el.contentStart;
      const end = ev.pos;
      const have = src.slice(start, end);
      if (!(key in en)) { problems.push({ line: ev.el.line, key, want: '(missing from en.json)', have }); return; }
      const want = renderMarked(en[key]);
      if (norm(have) !== norm(want)) {
        problems.push({ line: ev.el.line, key, want, have });
        if (write) edits.push([start, end, want]);
      }
    }
  });
  let out = src;
  edits.sort((a, b) => b[0] - a[0]);
  for (const [s, e, r] of edits) out = out.slice(0, s) + r + out.slice(e);
  return { problems, out };
}

function loadEn() {
  return flatten(JSON.parse(fs.readFileSync(EN_PATH, 'utf8')));
}

// ── CLI ───────────────────────────────────────────────────────────────────

function main() {
  const args = process.argv.slice(2);
  if (args[0] === '--list') {
    for (const f of args.slice(1)) {
      for (const w of literalsOf(path.resolve(f))) console.log(f + ':' + w.line + ': [' + w.kind + '] ' + JSON.stringify(w.text));
    }
    return;
  }
  if (args[0] === '--counts') {
    const c = counts();
    let total = 0;
    for (const [f, n] of Object.entries(c).sort((a, b) => b[1] - a[1])) { if (n) console.log(String(n).padStart(5) + ' ' + f); total += n; }
    console.log('total ' + total);
    return;
  }
  if (args[0] === '--html-check' || args[0] === '--html-write') {
    const write = args[0] === '--html-write';
    const en = loadEn();
    const files = args.length > 1 ? args.slice(1).map((f) => path.resolve(f)) : webFiles().filter((f) => f.endsWith('.html'));
    let bad = 0;
    for (const f of files) {
      const src = fs.readFileSync(f, 'utf8');
      const { problems, out } = htmlSync(src.replace(/\r\n/g, '\n'), en, write);
      for (const p of problems) {
        bad++;
        console.log(relWeb(f) + ':' + p.line + ' ' + p.key + (write ? ' (rewritten)' : '') + '\n   en.json: ' + JSON.stringify(p.want) + '\n   page:    ' + JSON.stringify(norm(p.have)));
      }
      if (write && problems.length) fs.writeFileSync(f, out);
    }
    if (!write && bad) process.exit(1);
    console.log(bad ? bad + (write ? ' rewritten' : ' differ') : 'static HTML matches en.json');
    return;
  }
  console.log('usage: node scripts/i18n-web.js --list <file>... | --counts | --html-check | --html-write [files]');
}

if (require.main === module) main();

module.exports = {
  isWords, walkMarkup, markupWords, jsTokens, jsLiterals, keysUsed, webFiles, relWeb, literalsOf, counts,
  markupControls, jsControls, controlsOf, webPageOf, hintCounts, stringChains,
  flatten, segments, plain, renderMarked, htmlSync, loadEn, parseAttrs, decodeEntities, escapeHtml,
  WEB, EN_PATH, WORD_ATTRS, I18N_ATTRS,
};
