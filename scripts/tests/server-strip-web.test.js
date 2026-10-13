// The server strip on the website (2026-10-12, docs/design/media-round-5-plan.md 3.11 and lane
// 1F of section 5): a bar fixed just under the top menu, on Media, Watch, Chat and Admin, saying
// "{server name}  {host}" with a dot and Switch, so a person scrolled far down a long page still
// sees which server they are on. The desktop app draws the same strip (lane 1E,
// src/gui/pages/server_strip.rs) with the same words.
//
// Run: node --test scripts/tests/server-strip-web.test.js   (to be listed in `just rig-tests`)
//
// web/shared/server-strip.js runs as in the browser, against a small stand-in document that holds
// ONLY what the script built in it: reading or setting innerHTML throws, so a test that finds the
// name in the strip proves the script wrote it as text. The network is a stand-in that records
// each request and answers like the relay (GET /api/server-info's `name`, GET
// /api/federation/servers's list, src/relay/api.rs and api_market.rs). web/shared/shell.js runs
// whole in node:vm over a permissive stand-in (every unknown property answers harmlessly), with a
// nav whose height the test sets, so the sticking test proves what loading shell.js does.
// HOS_WEB_DIR points the tests at another copy of web/ (used to see each test red).
//
// What it proves:
//  1. The strip names the server: the name from /api/server-info, the host from location.host,
//     as "{name}  {host}", right after shell.js's nav spacer; the dot is green once the server
//     answered, red when it did not (then the host alone).
//  2. It sticks under the nav: its style is `position: sticky; top: var(--hos-nav-h)`, and loading
//     shell.js sets --hos-nav-h on <html> to the nav's height (and again when the nav re-wraps).
//     shell.js's round help button goes under the strip, not over its Switch.
//  3. Switch links the same page on each linked server, https only: /chat on this server links
//     https://<their host>/chat; an item's page /media/<n> links /media; an address that is not
//     https (javascript:, data:, http:), one carrying a user name, and this server itself are
//     never links; the list is asked once.
//  4. A server name of `<img src=x onerror=alert(1)>` is shown as the words it is, in the strip and
//     in Switch's list.
//  5. /admin's Media panel is one line linking /media#admin; /admin loads none of the Media
//     section's scripts or styles, and admin-app.js no longer starts the section.
//  6. The strip's tag is on Watch, Chat (right after shell.js, the order CLAUDE.md lists) and Admin,
//     and the script mounts itself in the browser.
//  7. While the protected setup is on (10h), Switch shows no other server, as Chat's server list.
//
// Red first, 2026-10-12: each break made in a fresh copy of web/, this test run against it with
// HOS_WEB_DIR, and seen failing with the assertion named (each passing again on the real web/):
//  1: server-strip.js reading `info.server_name` (the field name the plan wrote; the relay sends
//     `name`): "the strip says the server's name and its host" (4 and 6 red with it).
//  2: shell.js without its setProperty('--hos-nav-h') line: "loading shell.js sets --hos-nav-h to
//     the nav's height plus its 1px line". And, each alone: shell.js's help place() measuring the
//     nav only, or without its DOMContentLoaded re-place: "the help button sits under the strip
//     once the page is read".
//  3: server-strip.js linkedOrigin passing the address through
//     (`return String(raw).replace(/\/+$/, '')`): "only https addresses are links", with
//     "javascript:alert(1)/chat" among the hrefs.
//  4: server-strip.js showName writing the name as `label.innerHTML = '<span ...>' + n + ...`: the
//     stand-in threw on innerHTML, the strip fell back to the host alone: "the name is shown as
//     text" (1 and 6 red with it).
//  5: admin.html and admin-app.js as they were before this lane (main at 60c9ae156): "the
//     Media panel holds one link, to /media#admin" (6 red with it: no strip tag yet).
//  6: watch.html without the strip's tag: "Watch loads the strip after shell.js".
//  7: server-strip.js hidesServers returning false: "no linked server is listed".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const STRIP_FILE = path.join(WEB, "shared", "server-strip.js");
const strip = require(STRIP_FILE);

const HOST = "united-humanity.us";
const EVIL_NAME = "<img src=x onerror=alert(1)>";

// ── A document that holds only what was built in it ─────────────────────

class FakeText {
  constructor(t) { this.nodeType = 3; this._t = String(t); this.parentNode = null; }
  get textContent() { return this._t; }
  set textContent(v) { this._t = String(v); }
}

