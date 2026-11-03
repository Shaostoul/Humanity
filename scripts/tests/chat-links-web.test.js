// Links to chat channels on the website (2026-11-03), run under Node:
//
//   node --test scripts/tests/chat-links-web.test.js      (in `just rig-tests`)
//
// The operator: "Can we make it so I can link people to specific channels like
// https://united-humanity.us/chat#global or https://united-humanity.us/chat#fire-disciples", and
// the same day: "I think we need to also fix the default channel to go to global".
//
// What it proves:
//  1. web/shared/chat-links.js gives every answer of the shared fixture
//     (scripts/tests/fixtures/chat-links-v1.json), the answers the desktop's channel_link.rs gives.
//  2. A signed-in person's /chat#<id> opens that channel, and so does its shown name in any case;
//     the address keeps showing the channel, replaced (no new step: navigation rung 5's to add).
//  3. The address follows a switch; a direct message, a group and the scratch pad leave it naming
//     no one; `#dm=<key>` opens the conversation and the address never holds the key.
//  4. An edited address (a hashchange) switches channel; a link in a message to this server's own
//     chat switches in place; a link naming no channel here says so and opens no other.
//  5. Copy link, in the channel's header and on the room's menu, copies
//     https://<this server>/chat#<id>; on a server with no public address it says so.
//  6. A visitor: /chat's redirect keeps a channel link's hash (a #dm= one is dropped); the read
//     view opens the channel's tab by its id or shown name, says so for a members' or verified
//     channel and for one that is not there (naming no other channel), and Choose a name to join
//     in carries the channel to the chat, which opens it.
//  7. THE DEFAULT IS #global: the read view with no link opens #global, not the first channel
//     listed (#announcements, position 2 in data/server/default_channels.json); the chat's first
//     open with nothing saved and no link is #global; a switch to another server (the server
//     strip's link) carries no channel, so it opens there the same way.
//
// The page scripts run as in the browser, in one shared global scope (node:vm), over a stand-in
// page (files-web-harness.js's elements where they are built, a stand-in elsewhere), with a
// stand-in address and history that record every change. HOS_WEB_DIR points it at another copy of
// web/ (used to see each test red).
//
// Seen red 2026-11-03 (a copy of web/ with today's files, then one file as it was before, or
// chat-links.js finding by id only, through HOS_WEB_DIR): see the note on each test. Test 1 with
// find() by id only: 'the channel "fire disciples" names' (actual null).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const H = require("./files-web-harness.js");
const ROOT = H.ROOT;
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const L = require(path.join(WEB, "shared", "chat-links.js"));
const FX = JSON.parse(fs.readFileSync(path.join(ROOT, "scripts", "tests", "fixtures", "chat-links-v1.json"), "utf8"));
const EN = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "i18n", "en.json"), "utf8"));

/** en.json's words by key with {{vars}} filled, [[term]] marks shown as their words. */
function t(key, vars) {
  const v = key.split(".").reduce((o, k) => (o && typeof o === "object" ? o[k] : undefined), EN);
  if (typeof v !== "string") return key;
  return v.replace(/\[\[([^\]|]+)(?:\|[^\]]+)?\]\]/g, "$1").replace(/\{\{(\w+)\}\}/g, (m, k) => (vars && k in vars ? String(vars[k]) : m));
}

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
    set: () => true,
    has: () => true,
    apply: () => anything(),
    construct: () => anything(),
  });
}

/** An element that keeps what is written into it (innerHTML too) and its listeners; anything else
 *  asked of it answers like the stand-in. */
function keeper(extra) {
  const kept = Object.assign({
    innerHTML: "", dataset: {}, style: {}, listeners: {},
    addEventListener(type, f) { (kept.listeners[type] = kept.listeners[type] || []).push(f); },
  }, extra || {});
  return new Proxy(kept, { get: (o, p) => (p in o ? o[p] : anything()), set: (o, p, v) => { o[p] = v; return true; } });
}

function fakeStorage() {
  const m = new Map();
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k), clear: () => m.clear() };
}

