// Voice survives a reconnect: the web chat re-sends its voice join once a NEW
// socket's identity is accepted (2026-10-02).
//
// Run:  node --test scripts/tests/voice-rejoin.test.js
//
// Why it matters: the relay records WHICH socket holds each person's place in
// voice (src/relay/handlers/live_conns.rs). When a network blip replaces the
// socket, the dead socket's close takes that place with it, so the person
// vanished from every roster while their call audio kept playing, and people
// joining later never connected to them. The relay half (a re-sent join moves
// the place to the new socket) is tested in src/relay/features.rs; this is the
// web half: app.js announces each socket's accepted identify ONCE
// ('socket-identified'), and chat-voice-rooms.js answers with the join.
//
// The page scripts run as they do in the browser, in one shared global scope
// (node:vm), with the DOM replaced by a stub that accepts anything.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = path.join(__dirname, "..", "..", "web");

// A value that is every DOM thing at once: any property is another one, it can
// be called or constructed, and it never runs a callback it is handed.
function anything() {
  const fn = function () {};
  return new Proxy(fn, {
    get(_t, prop) {
      if (prop === Symbol.toPrimitive) return () => "";
      if (prop === Symbol.iterator) return function* () {};
      if (prop === "then") return undefined; // not a promise
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

// A socket that records what it was sent.
function fakeSocket() {
  return { readyState: 1, sent: [], send(s) { this.sent.push(JSON.parse(s)); }, close() {} };
}

function loadChat() {
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
    WebSocket: Object.assign(function () { return fakeSocket(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    addEventListener: () => {},
    removeEventListener: () => {},
    matchMedia: () => anything(),
    requestAnimationFrame: () => 0,
    Notification: anything(),
    crypto: anything(),
    TextEncoder,
    TextDecoder,
    URLSearchParams,
    URL,
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  // The globals the voice script patches or reads, as the earlier chat
  // scripts define them in the page (app.js comes before it in index.html).
  run("shared/events.js");
  run("chat/app.js");
  for (const name of ["updateUserList", "updateStats", "renderServerList", "addSystemMessage", "hosIcon", "shortKey", "esc"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  run("chat/chat-voice-rooms.js");
  // Signed in earlier in the session: this is a RE-connect.
  vm.runInContext("myIdentity = { canSign: true }; identityConfirmed = true;", ctx);
  return ctx;
}

// Feed one relay frame through the page's real message handler.
async function relaySends(ctx, msg) {
  await vm.runInContext("handleMessage", ctx)(msg);
}

const joins = (sock) => sock.sent.filter((m) => m.type === "voice_room" && m.action === "join");

// Seen red 2026-10-02 three ways: with the 'socket-identified' emit removed
// from app.js's peer_list case, and with the hos.on line removed from
// chat-voice-rooms.js, "the new socket re-sends the join" failed (no join);
// with the once-per-socket guard (`ws._identityAccepted`) removed, "one join
// per socket" failed.
test("a new socket whose identity is accepted re-sends the voice join, once", async () => {
  const ctx = loadChat();
  ctx._currentRoomId = "lounge"; // in a call when the network blipped

  const reconnected = fakeSocket();
  vm.runInContext("(s) => { ws = s; }", ctx)(reconnected);
  await relaySends(ctx, { type: "peer_list", peers: [] });
  assert.deepStrictEqual(
    joins(reconnected).map((m) => m.room_id),
    ["lounge"],
    "the new socket re-sends the join"
  );

  // Later peer_list frames are broadcasts (a role or status change), not a
  // new sign-in: no second join.
  await relaySends(ctx, { type: "peer_list", peers: [] });
  assert.strictEqual(joins(reconnected).length, 1, "one join per socket");

  // The next reconnect is a new socket: it joins again.
  const again = fakeSocket();
  vm.runInContext("(s) => { ws = s; }", ctx)(again);
  await relaySends(ctx, { type: "peer_list", peers: [] });
  assert.strictEqual(joins(again).length, 1, "each new socket re-sends the join");
});

test("a socket signing in while not in a call sends no join", async () => {
  const ctx = loadChat();
  ctx._currentRoomId = null;
  const sock = fakeSocket();
  vm.runInContext("(s) => { ws = s; }", ctx)(sock);
  await relaySends(ctx, { type: "peer_list", peers: [] });
  assert.strictEqual(joins(sock).length, 0);
});