class FakeElement {
  constructor(doc, tag) {
    this.ownerDocument = doc;
    this.nodeType = 1;
    this.tagName = String(tag).toUpperCase();
    this.childNodes = [];
    this.parentNode = null;
    this.attributes = new Map();
    this.listeners = {};
    this.className = "";
    this.hidden = false;
  }
  get children() { return this.childNodes.filter((c) => c.nodeType === 1); }
  get firstChild() { return this.childNodes[0] || null; }
  get nextSibling() {
    if (!this.parentNode) return null;
    const sibs = this.parentNode.childNodes;
    return sibs[sibs.indexOf(this) + 1] || null;
  }
  get id() { return this.attributes.get("id") || ""; }
  get textContent() { return this.childNodes.map((c) => c.textContent).join(""); }
  set textContent(v) { this.replaceChildren(); if (String(v)) this.appendChild(new FakeText(v)); }
  get innerHTML() { throw new Error("the strip read innerHTML"); }
  set innerHTML(v) { throw new Error("the strip set innerHTML: " + String(v).slice(0, 80)); }
  appendChild(c) {
    if (c.parentNode) c.parentNode.removeChild(c);
    c.parentNode = this;
    this.childNodes.push(c);
    return c;
  }
  insertBefore(c, ref) {
    if (!ref) return this.appendChild(c);
    if (c.parentNode) c.parentNode.removeChild(c);
    c.parentNode = this;
    this.childNodes.splice(this.childNodes.indexOf(ref), 0, c);
    return c;
  }
  removeChild(c) {
    const i = this.childNodes.indexOf(c);
    if (i >= 0) this.childNodes.splice(i, 1);
    c.parentNode = null;
    return c;
  }
  replaceChildren(...cs) {
    for (const c of this.childNodes.slice()) this.removeChild(c);
    for (const c of cs) this.appendChild(c);
  }
  contains(n) {
    for (let x = n; x; x = x.parentNode) if (x === this) return true;
    return false;
  }
  setAttribute(k, v) { this.attributes.set(k, String(v)); }
  getAttribute(k) { return this.attributes.has(k) ? this.attributes.get(k) : null; }
  addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); }
  dispatch(type, extra) {
    let stopped = false;
    const ev = Object.assign({ type, target: this, preventDefault() {}, stopPropagation() { stopped = true; } }, extra || {});
    for (const f of (this.listeners[type] || []).slice()) f(ev);
    // Bubble to the document, as a browser does, unless a listener stopped it.
    if (!stopped && this.ownerDocument) this.ownerDocument.dispatch(type, ev);
  }
  click() { this.dispatch("click"); }
  focus() {}
  matches(sel) {
    let m;
    if ((m = /^#([\w-]+)$/.exec(sel))) return this.id === m[1];
    if ((m = /^\.([\w-]+)$/.exec(sel))) return this.className.split(/\s+/).includes(m[1]);
    if (/^[a-z][a-z0-9]*$/i.test(sel)) return this.tagName === sel.toUpperCase();
    throw new Error("the stand-in document does not know the selector " + sel);
  }
  querySelectorAll(sel) {
    const out = [];
    const walk = (n) => { for (const c of n.children) { if (c.matches(sel)) out.push(c); walk(c); } };
    walk(this);
    return out;
  }
  querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
}

/** A page as shell.js leaves it: its nav, then the spacer, then the page's own content. */
function fakePage(opts) {
  const o = opts || {};
  const listeners = {};
  const doc = {
    readyState: o.readyState || "complete",
    createElement: (tag) => new FakeElement(doc, tag),
    createTextNode: (t) => new FakeText(t),
    addEventListener: (t, f) => { (listeners[t] = listeners[t] || []).push(f); },
    dispatch: (t, ev) => { for (const f of (listeners[t] || []).slice()) f(ev); },
    listeners,
  };
  doc.head = new FakeElement(doc, "head");
  doc.body = new FakeElement(doc, "body");
  doc.getElementById = (id) => doc.head.querySelector("#" + id) || doc.body.querySelector("#" + id);
  doc.querySelector = (sel) => doc.head.querySelector(sel) || doc.body.querySelector(sel);
  if (o.withNav !== false) {
    const nav = new FakeElement(doc, "div");
    nav.className = "hub-nav-wrap";
    const spacer = new FakeElement(doc, "div");
    spacer.className = "hub-nav-spacer";
    doc.body.appendChild(nav);
    doc.body.appendChild(spacer);
  }
  const content = new FakeElement(doc, "div");
  content.setAttribute("id", "page-app");
  doc.body.appendChild(content);
  return doc;
}

/** Every element under `n`. */
function walkAll(n) {
  const out = [];
  const walk = (x) => { for (const c of x.children) { out.push(c); walk(c); } };
  walk(n);
  return out;
}

// ── The network ───────────────────────────────────────────────────────────

function respond(status, body) {
  return { ok: status >= 200 && status < 300, status, json: async () => body, text: async () => JSON.stringify(body) };
}

/** A fetch answering from `routes` ({path: answer}) and recording every request. */
function fakeFetch(routes) {
  const calls = [];
  const fetch = async (url) => {
    calls.push(String(url));
    const answer = routes[String(url).split("?")[0]];
    if (answer === undefined) return respond(404, "no route in the test for " + url);
    if (answer instanceof Error) throw answer;
    return typeof answer === "function" ? answer() : answer;
  };
  return { fetch, calls };
}

const settle = () => new Promise((r) => setImmediate(r));
async function settleAll() { for (let i = 0; i < 5; i++) await settle(); }

/** Mount the strip on a fake page. */
function mount(opts) {
  const o = opts || {};
  const doc = o.doc || fakePage();
  const net = fakeFetch(Object.assign({
    "/api/server-info": respond(200, { name: "United Humanity", description: "", version: "0.1489.0" }),
  }, o.routes || {}));
  const handle = strip.mountServerStrip({
    document: doc,
    location: { host: o.host || HOST, pathname: o.pathname || "/watch" },
    fetch: net.fetch,
    protectedRaw: () => (o.protectedRaw === undefined ? null : o.protectedRaw),
    protectedHidesRooms: o.protectedHidesRooms,
  });
  return { doc, net, handle };
}

const stripEl = (doc) => doc.getElementById("hos-server-strip");
const labelText = (doc) => stripEl(doc).querySelector(".hos-strip-label").textContent;
const dotState = (doc) => stripEl(doc).querySelector(".hos-strip-dot").getAttribute("data-state");
const menuOf = (doc) => doc.getElementById("hos-strip-menu");
const linksIn = (doc) => menuOf(doc).querySelectorAll("a");

// ── 1. The name ──────────────────────────────────────────────────────────

test("1. the strip names the server, from /api/server-info and the host", async () => {
  const { doc, net, handle } = mount();
  // Before the server answers: the host alone, the dot muted.
  assert.equal(labelText(doc), HOST, "the host shows at once");
  assert.equal(dotState(doc), "checking");
  await handle.ready;
  assert.equal(labelText(doc), "United Humanity  " + HOST, "the strip says the server's name and its host");
  assert.equal(dotState(doc), "connected", "green once the server answered");
  assert.deepEqual(net.calls, ["/api/server-info"], "it asks this server, once");
  // Where it sits: right after shell.js's spacer, before the page's own content.
  const spacer = doc.querySelector(".hub-nav-spacer");
  assert.equal(spacer.nextSibling, stripEl(doc), "right under the nav");
  assert.equal(stripEl(doc).nextSibling, doc.getElementById("page-app"));
  assert.ok(stripEl(doc).querySelector(".hos-strip-switch").textContent === "Switch", "the desktop's word");
  // Mounting twice keeps one strip.
  strip.mountServerStrip({ document: doc, location: { host: HOST, pathname: "/watch" }, fetch: net.fetch });
  assert.equal(walkAll(doc.body).filter((e) => e.id === "hos-server-strip").length, 1, "one strip");

  // A server that does not answer: the host alone, the dot red.
  const down = mount({ routes: { "/api/server-info": respond(503, "down") } });
  await down.handle.ready;
  assert.equal(labelText(down.doc), HOST);
  assert.equal(dotState(down.doc), "unreachable");
  const gone = mount({ routes: { "/api/server-info": new Error("offline") } });
  await gone.handle.ready;
  assert.equal(dotState(gone.doc), "unreachable");
  // A server with no name set: the host alone.
  const unnamed = mount({ routes: { "/api/server-info": respond(200, { name: "  " }) } });
  await unnamed.handle.ready;
  assert.equal(labelText(unnamed.doc), HOST);
  // A page without shell.js: the top of the page.
  const bare = mount({ doc: fakePage({ withNav: false }) });
  assert.equal(bare.doc.body.firstChild, stripEl(bare.doc), "at the top when there is no nav");
  // The words, as the desktop strip says them.
  assert.equal(strip.stripLabel("Public Guide", "public.guide"), "Public Guide  public.guide");
  assert.equal(strip.stripLabel("", "localhost:8099"), "localhost:8099");
  const EM_DASH = String.fromCharCode(0x2014);
  for (const w of Object.values(strip.STRIP_WORDS)) assert.ok(!w.includes(EM_DASH), "no em dash: " + w);
});

// ── 2. Sticking under the nav ─────────────────────────────────────────────

// A stand-in that answers anything harmlessly, so shell.js runs whole.
function anything() {
  const fn = function () {};
  return new Proxy(fn, {
    get(_t, prop) {
      if (prop === Symbol.toPrimitive) return () => "";
      if (prop === Symbol.iterator) return function* () {};
      if (prop === "then") return undefined;
      if (prop === "length") return 0;
      return anything();
    },
    set: () => true, has: () => true, apply: () => anything(), construct: () => anything(),
  });
}

/**
 * Run web/shared/shell.js as a page loads it; the nav measures `nav.h` tall. While `nav.stripBottom`
 * is set, the page has a server strip whose bottom edge is there.
 */
function loadShell(nav) {
  const rootStyle = {};
  const observers = [];
  const spacers = [];
  const created = [];
  const docListeners = {};
  const stripStub = { getBoundingClientRect: () => ({ top: nav.h + 1, bottom: nav.stripBottom, height: nav.stripBottom - nav.h - 1 }) };
  const el = (tag) => {
    const t = {
      tagName: String(tag).toUpperCase(), style: {}, dataset: {}, className: "",
      getBoundingClientRect: () => ({ height: nav.h, width: 1200, top: 0, left: 0, right: 1200, bottom: nav.h }),
      querySelector: () => el("div"), querySelectorAll: () => [],
      appendChild: (c) => c, prepend() {}, append() {}, insertBefore: (c) => c, insertAdjacentHTML() {},
      insertAdjacentElement(_where, c) { spacers.push(c); },
      addEventListener() {}, removeEventListener() {}, setAttribute() {}, getAttribute: () => null,
      removeAttribute() {}, hasAttribute: () => false, remove() {}, contains: () => false, closest: () => null,
    };
    return new Proxy(t, { get: (o, p) => (p in o ? o[p] : anything()), set: (o, p, v) => { o[p] = v; return true; } });
  };
  const html = el("html");
  html.style = { setProperty: (k, v) => { rootStyle[k] = v; } };
  const doc = el("#document");
  Object.assign(doc, {
    documentElement: html, body: el("body"), head: el("head"), currentScript: el("script"), readyState: "complete",
    createElement: (t) => { const e = el(t); created.push(e); return e; },
    createTextNode: () => el("#text"),
    getElementById: (id) => (id === "hos-server-strip" && nav.stripBottom ? stripStub : null),
    addEventListener: (t, f) => { (docListeners[t] = docListeners[t] || []).push(f); },
  });
  const store = new Map();
  const ctx = {
    document: doc,
    location: { hostname: HOST, host: HOST, pathname: "/watch", href: "https://" + HOST + "/watch", search: "", hash: "" },
    localStorage: { getItem: (k) => (store.has(k) ? store.get(k) : null), setItem: (k, v) => store.set(k, String(v)), removeItem: (k) => store.delete(k) },
    navigator: anything(), matchMedia: () => ({ matches: false, addEventListener() {} }),
    fetch: () => new Promise(() => {}), XMLHttpRequest: class { open() {} send() {} setRequestHeader() {} },
    setTimeout: () => 0, clearTimeout() {}, setInterval: () => 0, clearInterval() {}, requestAnimationFrame: () => 0,
    ResizeObserver: class { constructor(f) { observers.push(f); } observe() {} },
    MutationObserver: class { observe() {} disconnect() {} },
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    addEventListener() {}, removeEventListener() {},
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "shared", "shell.js"), "utf8"), ctx, { filename: "shell.js" });
  return { rootStyle, observers, docListeners, created, spacer: spacers.find((s) => s && s.className === "hub-nav-spacer") };
}

