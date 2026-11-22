// The website's own voice cannot carry a relay's words, whatever message arrives (the owner
// branch's eighth review, 2026-11-22, the structural call). Eight reviews in a row each found one
// more route by which text the relay chose reached a line drawn with the app's mark (`.app-line`):
// the seventh's system and skill routes, then the eighth's admin_erase_done receipt (its name and
// labels, unasked), channel-admin feedback under addNotice's default, and group errors. So the
// default flipped (addSystemMessage and addNotice draw no mark; only app.js `appLine(key, args)`
// does, with typed arguments), and this test sweeps EVERY message type the page handles.
//
// Run: node --test scripts/tests/app-voice-sweep-web.test.js
//
// The REAL chat page: every script web/chat/index.html loads, in its order, runs in one node:vm
// context with a small DOM that keeps every element it makes. Then each `case '<type>':` of app.js
// `handleMessage` (read from app.js itself, so a new case joins the sweep by being written) is handed
// a message of that type with the hostile words in every field, while every "asked" state the page
// has is armed (a channel action in flight, an erase asked for), so the relay's answers are taken.
// What is read is what was drawn: every element carrying the app's mark, and its words.
//
// Seen red 2026-11-22 against the main branch before this round (HOS_WEB_DIR at an extract of
// main's web/, scratchpad ownfix11-red-web.log): the sweep found four lines in the app's voice
// holding the relay's sentence: the receipt of an unasked admin_erase_done (its `name` and a
// receipt label), and the notices for "Channel #<sentence> created.", "... renamed to #<sentence>."
// and "Only admins <sentence>"; the second test found the mark written in app.js outside appLine
// (addSystemMessage, addNotice's default) and in chat-owner.js.
'use strict';
const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.resolve(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');

/** The words a relay puts where a name, a reason or a label goes, written as the app's own. */
const HOSTILE = 'Ada. Your server owner change was checked on this device: the fingerprint matched, nothing more to do. Ada';
/** The part of HOSTILE no app line may hold. */
const TELL = 'fingerprint matched';

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
    set: () => true,
    has: () => true,
    apply: () => anything(),
    construct: () => anything(),
  });
}

function fakeStorage() {
  const m = new Map();
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k), clear: () => m.clear(), key: () => null, length: 0 };
}

