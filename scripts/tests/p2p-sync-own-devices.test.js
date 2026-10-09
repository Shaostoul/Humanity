// Web data sync is between your OWN devices only (2026-10-09).
//
// Run:  node --test scripts/tests/p2p-sync-own-devices.test.js
//
// Why it matters: web/chat/chat-p2p.js keeps a "data sync" that copies the
// calendar, home records, notes, inventory, map pins and more between devices
// over a direct (peer-to-peer) connection. Until this date it answered ANY peer
// who opened a direct connection: a stranger's sync_offer got the whole bundle
// back (map pins and home records can locate a real home), and a stranger's
// sync_data was merged into this browser's storage unasked
// (docs/design/blocking-and-safe-mode.md, defect 3.7.1). Now a sync frame is
// answered only when the peer's key is this identity's own key (the same
// recovery phrase gives every device the same key), and your other device's
// data is merged only when this browser asked for it and you confirm.
//
// The page scripts run as they do in the browser, in one shared global scope
// (node:vm), with the DOM replaced by a stub, as in voice-rejoin.test.js.
// HOS_WEB_DIR points the test at another copy of web/ (used to see it red
// against the code before the fix).

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

const ME = "a1".repeat(32);
const STRANGER = "b2".repeat(32);

// An open data channel that records what was sent on it.
function fakeChannel() {
  return { readyState: "open", sent: [], send(s) { this.sent.push(JSON.parse(s)); } };
}

function loadChat({ confirmAnswer = true } = {}) {
  const notes = [];
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: anything(),
    navigator: anything(),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: () => 0,
    clearTimeout: () => {},
    setInterval: () => 0,
    clearInterval: () => {},
    WebSocket: Object.assign(function () { return anything(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    RTCPeerConnection: function () { return anything(); },
    RTCSessionDescription: function () { return anything(); },
    RTCIceCandidate: function () { return anything(); },
    addEventListener: () => {},
    removeEventListener: () => {},
    matchMedia: () => anything(),
    requestAnimationFrame: () => 0,
    Notification: anything(),
    crypto: anything(),
    confirm: () => confirmAnswer,
    TextEncoder,
    TextDecoder,
    URLSearchParams,
    URL,
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  run("shared/events.js");
  run("chat/app.js");
  for (const name of ["updateUserList", "updateStats", "renderServerList", "hosIcon", "shortKey", "esc", "renderMessage"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  ctx.addSystemMessage = (t) => notes.push(String(t));
  run("chat/chat-p2p.js");
  vm.runInContext(`myKey = ${JSON.stringify(ME)};`, ctx);
  ctx.localStorage.setItem("hos_calendar_v1", JSON.stringify([{ id: "dentist", title: "Dentist", updatedAt: 1 }]));
  ctx.localStorage.setItem("map_pins_v1", JSON.stringify([{ lat: 47.6, lon: -122.3, label: "Home" }]));
  return { ctx, notes };
}

// A direct connection from `peer` is open; returns its channel.
function openChannel(ctx, peer) {
  const dc = fakeChannel();
  vm.runInContext("(k, dc) => { p2pDataChannels[k] = dc; }", ctx)(peer, dc);
  return dc;
}

const handle = (ctx, msg, peer) => vm.runInContext("handleSyncFrame", ctx)(msg, peer);
const calendar = (ctx) => JSON.parse(ctx.localStorage.getItem("hos_calendar_v1"));
const foreignEvent = { hos_calendar_v1: [{ id: "planted", title: "Meet me here", updatedAt: 99 }] };

// Seen red 2026-10-09 against the code before the fix (HOS_WEB_DIR pointed at
// HEAD's web/): the stranger got sync_accept and sync_data carrying the
// calendar and the map pins back, and "planted" was merged into the calendar.
test("a stranger's sync request gets nothing back and changes nothing", async () => {
  const { ctx, notes } = loadChat();
  const dc = openChannel(ctx, STRANGER);

  await handle(ctx, { type: "sync_offer", keys: ["hos_calendar_v1", "map_pins_v1"] }, STRANGER);
  assert.deepStrictEqual(dc.sent, [], "nothing is sent to a stranger");

  await handle(ctx, { type: "sync_data", data: foreignEvent }, STRANGER);
  assert.deepStrictEqual(calendar(ctx).map((e) => e.id), ["dentist"], "a stranger's data is not merged");
  assert.ok(notes.some((n) => n.startsWith("Ignored a data-sync request")), "the person is told it was ignored");
});

test("your own device's offer is answered with a two-way sync", async () => {
  const { ctx } = loadChat();
  const dc = openChannel(ctx, ME);
  await handle(ctx, { type: "sync_offer", keys: ["hos_calendar_v1"] }, ME);
  assert.deepStrictEqual(dc.sent.map((m) => m.type), ["sync_accept", "sync_data"]);
  assert.deepStrictEqual(dc.sent[1].data.hos_calendar_v1.map((e) => e.id), ["dentist"]);
});

test("your own device's data is merged only when this browser asked and you confirm", async () => {
  // Not asked for: ignored, even from your own key.
  let { ctx } = loadChat();
  openChannel(ctx, ME);
  await handle(ctx, { type: "sync_data", data: foreignEvent }, ME);
  assert.deepStrictEqual(calendar(ctx).map((e) => e.id), ["dentist"], "unasked data is not merged");

  // Asked for and confirmed: merged.
  ({ ctx } = loadChat({ confirmAnswer: true }));
  openChannel(ctx, ME);
  vm.runInContext("offerDataSync", ctx)(ME);
  await handle(ctx, { type: "sync_data", data: foreignEvent }, ME);
  assert.deepStrictEqual(calendar(ctx).map((e) => e.id).sort(), ["dentist", "planted"], "asked for and confirmed: merged");

  // Asked for but declined: not merged.
  ({ ctx } = loadChat({ confirmAnswer: false }));
  openChannel(ctx, ME);
  vm.runInContext("offerDataSync", ctx)(ME);
  await handle(ctx, { type: "sync_data", data: foreignEvent }, ME);
  assert.deepStrictEqual(calendar(ctx).map((e) => e.id), ["dentist"], "declined: not merged");
});

test("the Sync button offers only to your own devices", () => {
  const { ctx } = loadChat();
  const stranger = openChannel(ctx, STRANGER);
  vm.runInContext("syncAllPeers", ctx)();
  assert.deepStrictEqual(stranger.sent, [], "no sync offer goes to a stranger");
});
