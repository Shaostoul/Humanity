// The website's event handlers, read from source (2026-11-20, BUG-331): shared by
// scripts/tests/inline-script-web.test.js and scripts/tests/safe-markup-trust-web.test.js. Not a
// test itself (its name does not end in .test.js).
//
// A handler is written one of two ways (web/shared/safe-markup.js):
//  - in a page's static HTML: data-on-click="showSection('profile')";
//  - in markup a script builds: hosOn.attr('click', 'openNote(' + hosArg(id) + ')'), whose text
//    may take data only through hosArg(...) (a string) and hosNum(...) (a number).
// pageHandlers() reads the first kind, attrHandlers() the second, each as {type, text, line, bad}
// with hosArg values standing as the string "H" and hosNum values as 0.

const vm = require('node:vm');

const lineOf = (src, idx) => src.slice(0, idx).split('\n').length;

function decodeEntities(s) {
  return s.replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&apos;/g, "'")
    .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');
}

/** Each data-on-* attribute in a page's HTML: {type, text, line}. */
function pageHandlers(src) {
  const out = [];
  const re = /\sdata-on-([a-z]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/g;
  let m;
  while ((m = re.exec(src))) out.push({ type: m[1], text: decodeEntities(m[2] !== undefined ? m[2] : m[3]), line: lineOf(src, m.index), bad: null });
  return out;
}

// ── A small reader of JS source: strings, templates, comments, brackets ──
/** Index just past the string literal (quote q) that starts at i. */
function skipString(src, i) {
  const q = src[i];
  i++;
  while (i < src.length && src[i] !== q) { if (src[i] === '\\') i++; i++; }
  return i + 1;
}
/** Index just past the template literal that starts at i, its ${...} parts included. */
function skipTemplate(src, i) {
  i++;
  while (i < src.length && src[i] !== '`') {
    if (src[i] === '\\') { i += 2; continue; }
    if (src[i] === '$' && src[i + 1] === '{') { i = skipCode(src, i + 2, '}'); continue; }
    i++;
  }
  return i + 1;
}
/** Index just past the `closer` that ends the code starting at i (brackets balanced). */
function skipCode(src, i, closer) {
  const stack = [closer];
  while (i < src.length) {
    const c = src[i];
    if (c === "'" || c === '"') { i = skipString(src, i); continue; }
    if (c === '`') { i = skipTemplate(src, i); continue; }
    if (c === '/' && src[i + 1] === '/') { while (i < src.length && src[i] !== '\n') i++; continue; }
    if (c === '/' && src[i + 1] === '*') { const e = src.indexOf('*/', i + 2); i = e < 0 ? src.length : e + 2; continue; }
    if (c === '(') stack.push(')');
    else if (c === '[') stack.push(']');
    else if (c === '{') stack.push('}');
    else if (c === stack[stack.length - 1]) { stack.pop(); if (!stack.length) return i + 1; }
    i++;
  }
  return i;
}
/** The top-level comma-separated argument sources of the call whose "(" is at i: {args, end}. */
function callArgs(src, i) {
  const end = skipCode(src, i + 1, ')');
  const body = src.slice(i + 1, end - 1);
  const args = [];
  let start = 0;
  let j = 0;
  while (j < body.length) {
    const c = body[j];
    if (c === "'" || c === '"') { j = skipString(body, j); continue; }
    if (c === '`') { j = skipTemplate(body, j); continue; }
    if (c === '(' ) { j = skipCode(body, j + 1, ')'); continue; }
    if (c === '[') { j = skipCode(body, j + 1, ']'); continue; }
    if (c === '{') { j = skipCode(body, j + 1, '}'); continue; }
    if (c === ',') { args.push(body.slice(start, j)); start = j + 1; }
    j++;
  }
  if (body.slice(start).trim()) args.push(body.slice(start));
  return { args: args.map((a) => a.trim()), end };
}
/** The source with every comment blanked to spaces (strings, templates and line numbers kept). */
function stripComments(src) {
  let out = '';
  let i = 0;
  let prev = '';
  while (i < src.length) {
    const c = src[i];
    if (c === "'" || c === '"') { const e = skipString(src, i); out += src.slice(i, e); i = e; prev = ')'; continue; }
    if (c === '`') { const e = skipTemplate(src, i); out += src.slice(i, e); i = e; prev = ')'; continue; }
    if (c === '/' && (src[i + 1] === '/' || src[i + 1] === '*')) {
      const e = src[i + 1] === '/' ? (src.indexOf('\n', i) < 0 ? src.length : src.indexOf('\n', i)) : (src.indexOf('*/', i + 2) < 0 ? src.length : src.indexOf('*/', i + 2) + 2);
      out += src.slice(i, e).replace(/[^\n]/g, ' ');
      i = e;
      continue;
    }
    // A regular expression literal, so a quote inside one does not start a string.
    if (c === '/' && !/[\w$)\]]/.test(prev)) {
      let j = i + 1;
      let cls = false;
      while (j < src.length && src[j] !== '\n') {
        if (src[j] === '\\') { j += 2; continue; }
        if (src[j] === '[') cls = true; else if (src[j] === ']') cls = false; else if (src[j] === '/' && !cls) break;
        j++;
      }
      if (src[j] === '/') { out += src.slice(i, j + 1); i = j + 1; prev = ')'; continue; }
    }
    if (!/\s/.test(c)) prev = c;
    out += c;
    i++;
  }
  return out;
}