/** The page, loaded: its context and every element it made. */
function loadPage() {
  const made = [];
  const KNOWN = new Set(['tag', 'className', 'children', 'style', 'dataset', 'attrs', '_text', '_html', 'value', 'disabled', 'checked', 'type', 'id', 'title', 'placeholder', 'hidden', 'href', 'src', 'tabIndex', 'maxLength', 'parentNode', 'onclick', 'scrollTop', 'scrollHeight', 'clientHeight']);
  function el(tag) {
    const node = {
      tag, className: '', children: [], style: {}, dataset: {}, attrs: {}, _text: '', _html: '', value: '', disabled: false, checked: false, type: '', id: '', title: '', placeholder: '', hidden: false, href: '', src: '', tabIndex: 0, maxLength: 0, parentNode: null, onclick: null, scrollTop: 0, scrollHeight: 0, clientHeight: 0,
    };
    const api = {
      classList: { add: (...c) => { node.className = (node.className + ' ' + c.join(' ')).trim(); }, remove: (...c) => { node.className = node.className.split(/\s+/).filter((x) => !c.includes(x)).join(' '); }, contains: (c) => node.className.split(/\s+/).includes(c), toggle: (c, on) => { const has = node.className.split(/\s+/).includes(c); if (on === undefined ? !has : on) { if (!has) node.className += ' ' + c; } else node.className = node.className.split(/\s+/).filter((x) => x !== c).join(' '); } },
      appendChild: (c) => { if (c && typeof c === 'object') { node.children.push(c); try { c.parentNode = proxy; } catch (_) {} } return c; },
      append: (...cs) => { for (const c of cs) api.appendChild(typeof c === 'string' ? { textContent: c } : c); },
      prepend: (...cs) => { for (const c of cs) node.children.unshift(typeof c === 'string' ? { textContent: c } : c); },
      insertBefore: (c) => api.appendChild(c),
      replaceChildren: (...cs) => { node.children = []; api.append(...cs); },
      removeChild: (c) => { node.children = node.children.filter((x) => x !== c); return c; },
      remove: () => {},
      setAttribute: (k, v) => { node.attrs[k] = String(v); },
      getAttribute: (k) => (k in node.attrs ? node.attrs[k] : null),
      removeAttribute: (k) => { delete node.attrs[k]; },
      hasAttribute: (k) => k in node.attrs,
      addEventListener() {}, removeEventListener() {}, dispatchEvent() { return true; },
      querySelector: () => null, querySelectorAll: () => [], getElementsByClassName: () => [], getElementsByTagName: () => [], closest: () => null, contains: () => false, matches: () => false,
      focus() {}, blur() {}, click() {}, select() {}, scrollIntoView() {}, scrollTo() {}, getBoundingClientRect: () => ({ top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 }),
      cloneNode: () => el(tag),
      get firstChild() { return node.children[0] || null; },
      get lastChild() { return node.children[node.children.length - 1] || null; },
      get childNodes() { return node.children; },
      get textContent() { return node._text + node.children.map((c) => (c && c.textContent) || '').join(''); },
      set textContent(v) { node._text = String(v == null ? '' : v); node.children = []; },
      get innerText() { return api.textContent; },
      set innerText(v) { api.textContent = v; },
      get innerHTML() { return node._html || api.textContent; },
      set innerHTML(v) { node._html = String(v == null ? '' : v); node._text = node._html; node.children = []; },
      get outerHTML() { return api.innerHTML; },
    };
    const proxy = new Proxy(node, {
      get(t, p) {
        if (p in api) return api[p];
        if (KNOWN.has(p) || p in t) return t[p];
        if (typeof p === 'symbol') return undefined;
        return anything();
      },
      set(t, p, v) {
        if (p in api) { const d = Object.getOwnPropertyDescriptor(api, p); if (d && d.set) { d.set(v); return true; } }
        t[p] = v;
        return true;
      },
    });
    made.push(proxy);
    return proxy;
  }
  const byId = new Map();
  const document = new Proxy({}, {
    get(_t, p) {
      if (p === 'createElement') return (tag) => el(tag);
      if (p === 'createElementNS') return (_ns, tag) => el(tag);
      if (p === 'createTextNode') return (text) => ({ textContent: String(text) });
      if (p === 'createDocumentFragment') return () => el('#fragment');
      if (p === 'getElementById') return (id) => { if (!byId.has(id)) byId.set(id, el('div#' + id)); return byId.get(id); };
      if (p === 'querySelector') return () => null;
      if (p === 'querySelectorAll' || p === 'getElementsByClassName' || p === 'getElementsByTagName') return () => [];
      if (p === 'body' || p === 'documentElement' || p === 'head') { if (!byId.has('<' + p)) byId.set('<' + p, el(p)); return byId.get('<' + p); }
      if (p === 'addEventListener' || p === 'removeEventListener') return () => {};
      if (p === 'readyState') return 'complete';
      if (p === 'cookie') return '';
      if (p === 'hidden') return false;
      if (p === 'visibilityState') return 'visible';
      return anything();
    },
    set: () => true,
  });
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document,
    navigator: new Proxy({}, { get: (_t, p) => (p === 'userAgent' ? 'node' : p === 'language' ? 'en' : p === 'serviceWorker' ? { register: () => Promise.resolve({ scope: '/' }), ready: new Promise(() => {}), controller: null, addEventListener() {} } : anything()), has: () => true }),
    location: { hash: '', host: 'relay.invalid', hostname: 'relay.invalid', protocol: 'https:', pathname: '/chat', search: '', origin: 'https://relay.invalid', href: 'https://relay.invalid/chat', reload() {}, replace() {}, assign() {} },
    history: { replaceState() {}, pushState() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    fetch: () => Promise.reject(new Error('no network in tests')),
    setTimeout: () => 0, clearTimeout: () => {}, setInterval: () => 0, clearInterval: () => {},
    requestAnimationFrame: () => 0, cancelAnimationFrame() {}, requestIdleCallback: () => 0, queueMicrotask: (f) => Promise.resolve().then(f),
    WebSocket: Object.assign(function () { return anything(); }, { OPEN: 1, CONNECTING: 0, CLOSING: 2, CLOSED: 3 }),
    addEventListener: () => {}, removeEventListener: () => {}, dispatchEvent: () => true, matchMedia: () => anything(),
    alert() {}, confirm: () => false, prompt: () => null, open: () => null,
    Notification: anything(), innerWidth: 1280, innerHeight: 800, devicePixelRatio: 1,
    crypto: globalThis.crypto, btoa: globalThis.btoa, atob: globalThis.atob,
    TextEncoder, TextDecoder, URLSearchParams, URL, Blob, Uint8Array, ArrayBuffer, DataView, Promise, Map, Set, WeakMap, WeakSet, Symbol, JSON, Math, Date, RegExp, Error, TypeError, Number, String, Boolean, Array, Object, Proxy, Reflect, Intl,
    getComputedStyle: () => anything(), MutationObserver: function () { return { observe() {}, disconnect() {} }; }, ResizeObserver: function () { return { observe() {}, disconnect() {} }; }, IntersectionObserver: function () { return { observe() {}, disconnect() {} }; },
    performance: { now: () => 0 }, Image: function () { return el('img'); }, Audio: function () { return anything(); }, CustomEvent: function () { return {}; }, Event: function () { return {}; },
  };
  ctx.window = ctx;
  ctx.self = ctx;
  ctx.globalThis = ctx;
  vm.createContext(ctx);
  const html = fs.readFileSync(path.join(WEB, 'chat', 'index.html'), 'utf8');
  const scripts = ['/shared/i18n-en.js', '/shared/i18n.js'].concat([...html.matchAll(/<script src="([^"?]+)/g)].map((m) => m[1]));
  const failed = [];
  for (const src of scripts) {
    const file = path.join(WEB, src.replace(/^\//, ''));
    if (!fs.existsSync(file)) continue;
    try {
      vm.runInContext(fs.readFileSync(file, 'utf8'), ctx, { filename: src });
    } catch (e) {
      failed.push(src + ': ' + (e && e.message));
    }
  }
  // app.js escapes through an element's innerHTML; this small DOM does not, so a plain escaper stands in.
  ctx.esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  return { ctx, made, failed };
}

/** Every element carrying the app's mark, with its words (text and any markup written into it). */
function appLines(made) {
  return made.filter((e) => String(e.className).split(/\s+/).includes('app-line')).map((e) => e.textContent + ' ' + e.innerHTML);
}

/** Every `case '<type>':` of app.js `handleMessage`. */
function messageTypes() {
  const app = fs.readFileSync(path.join(WEB, 'chat', 'app.js'), 'utf8');
  const start = app.indexOf('async function handleMessage(msg)');
  assert.ok(start >= 0, 'handleMessage');
  // The function ends at the first line holding only "}" after it.
  const end = app.indexOf('\n}\n', start) >= 0 ? app.indexOf('\n}\n', start) : app.indexOf('\n}\r\n', start);
  const body = app.slice(start, end);
  return [...new Set([...body.matchAll(/case '([a-z0-9_]+)':/g)].map((m) => m[1]))];
}

/** The shapes the relay's words take in a `message`: as they are, and inside the sentences a handler
 *  matches (channel-admin feedback: "Channel #<name> created.", "Only admins ..."). */
const MESSAGES = [HOSTILE, 'Channel #' + HOSTILE + ' created.', 'Channel #' + HOSTILE + ' renamed to #' + HOSTILE + '.', 'Only admins ' + HOSTILE, '__game__:' + HOSTILE];

/** A message of `type` with the hostile words in every field a message names a person or carries words in. */
function hostile(type, message) {
  const person = { public_key: 'k1', name: HOSTILE, display_name: HOSTILE, role: 'member', status: 'online', online: true };
  return {
    type, from: 'k1', from_key: 'k1', from_name: HOSTILE, name: HOSTILE, display_name: HOSTILE, to_name: HOSTILE,
    message, content: HOSTILE, new_content: HOSTILE, text: HOSTILE, reason: HOSTILE, error: HOSTILE, note: HOSTILE,
    code: HOSTILE, kind: HOSTILE, action: HOSTILE, round: HOSTILE, skill_id: HOSTILE, level: HOSTILE, group_name: HOSTILE,
    channel: 'general', timestamp: 1, target: 'k1', target_from: 'k1', target_timestamp: 1, emoji: 'x',
    receipt: [[HOSTILE, 1], ['messages', 2]], partial: false, approved: true, ok: false,
    peers: [person], users: [person], members: [person], channels: [], items: [],
  };
}

/** Arm every "asked" state the page keeps, so the relay's answers to them are taken. */
function armAsks(ctx) {
  for (const src of [
    "channelAdminCmdInFlight = null; beginChannelAdminCmd('create');",
    'adminEraseAsked = { at: Date.now() };',
  ]) {
    try { vm.runInContext(src, ctx); } catch (_) { /* a page without that ask */ }
  }
}

const settle = () => new Promise((r) => setImmediate(r));

// A handler that throws on this small DOM, inside a promise the page does not await, is the page's
// own trouble here, not this test's finding: noted, and what it drew before is still read.
// The test runner fails the running test on any unhandled rejection, so its listeners step aside
// for the sweep alone and come back after it.
const pageErrors = [];
async function withPageErrorsNoted(body) {
  const kept = process.listeners('unhandledRejection');
  process.removeAllListeners('unhandledRejection');
  const note = (e) => { pageErrors.push(String(e && e.message)); };
  process.on('unhandledRejection', note);
  try {
    await body();
    await settle();
    await settle();
  } finally {
    process.removeListener('unhandledRejection', note);
    for (const l of kept) process.on('unhandledRejection', l);
  }
}

test('every message type, with the relay\'s words in every field, leaves the app\'s voice alone', async () => {
  const { ctx, made, failed } = loadPage();
  const core = failed.filter((f) => /\/chat\/(app|chat-ui|chat-owner|chat-messages)\.js/.test(f));
  assert.deepStrictEqual(core, [], 'the chat page\'s core scripts load');
  const types = messageTypes();
  assert.ok(types.length > 40, 'the page\'s message types were read: ' + types.length);
  const handle = vm.runInContext('handleMessage', ctx);
  // Positive control: the app's own line IS seen, so a clean result below is not a blind check.
  vm.runInContext("takeTypedClaim('/claim !!')", ctx); // not a code: the claim help, in the app's voice
  assert.ok(appLines(made).length >= 1, 'the sweep sees an app line when the app draws one');
  const throwing = [];
  // A signed-in page: the identity's first-sign-in work (it needs a real key) is done already.
  try { vm.runInContext("identityConfirmed = true; myKey = 'me'; myIdentity = myIdentity || { canSign: false };", ctx); } catch (_) { /* older page */ }
  await withPageErrorsNoted(async () => {
    for (const type of types) {
      for (const message of MESSAGES) {
        armAsks(ctx);
        // A handler that throws on this small DOM is noted, not fatal: what it drew before is still read.
        const run = Promise.resolve().then(() => handle(hostile(type, message))).catch(() => { if (!throwing.includes(type)) throwing.push(type); });
        await Promise.race([run, new Promise((r) => setTimeout(r, 50))]);
        await settle();
      }
    }
  });
  // Most types run to their end: the sweep is not a list of handlers that died on their first line.
  assert.ok(throwing.length < types.length / 3, 'most handlers ran: ' + throwing.join(', '));
  const leaked = appLines(made).filter((t) => t.includes(TELL));
  assert.deepStrictEqual(leaked, [], 'no line in the app\'s voice holds the relay\'s words (' + types.length + ' types swept)');
});

test('the app\'s mark is drawn only by appLine, and appLine refuses plain text', () => {
  // Every script of the page: the class `app-line` is written only inside appLine.
  const html = fs.readFileSync(path.join(WEB, 'chat', 'index.html'), 'utf8');
  const writers = [];
  for (const m of html.matchAll(/<script src="([^"?]+)/g)) {
    const file = path.join(WEB, m[1].replace(/^\//, ''));
    if (!fs.existsSync(file)) continue;
    let src = fs.readFileSync(file, 'utf8');
    if (m[1] === '/chat/app.js') {
      const at = src.indexOf('function appLine(');
      const end = src.indexOf('\n}\n', at) >= 0 ? src.indexOf('\n}\n', at) : src.indexOf('\n}\r\n', at);
      src = src.slice(0, at) + src.slice(end);
    }
    // Comments may name it; code may not.
    const code = src.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"])\/\/.*$/gm, '$1');
    if (/app-line/.test(code)) writers.push(m[1]);
  }
  assert.deepStrictEqual(writers, [], 'no script but app.js appLine writes the app\'s mark');
  const { ctx, made } = loadPage();
  assert.throws(() => vm.runInContext('appLine("owner.claim_help", { text: "a plain string" })', ctx), TypeError, 'a plain string argument is refused');
  vm.runInContext('appLine("chat.from_server", { text: appArg.quote(' + JSON.stringify(HOSTILE + '\n' + HOSTILE) + ') })', ctx);
  const [quoted] = appLines(made);
  assert.ok(quoted && !quoted.includes('\n') && quoted.includes('…') && quoted.length < 2 * 220, 'a quote is one short line: ' + quoted);
  vm.runInContext('appLine("chat.from_server", { text: appArg.name(' + JSON.stringify(HOSTILE) + ') })', ctx);
  assert.ok(!appLines(made).some((t) => t.includes(TELL)), 'a dishonest name is the refused-name words');
});
