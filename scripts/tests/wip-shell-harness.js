// The stand-in page the unfinished-software warning's mute is tested on (2026-11-02): shared by
// scripts/tests/wip-mute-web.test.js and scripts/tests/wip-strip-room-web.test.js. Not a test
// itself (its name does not end in .test.js).
//
// web/shared/shell.js runs WHOLE in node:vm, as a page loads it, over a document that answers
// anything harmlessly (the shell does far more than the strip), except for what the strip is made
// of: the element it builds keeps its attributes, its `hidden`, the listeners added to it and to
// its Mute and menu, and the markup written into the menu, so a test can press Mute, read the menu
// the shell wrote, and press one of its choices the way the browser would deliver the click.
// Storage is a Map per store (or one that throws: a blocked storage); fetch answers
// /data/gui/wip_mute.json from data/; the clock is fixed; i18n answers from data/i18n/en.json.
// HOS_WEB_DIR points it at another copy of web/ (how each test was seen red).

const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const EN = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8'));

/** en.json's words for a key, {{vars}} filled, glossary marks taken out (i18n.js `t`). */
function t(key, vars) {
  let v = EN;
  for (const part of String(key).split('.')) v = v && typeof v === 'object' ? v[part] : undefined;
  if (typeof v !== 'string') return key;
  v = v.replace(/\[\[([^\]|]+)(?:\|[^\]]+)?\]\]/g, '$1');
  return v.replace(/\{\{(\w+)\}\}/g, (_m, k) => (vars && vars[k] !== undefined ? String(vars[k]) : ''));
}

// Answers anything harmlessly, so the rest of the shell runs.
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

/** A Map-backed Storage, or one whose every call throws (`blocked`). */
function storage(blocked) {
  const m = new Map();
  const no = () => { throw new Error('SecurityError: storage is blocked'); };
  return {
    map: m,
    getItem: blocked ? no : (k) => (m.has(k) ? m.get(k) : null),
    setItem: blocked ? no : (k, v) => { m.set(k, String(v)); },
    removeItem: blocked ? no : (k) => { m.delete(k); },
  };
}

/**
 * Load shell.js as a page loads it.
 *   opts.version   the site's version (CURRENT_VERSION), default '0.1547.0'
 *   opts.nowSecs   the clock, seconds since 1970
 *   opts.path      location.pathname, default '/pages/notes.html'; opts.hash, default ''
 *   opts.local / opts.session   { key: value } put in localStorage / sessionStorage first
 *   opts.blocked   both storages throw
 * Returns { ctx, rootStyle, strip, mute, menu, local, session, timers, events, readDone, fire, settle }.
 */