test("2. the strip sticks under the nav: sticky at --hos-nav-h, which shell.js sets", () => {
  // The strip's own style.
  const { doc } = mount();
  const style = doc.getElementById("hos-server-strip-style");
  assert.ok(style, "the strip brings its style");
  const css = style.textContent;
  const rule = /\.hos-server-strip\s*\{([^}]*)\}/.exec(css);
  assert.ok(rule, "a rule for the strip");
  assert.match(rule[1], /position:\s*sticky;/, "it sticks");
  assert.match(rule[1], /top:\s*var\(--hos-nav-h\b/, "under the nav's height");
  // Theme tokens only: no colour literal anywhere in the strip's style.
  assert.ok(!/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i.test(css), "colours are theme tokens");

  // shell.js sets the variable as the page loads, to the same height as its spacer.
  const nav = { h: 40 };
  const shell = loadShell(nav);
  assert.equal(shell.rootStyle["--hos-nav-h"], "41px", "loading shell.js sets --hos-nav-h to the nav's height plus its 1px line");
  assert.ok(shell.spacer, "shell.js makes its spacer");
  assert.equal(shell.spacer.style.height, shell.rootStyle["--hos-nav-h"], "the same height as the spacer");
  // shell.js's round help button floats at the right, 8px under what is above it. Seen in the
  // browser pane, 2026-10-12: placed under the nav it sat on the strip's Switch. It goes under the
  // strip, measured again once the page is read (the strip's script runs after shell.js).
  const help = shell.created.find((e) => e.id === "hos-help-toggle");
  assert.ok(help, "shell.js makes its help button");
  assert.equal(help.style.top, "48px", "with no strip yet, under the nav");
  nav.stripBottom = 72;
  for (const f of shell.docListeners.DOMContentLoaded || []) f();
  assert.equal(help.style.top, "80px", "the help button sits under the strip once the page is read");
  // The nav wraps to two rows (a narrow window): the variable follows.
  nav.h = 74;
  assert.ok(shell.observers.length > 0, "shell.js watches the nav's size");
  for (const f of shell.observers) f();
  assert.equal(shell.rootStyle["--hos-nav-h"], "75px", "the variable follows a wrapped nav");
});

