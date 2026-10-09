// Web answers a direct-connection offer only from people it already connects
// with (2026-10-09).
//
// Run:  node --test scripts/tests/p2p-direct-offers.test.js
//
// Why it matters: answering a direct-connection offer (`dc_offer`) hands the
// other side this device's network address, which gives away a rough location
// and the internet provider, and opens a channel to them. The relay forwards an
// offer from anyone online, and until this date web/chat/chat-p2p.js
// (handleDCOffer) answered every one, so anyone could learn your address
// without a call (docs/design/blocking-and-safe-mode.md, defects 3.7.1 and
// 3.7.2, section 7.1 item 4). Now only these get an answer: your own key, a
// contact you added, a member of a P2P group you are in, the person you are in
// an accepted call with, and someone in your voice room. Anyone else gets no
// answer at all.
//
// The second half is the same mistake in the 1:1 call path
// (chat-voice-calls.js, handleWebrtcSignalMessage): a call `offer` was taken
// from someone who was only RINGING you, and handleOffer turned the microphone
// on and answered before Accept was pressed. Call signals now count only once
// the call is accepted.
//
// The page scripts run as they do in the browser, in one shared global scope
// (node:vm), with the DOM replaced by a stub, as in p2p-sync-own-devices.test.js.
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

// A socket that records what it was sent.
function fakeSocket() {
  return { readyState: 1, sent: [], send(s) { this.sent.push(JSON.parse(s)); }, close() {} };
}

// Just enough of a peer connection to be answered: it records the remote
// description and hands back an answer.
function FakePeerConnection() {
  this.remote = null;
  this.setRemoteDescription = async (d) => { this.remote = d; };
  this.setLocalDescription = async () => {};
  this.createAnswer = async () => ({ type: "answer", sdp: "v=0 test-answer" });
  this.createOffer = async () => ({ type: "offer", sdp: "v=0 test-offer" });
  this.createDataChannel = () => ({ readyState: "connecting", send() {} });
  this.addTrack = () => {};
  this.addIceCandidate = async () => {};
  this.close = () => {};
}

const ME = "a1".repeat(32);
const STRANGER = "b2".repeat(32);
const CONTACT = "c3".repeat(32);
const GROUP_MATE = "d4".repeat(32);
const ROSTER_MATE = "e5".repeat(32);
const CALLER = "f6".repeat(32);
const ROOM_MATE = "07".repeat(32);