/** An address and its history: every change recorded, as `["replace" | "push", url]`. */
function fakeAddress(host, pathname, hash) {
  const location = { hash: hash || "", pathname, search: "", host, protocol: host.startsWith("127.") || host.startsWith("localhost") ? "http:" : "https:", reload() {} };
  location.origin = location.protocol + "//" + host;
  const steps = [];
  const setFrom = (url) => {
    const at = String(url).indexOf("#");
    location.hash = at < 0 ? "" : String(url).slice(at);
  };
  const history = {
    state: null,
    replaceState(_s, _t, url) { steps.push(["replace", String(url)]); setFrom(url); },
    pushState(_s, _t, url) { steps.push(["push", String(url)]); setFrom(url); },
  };
  return { location, history, steps };
}

const CHANNELS = [
  { id: "announcements", name: "announcements", read_level: "anyone", post_level: "mods" },
  { id: "local", name: "local", read_level: "members", post_level: "members", local_only: true },
  { id: "global", name: "global", read_level: "anyone", post_level: "members" },
  { id: "feedback", name: "feedback", read_level: "anyone", post_level: "members" },
  { id: "fire-disciples", name: "Fire Disciples", read_level: "members", post_level: "members" },
];

const settle = async () => { for (let i = 0; i < 6; i++) await new Promise((r) => setImmediate(r)); };