// ── 3. Switch ─────────────────────────────────────────────────────────────

const LINKED = [
  { server_id: "a", name: "Public Guide", url: "https://public.guide/", status: "online", trust_tier: 2 },
  { server_id: "b", name: "Trap", url: "javascript:alert(1)//", status: "online", trust_tier: 2 },
  { server_id: "c", name: "Data", url: "data:text/html,<script>alert(1)</script>", status: "online", trust_tier: 2 },
  { server_id: "d", name: "Plain", url: "http://plain.example", status: "online", trust_tier: 2 },
  { server_id: "e", name: "Ourselves", url: "https://" + HOST, status: "online", trust_tier: 3 },
  { server_id: "f", name: "Creds", url: "https://user:pw@creds.example", status: "online", trust_tier: 2 },
  { server_id: "g", name: "Deep", url: "https://deep.example:8443/some/path?q=1#x", status: "offline", trust_tier: 0 },
  { server_id: "h", name: "Pinned by key", url: "", status: "unknown", trust_tier: 2 },
  { server_id: "i", name: "Again", url: "https://public.guide", status: "online", trust_tier: 1 },
];

test("3. Switch links the same page on each linked server, https only", async () => {
  const { doc, net, handle } = mount({ pathname: "/chat", routes: { "/api/federation/servers": respond(200, LINKED) } });
  await handle.ready;
  const button = stripEl(doc).querySelector(".hos-strip-switch");
  assert.equal(menuOf(doc).hidden, true, "the list starts closed");
  assert.ok(!net.calls.includes("/api/federation/servers"), "nothing asked before Switch is pressed");
  button.click();
  await settleAll();
  assert.equal(menuOf(doc).hidden, false, "Switch opens the list");
  assert.equal(button.getAttribute("aria-expanded"), "true");
  const hrefs = linksIn(doc).map((a) => a.getAttribute("href"));
  assert.ok(hrefs.every((h) => h.startsWith("https://")), "only https addresses are links: " + JSON.stringify(hrefs));
  assert.deepEqual(hrefs, ["https://public.guide/chat", "https://deep.example:8443/chat"],
    "the same page on each linked server; never this server, a user name, plain http or a second copy");
  assert.ok(linksIn(doc).every((a) => a.getAttribute("rel") === "noreferrer"), "the other server is not told the page");
  assert.deepEqual(linksIn(doc).map((a) => a.textContent), ["Public Guide  public.guide", "Deep  deep.example:8443"]);
  assert.deepEqual(linksIn(doc).map((a) => a.querySelector(".hos-strip-dot").getAttribute("data-state")), ["online", "unknown"]);
  assert.ok(menuOf(doc).textContent.includes("You sign in on each server's website separately."), "the sign-in line");
  // Closing (a click elsewhere) and opening again asks nothing new.
  doc.dispatch("click", { target: doc.getElementById("page-app") });
  assert.equal(menuOf(doc).hidden, true, "a click elsewhere closes the list");
  button.click();
  await settleAll();
  assert.equal(net.calls.filter((c) => c === "/api/federation/servers").length, 1, "the list is asked once");
  // Escape closes it.
  doc.dispatch("keydown", { key: "Escape" });
  assert.equal(menuOf(doc).hidden, true, "Escape closes the list");

  // An item's page links the Media page there: item numbers are each server's own.
  const item = mount({ pathname: "/media/12", routes: { "/api/federation/servers": respond(200, LINKED) } });
  await item.handle.openMenu();
  assert.equal(linksIn(item.doc)[0].getAttribute("href"), "https://public.guide/media");
  // The pure rules.
  assert.equal(strip.samePagePath("//elsewhere.example/x"), "/elsewhere.example/x", "a path can never name another host");
  assert.equal(strip.samePagePath("/media"), "/media");
  assert.equal(strip.linkedOrigin("javascript:alert(1)//"), null);
  assert.equal(strip.linkedOrigin("HTTPS://Public.Guide/x"), "https://public.guide");

  // No linked server, and a list that could not be had.
  const none = mount({ routes: { "/api/federation/servers": respond(200, []) } });
  await none.handle.openMenu();
  assert.equal(linksIn(none.doc).length, 0);
  assert.ok(menuOf(none.doc).textContent.includes("This server is not linked with any other server yet."));
  const failed = mount({ routes: { "/api/federation/servers": respond(500, "no") } });
  await failed.handle.openMenu();
  assert.ok(menuOf(failed.doc).textContent.includes("Could not get the list of linked servers."));
});