function loadShell(opts = {}) {
  const rootStyle = {};
  const appended = [];
  const docListeners = {};
  const winListeners = {};
  let stripHolder = null;
  const el = (tag) => {
    const kids = {};
    const listeners = {};
    const attrs = {};
    const t = {
      tagName: String(tag).toUpperCase(), style: {}, dataset: {}, className: '', listeners, attrs,
      hidden: undefined, textContent: '', innerHTML: '',
      getBoundingClientRect: () => ({ height: 26.4, width: 1200, top: 0, left: 0, right: 1200, bottom: 26 }),
      querySelector: (sel) => (kids[sel] = kids[sel] || el('span')), querySelectorAll: () => [],
      appendChild: (c) => { appended.push(c); return c; }, prepend() {}, append() {}, insertBefore: (c) => c,
      insertAdjacentHTML() {}, insertAdjacentElement() {},
      addEventListener: (type, f) => { (listeners[type] = listeners[type] || []).push(f); }, removeEventListener() {},
      setAttribute: (k, v) => { attrs[k] = String(v); }, getAttribute: (k) => (k in attrs ? attrs[k] : null),
      removeAttribute: (k) => { delete attrs[k]; }, hasAttribute: (k) => k in attrs, remove() {}, contains: () => true, closest: () => null,
      focus() {},
    };
    const proxy = new Proxy(t, {
      get: (o, p) => (p === 'firstChild' ? (o.firstChild = o.firstChild || el('div')) : p in o ? o[p] : anything()),
      set: (o, p, v) => {
        if (p === 'innerHTML' && String(v).includes('id="hos-wip-strip"')) stripHolder = proxy;
        o[p] = v;
        return true;
      },
    });
    return proxy;
  };
  const html = el('html');
  html.style = { setProperty: (k, v) => { rootStyle[k] = v; } };
  const doc = el('#document');
  Object.assign(doc, {
    documentElement: html, body: el('body'), head: el('head'), currentScript: el('script'), readyState: 'loading',
    createElement: (tg) => el(tg), createTextNode: () => el('#text'),
    getElementById: () => null,
    querySelector: (sel) => (String(sel).includes('hos-wip') ? null : el('div')),
    addEventListener: (type, f) => { (docListeners[type] = docListeners[type] || []).push(f); },
  });
  const local = storage(opts.blocked);
  const session = storage(opts.blocked);
  for (const [k, v] of Object.entries(opts.local || {})) local.map.set(k, typeof v === 'string' ? v : JSON.stringify(v));
  for (const [k, v] of Object.entries(opts.session || {})) session.map.set(k, typeof v === 'string' ? v : JSON.stringify(v));
  const clock = { now: (opts.nowSecs || 1800000000) * 1000 };
  const RealDate = Date;
  class FixedDate extends RealDate {
    constructor(...a) { if (a.length) super(...a); else super(clock.now); }
    static now() { return clock.now; }
  }
  const timers = [];
  const events = [];
  const fetchFile = (url) => {
    const rel = String(url).split('?')[0];
    if (rel === '/data/gui/wip_mute.json') {
      const text = fs.readFileSync(path.join(ROOT, 'data', 'gui', 'wip_mute.json'), 'utf8');
      return Promise.resolve({ ok: true, json: () => Promise.resolve(JSON.parse(text)) });
    }
    return new Promise(() => {});
  };
  const ctx = {
    document: doc,
    location: { hostname: 'united-humanity.us', host: 'united-humanity.us', pathname: opts.path || '/pages/notes.html', href: 'https://united-humanity.us' + (opts.path || '/pages/notes.html') + (opts.hash || ''), search: '', hash: opts.hash || '' },
    localStorage: local, sessionStorage: session,
    navigator: anything(), matchMedia: () => ({ matches: false, addEventListener() {} }),
    fetch: fetchFile, XMLHttpRequest: class { open() {} send() {} setRequestHeader() {} },
    setTimeout: (f, ms) => { timers.push({ f, ms }); return timers.length; }, clearTimeout() {}, setInterval: () => 0, clearInterval() {}, requestAnimationFrame: () => 0,
    ResizeObserver: class { observe() {} }, MutationObserver: class { observe() {} disconnect() {} },
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    addEventListener: (type, f) => { (winListeners[type] = winListeners[type] || []).push(f); }, removeEventListener() {},
    dispatchEvent: (e) => { events.push(e && e.type); return true; },
    Event: class { constructor(type) { this.type = type; } },
    Date: FixedDate,
    i18n: { t, load: () => Promise.resolve(), getStoredLanguage: () => 'en', hint() {} },
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const version = opts.version || '0.1547.0';
  const src = fs.readFileSync(path.join(WEB, 'shared', 'shell.js'), 'utf8').replace(/var CURRENT_VERSION = '[^']*';/, "var CURRENT_VERSION = '" + version + "';");
  vm.runInContext(src, ctx, { filename: 'shell.js' });
  const strip = stripHolder ? stripHolder.firstChild : null;
  const menu = strip ? strip.querySelector('.hos-wip-menu') : null;
  // The markup writes the menu `hidden` (a test checks it does); this stand-in reads no markup.
  if (menu && menu.hidden === undefined && /class="hos-wip-menu" role="menu" hidden>/.test(String(stripHolder.innerHTML))) menu.hidden = true;
  return {
    ctx, rootStyle, appended, local: local.map, session: session.map, timers, events, clock,
    strip,
    mute: strip ? strip.querySelector('.hos-wip-mute') : null,
    menu,
    readDone: () => (docListeners.DOMContentLoaded || []).forEach((f) => f()),
    /** Deliver a window event (storage, hashchange, hos-library-doc, ...). */
    fire: (type, e) => (winListeners[type] || []).forEach((f) => f(e || { type })),
    /** Let the promises the shell started (the menu's fetch) settle. */
    settle: async () => { for (let i = 0; i < 4; i++) await new Promise((r) => setImmediate(r)); },
  };
}

/** The menu's items as the shell wrote them: { id, again, attrs, label } per <button>. */
function menuItems(menu) {
  const out = [];
  const re = /<button\b([^>]*)>([^<]*)<\/button>/g;
  let m;
  while ((m = re.exec(String(menu.innerHTML || '')))) {
    const attrs = {};
    for (const a of m[1].matchAll(/([\w-]+)="([^"]*)"/g)) attrs[a[1]] = a[2].replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');
    out.push({ id: attrs['data-wip-choice'] || null, again: 'data-wip-again' in attrs, attrs, label: m[2] });
  }
  return out;
}

/** A click on one of the menu's items, delivered to the menu's own listener as the browser would
 *  (the event's target is the item; `closest` finds it). */
function clickItem(menu, item) {
  const target = {
    closest: () => ({ hasAttribute: (k) => k in item.attrs, getAttribute: (k) => (k in item.attrs ? item.attrs[k] : null) }),
  };
  for (const f of menu.listeners.click || []) f({ target, type: 'click' });
}

module.exports = { loadShell, menuItems, clickItem, t, EN, ROOT, WEB };