/** The chat page's scripts, signed in as `me`; the address at `/chat` + `hash`. */
function loadChat(o) {
  const opts = Object.assign({ host: "united-humanity.us", hash: "", saved: null, carried: null }, o || {});
  const doc = H.fakeDocument();
  const copyBtn = { onclick: null };
  const els = {
    messages: keeper(),
    "channel-header": keeper({ querySelector: (sel) => (sel === ".ch-copy-link" ? copyBtn : null) }),
    "server-list": keeper(),
  };
  const document = new Proxy({}, {
    get(_t, p) {
      if (p === "getElementById") return (id) => els[id] || doc.getElementById(id) || anything();
      if (p === "createElement") return (tag) => doc.createElement(tag);
      if (p === "createTextNode") return (s) => doc.createTextNode(s);
      if (p === "body") return doc.body;
      if (p === "readyState") return "complete";
      if (p === "addEventListener" || p === "removeEventListener") return () => {};
      return anything();
    },
  });
  const addr = fakeAddress(opts.host, "/chat", opts.hash);
  const copied = [];
  const prompted = [];
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  const clipboard = { writeText: async (s) => { copied.push(String(s)); } };
  const navigator = new Proxy({}, {
    get: (_t, p) => (p === "clipboard" ? clipboard : p === "serviceWorker" ? serviceWorker : anything()),
    has: () => true,
  });
  const winListeners = {};
  const localStorage = fakeStorage();
  if (opts.saved) localStorage.setItem("humanity_channel", opts.saved);
  const sessionStorage = fakeStorage();
  if (opts.carried) sessionStorage.setItem("hos_chat_link", opts.carried);
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document, navigator, location: addr.location, history: addr.history,
    localStorage, sessionStorage,
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: () => 0, clearTimeout: () => {}, setInterval: () => 0, clearInterval: () => {},
    WebSocket: Object.assign(function () { return anything(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    addEventListener: (type, f) => { (winListeners[type] = winListeners[type] || []).push(f); },
    removeEventListener: () => {}, matchMedia: () => anything(),
    requestAnimationFrame: () => 0, Notification: anything(), innerWidth: 1280, innerHeight: 800,
    crypto: globalThis.crypto, btoa: globalThis.btoa, atob: globalThis.atob, prompt: (q, v) => { prompted.push(v); },
    TextEncoder, TextDecoder, URLSearchParams, URL, CustomEvent: function () {},
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  for (const rel of ["shared/i18n-en.js", "shared/i18n.js", "shared/events.js", "shared/friend-pass.js", "shared/reach.js", "shared/block.js",
    "chat/crypto.js", "chat/chat-dm-store.js", "chat/view/timestampPill.js", "chat/view/messageRow.js",
    "shared/chat-access.js", "shared/chat-links.js", "chat/app.js", "chat/chat-messages.js", "chat/chat-dms.js", "chat/chat-social.js",
    "chat/chat-groups-p2p.js", "shared/notify-rules.js", "chat/chat-notify.js", "chat/chat-ui.js"]) {
    if (fs.existsSync(path.join(WEB, rel))) run(rel);
  }
  ctx.hosIcon = () => "";
  // app.js escapes through an element's innerHTML; the stand-in document has none, so a plain escaper stands in.
  ctx.esc = (str) => String(str).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  // What a switch fetches from the server, and the notices it says: recorded, not sent.
  const loads = [];
  ctx.loadHistory = () => { loads.push(vm.runInContext("activeChannel", ctx)); return Promise.resolve(); };
  for (const f of ["loadPinsForChannel", "loadReactionsForChannel", "renderScratchPadNotes", "renderDmConversationFromStore"]) ctx[f] = () => {};
  const notices = [];
  ctx.addNotice = (text) => notices.push(String(text));
  ctx.addSystemMessage = (text) => notices.push(String(text));
  const set = (js) => vm.runInContext(js, ctx);
  set("myKey = 'me'; myName = 'Me'; myIdentity = { canSign: true }; peerData = { me: { public_key: 'me', role: '' } };");
  const channelList = (list) => ctx.handleMessage({ type: "channel_list", channels: JSON.parse(JSON.stringify(list || CHANNELS)) });
  const hashchange = (hash) => { addr.location.hash = hash; for (const f of winListeners.hashchange || []) f({}); };
  return { ctx, set, addr, copied, prompted, notices, loads, els, copyBtn, doc, channelList, hashchange, localStorage, sessionStorage };
}

test("1. the website's link rules give every answer of the shared fixture", () => {
  for (const [hash, word] of FX.words) assert.strictEqual(L.wordOf(hash), word, "the word of " + JSON.stringify(hash));
  for (const [word, id] of FX.finds) {
    const ch = L.find(word, FX.channels);
    assert.strictEqual(ch ? ch.id : null, id, "the channel " + JSON.stringify(word) + " names");
  }
  for (const [origin, id, want] of FX.links) assert.strictEqual(L.link(origin, id), want, "the address of " + id + " on " + origin);
  for (const [host, yes] of FX.public_hosts) assert.strictEqual(L.isPublicHost(host), yes, "is " + JSON.stringify(host) + " public");
  for (const [url, want] of FX.parses) assert.deepStrictEqual(L.parse(url), want, "what " + url + " asks for");
});

// Seen red with the app.js before: "nothing loads before the channel list says which channel the
// link names" (actual ['fire-disciples']: it took the hash as an id at once, then cleaned the
// address to /chat); with find() by id only: "its shown name opens it" (actual 'global', the
// room-gone rule landing in #global).
test("2. /chat#<id> opens the channel, so does its shown name in any case, and the address keeps showing it", async () => {
  const a = loadChat({ hash: "#fire-disciples" });
  a.set("onIdentityConfirmed()");
  assert.deepStrictEqual(a.loads, [], "nothing loads before the channel list says which channel the link names");
  await a.channelList();
  assert.strictEqual(a.set("activeChannel"), "fire-disciples", "the link's channel opens");
  assert.strictEqual(a.addr.location.hash, "#fire-disciples", "the address keeps showing the channel");
  assert.ok(a.addr.steps.every(([how]) => how === "replace"), "replaced, never a new step: " + JSON.stringify(a.addr.steps));
  assert.strictEqual(a.localStorage.getItem("humanity_channel"), "fire-disciples", "and it is the channel to come back to");
  assert.deepStrictEqual(a.loads, ["fire-disciples"], "its history loads, once");

  const b = loadChat({ hash: "#Fire%20Disciples", saved: "global" });
  b.set("onIdentityConfirmed()");
  await b.channelList();
  assert.strictEqual(b.set("activeChannel"), "fire-disciples", "its shown name opens it");
  assert.strictEqual(b.addr.location.hash, "#fire-disciples", "and the address shows its id");

  const c = loadChat({ hash: "#GLOBAL", saved: "feedback" });
  c.set("onIdentityConfirmed()");
  await c.channelList();
  assert.strictEqual(c.set("activeChannel"), "global", "an id in another case opens it, over the saved channel");
});

// Seen red with the app.js before: "the first open shows its channel" (actual ''); with the
// chat-dms.js before: "a direct message names no one in the address" (actual '#feedback').
test("3. the address follows a switch, and never names a direct message, a group or the scratch pad", async () => {
  const a = loadChat({ saved: "global" });
  a.set("onIdentityConfirmed()");
  await a.channelList();
  assert.strictEqual(a.addr.location.hash, "#global", "the first open shows its channel");
  a.set("switchChannel('feedback')");
  assert.strictEqual(a.addr.location.hash, "#feedback", "the address follows a switch");
  a.set("openDmConversation('" + "ab".repeat(20) + "', 'Ann')");
  assert.strictEqual(a.addr.location.hash, "", "a direct message names no one in the address");
  assert.ok(!a.addr.steps.some(([, url]) => url.includes("ab".repeat(20))), "the key is never in the address");
  a.set("closeDmView()");
  assert.strictEqual(a.addr.location.hash, "#feedback", "back to the channel, back in the address");
  a.set("switchChannel(SCRATCH_PAD_ID)");
  assert.strictEqual(a.addr.location.hash, "", "the scratch pad is no channel of the server");
  a.set("switchChannel('global')");
  a.set("window.activeP2pGroup = { id: 'g1' }; syncChatAddress()");
  assert.strictEqual(a.addr.location.hash, "", "a group is not in the address");
  a.set("window.activeP2pGroup = null");

  // A conversation link: opened, and the address left holding no key.
  const key = "cd".repeat(20);
  const b = loadChat({ hash: "#dm=" + key, saved: "global" });
  b.set("onIdentityConfirmed()");
  assert.strictEqual(b.set("pendingDmFromLink"), key, "#dm=<key> waits for the conversations to load, as before");
  assert.ok(!b.addr.location.hash.includes(key), "and is gone from the address at once");
  await b.channelList();
  await settle();
  assert.strictEqual(b.set("activeDmPartner"), key, "the conversation opens once the conversations are loaded");
  assert.strictEqual(b.addr.location.hash, "", "and the address names no one");
  assert.ok(!b.addr.steps.some(([, url]) => url.includes(key)), "never, at any step");
});

// Seen red with the app.js before: "an edited address switches channel" (actual 'global').
test("4. an edited address switches channel, a link in a message opens in place, and a missing channel says so", async () => {
  const a = loadChat({ saved: "global" });
  a.set("onIdentityConfirmed()");
  await a.channelList();
  a.hashchange("#feedback");
  assert.strictEqual(a.set("activeChannel"), "feedback", "an edited address switches channel");
  const before = a.loads.length;
  a.hashchange("#Feedback");
  assert.strictEqual(a.loads.length, before, "naming the channel already open loads nothing again");
  assert.strictEqual(a.addr.location.hash, "#feedback");

  a.hashchange("#nowhere");
  assert.strictEqual(a.set("activeChannel"), "feedback", "a channel that is not here opens no other");
  assert.ok(a.notices.includes(t("chat.link_no_channel", { name: "nowhere" })), "and says so: " + JSON.stringify(a.notices));
  assert.strictEqual(a.addr.location.hash, "#feedback", "the address shows the channel open again");

  a.hashchange("");
  assert.strictEqual(a.addr.location.hash, "#feedback", "a cleared address shows the channel again");

  // A link in a message to this server's own chat, clicked.
  let prevented = false;
  const anchor = { href: "https://united-humanity.us/chat#announcements" };
  const click = (target, extra) => {
    prevented = false;
    const ev = Object.assign({ target: { closest: (sel) => (sel === "a[href]" ? target : null) }, button: 0, preventDefault() { prevented = true; } }, extra || {});
    for (const f of a.els.messages.listeners.click || []) f(ev);
  };
  click(anchor);
  assert.ok(prevented, "a link to this server's chat opens here, not in a new tab");
  assert.strictEqual(a.set("activeChannel"), "announcements", "in its channel");
  click({ href: "https://public.guide/chat#global" });
  assert.ok(!prevented, "a link to another server's chat opens as any link does");
  click({ href: "https://united-humanity.us/chat#feedback" }, { ctrlKey: true });
  assert.ok(!prevented, "and so does a click with a modifier key");
  assert.strictEqual(a.set("activeChannel"), "announcements");

  // A link the chat opens with, naming no channel here: said, and the channel saved opens.
  const b = loadChat({ hash: "#nowhere", saved: "feedback" });
  b.set("onIdentityConfirmed()");
  await b.channelList();
  assert.strictEqual(b.set("activeChannel"), "feedback", "no other channel opens in its place");
  assert.ok(b.notices.includes(t("chat.link_no_channel", { name: "nowhere" })), "it says so");
  assert.deepStrictEqual(b.loads, ["feedback"], "and the channel open loads");
  assert.strictEqual(b.addr.location.hash, "#feedback");
});

// Seen red with the app.js before: "the header has Copy link"; with the chat-notify.js before:
// "the room's menu has Copy link".
test("5. Copy link, in the header and on the room's menu, copies https://<this server>/chat#<id>", async () => {
  const a = loadChat({ saved: "fire-disciples" });
  a.set("onIdentityConfirmed()");
  await a.channelList();
  const header = a.els["channel-header"].innerHTML;
  assert.ok(header.includes('class="ch-copy-link"'), "the header has Copy link: " + header);
  assert.ok(header.includes(">" + t("chat.copy_link") + "<") && header.includes('data-i18n-tip="chat.copy_link_hint"'), "its words and its hint by key");
  a.copyBtn.onclick();
  await settle();
  assert.deepStrictEqual(a.copied, ["https://united-humanity.us/chat#fire-disciples"], "the channel's address on this server");
  assert.ok(a.notices.includes(t("chat.copy_link_done", { link: "https://united-humanity.us/chat#fire-disciples" })), "and says what was copied");

  // The room's menu (a right-click on its row, chat-notify.js).
  const row = { getAttribute: (k) => (k === "data-channel-id" ? "global" : null), getBoundingClientRect: () => ({ left: 0, bottom: 0 }) };
  const ev = { target: { closest: () => row }, clientX: 10, clientY: 10, preventDefault() {}, stopPropagation() {} };
  for (const f of a.els["server-list"].listeners.contextmenu || []) f(ev);
  const items = H.everyElement(a.doc).filter((e) => e.tagName === "BUTTON");
  const copy = items.find((b) => b.textContent === t("chat.copy_link"));
  assert.ok(copy, "the room's menu has Copy link: " + items.map((b) => b.textContent).join(", "));
  assert.strictEqual(copy.getAttribute("data-i18n-tip"), "chat.copy_link_hint", "with its hint");
  copy.click();
  await settle();
  assert.strictEqual(a.copied[1], "https://united-humanity.us/chat#global", "the room's own address");

  // A server with no public address: copied all the same, and said so.
  const local = loadChat({ host: "127.0.0.1:3210", saved: "global" });
  local.set("onIdentityConfirmed()");
  await local.channelList();
  local.copyBtn.onclick();
  await settle();
  assert.deepStrictEqual(local.copied, ["http://127.0.0.1:3210/chat#global"]);
  assert.ok(local.notices.includes(t("chat.copy_link_done_private", { link: "http://127.0.0.1:3210/chat#global" })), "it says the link opens only here: " + JSON.stringify(local.notices));
});

// ── 6 and 7. A visitor ──

const READ_IDS = ["read-tabs", "read-refresh", "read-list", "read-status", "read-join"];

async function bootRead(answers, hash) {
  const doc = H.fakeDocument();
  for (const id of READ_IDS) {
    const e = doc.createElement(id === "read-refresh" ? "button" : id === "read-join" ? "a" : "div");
    e.setAttribute("id", id);
    if (id === "read-refresh") doc.getElementById("read-tabs").appendChild(e);
    else doc.body.appendChild(e);
  }
  const asked = [];
  const fetch = (url) => {
    asked.push(String(url));
    const p = String(url).split("?")[0];
    const body = answers[p];
    const a = typeof body === "function" ? body(String(url)) : body;
    if (a && a.status) return Promise.resolve({ ok: false, status: a.status, json: () => Promise.resolve(a.body || {}) });
    if (a === undefined) return Promise.resolve({ ok: false, status: 404, json: () => Promise.resolve({ error: "no" }) });
    return Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve(JSON.parse(JSON.stringify(a))) });
  };
  const addr = fakeAddress("united-humanity.us", "/chat/read", hash || "");
  const sessionStorage = fakeStorage();
  const winListeners = {};
  const ctx = {
    document: doc, fetch, console, Date, URLSearchParams, encodeURIComponent, decodeURIComponent, URL,
    location: addr.location, history: addr.history, sessionStorage,
    addEventListener: (type, f) => { (winListeners[type] = winListeners[type] || []).push(f); },
  };
  ctx.window = ctx;
  ctx.i18n = { t, hint: (el, key) => { el.setAttribute("data-i18n-tip", key); el.setAttribute("data-tip-desc", t(key)); return el; } };
  vm.createContext(ctx);
  for (const rel of ["shared/chat-access.js", "shared/chat-links.js", "chat/read.js"]) {
    if (fs.existsSync(path.join(WEB, rel))) vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  }
  await H.settle();
  await settle();
  const hashchange = async (h) => { addr.location.hash = h; for (const f of winListeners.hashchange || []) f({}); await settle(); };
  const tabs = () => H.everyElement(doc).filter((e) => e.className === "read-tab");
  const open = () => tabs().filter((b) => b.getAttribute("aria-selected") === "true").map((b) => b.dataset.channel);
  return { doc, asked, addr, sessionStorage, hashchange, tabs, open, status: () => doc.getElementById("read-status").textContent };
}

const ANYONE = { channels: [{ id: "announcements", name: "announcements" }, { id: "general", name: "global" }, { id: "feedback", name: "feedback" }] };
const ANYONE_WITH_GLOBAL = { channels: [{ id: "announcements", name: "announcements" }, { id: "global", name: "global" }, { id: "feedback", name: "feedback" }] };

/** The preflight in web/chat/index.html, run for a browser with no key at `/chat` + `hash`: where it sends it. */
function preflight(hash) {
  const html = fs.readFileSync(path.join(WEB, "chat", "index.html"), "utf8");
  const m = html.match(/<script>\s*(\(function preflightIdentity\(\)[\s\S]*?\}\)\(\);)\s*<\/script>/);
  assert.ok(m, "index.html has the preflight");
  const went = [];
  const location = { hash, pathname: "/chat", replace: (u) => went.push(String(u)) };
  const ctx = { location, localStorage: fakeStorage(), history: { replaceState() {} } };
  vm.createContext(ctx);
  vm.runInContext(m[1], ctx);
  return went;
}