// ── 4. Names are words, never markup ──────────────────────────────────────

test("4. a server name of <img src=x onerror=alert(1)> is shown as text", async () => {
  const { doc, handle } = mount({
    routes: {
      "/api/server-info": respond(200, { name: EVIL_NAME }),
      "/api/federation/servers": respond(200, [{ name: EVIL_NAME, url: "https://other.example", status: "online" }]),
    },
  });
  await handle.ready;
  assert.equal(labelText(doc), EVIL_NAME + "  " + HOST, "the name is shown as text");
  await handle.openMenu();
  assert.equal(linksIn(doc)[0].textContent, EVIL_NAME + "  other.example", "a linked server's name is text too");
  assert.equal(walkAll(doc.body).filter((e) => e.tagName === "IMG").length, 0, "no picture was made from a name");
});

// ── 5. /admin points to the Media page ────────────────────────────────────

test("5. /admin's Media panel is one line linking /media#admin", () => {
  const html = fs.readFileSync(path.join(WEB, "pages", "admin.html"), "utf8");
  const at = html.indexOf('id="media-admin-panel"');
  assert.ok(at > 0, "the Media panel is still there");
  const end = html.indexOf("</div>", at);
  const panel = html.slice(at, end);
  const links = [...panel.matchAll(/<a\b[^>]*href="([^"]*)"/g)].map((m) => m[1]);
  assert.deepEqual(links, ["/media#admin"], "the Media panel holds one link, to /media#admin");
  assert.equal((panel.match(/<p\b/g) || []).length, 1, "one line");
  assert.ok(panel.includes("This server's Media settings, categories and space are on the Media page, under Admin."), "the line's words");
  assert.ok(!html.includes("media-admin-root"), "nothing is mounted there any more");
  for (const gone of ["/pages/media-admin.js", "/pages/media-app.js", "/pages/media-upload.js", "/shared/relay-session.js", "/shared/media.css"]) {
    assert.ok(!html.includes(gone), "/admin no longer loads " + gone);
  }
  const app = fs.readFileSync(path.join(WEB, "pages", "admin-app.js"), "utf8");
  assert.ok(!app.includes("startMediaAdmin"), "admin-app.js no longer starts the Media section");
});

