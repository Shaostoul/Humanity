#!/usr/bin/env node
/**
 * a11y-web.js: static accessibility checks of the website (the WCAG 2.1 AA audit,
 * docs/design/wcag-audit-2026-10-23.md). Used by scripts/tests/a11y-web.test.js.
 *
 *  - NAMES (WCAG 4.1.2): every control (scripts/i18n-web.js's list: button, link, input,
 *    select, textarea, summary, a control role, an onclick, a tabindex) has an accessible
 *    name: its own text, aria-label / aria-labelledby, title, an <img alt> inside, a
 *    <label for> or a wrapping <label>, a button-type input's value, or (the browser's last
 *    resort) a placeholder.
 *  - KEYBOARD (2.1.1): an element made clickable with onclick that is not itself a control
 *    (a div, a span, a row) has a role and a tabindex, or a keyboard user cannot reach it.
 *  - FOCUS ORDER (2.4.3): no tabindex above 0 anywhere (it pulls an element out of reading order).
 *  - LANGUAGE (3.1.1) and TITLE (2.4.2): every page has <html lang> and a <title>.
 *  - ZOOM (1.4.4): no viewport that stops pinch zoom (user-scalable=no, maximum-scale=1).
 *  - IMAGES (1.1.1): every <img> has an alt (empty for a picture that is decoration).
 *
 * Usage: node scripts/a11y-web.js [--list <file>...]
 */
'use strict';

const fs = require('fs');
const path = require('path');
const W = require('./i18n-web.js');

/** Problems in one piece of markup: [{line, rule, what}]. */
function markupProblems(src, baseLine = 1, opts = {}) {
  const out = [];
  const labelsFor = new Set();
  const re = /<label[^>]*\sfor\s*=\s*["']([^"']+)["']/gi;
  let m;
  while ((m = re.exec(src)) !== null) labelsFor.add(m[1]);
  const open = []; // controls waiting for their text: {el, text, line, named}
  W.walkMarkup(src, (ev) => {
    if (ev.kind === 'open') {
      const a = ev.attrs;
      const tab = a.tabindex !== undefined ? parseInt(a.tabindex, 10) : null;
      if (tab !== null && tab > 0) out.push({ line: ev.line, rule: 'focus-order', what: '<' + ev.name + ' tabindex="' + a.tabindex + '">' });
      if (ev.name === 'img' && a.alt === undefined && !ev.stack.some((e) => e.attrs['aria-hidden'] === 'true')) out.push({ line: ev.line, rule: 'img-alt', what: '<img src="' + (a.src || '') + '">' });
      if (ev.name === 'meta' && (a.name || '').toLowerCase() === 'viewport' && /user-scalable\s*=\s*(no|0)|maximum-scale\s*=\s*1(\.0)?\b/i.test(a.content || '')) {
        out.push({ line: ev.line, rule: 'zoom', what: a.content });
      }
      const isCtl = isControl(ev.name, a);
      // A dialog's own backdrop closes it on a click outside the box: a mouse shortcut,
      // where the keyboard has Escape and the close button.
      const backdrop = a.role === 'dialog' || /overlay|backdrop/i.test((a.id || '') + ' ' + (a.class || ''));
      if (a.onclick !== undefined && !backdrop && !['button', 'a', 'input', 'select', 'textarea', 'summary', 'label', 'option'].includes(ev.name) && !(a.role && a.tabindex !== undefined)) {
        out.push({ line: ev.line, rule: 'keyboard', what: '<' + ev.name + (a.id ? ' id="' + a.id + '"' : '') + ' onclick> without role and tabindex' });
      }
      if (!isCtl || a['aria-hidden'] === 'true' || (ev.name === 'input' && (a.type || '').toLowerCase() === 'hidden')) {
        // An <img alt> inside an open control names it.
        if (ev.name === 'img' && a.alt) for (const o of open) o.named = true;
        return;
      }
      let named = !!(a['aria-label'] || a['aria-labelledby'] || a['data-i18n-aria-label'] || a.title || a['data-i18n-title'] || a['data-native-title']);
      if (ev.name === 'input' || ev.name === 'select' || ev.name === 'textarea') {
        if (a.id && labelsFor.has(a.id)) named = true;
        if (ev.stack.some((e) => e.name === 'label')) named = true;
        if (ev.name === 'input' && ['button', 'submit', 'reset'].includes((a.type || '').toLowerCase()) && a.value) named = true;
        if (ev.name === 'input' && (a.type || '').toLowerCase() === 'image' && a.alt) named = true;
        if (a.placeholder || a['data-i18n-placeholder']) named = true;
        if (!named) out.push({ line: ev.line, rule: 'name', what: '<' + ev.name + (a.id ? ' id="' + a.id + '"' : '') + (a.type ? ' type="' + a.type + '"' : '') + '>' });
        if (ev.name === 'select' || ev.name === 'textarea') open.push({ el: ev.el, line: ev.line, named: true, skip: true });
        return;
      }
      open.push({ el: ev.el, line: ev.line, named, text: '', name: ev.name, id: a.id });
    } else if (ev.kind === 'text') {
      const t = W.decodeEntities(ev.text).trim();
      if (t) for (const o of open) o.named = true;
    } else if (ev.kind === 'close') {
      const i = open.findIndex((o) => o.el === ev.el);
      if (i >= 0) {
        const o = open.splice(i, 1)[0];
        if (!o.named && !o.skip && !opts.textFilled) out.push({ line: o.line, rule: 'name', what: '<' + o.name + (o.id ? ' id="' + o.id + '"' : '') + '> with no words, label or title' });
      }
    }
  }, baseLine);
  return out;
}