// Seen red with the index.html before: "the redirect keeps the link's channel" (actual
// ['/chat/read']); with the read.js before: "the link's channel opens" (actual ['announcements']);
// with the app.js before: "and the chat opens it once they have a name"; with find() by id only:
// "by its shown name (id general shown as global)" (actual []).
test("6. a visitor's link keeps its channel to the read view, which opens it, or says it is for members or not there", async () => {
  assert.deepStrictEqual(preflight("#fire-disciples"), ["/chat/read#fire-disciples"], "the redirect keeps the link's channel");
  assert.deepStrictEqual(preflight(""), ["/chat/read"], "no link, no hash");
  assert.deepStrictEqual(preflight("#dm=" + "ab".repeat(20)), ["/chat/read"], "a conversation link is no channel: not carried");

  const posts = { messages: [{ from: "", from_name: "ann", content: "hello", timestamp: 1_792_800_000_000 }], cursor: 1 };
  const byId = await bootRead({ "/api/channels": ANYONE_WITH_GLOBAL, "/api/messages": posts }, "#feedback");
  assert.deepStrictEqual(byId.open(), ["feedback"], "the link's channel opens");
  assert.ok(byId.asked.some((u) => u.startsWith("/api/messages?channel=feedback")), "and is read");
  assert.strictEqual(byId.addr.location.hash, "#feedback");

  const byName = await bootRead({ "/api/channels": ANYONE, "/api/messages": posts }, "#Global");
  assert.deepStrictEqual(byName.open(), ["general"], "by its shown name (id general shown as global)");
  assert.strictEqual(byName.addr.location.hash, "#general", "the address shows its id");

  // A members' channel: the server says 401 and its level; the page says so, opens no tab, and
  // names no channel but the link's.
  const members = await bootRead({
    "/api/channels": ANYONE_WITH_GLOBAL,
    "/api/messages": (u) => (u.includes("channel=fire-disciples") ? { status: 401, body: { error: "x", read_level: "members" } } : posts),
  }, "#fire-disciples");
  assert.strictEqual(members.status(), t("chat_read.link_members", { name: "fire-disciples" }), "a members' channel says so");
  assert.deepStrictEqual(members.open(), [], "and no other channel opens in its place");
  assert.strictEqual(members.doc.getElementById("read-list").children.length, 0, "nothing of it is drawn");
  assert.deepStrictEqual(members.tabs().map((b) => b.dataset.channel), ["announcements", "global", "feedback"], "the tabs are the channels anyone may read, no other");
  assert.ok(t("chat_read.link_members", { name: "x" }).includes(t("chat_read.join")), "with Choose a name to join in");

  const verified = await bootRead({ "/api/channels": ANYONE_WITH_GLOBAL, "/api/messages": { status: 401, body: { read_level: "verified" } } }, "#vip");
  assert.strictEqual(verified.status(), t("chat_read.link_verified", { name: "vip" }), "a verified channel says so");

  const missing = await bootRead({ "/api/channels": ANYONE_WITH_GLOBAL, "/api/messages": (u) => (u.includes("channel=nowhere") ? { status: 404, body: { error: "x" } } : posts) }, "#nowhere");
  assert.strictEqual(missing.status(), t("chat_read.link_missing", { name: "nowhere" }), "a channel that is not there says so");
  assert.deepStrictEqual(missing.open(), [], "and opens no other");

  // An edited address with the view open.
  await missing.hashchange("#feedback");
  assert.deepStrictEqual(missing.open(), ["feedback"], "an edited address opens its channel");
  missing.tabs().find((b) => b.dataset.channel === "announcements").click();
  await settle();
  assert.strictEqual(missing.addr.location.hash, "#announcements", "the address follows the tab chosen");

  // Choose a name to join in carries the channel: the chat opens it after the welcome step.
  members.doc.getElementById("read-join").click();
  assert.strictEqual(members.sessionStorage.getItem("hos_chat_link"), "fire-disciples", "the link's channel is carried");
  const chat = loadChat({ carried: "fire-disciples" });
  chat.set("onIdentityConfirmed()");
  await chat.channelList();
  assert.strictEqual(chat.set("activeChannel"), "fire-disciples", "and the chat opens it once they have a name");
  assert.strictEqual(chat.sessionStorage.getItem("hos_chat_link"), null, "read once");
});