// ── 6. The pages that carry it ────────────────────────────────────────────

test("6. Watch, Chat and Admin load the strip after shell.js, and it mounts itself", async () => {
  const pages = { Watch: ["pages", "watch.html"], Chat: ["chat", "index.html"], Admin: ["pages", "admin.html"] };
  for (const [name, rel] of Object.entries(pages)) {
    const html = fs.readFileSync(path.join(WEB, ...rel), "utf8");
    const shellAt = html.search(/src="?\/shared\/shell\.js/);
    const stripAt = html.indexOf('src="/shared/server-strip.js?v=');
    assert.ok(shellAt > 0 && stripAt > shellAt, name + " loads the strip after shell.js");
  }
  // Chat: right after shell.js, before the rest of the order (CLAUDE.md lists it there).
  const chat = fs.readFileSync(path.join(WEB, "chat", "index.html"), "utf8");
  const srcs = [...chat.matchAll(/<script\s+src="([^"?]+)/g)].map((m) => m[1]);
  assert.equal(srcs[srcs.indexOf("/shared/shell.js") + 1], "/shared/server-strip.js", "Chat loads it right after shell.js");
  const claude = fs.readFileSync(path.join(ROOT, "CLAUDE.md"), "utf8");
  assert.ok(claude.includes("`/shared/shell.js` → `/shared/server-strip.js` → `/shared/defaults.js`"), "CLAUDE.md's load order says so");

  // In the browser the script mounts itself under the nav, with no module system.
  const doc = fakePage();
  const net = fakeFetch({ "/api/server-info": respond(200, { name: "United Humanity" }) });
  const ctx = { document: doc, location: { host: HOST, pathname: "/watch" }, fetch: net.fetch, localStorage: { getItem: () => null }, URL, Set };
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(STRIP_FILE, "utf8"), ctx, { filename: "server-strip.js" });
  await settleAll();
  assert.ok(ctx.hosServerStrip, "the helpers are on window");
  assert.equal(doc.querySelector(".hub-nav-spacer").nextSibling, stripEl(doc), "it put itself under the nav");
  assert.equal(labelText(doc), "United Humanity  " + HOST);
  // A page still loading, with the tag before the nav exists: it waits for the page.
  const early = fakePage({ withNav: false, readyState: "loading" });
  const ctx2 = { document: early, location: { host: HOST, pathname: "/media" }, fetch: net.fetch, localStorage: { getItem: () => null }, URL, Set };
  vm.createContext(ctx2);
  vm.runInContext(fs.readFileSync(STRIP_FILE, "utf8"), ctx2, { filename: "server-strip.js" });
  assert.equal(stripEl(early), null, "nothing yet");
  early.dispatch("DOMContentLoaded", {});
  assert.ok(stripEl(early), "mounted once the page is there");
});

