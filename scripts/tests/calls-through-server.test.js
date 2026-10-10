// Calls and voice rooms go through the server, in the web chat client (step E, 2026-10-09,
// docs/design/blocking-and-safe-mode.md 10f, with 7.1 to 7.4 for why).
//
// Run: node --test scripts/tests/calls-through-server.test.js   (in `just rig-tests`)
//
// Why it matters: a direct WebRTC connection shows each person in a call or voice room everyone
// else's network address (a rough location and the internet provider), and every call asked
// Google's address lookup (STUN) too. Now a room or an accepted call asks the relay for
// `call_credentials` over the signed-in chat socket, waits for the reply, and builds its peer
// connection from exactly the reply's urls, username and credential with iceTransportPolicy
// 'relay', so the others see only the server's address. No credentials: the page says 10f's
// sentence and connects nothing, never falling back to a direct connection. The P2P group mesh and
// the contact-card channels, which also connected people directly, are not opened; a link between
// your own devices (data sync) still is.
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with the
// DOM replaced by a stub that keeps what is written to it, and settle() waiting until the page is
// idle, as in block-web.test.js: the real events.js, friend-pass.js, reach.js, block.js, app.js,
// chat-groups-p2p.js, chat-voice-rooms.js, chat-voice-calls.js, chat-voice-webrtc.js,
// chat-voice-streaming.js, chat-privacy.js and chat-p2p.js, in index.html's order (so every
// wrapper the later voice scripts put around the room and call functions is in the path). The
// peer connection, the microphone, the timers and the network are stand-ins that record what they
// were asked. HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves (10f's client items, the web half):
//  1. No Google STUN host anywhere under web/, and no call or room fetches /api/turn-credentials.
//  2. Joining a voice room sends the join, then `call_credentials` for that room, and asks for the
//     microphone and builds peer connections only after the reply, each with exactly the reply's
//     urls, username and credential and iceTransportPolicy 'relay' (one request for the room).
//  3. Accepting a call (and the caller's side when the accept arrives) sends `call_credentials`
//     for that call, and the call's peer connection is built from the reply the same way.
//  4. No credentials (no reply in time, or a reply missing a field): 10f's sentence in the call
//     bar's status line and in the chat, no peer connection, no microphone; the room is left and the
//     call is hung up, and a later offer for that room connects nothing either.
//  5. The group mesh is not opened (a refresh of an open group with an online member sends no
//     direct-connection offer and builds no peer connection), nor a contact-card channel, while a
//     link to my own other device still opens, with the server's own STUN list.
//  6. Credentials came but the forwarder gave a connection no address by the end of gathering (its
//     port not open yet, 10f): the sentence, the room is left or the call hung up, nothing retried;
//     a connection that did get an address is left alone.
//
// Red first, 2026-10-09: each run with HOS_WEB_DIR at a copy of web/ holding the mutation named,
// and seen failing with the assertion named (each passing again on the real web/):
//  1: HEAD's web/ (before step E): "no Google STUN host under web/" (chat-voice-rooms.js); the same
//     with its two Google lines taken out: "chat-voice-rooms.js does not fetch
//     /api/turn-credentials"; a fetch of it added at load to chat-p2p.js (ownDeviceRtcConfig()):
//     "no call or room fetches /api/turn-credentials".
//  2: HEAD's web/: "the join, then this room's call_credentials, in that order" (no request went
//     out). connectToRoomPeer building `new RTCPeerConnection({ iceServers: [] })` instead of
//     relayOnlyRtcConfig(creds): "built from exactly the reply, relay only"; relayOnlyRtcConfig
//     without its iceTransportPolicy line: the same assertion, here and in test 3.
//  3: acceptIncomingCall without its callCredentialsFor line: "accept, then this call's
//     call_credentials"; setupPeerConnection not awaiting credentials (building at once): "no peer
//     connection before the reply".
//  4: joinVoiceRoom without its `if (!creds)` line: "the sentence, in the chat"; joinVoiceRoom
//     asking for the microphone before the credentials: "no microphone" (and "no microphone before
//     the reply" in test 2); requestCallCredentials's timer not set: "the sentence, in the chat"
//     (nothing ended the wait); callCredentialsFromReply not checking `credential`: "a reply
//     without a credential is no credentials".
//  5: HEAD's web/: "no direct-connection offer to a group member" (the mesh offered to the member
//     whose key sorts below mine); initDataChannel without its mayLinkDirectly line, with the old
//     mesh call put back in _p2pRefresh: the same assertion (with the mesh put back but that line
//     kept, the test passes: initDataChannel refuses on its own); mayAnswerDirectOffer also true
//     for a contact: "a contact's offer is not answered, nor a group member's".
//  6: the room's end-of-gathering check taken out: "one that got none: the sentence"; the call's:
//     "the sentence"; the room's candidate count taken out: "a connection that got an address from
//     the forwarder goes on".

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