// Seen red with the read.js before: "with no link the read view opens #global" (it opened
// #announcements, the first channel listed); with the app.js before: "and the address says so".
test("7. with no link and nothing saved, every entry point lands on #global", async () => {
  const read = await bootRead({ "/api/channels": ANYONE_WITH_GLOBAL, "/api/messages": { messages: [], cursor: 0 } });
  assert.deepStrictEqual(read.open(), ["global"], "with no link the read view opens #global");
  const noGlobal = await bootRead({ "/api/channels": { channels: [{ id: "announcements", name: "announcements" }, { id: "feedback", name: "feedback" }] }, "/api/messages": { messages: [], cursor: 0 } });
  assert.deepStrictEqual(noGlobal.open(), ["announcements"], "on a server without #global, the first channel listed");

  const chat = loadChat({});
  assert.strictEqual(chat.set("activeChannel"), "global", "the chat's first open, nothing saved, is #global");
  chat.set("onIdentityConfirmed()");
  await chat.channelList();
  assert.strictEqual(chat.set("activeChannel"), "global");
  assert.strictEqual(chat.addr.location.hash, "#global", "and the address says so");

  // A switch to another server is a link to its chat with no channel: it lands as a first open does.
  const strip = require(path.join(WEB, "shared", "server-strip.js"));
  const links = strip.switchLinks([{ name: "Public Guide", url: "https://public.guide", status: "online" }], "united-humanity.us", "/chat");
  assert.strictEqual(links[0].href, "https://public.guide/chat", "the switch carries no channel");
  assert.strictEqual(L.parse(links[0].href), null, "so it names none, and the other server opens its saved channel or #global");
});