function isControl(name, a) {
  if (['button', 'select', 'textarea', 'summary', 'input'].includes(name)) return true;
  if (name === 'a' && a.href !== undefined) return true;
  if (a.role && /^(button|link|tab|checkbox|switch|menuitem|option|radio|slider|combobox|textbox)$/.test(a.role)) return true;
  return false;
}

/** Problems in a page file (HTML, its inline scripts' markup strings) or a script. */
function fileProblems(file) {
  const src = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
  if (!file.endsWith('.html')) {
    // Script: markup strings only. A control whose text comes from an expression
    // (`'<button>' + label + '</button>'`) has its words filled in at run time, so a
    // `{}` placeholder inside counts as words.
    const out = [];
    for (const chain of W.stringChains(W.jsTokens(src)).chains) {
      if (!/<\/?[a-zA-Z]/.test(chain.text)) continue;
      for (const p of markupProblems(chain.text.replace(/\{\}/g, 'X'), chain.line)) out.push(p);
    }
    return out;
  }
  const out = markupProblems(src, 1);
  if (!/<html[^>]*\slang\s*=\s*["']?[a-zA-Z]/i.test(src)) out.push({ line: 1, rule: 'lang', what: '<html> has no lang' });
  if (!/<title[^>]*>[^<]+<\/title>/i.test(src) && /<html/i.test(src)) out.push({ line: 1, rule: 'title', what: 'no <title>' });
  return out;
}

function counts(webDir) {
  const files = W.webFiles(webDir);
  const htmlPages = new Set(files.map((f) => W.relWeb(f, webDir)).map((r) => /^web\/pages\/([^/]+)\.html$/.exec(r)).filter(Boolean).map((m) => m[1]));
  const out = {};
  for (const f of files) {
    const ps = fileProblems(f);
    if (!ps.length) continue;
    const page = W.webPageOf(W.relWeb(f, webDir), htmlPages);
    for (const p of ps) {
      const k = page + ' ' + p.rule;
      out[k] = (out[k] || 0) + 1;
    }
  }
  return out;
}

if (require.main === module) {
  const args = process.argv.slice(2);
  if (args[0] === '--list') {
    for (const f of args.slice(1)) for (const p of fileProblems(path.resolve(f))) console.log(f + ':' + p.line + ' [' + p.rule + '] ' + p.what);
  } else {
    const c = counts(W.WEB);
    for (const [k, n] of Object.entries(c).sort()) console.log(String(n).padStart(4) + ' ' + k);
  }
}

module.exports = { markupProblems, fileProblems, counts };