/** The value of a string literal with no interpolation ('..', "..", or `..` without ${). */
function literalValue(lit) { return vm.runInNewContext(lit); }

/**
 * The handler text a hosOn.attr second argument makes, with hosArg(...) standing as "H" and
 * hosNum(...) as 0, or {bad} naming the first part that is neither a written string nor one of
 * those two.
 */
function handlerText(expr) {
  let text = '';
  let i = 0;
  let wantValue = true;
  const value = (s) => { text += s; wantValue = false; };
  while (i < expr.length) {
    const c = expr[i];
    if (/\s/.test(c)) { i++; continue; }
    if (!wantValue) {
      if (c === '+') { wantValue = true; i++; continue; }
      return { bad: 'expected + between the parts of the handler text, at: ' + expr.slice(i, i + 40) };
    }
    if (c === "'" || c === '"') { const e = skipString(expr, i); value(literalValue(expr.slice(i, e))); i = e; continue; }
    if (c === '`') {
      const e = skipTemplate(expr, i);
      const body = expr.slice(i + 1, e - 1);
      let k = 0;
      let raw = '';
      while (k < body.length) {
        if (body[k] === '\\') { raw += body.slice(k, k + 2); k += 2; continue; }
        if (body[k] === '$' && body[k + 1] === '{') {
          const close = skipCode(body, k + 2, '}');
          const inner = body.slice(k + 2, close - 1).trim();
          const part = /^hosArg\(/.test(inner) && skipCode(inner, 7, ')') === inner.length ? '"H"'
            : /^hosNum\(/.test(inner) && skipCode(inner, 7, ')') === inner.length ? '0' : null;
          if (part === null) return { bad: 'a ${...} value not written with hosArg() or hosNum(): ${' + inner + '}' };
          text += literalValue('`' + raw + '`') + part;
          raw = '';
          k = close;
          continue;
        }
        raw += body[k];
        k++;
      }
      value(literalValue('`' + raw + '`'));
      i = e;
      continue;
    }
    const call = /^(hosArg|hosNum)\s*\(/.exec(expr.slice(i));
    if (call) {
      const e = skipCode(expr, i + call[0].length, ')');
      value(call[1] === 'hosArg' ? '"H"' : '0');
      i = e;
      continue;
    }
    return { bad: 'a value joined into the handler text without hosArg() or hosNum(), at: ' + expr.slice(i, i + 40) };
  }
  return { text };
}

/**
 * Each hosOn.attr(type, text) call in a script: {type, text, line, bad}. A line marked
 * `// handler-exempt` (a builder whose every caller passes a constant handler, never data) is
 * read with bad = null and text = null.
 */
function attrHandlers(src) {
  const out = [];
  const code = stripComments(src); // a mention in a comment is not a call
  const re = /\bhosOn\.attr\s*\(/g;
  let m;
  while ((m = re.exec(code))) {
    const line = lineOf(src, m.index);
    const eol = src.indexOf('\n', m.index);
    const lineText = src.slice(src.lastIndexOf('\n', m.index) + 1, eol < 0 ? src.length : eol);
    const { args } = callArgs(code, m.index + m[0].length - 1);
    let type = null;
    try { type = /^['"`][a-z]+['"`]$/.test(args[0]) ? literalValue(args[0]) : null; } catch (e) { type = null; }
    if (/\/\/ handler-exempt\b/.test(lineText)) { out.push({ type, text: null, line, bad: null }); continue; }
    if (args.length !== 2) { out.push({ type, text: null, line, bad: 'hosOn.attr takes (type, text)' }); continue; }
    if (type === null) { out.push({ type, text: null, line, bad: 'the event type is not a written string' }); continue; }
    const r = handlerText(args[1]);
    out.push({ type, text: r.text === undefined ? null : r.text, line, bad: r.bad || null });
  }
  return out;
}

/**
 * Rendered markup with each handler token shown as the text it stands for, as the attribute held
 * it before tokens (for a test that reads what a control does). `hosOn` is the page's own: the
 * one whose hosOn.attr made the tokens.
 */
function showHandlers(html, hosOn) {
  return String(html).replace(/data-on-([a-z]+)="(h[0-9a-f]{32})"/g, (m, type, tok) => {
    const text = hosOn.handlerOf({ getAttribute: () => tok }, type);
    if (text === null) return m;
    return 'data-on-' + type + '="' + text.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;') + '"';
  });
}

module.exports = { pageHandlers, attrHandlers, handlerText, decodeEntities, lineOf, callArgs, stripComments, showHandlers };