function loadChat() {
  const media = { asked: 0 };
  media.getUserMedia = async () => {
    media.asked++;
    return { getTracks: () => [], getAudioTracks: () => [] };
  };
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: anything(),
    // Every navigator property is the stub, except the microphone, which counts
    // how often it was asked for.
    navigator: new Proxy({}, { get: (_t, p) => (p === "mediaDevices" ? media : anything()) }),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: () => 0,
    clearTimeout: () => {},
    setInterval: () => 0,
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
  // In index.html order: app.js, then the group, voice and P2P scripts.
  run("shared/events.js");
  run("chat/app.js");
  for (const name of ["updateUserList", "updateStats", "renderServerList", "renderGroupList", "addSystemMessage", "hosIcon", "shortKey", "esc", "playNotificationChime"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  run("chat/chat-groups-p2p.js");
  run("chat/chat-voice-rooms.js");
  run("chat/chat-voice-calls.js");
  run("chat/chat-p2p.js");
  const sock = fakeSocket();
  vm.runInContext("(s, me) => { ws = s; myKey = me; }", ctx)(sock, ME);
  return { ctx, sock, media };
}

// Let the async handlers (handleDCOffer, handleOffer) run to the end.
async function settle() {
  for (let i = 0; i < 5; i++) await new Promise((r) => setImmediate(r));
}

// Feed one relay frame through the page's real message handler.
async function relaySends(ctx, msg) {
  await vm.runInContext("handleMessage", ctx)(msg);
  await settle();
}

function dcOffer(from) {
  return { type: "webrtc_signal", from, to: ME, signal_type: "dc_offer", data: JSON.stringify({ type: "offer", sdp: "v=0 x" }) };
}

const answersTo = (sock, key, kind = "dc_answer") =>
  sock.sent.filter((m) => m.type === "webrtc_signal" && m.signal_type === kind && m.to === key);

// Seen red 2026-10-09 two ways: against the code before the fix (HOS_WEB_DIR
// pointed at HEAD's web/), and with only the mayAnswerDirectOffer line in
// handleDCOffer disabled. Both times the stranger got a dc_answer here, and the
// group, voice-room and call tests below failed on their stranger or
// still-ringing case.
test("a stranger's direct-connection offer gets no answer", async () => {
  const { ctx, sock } = loadChat();
  await relaySends(ctx, dcOffer(STRANGER));
  assert.deepStrictEqual(answersTo(sock, STRANGER), [], "no dc_answer goes to a stranger");
  assert.strictEqual(vm.runInContext("(k) => k in p2pConnections", ctx)(STRANGER), false, "no connection is kept for them");
});

test("your own device's offer is answered", async () => {
  const { ctx, sock } = loadChat();
  await relaySends(ctx, dcOffer(ME));
  assert.strictEqual(answersTo(sock, ME).length, 1);
});

test("a contact you added is answered", async () => {
  const { ctx, sock } = loadChat();
  vm.runInContext("(k) => { p2pContacts[k] = { name: 'Contact', kyber_pub: null, added_at: 1, dc_status: 'idle' }; }", ctx)(CONTACT);
  await relaySends(ctx, dcOffer(CONTACT));
  assert.strictEqual(answersTo(sock, CONTACT).length, 1);
});

test("a member of one of your P2P groups is answered, from the group list or the open group's roster", async () => {
  const { ctx, sock } = loadChat();
  // The group list loaded on connect (the shape /api/v2/groups?pubkey= returns).
  ctx._p2pGroups = [{ group_id: "g1", name: "Garden", members: [ME, GROUP_MATE], is_creator: false }];
  // The open group's roster, fingerprint to key (ensureGroupMesh offers to these).
  ctx.activeP2pGroup = { id: "g2", name: "Build", fpToKey: { fp_me: ME, fp_mate: ROSTER_MATE } };
  await relaySends(ctx, dcOffer(GROUP_MATE));
  await relaySends(ctx, dcOffer(ROSTER_MATE));
  await relaySends(ctx, dcOffer(STRANGER));
  assert.strictEqual(answersTo(sock, GROUP_MATE).length, 1, "a member from the group list is answered");
  assert.strictEqual(answersTo(sock, ROSTER_MATE).length, 1, "a member of the open group is answered");
  assert.deepStrictEqual(answersTo(sock, STRANGER), [], "someone in no group of yours is not");
});

test("someone in your voice room is answered; someone in another room is not", async () => {
  const { ctx, sock } = loadChat();
  ctx._voiceChannels = [
    { id: 1, name: "Lounge", participants: [{ public_key: ME }, { public_key: ROOM_MATE }] },
    { id: 2, name: "Other", participants: [{ public_key: STRANGER }] },
  ];
  ctx._currentRoomId = "1";
  await relaySends(ctx, dcOffer(ROOM_MATE));
  await relaySends(ctx, dcOffer(STRANGER));
  assert.strictEqual(answersTo(sock, ROOM_MATE).length, 1);
  assert.deepStrictEqual(answersTo(sock, STRANGER), []);
});

// Seen red 2026-10-09 with only the call line in handleWebrtcSignalMessage put
// back as it was (`if (msg.from !== callPeerKey) return;`): while CALLER was
// only ringing, its call `offer` got an `answer` ("no call answer while it
// rings" failed), which means handleOffer had already asked for the microphone.
// Against HEAD's web/ it failed one step earlier, on the dc_answer.
test("a caller is answered only after you accept: no microphone and no answer while it rings", async () => {
  const { ctx, sock, media } = loadChat();
  await relaySends(ctx, { type: "voice_call", from: CALLER, from_name: "Caller", to: ME, action: "ring" });
  assert.strictEqual(vm.runInContext("callState", ctx), "ringing-in");

  // Ringing is not a call: neither a direct-connection offer nor a call offer is answered.
  await relaySends(ctx, dcOffer(CALLER));
  await relaySends(ctx, { type: "webrtc_signal", from: CALLER, to: ME, signal_type: "offer", data: { type: "offer", sdp: "v=0 x" } });
  assert.deepStrictEqual(answersTo(sock, CALLER), [], "no dc_answer while it rings");
  assert.deepStrictEqual(answersTo(sock, CALLER, "answer"), [], "no call answer while it rings");
  assert.strictEqual(media.asked, 0, "the microphone is not asked for while it rings");

  // Accepted: the call connects as before, and the partner may open a direct connection.
  vm.runInContext("acceptIncomingCall", ctx)();
  assert.strictEqual(vm.runInContext("callState", ctx), "in-call");
  await relaySends(ctx, { type: "webrtc_signal", from: CALLER, to: ME, signal_type: "offer", data: { type: "offer", sdp: "v=0 x" } });
  assert.strictEqual(media.asked, 1, "the microphone is asked for after Accept");
  assert.strictEqual(answersTo(sock, CALLER, "answer").length, 1, "the call offer is answered after Accept");
  await relaySends(ctx, dcOffer(CALLER));
  assert.strictEqual(answersTo(sock, CALLER).length, 1, "the call partner's direct-connection offer is answered");
});

test("the group mesh still connects: an offer you send is completed by the member's answer", async () => {
  // The offering side of ensureGroupMesh is unchanged: it offers to a roster
  // member, and that member's dc_answer is applied to the connection.
  const { ctx, sock } = loadChat();
  await vm.runInContext("initDataChannel", ctx)(GROUP_MATE);
  const offers = sock.sent.filter((m) => m.signal_type === "dc_offer" && m.to === GROUP_MATE);
  assert.strictEqual(offers.length, 1, "the offer goes out");
  await relaySends(ctx, { type: "webrtc_signal", from: GROUP_MATE, to: ME, signal_type: "dc_answer", data: JSON.stringify({ type: "answer", sdp: "v=0 a" }) });
  const pc = vm.runInContext("(k) => p2pConnections[k]", ctx)(GROUP_MATE);
  // (Through JSON: the object was made inside the page's realm, with its own Object.)
  assert.deepStrictEqual(JSON.parse(JSON.stringify(pc.remote)), { type: "answer", sdp: "v=0 a" }, "the answer is applied");
});