// ── 7. The protected setup ────────────────────────────────────────────────

test("7. while the protected setup is on, Switch shows no other server", async () => {
  const routes = { "/api/federation/servers": respond(200, LINKED) };
  const on = mount({ routes, protectedRaw: JSON.stringify({ v: 1, on: true, public_rooms: "read_only_only" }) });
  await on.handle.openMenu();
  assert.equal(linksIn(on.doc).length, 0, "no linked server is listed");
  assert.ok(menuOf(on.doc).textContent.includes("Other servers are hidden by the protected setup."));
  assert.ok(!on.net.calls.includes("/api/federation/servers"), "and none is asked for");
  // Damaged text counts as on (protected.js's rule): hidden.
  const damaged = mount({ routes, protectedRaw: "{not json" });
  await damaged.handle.openMenu();
  assert.equal(linksIn(damaged.doc).length, 0, "a damaged setup hides them");
  // Chat's own rule wins where the page has it.
  const chat = mount({ routes, protectedHidesRooms: () => true });
  await chat.handle.openMenu();
  assert.equal(linksIn(chat.doc).length, 0, "Chat's rule hides them");
  // Off, turned off, or set to show every public room: listed.
  for (const raw of [null, JSON.stringify({ on: false }), JSON.stringify({ on: true, public_rooms: "all" }), JSON.stringify({ on: true, rules: { public_rooms: "all" } })]) {
    const m = mount({ routes, protectedRaw: raw });
    await m.handle.openMenu();
    assert.equal(linksIn(m.doc).length, 2, "listed with the setup " + raw);
  }
});
