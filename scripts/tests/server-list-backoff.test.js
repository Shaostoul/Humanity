// The web chat's server list asks for /api/federation/servers once, and after a
// failure waits before asking again (2026-10-09).
//
// Run:  node --test scripts/tests/server-list-backoff.test.js
//
// Why it matters: renderServerList (web/chat/chat-ui.js) fetched the federated
// server list whenever it was not loaded yet, and redrew itself after EVERY fetch,
// failed or not. A failed fetch left the list "not loaded", so the redraw fetched
// again at once: a 404 or an unreachable server became thousands of requests from
// every open chat tab, against our own server. Found by the Block build's test,
// which looped on it.
//
// The page scripts run as they do in the browser, in one shared global scope
// (node:vm), with the DOM replaced by a stub, as in voice-rejoin.test.js.
// HOS_WEB_DIR points the test at another copy of web/ (used to see it red).

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

// Let every pending promise and microtask run (the fetch, its await, the .then).
const settle = () => new Promise((r) => setImmediate(r));

function loadChat(serverListAnswer) {
  // Only the federated server list is answered (by the test); every other request fails at once.
  const fetchImpl = (url) => (String(url).startsWith("/api/federation/servers") ? serverListAnswer() : Promise.reject(new Error("no network in tests")));
  // chat-ui.js registers the service worker when it loads.
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: anything(),
    navigator: new Proxy({}, { get: (_t, p) => (p === "serviceWorker" ? serviceWorker : anything()), has: () => true }),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", origin: "https://localhost", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    fetch: fetchImpl,
    setTimeout: () => 0,
    clearTimeout: () => {},
    setInterval: () => 0,
    clearInterval: () => {},
    WebSocket: Object.assign(function () { return anything(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    addEventListener: () => {},
    removeEventListener: () => {},
    matchMedia: () => anything(),
    requestAnimationFrame: () => 0,
    Notification: anything(),
    innerWidth: 1280,
    innerHeight: 800,
    crypto: globalThis.crypto,
    btoa: globalThis.btoa,
    atob: globalThis.atob,
    TextEncoder,
    TextDecoder,
    URLSearchParams,
    URL,
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  // The chat page's own load order up to chat-ui.js (web/chat/index.html), as the Block
  // test loads it; chat-ui.js reads helpers the earlier scripts define.
  for (const rel of ["shared/events.js", "shared/friend-pass.js", "shared/reach.js", "shared/block.js",
    "chat/crypto.js", "chat/chat-dm-store.js", "chat/view/timestampPill.js", "chat/view/messageRow.js",
    "chat/app.js", "chat/chat-messages.js", "chat/chat-dms.js", "chat/chat-social.js",
    "chat/chat-groups-p2p.js", "chat/chat-ui.js"]) {
    run(rel);
  }
  return ctx;
}

// Seen red 2026-10-09 against the code before the fix (HOS_WEB_DIR pointed at a
// copy of web/ with chat-ui.js from HEAD): this test, 51 requests by the time the
// stub stopped answering ("one request, not a loop": 51 !== 1); the next test, 2
// requests where 1 was expected (the page redraws the list twice while it loads,
// and with no one-at-a-time guard each redraw sent its own request).
test("a failing server list is asked for once, not in a loop", async () => {
  let calls = 0;
  const ctx = loadChat(async () => { calls += 1; if (calls >= 50) await new Promise(() => {}); return { ok: false, status: 404, json: async () => [] }; });
  vm.runInContext("renderServerList", ctx)();
  for (let i = 0; i < 20; i++) await settle();
  assert.strictEqual(calls, 1, "one request, not a loop");

  // Redrawing again inside the wait asks nothing more.
  vm.runInContext("renderServerList", ctx)();
  for (let i = 0; i < 5; i++) await settle();
  assert.strictEqual(calls, 1, "no new request before the wait is over");

  // Once the wait is over, one more request.
  vm.runInContext("federatedRetryAt = 0", ctx);
  vm.runInContext("renderServerList", ctx)();
  for (let i = 0; i < 5; i++) await settle();
  assert.strictEqual(calls, 2, "after the wait, asked once more");
});

test("a list that arrives is used and not asked for again", async () => {
  let calls = 0;
  const ctx = loadChat(async () => { calls += 1; return { ok: true, status: 200, json: async () => [{ name: "Elsewhere", url: "https://elsewhere.example" }] }; });
  vm.runInContext("renderServerList", ctx)();
  for (let i = 0; i < 20; i++) await settle();
  assert.strictEqual(calls, 1);
  assert.strictEqual(vm.runInContext("federatedServersFetched", ctx), true);
  assert.strictEqual(vm.runInContext("federatedServers.length", ctx), 1);
});