// An element that keeps what is written to it (text, classes, children, style) and absorbs the rest.
function fakeElement(tag) {
  const classes = new Set();
  const own = {
    tag, children: [], style: {}, dataset: {}, className: "", textContent: "", innerHTML: "", value: "",
    classList: {
      add: (...c) => c.forEach((x) => classes.add(x)),
      remove: (...c) => c.forEach((x) => classes.delete(x)),
      toggle: (c, force) => { const on = force === undefined ? !classes.has(c) : !!force; if (on) classes.add(c); else classes.delete(c); return on; },
      contains: (c) => classes.has(c),
    },
  };
  own.appendChild = (c) => { own.children.push(c); return c; };
  return new Proxy(own, {
    get(t, prop) {
      if (prop === "then") return undefined;
      if (prop in t) return t[prop];
      return anything();
    },
    set(t, prop, v) { t[prop] = v; return true; },
  });
}

// The document: one kept element per id; no element matches a selector.
function fakeDocument() {
  const byId = new Map();
  return new Proxy({}, {
    get(_t, prop) {
      if (prop === "createElement") return fakeElement;
      if (prop === "getElementById") return (id) => { if (!byId.has(id)) byId.set(id, fakeElement(id)); return byId.get(id); };
      if (prop === "querySelectorAll") return () => [];
      if (prop === "querySelector") return () => null;
      if (prop === "hidden") return false;
      return anything();
    },
    set: () => true,
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

function fakeSocket() {
  return { readyState: 1, sent: [], send(s) { this.sent.push(JSON.parse(s)); }, close() {} };
}

const ME = "a1".repeat(32);
const ANN = "b2".repeat(32);
const BEN = "c3".repeat(32);
const CONTACT = "d4".repeat(32);
const CALLER = "f6".repeat(32);
const GROUP_MATE = "07".repeat(32); // sorts below ME, so the old mesh had me offer to them
const ROOM = "7";
const TURN_URL = "turn:calls.example.org:3478?transport=udp";
const STUN_URL = "stun:calls.example.org:3478";
const OWN_STUN = "stun:ours.example.org:3478";
const SENTENCE = "Calls go through the server to keep your address private; this server is not set up for that yet.";

// The page's asynchronous work settles in microtasks and setImmediate turns (no WebCrypto is
// loaded here, but the counter is kept from block-web.test.js so a page that starts using it is
// still waited for): wait until it has been idle for a dozen turns rather than a fixed count.
let cryptoInFlight = 0;
const countedSubtle = new Proxy(globalThis.crypto.subtle, {
  get(t, prop) {
    const v = t[prop];
    if (typeof v !== "function") return v;
    return (...args) => {
      cryptoInFlight++;
      return Promise.resolve(v.apply(t, args)).finally(() => { cryptoInFlight--; });
    };
  },
});
async function settle() {
  let idle = 0;
  for (let i = 0; i < 5000 && idle < 12; i++) {
    await new Promise((r) => setImmediate(r));
    idle = cryptoInFlight === 0 ? idle + 1 : 0;
  }
}

function loadChat() {
  const pcs = [];
  const fetches = [];
  const media = { asked: 0 };
  const timers = [];
  const intervals = [];
  const appended = [];
  // Every peer connection the page builds, with the settings it was built with.
  function FakePeerConnection(config) {
    pcs.push(this);
    this.config = JSON.parse(JSON.stringify(config === undefined ? null : config));
    this.connectionState = "new";
    this.iceGatheringState = "new";
    this.remote = null;
    this.addTrack = () => {};
    this.getSenders = () => [];
    this.createOffer = async () => ({ type: "offer", sdp: "v=0 test-offer" });
    this.createAnswer = async () => ({ type: "answer", sdp: "v=0 test-answer" });
    this.setLocalDescription = async () => {};
    this.setRemoteDescription = async (d) => { this.remote = d; };
    this.addIceCandidate = async () => {};
    this.createDataChannel = () => ({ readyState: "connecting", send() {} });
    this.getStats = async () => new Map();
    this.setConfiguration = () => {};
    this.close = () => { this.connectionState = "closed"; };
  }
  const track = { kind: "audio", enabled: true, stop() {}, getSettings: () => ({}) };
  const mediaDevices = {
    getUserMedia: async () => { media.asked++; return { getTracks: () => [track], getAudioTracks: () => [track], getVideoTracks: () => [] }; },
    enumerateDevices: async () => [],
    addEventListener() {},
  };
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDocument(),
    navigator: new Proxy({}, { get: (_t, p) => (p === "mediaDevices" ? mediaDevices : anything()), has: () => true }),
    location: { hash: "", host: "localhost", hostname: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    // No network but the server's own STUN list (a link between my own devices may use it).
    fetch: (url) => {
      fetches.push(String(url));
      if (String(url).startsWith("/api/turn-credentials")) {
        return Promise.resolve({ ok: true, json: async () => ({ iceServers: [{ urls: OWN_STUN }], ttl: 3600 }) });
      }
      return Promise.reject(new Error("no network in tests"));
    },
    // Timers are kept, not run: a test fires the ones it means to (the credentials wait).
    setTimeout: (fn, ms) => { timers.push({ fn, ms: Number(ms) || 0, live: true }); return timers.length; },
    clearTimeout: (id) => { const t = timers[id - 1]; if (t) t.live = false; },
    setInterval: (fn, ms) => { intervals.push({ fn, ms: Number(ms) || 0 }); return 100000 + intervals.length; },
    clearInterval: () => {},
    WebSocket: Object.assign(function () { return fakeSocket(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    RTCPeerConnection: FakePeerConnection,
    RTCSessionDescription: function (d) { return d; },
    RTCIceCandidate: function (c) { return c; },
    Audio: function () { return anything(); },
    addEventListener: () => {},
    removeEventListener: () => {},
    matchMedia: () => anything(),
    requestAnimationFrame: () => 0,
    Notification: anything(),
    crypto: { subtle: countedSubtle, getRandomValues: (a) => globalThis.crypto.getRandomValues(a), randomUUID: () => globalThis.crypto.randomUUID() },
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
  // In index.html's order.
  run("shared/events.js");
  run("shared/friend-pass.js");
  run("shared/reach.js");
  run("shared/block.js");
  run("chat/app.js");
  // Page pieces from scripts this test does not load (chat-ui.js, icons.js, hold-confirm.js).
  for (const name of ["updateUserList", "updateStats", "renderServerList", "renderGroupList", "hosIcon", "shortKey", "holdConfirm", "playNotificationChime"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  run("chat/chat-groups-p2p.js");
  run("chat/chat-voice-rooms.js");
  run("chat/chat-voice-calls.js");
  run("chat/chat-voice-webrtc.js");
  run("chat/chat-voice-streaming.js");
  run("chat/chat-privacy.js");
  run("chat/chat-p2p.js");
  // What is written into the message area (system lines).
  ctx.appendMessage = (el) => { appended.push(el); };
  const sock = fakeSocket();
  vm.runInContext("(s, me) => { ws = s; myKey = me; myName = 'Me_1'; }", ctx)(sock, ME);
  const handle = async (msg) => { await vm.runInContext("handleMessage", ctx)(msg); await settle(); };
  const fn = (name) => vm.runInContext(name, ctx);
  // Run the kept timers of one delay (the credentials wait), as if that much time had passed.
  const fire = async (ms) => {
    for (const t of timers.filter((x) => x.live && x.ms === ms)) { t.live = false; t.fn(); }
    await settle();
  };
  const waitMs = vm.runInContext("typeof CALL_CREDENTIALS_WAIT_MS === 'number' ? CALL_CREDENTIALS_WAIT_MS : 8000", ctx);
  const status = () => ctx.document.getElementById("ringing-status");
  const said = () => appended.map((e) => String(e.textContent || ""));
  return { ctx, sock, pcs, fetches, media, intervals, handle, fn, fire, waitMs, status, said };
}

// What the relay sends back for a room or a call (10f's reply).
function reply(scope, extra = {}) {
  return { type: "call_credentials", ...scope, urls: [TURN_URL, STUN_URL], username: "1760000000:abc123", credential: "c2VjcmV0LW1hYw==", ttl: 3600, ...extra };
}

// The settings 10f says a room's or call's peer connection is built with.
const RELAY_ONLY = {
  iceServers: [{ urls: [TURN_URL, STUN_URL], username: "1760000000:abc123", credential: "c2VjcmV0LW1hYw==" }],
  iceTransportPolicy: "relay",
};

const ofType = (sock, type) => sock.sent.filter((m) => m.type === type);
const kinds = (sock) => sock.sent.map((m) => m.type + (m.action ? ":" + m.action : "") + (m.signal_type ? ":" + m.signal_type : ""));

function walk(dir) {
  const out = [];
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...walk(p));
    else out.push(p);
  }
  return out;
}

test("no Google STUN host anywhere under web/, and no call or room fetches /api/turn-credentials", { timeout: 20000 }, () => {
  const google = /stun\d*\.l\.google\.com|stun:[^\s'"`]*google/i;
  const found = walk(WEB).filter((f) => google.test(fs.readFileSync(f, "latin1")));
  assert.deepStrictEqual(found.map((f) => path.relative(path.join(WEB, ".."), f).replace(/\\/g, "/")), [],
    "no Google STUN host under web/");
  // The voice scripts never name the endpoint (a link between my own devices, in chat-p2p.js, may).
  const voice = fs.readdirSync(path.join(WEB, "chat")).filter((n) => /^chat-voice-.*\.js$/.test(n));
  assert.ok(voice.length >= 4, "the voice scripts were found");
  for (const n of voice) {
    assert.ok(!fs.readFileSync(path.join(WEB, "chat", n), "utf8").includes("/api/turn-credentials"), n + " does not fetch /api/turn-credentials");
  }
  // And loading the page fetches nothing for calls.
  const { fetches } = loadChat();
  assert.deepStrictEqual(fetches.filter((u) => u.includes("turn-credentials")), [], "no call or room fetches /api/turn-credentials");
});

test("joining a voice room asks for its credentials, then connects through the server only", { timeout: 20000 }, async () => {
  const { ctx, sock, pcs, fetches, media, handle, fn } = loadChat();
  ctx._voiceChannels = [{ id: 7, name: "Lounge", participants: [{ public_key: ANN, display_name: "Ann" }] }];

  fn("joinVoiceRoom")(7); // not awaited: it waits for the reply below
  await settle();
  assert.deepStrictEqual(kinds(sock), ["voice_room:join", "call_credentials"], "the join, then this room's call_credentials, in that order");
  assert.deepStrictEqual(ofType(sock, "call_credentials")[0], { type: "call_credentials", room: ROOM }, "exactly 10f's request");
  assert.strictEqual(pcs.length, 0, "no peer connection before the reply");
  assert.strictEqual(media.asked, 0, "no microphone before the reply");

  await handle(reply({ room: ROOM }));
  assert.strictEqual(media.asked, 1, "the microphone once the credentials came");

  // The relay's roster: Ann and me. The page offers to Ann.
  await handle({ type: "voice_channel_list", channels: [{ id: 7, name: "Lounge", participants: [{ public_key: ME, display_name: "Me_1" }, { public_key: ANN, display_name: "Ann" }] }] });
  assert.strictEqual(pcs.length, 1, "one peer connection, to Ann");
  assert.deepStrictEqual(pcs[0].config, RELAY_ONLY, "built from exactly the reply, relay only");
  assert.strictEqual(sock.sent.filter((m) => m.type === "voice_room_signal" && m.signal_type === "offer" && m.to === ANN).length, 1, "the offer to Ann went out");

  // Ben joins and offers to me: answered over a connection built the same way, no second request.
  await handle({ type: "voice_room_signal", from: BEN, to: ME, room_id: ROOM, signal_type: "offer", data: { type: "offer", sdp: "v=0 ben" } });
  assert.strictEqual(pcs.length, 2, "one more peer connection, to Ben");
  assert.deepStrictEqual(pcs[1].config, RELAY_ONLY, "Ben's built the same way");
  assert.strictEqual(sock.sent.filter((m) => m.type === "voice_room_signal" && m.signal_type === "answer" && m.to === BEN).length, 1, "Ben's offer is answered");
  assert.strictEqual(ofType(sock, "call_credentials").length, 1, "one request for the room");
  assert.deepStrictEqual(fetches.filter((u) => u.includes("turn-credentials")), [], "no /api/turn-credentials fetch");
});

test("accepting a call asks for its credentials, then connects through the server only; so does the caller", { timeout: 20000 }, async () => {
  const { ctx, sock, pcs, fetches, media, handle, fn } = loadChat();

  // Called: ring, Accept, then the caller's offer.
  await handle({ type: "voice_call", from: CALLER, from_name: "Caller", to: ME, action: "ring" });
  fn("acceptIncomingCall")();
  await settle();
  assert.deepStrictEqual(kinds(sock), ["voice_call:accept", "call_credentials"], "accept, then this call's call_credentials");
  assert.deepStrictEqual(ofType(sock, "call_credentials")[0], { type: "call_credentials", call: CALLER }, "exactly 10f's request");
  await handle({ type: "webrtc_signal", from: CALLER, to: ME, signal_type: "offer", data: { type: "offer", sdp: "v=0 caller" } });
  assert.strictEqual(pcs.length, 0, "no peer connection before the reply");
  assert.strictEqual(sock.sent.filter((m) => m.signal_type === "answer").length, 0, "no answer before the reply");

  await handle(reply({ call: CALLER }));
  assert.strictEqual(pcs.length, 1, "the call's peer connection");
  assert.deepStrictEqual(pcs[0].config, RELAY_ONLY, "built from exactly the reply, relay only");
  assert.strictEqual(media.asked, 1, "the microphone once");
  assert.strictEqual(sock.sent.filter((m) => m.type === "webrtc_signal" && m.signal_type === "answer" && m.to === CALLER).length, 1, "the offer is answered");
  fn("hangupCall")();
  await settle();

  // Calling: ring Ann; her accept arrives; the page asks, then offers.
  sock.sent.length = 0;
  fn("startCall")(ANN, "Ann");
  await handle({ type: "voice_call", from: ANN, from_name: "Ann", to: ME, action: "accept" });
  assert.deepStrictEqual(kinds(sock), ["voice_call:ring", "call_credentials"], "ring, then (on her accept) this call's call_credentials");
  assert.deepStrictEqual(ofType(sock, "call_credentials")[0], { type: "call_credentials", call: ANN });
  assert.strictEqual(pcs.length, 1, "no new peer connection before the reply");
  await handle(reply({ call: ANN }));
  assert.strictEqual(pcs.length, 2, "the call's peer connection");
  assert.deepStrictEqual(pcs[1].config, RELAY_ONLY, "built from exactly the reply, relay only");
  assert.strictEqual(sock.sent.filter((m) => m.type === "webrtc_signal" && m.signal_type === "offer" && m.to === ANN).length, 1, "the offer went out");
  assert.strictEqual(vm.runInContext("callState", ctx), "in-call");
  assert.deepStrictEqual(fetches.filter((u) => u.includes("turn-credentials")), [], "no /api/turn-credentials fetch");
});

test("no credentials: the sentence, no peer connection, no microphone, and nothing direct", { timeout: 20000 }, async () => {
  // A voice room: no reply comes.
  {
    const { ctx, sock, pcs, media, handle, fn, fire, waitMs, status, said } = loadChat();
    fn("joinVoiceRoom")(ROOM); // not awaited: it waits for the credentials
    await settle();
    await fire(waitMs);
    assert.ok(said().some((t) => t.includes(SENTENCE)), "the sentence, in the chat");
    assert.strictEqual(status().textContent, SENTENCE, "and in the call bar's status line");
    assert.ok(status().classList.contains("active"), "which is shown");
    assert.strictEqual(pcs.length, 0, "no peer connection");
    assert.strictEqual(media.asked, 0, "no microphone");
    assert.deepStrictEqual(kinds(sock), ["voice_room:join", "call_credentials", "voice_room:leave"], "the room is left again");
    assert.strictEqual(ctx._currentRoomId, null);
    // Someone still in the room offers: nothing connects, nothing is asked.
    await handle({ type: "voice_room_signal", from: ANN, to: ME, room_id: ROOM, signal_type: "offer", data: { type: "offer", sdp: "v=0 ann" } });
    assert.strictEqual(pcs.length, 0, "a later offer for the room connects nothing");
    assert.strictEqual(media.asked, 0, "nor asks for the microphone");
  }
  // A call I accepted: no reply comes.
  {
    const { ctx, sock, pcs, media, handle, fn, fire, waitMs, status, said } = loadChat();
    await handle({ type: "voice_call", from: CALLER, from_name: "Caller", to: ME, action: "ring" });
    fn("acceptIncomingCall")();
    await handle({ type: "webrtc_signal", from: CALLER, to: ME, signal_type: "offer", data: { type: "offer", sdp: "v=0 caller" } });
    await fire(waitMs);
    assert.ok(said().some((t) => t.includes(SENTENCE)), "the sentence, in the chat");
    assert.strictEqual(status().textContent, SENTENCE, "and in the call bar's status line");
    assert.strictEqual(pcs.length, 0, "no peer connection");
    assert.strictEqual(media.asked, 0, "no microphone");
    assert.deepStrictEqual(sock.sent.filter((m) => m.type === "voice_call").map((m) => m.action), ["accept", "hangup"], "the call is hung up");
    assert.strictEqual(vm.runInContext("callState", ctx), "idle");
  }
  // A call I made: the reply has no credential in it.
  {
    const { pcs, media, handle, fn, status } = loadChat();
    fn("startCall")(ANN, "Ann");
    await handle({ type: "voice_call", from: ANN, from_name: "Ann", to: ME, action: "accept" });
    await handle(reply({ call: ANN }, { credential: undefined }));
    assert.strictEqual(status().textContent, SENTENCE, "a reply without a credential is no credentials");
    assert.strictEqual(pcs.length, 0, "no peer connection");
    assert.strictEqual(media.asked, 0, "no microphone");
  }
});

test("the group mesh is not opened, nor a contact's channel; a link to my own other device is", { timeout: 20000 }, async () => {
  const { ctx, sock, pcs, intervals, handle, fn } = loadChat();
  // Signed in enough to open a P2P group.
  ctx.pqSignMessage = async () => new Uint8Array(4);
  vm.runInContext("myDilithiumSecret = new Uint8Array(4);", ctx);

  // Open a group; its first load finds no key (no network here) and stops before the members.
  const before = intervals.length;
  fn("openP2pGroup")("g1", "Garden");
  await settle();
  const poll = intervals.slice(before).find((t) => t.ms === 4000);
  assert.ok(poll, "the group's 4-second poll is set");
  // Now the group has its key and roster, with a member online whose key sorts below mine (the
  // old mesh had me offer to them), and the poll runs: the step that opened the mesh.
  Object.assign(ctx.activeP2pGroup, {
    myFp: "fp-me", rekeyChecked: true, epoch: 1, epochKey: new Uint8Array(32),
    fpToName: { "fp-me": "Me_1", "fp-mate": "Mate" }, fpToKey: { "fp-me": ME, "fp-mate": GROUP_MATE },
  });
  await Promise.resolve(poll.fn()).catch(() => {}); // it then fails to decrypt (no modules here); that is not the point
  await settle();
  assert.deepStrictEqual(sock.sent.filter((m) => m.signal_type === "dc_offer"), [], "no direct-connection offer to a group member");
  assert.strictEqual(pcs.length, 0, "and no peer connection");

  // A contact added from their card: nothing is opened to them, and their offer is not answered.
  vm.runInContext("(k) => { p2pContacts[k] = { name: 'Contact', kyber_pub: null, added_at: 1, dc_status: 'idle' }; }", ctx)(CONTACT);
  await fn("initDataChannel")(CONTACT);
  await settle();
  assert.deepStrictEqual(sock.sent, [], "nothing is opened to a contact");
  const dcOffer = (from) => ({ type: "webrtc_signal", from, to: ME, signal_type: "dc_offer", data: JSON.stringify({ type: "offer", sdp: "v=0 x" }) });
  await handle(dcOffer(CONTACT));
  await handle(dcOffer(GROUP_MATE));
  assert.deepStrictEqual(sock.sent.filter((m) => m.signal_type === "dc_answer"), [], "a contact's offer is not answered, nor a group member's");
  assert.strictEqual(pcs.length, 0);

  // My own other device: the link opens, with the server's own STUN list (not relay only).
  await fn("initDataChannel")(ME);
  await settle();
  assert.strictEqual(sock.sent.filter((m) => m.signal_type === "dc_offer" && m.to === ME).length, 1, "the offer to my own device goes out");
  assert.strictEqual(pcs.length, 1);
  assert.deepStrictEqual(pcs[0].config, { iceServers: [{ urls: OWN_STUN }] }, "with the server's own STUN list");
});

test("credentials but no address from the forwarder (its port closed): the sentence, and the room or call stops", { timeout: 20000 }, async () => {
  const gatheringEnds = (pc) => { pc.iceGatheringState = "complete"; pc.onicegatheringstatechange(); };
  const relayCandidate = { candidate: { toJSON: () => ({ candidate: "candidate:1 1 udp 2 203.0.113.5 50000 typ relay", sdpMid: "0" }) } };
  // A voice room: Ann's connection gathers nothing; Ben's gathered an address and is left alone.
  {
    const { ctx, sock, pcs, handle, fn, status } = loadChat();
    fn("joinVoiceRoom")(ROOM); // not awaited: it waits for the reply below
    await settle();
    await handle(reply({ room: ROOM }));
    await handle({ type: "voice_channel_list", channels: [{ id: 7, name: "Lounge", participants: [{ public_key: ME }, { public_key: BEN }, { public_key: ANN }] }] });
    assert.strictEqual(pcs.length, 2);
    const ben = pcs.find((p) => vm.runInContext("(k) => window._roomPeerConnections[k]", ctx)(BEN) === p);
    const ann = pcs.find((p) => p !== ben);
    ben.onicecandidate(relayCandidate);
    gatheringEnds(ben);
    await settle();
    assert.notStrictEqual(status().textContent, SENTENCE, "a connection that got an address from the forwarder goes on");
    assert.strictEqual(ctx._currentRoomId, ROOM);
    gatheringEnds(ann);
    await settle();
    assert.strictEqual(status().textContent, SENTENCE, "one that got none: the sentence");
    assert.strictEqual(sock.sent.filter((m) => m.type === "voice_room" && m.action === "leave").length, 1, "and the room is left");
    assert.ok(pcs.every((p) => p.connectionState === "closed"), "its connections closed, none retried");
    assert.strictEqual(pcs.length, 2, "no other connection made");
  }
  // A call: the same.
  {
    const { ctx, sock, pcs, handle, fn, status } = loadChat();
    fn("startCall")(ANN, "Ann");
    await handle({ type: "voice_call", from: ANN, from_name: "Ann", to: ME, action: "accept" });
    await handle(reply({ call: ANN }));
    assert.strictEqual(pcs.length, 1);
    gatheringEnds(pcs[0]);
    await settle();
    assert.strictEqual(status().textContent, SENTENCE, "the sentence");
    assert.deepStrictEqual(sock.sent.filter((m) => m.type === "voice_call").map((m) => m.action), ["ring", "hangup"], "the call is hung up");
    assert.strictEqual(vm.runInContext("callState", ctx), "idle");
    assert.strictEqual(pcs.length, 1, "no other connection made");
  }
});
