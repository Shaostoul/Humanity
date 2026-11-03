// The website's channel list (2026-11-02, docs/design/chat-channels.md section 5), run under Node:
//
//   node --test scripts/tests/chat-channels-web.test.js      (in `just rig-tests`)
//
// The operator: "We need to just make local visible like other channels are. I feel like they're
// too hard to find and add a layer of complexity that we don't need."
//
// What it proves: the server's #local is an ordinary row in the list (web/chat/chat-ui.js
// renderServerList), with its level badge and its room bell like #global, #announcements and
// #feedback; the server's name above them is the fold (data-server-toggle) and no room's door, and
// carries its hint by key; another server in the Federation list is one line, no rooms under it.
//
// The page scripts run as in the browser, in one shared global scope (node:vm), as in
// server-list-backoff.test.js; the list's container keeps what is written into it. HOS_WEB_DIR
// points it at another copy of web/ (used to see it red).
//
// Seen red 2026-11-02 with #local filtered out of renderServerList's rows (a copy of chat-ui.js
// with `.filter((ch) => !ch.local_only)` on the shown rooms): "#local is an ordinary row".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");

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

function fakeStorage() {
  const m = new Map();
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
    clear: () => m.clear(),
  };
}

const settle = () => new Promise((r) => setImmediate(r));

/** The chat page's scripts up to chat-ui.js; the server list's container keeps its HTML. */
function loadChat(federated) {
  // Keeps the HTML written into it; anything else asked of it answers like the stand-in.
  const kept = { innerHTML: "", dataset: {} };
  const list = new Proxy(kept, { get: (t, p) => (p in t ? t[p] : anything()), set: (t, p, v) => { t[p] = v; return true; } });
  const document = new Proxy({}, {
    get(_t, p) {
      if (p === "getElementById") return (id) => (id === "server-list" ? list : anything());
      return anything();
    },
  });
  const fetchImpl = (url) => (String(url).startsWith("/api/federation/servers")
    ? Promise.resolve({ ok: true, status: 200, json: async () => federated })
    : Promise.reject(new Error("no network in tests")));
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document,
    navigator: new Proxy({}, { get: (_t, p) => (p === "serviceWorker" ? serviceWorker : anything()), has: () => true }),
    location: { hash: "", host: "united-humanity.us", protocol: "https:", pathname: "/chat", search: "", origin: "https://united-humanity.us", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    fetch: fetchImpl,
    setTimeout: () => 0, clearTimeout: () => {}, setInterval: () => 0, clearInterval: () => {},
    WebSocket: Object.assign(function () { return anything(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    addEventListener: () => {}, removeEventListener: () => {}, matchMedia: () => anything(),
    requestAnimationFrame: () => 0, Notification: anything(), innerWidth: 1280, innerHeight: 800,
    crypto: globalThis.crypto, btoa: globalThis.btoa, atob: globalThis.atob,
    TextEncoder, TextDecoder, URLSearchParams, URL,
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  for (const rel of ["shared/i18n-en.js", "shared/i18n.js", "shared/events.js", "shared/friend-pass.js", "shared/reach.js", "shared/block.js",
    "chat/crypto.js", "chat/chat-dm-store.js", "chat/view/timestampPill.js", "chat/view/messageRow.js",
    "shared/chat-access.js", "chat/app.js", "chat/chat-messages.js", "chat/chat-dms.js", "chat/chat-social.js",
    "chat/chat-groups-p2p.js", "shared/notify-rules.js", "chat/chat-notify.js", "chat/chat-ui.js"]) {
    run(rel);
  }
  // app.js escapes through an element's innerHTML; the stand-in document has none, so a plain escaper stands in.
  ctx.esc = (str) => String(str).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  return { ctx, list };
}

const CHANNELS = [
  { id: "announcements", name: "announcements", read_level: "anyone", post_level: "mods", federated: true },
  { id: "local", name: "local", read_level: "members", post_level: "members", local_only: true, voice_enabled: true },
  { id: "global", name: "global", read_level: "anyone", post_level: "members", federated: true, voice_enabled: true },
  { id: "feedback", name: "feedback", read_level: "anyone", post_level: "members", federated: true },
];

/** The row of channel `id` in the list's HTML: from its opening tag to the next row. */
function rowOf(html, id) {
  const at = html.indexOf(`data-channel-id="${id}"`);
  if (at < 0) return null;
  const start = html.lastIndexOf("<div", at);
  const next = html.indexOf('<div class="channel-item', at);
  return html.slice(start, next < 0 ? undefined : next);
}

test("#local is an ordinary row, and the server's name is the fold", async () => {
  const { ctx, list } = loadChat([{ name: "public.guide", url: "https://public.guide", trust_tier: 2, status: "online", server_id: "pg" }]);
  vm.runInContext("channelList = " + JSON.stringify(CHANNELS) + "; activeChannel = 'global';", ctx);
  vm.runInContext("renderServerList", ctx)();
  for (let i = 0; i < 5; i++) await settle();
  vm.runInContext("renderServerList", ctx)();
  const html = list.innerHTML;

  const local = rowOf(html, "local");
  assert.ok(local, "#local is an ordinary row");
  assert.ok(local.includes('class="channel-item srv-chan'), "drawn as every room is: " + local);
  assert.ok(local.includes("# local"), "under its own name");
  assert.ok(local.includes('data-level="members"'), "with its level badge (Members)");
  const global = rowOf(html, "global");
  assert.ok(local.includes('data-room="local"'), "with its room bell (Mute room), as every room has: " + local);
  assert.ok(global.includes('data-room="global"'), "#global has its own");
  for (const id of ["announcements", "global", "feedback"]) assert.ok(rowOf(html, id), `#${id} is a row too`);
  assert.ok(html.indexOf('data-channel-id="announcements"') < html.indexOf('data-channel-id="local"'), "in the server's order");

  const header = html.slice(html.indexOf('class="server-group-header"'), html.indexOf('class="server-group-channels"'));
  assert.ok(header.includes('data-server-toggle="Humanity"'), "the server's name folds the list");
  assert.ok(!header.includes("data-channel-id"), "and opens no room");
  assert.ok(header.includes('data-i18n-tip="chat.server_row_hint"'), "its hint by key");

  const fed = html.slice(html.indexOf("public.guide"));
  assert.ok(fed.length > 0 && !fed.includes("data-channel-id"), "another server is one line, no rooms under it");
});
