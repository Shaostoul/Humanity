// Block in the web chat client (step C, 2026-10-09, docs/design/blocking-and-safe-mode.md 10d),
// mirroring the desktop app.
//
// Run: node --test scripts/tests/block-web.test.js   (in `just rig-tests`)
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with the
// DOM replaced by a stub that keeps what is written to it, as in reach-web.test.js: the real
// friend-pass.js, reach.js, block.js, crypto.js, chat-dm-store.js (over a stand-in IndexedDB),
// app.js, chat-messages.js, chat-dms.js, chat-social.js, chat-groups-p2p.js, chat-ui.js,
// chat-voice-rooms.js, chat-voice-calls.js, chat-privacy.js and chat-p2p.js, in index.html's order.
// Only the Dilithium and Kyber primitives are stand-ins: "signing" returns the signed words and
// "checking" compares them; "sealing" base64s the plaintext and names the key it was sealed to, so a
// test can read exactly what would travel inside the ciphertext. The real primitives are covered by
// scripts/pq-kat.mjs and the relay's tests. HOS_WEB_DIR points the test at another copy of web/
// (used to see each test red).
//
// What it proves (10d's Proof list, the web items, and the rest of 10d's web surface):
//  0. The note markers are the desktop app's: read out of src/net/dm_pq.rs when it declares them,
//     else out of the spec's own text (10d), saying so. A note is exactly a marker and a key.
//  1. Block withdraws every pass I gave them (one cert_revoke per serial) and unfollows them here,
//     sends exactly one put, a note to my own mailbox (from me, to me, signed, padded, sealed to my
//     own DM key, its text [[hum:block:v1]]<key>), sends nothing to them, and shows the one line.
//  2. A blocked key's DM is not stored or notified, live, from the mailbox, or over a direct
//     channel; their follow notices and passes are dropped too.
//  3. A blocked key's contact request is dropped, never listed; Block on a listed request
//     (instead of Ignore) blocks them and takes it off the list.
//  4. A blocked key's channel post is not drawn or notified, one already drawn is hidden (and shown
//     again on Unblock); their typing, reactions, thread replies and group messages are not shown,
//     nor their words quoted in someone else's reply; the member list marks them.
//  5. A ring from a blocked key sends nothing back (not even the busy reject), opens nothing and
//     notifies nothing; their direct-connection offer is not answered, even as a contact.
//  6. The notes round-trip between two devices of mine: device A's block note blocks on device B
//     with A's date and withdraws B's passes, and its unblock note unblocks; a note addressed to
//     anyone but myself is ignored and never shown.
//  7. Unblock takes them off the list and sends the unblock note, and does not follow them again or
//     give them a pass.
//  8. Settings > Safety > Blocked people lists each by the member list's name (or short key) with
//     the date and Unblock; /block <name> and /unblock <name>; Block beside Report in the message
//     and member list menu and in the DM header; a Block made while offline is sent on reconnect.
//  9. Web's old list of names is gone.
//
// Red first, 2026-10-09: each mutation made in a copy of web/, this test run against it with
// HOS_WEB_DIR, and seen failing with the assertion named:
//  0: block.js's CTL_UNBLOCK spelled "[[hum:unblock]]": "the unblock marker is
//     docs/design/blocking-and-safe-mode.md 10d's" (src/net/dm_pq.rs had no markers yet).
//  1: blockLocally without `withdrawPassesTo(key)`: "every pass I gave them is withdrawn".
//  2: the blockScreenDm call taken out of app.js's dm_new: "not stored"; out of dm_batch: "from the
//     mailbox: not stored"; out of chat-p2p.js onDCMessage: "over a direct channel: not stored".
//  3: receiveContactRequest's isBlocked line taken out: "and the Requests list itself refuses them".
//  4: addChatMessage's isBlockedKey line taken out: "nor their group message"; blockScreenFrame's
//     `chat` case taken out: "or notified (a mention in it too)"; renderReactions counting every
//     reactor: "and their reaction no longer counted"; app.js's typing line and the frame's
//     `typing` case taken out: "nor their typing"; applyBlockToView not hiding drawn posts: "the
//     post already on screen is hidden".
//  5: handleVoiceCallMessage's blocked-ring line and blockScreenFrame's `voice_call` case taken
//     out: "no incoming-call screen"; mayAnswerDirectOffer's isBlockedKey line taken out: "the
//     direct-offer gate refuses them". (Step E, the same day, made that gate answer my own devices
//     only, so the isBlockedKey line went; with the gate answering contacts again, the same
//     assertion fails.)
//  6: blockScreenDm reading a note with blockNoteParse instead of blockNoteFromSelf: "no one is
//     blocked by them" (Ann's note to me blocked Cy).
//  7: unblockKey also calling setFollowLocal(key, true): "one note, to my own mailbox, nothing else".
//  8: the Blocked people rows without data-unblock: "each row has the date and Unblock";
//     onBlockListLoaded without flushBlockNotes(): "sent on reconnect".
//  9: a localStorage.getItem('humanity_blocks') put back in chat-profile.js: "no name-based block
//     list or its localStorage key remains in web/".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const block = require(path.join(WEB, "shared", "block.js"));
const reach = require(path.join(WEB, "shared", "reach.js"));
const fp = require(path.join(WEB, "shared", "friend-pass.js"));

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

// An element that keeps what is written to it (text, children, handlers, style) and absorbs the
// rest. Until its HTML is written, its innerHTML is its text escaped, as a browser's is (app.js
// esc() relies on that).
const escapeText = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
function fakeElement(tag) {
  const own = { tag, children: [], style: {}, dataset: {}, className: "", textContent: "", value: "", disabled: false };
  own.appendChild = (c) => { own.children.push(c); return c; };
  return new Proxy(own, {
    get(t, prop) {
      if (prop === "then") return undefined;
      if (prop === "innerHTML") return "innerHTML" in t ? t.innerHTML : escapeText(t.textContent);
      if (prop in t) return t[prop];
      return anything();
    },
    set(t, prop, v) { t[prop] = v; return true; },
  });
}

// The document: one kept element per id, the drawn messages for `.message[data-from]`, one kept
// element per reactions row, and `hidden` as the test sets it.
function fakeDocument(state) {
  const byId = new Map();
  const bySelector = new Map();
  const kept = (map, k) => { if (!map.has(k)) map.set(k, fakeElement(k)); return map.get(k); };
  return new Proxy({}, {
    get(_t, prop) {
      if (prop === "createElement") return fakeElement;
      if (prop === "getElementById") return (id) => kept(byId, id);
      if (prop === "querySelector") return (sel) => (String(sel).startsWith(".reactions[") ? kept(bySelector, sel) : anything());
      if (prop === "querySelectorAll") return (sel) => (sel === ".message[data-from]" ? state.appended.filter((el) => el && el.dataset && el.dataset.from) : []);
      if (prop === "hidden") return state.hidden;
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

// Just enough IndexedDB for chat-dm-store.js: requests answer on the next turn.
function fakeIndexedDB() {
  const stores = { msgs: new Map(), meta: new Map() };
  const req = (result) => {
    const r = { result: undefined, onsuccess: null, onerror: null };
    setImmediate(() => { r.result = result; if (r.onsuccess) r.onsuccess(); });
    return r;
  };
  const objectStore = (name) => ({
    get: (k) => req(stores[name].get(k)),
    put: (v) => { stores[name].set(v.k ?? v.scope, v); return req(undefined); },
    delete: (k) => { stores[name].delete(k); return req(undefined); },
    index: () => ({ getAll: (scope) => req([...stores[name].values()].filter((v) => v.scope === scope)) }),
  });
  const db = { objectStoreNames: { contains: () => true }, transaction: (name) => ({ objectStore: () => objectStore(name) }) };
  return { open: () => req(db) };
}

function fakeSocket() {
  return { readyState: 1, sent: [], send(s) { this.sent.push(JSON.parse(s)); }, close() {} };
}

// Just enough of a peer connection to be answered (as in p2p-direct-offers.test.js).
function FakePeerConnection() {
  this.setRemoteDescription = async () => {};
  this.setLocalDescription = async () => {};
  this.createAnswer = async () => ({ type: "answer", sdp: "v=0 test-answer" });
  this.createOffer = async () => ({ type: "offer", sdp: "v=0 test-offer" });
  this.createDataChannel = () => ({ readyState: "connecting", send() {} });
  this.addTrack = () => {};
  this.addIceCandidate = async () => {};
  this.close = () => {};
}

const ME = "a1".repeat(32);
const ANN = "b2".repeat(32);
const BEN = "c3".repeat(32);
const CY = "d4".repeat(32);
const STRANGER = "e5".repeat(32);
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const CTL_FOLLOW = "[[hum:follow]]";
const CTL_FRIEND_CERT = "[[hum:friend-cert]]";
const MY_KYBER = "my-kyber";
const kyberOf = (k) => "kyber-" + k.slice(0, 4);
const MAY = "invite,message,trade,voice_message";

// The page's real asynchronous work is WebCrypto (the store hashes and encrypts every record) and
// the stand-in IndexedDB; the rest settles in microtasks. Counting the WebCrypto calls in flight lets
// settle() wait until the page is idle rather than for a fixed number of turns, which a loaded
// machine outruns (reach-web.test.js's fixed 12 turns fail there now and then).
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
const countedCrypto = {
  subtle: countedSubtle,
  getRandomValues: (a) => globalThis.crypto.getRandomValues(a),
  randomUUID: () => globalThis.crypto.randomUUID(),
};

async function settle() {
  let idle = 0;
  for (let i = 0; i < 5000 && idle < 12; i++) {
    await new Promise((r) => setImmediate(r));
    idle = cryptoInFlight === 0 ? idle + 1 : 0;
  }
}

const b64 = (s) => Buffer.from(s, "utf8").toString("base64");
const unb64 = (s) => Buffer.from(s, "base64").toString("utf8");

async function loadChat() {
  const state = { appended: [], hidden: false };
  const notified = [];
  const swNotes = [];
  const chimes = [];
  // chat-ui.js registers the service worker at load and chains on the promise.
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDocument(state),
    navigator: new Proxy({}, { get: (_t, p) => (p === "serviceWorker" ? serviceWorker : anything()), has: () => true }),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    indexedDB: fakeIndexedDB(),
    // No network. The federated server list answers (empty): chat-ui.js renderServerList asks
    // again each time that fetch fails, which a fetch failing at once turns into an endless loop.
    fetch: (url) => (String(url).startsWith("/api/federation/servers")
      ? Promise.resolve({ ok: true, json: async () => [] })
      : Promise.reject(new Error("no network in tests"))),
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
    innerWidth: 1280,
    innerHeight: 800,
    crypto: countedCrypto,
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
  run("chat/crypto.js");
  run("chat/chat-dm-store.js");
  run("chat/view/timestampPill.js");
  run("chat/view/messageRow.js");
  run("chat/app.js");
  run("chat/chat-messages.js");
  run("chat/chat-dms.js");
  run("chat/chat-social.js");
  run("chat/chat-groups-p2p.js");
  run("chat/chat-ui.js");
  run("chat/chat-voice-rooms.js");
  run("chat/chat-voice-calls.js");
  run("chat/chat-privacy.js");
  run("chat/chat-p2p.js");
  // Page pieces from scripts this test does not load (icons.js, hold-confirm.js, twemoji).
  for (const name of ["hosIcon", "holdToConfirm", "holdConfirm", "updateStats"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  // Record what is drawn into the message area, notified, chimed, and pushed to the system tray.
  ctx.appendMessage = (el) => { state.appended.push(el); };
  ctx.notifyNewMessage = (...args) => { notified.push(args); };
  ctx.playNotificationChime = () => { chimes.push(1); };
  ctx.sendSWNotification = (...args) => { swNotes.push(args); };
  // The stand-in primitives (see the top of this file).
  const key = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => key;
  ctx.pqSignMessage = async (_secret, bytes) => new Uint8Array(bytes);
  ctx.pqVerifyMessage = async (_pk, bytes, sig) => Buffer.from(bytes).equals(Buffer.from(sig));
  ctx.pqDmSeal = async (pub, plaintext) => ({ ek_ct_b64: b64(pub), nonce_b64: "AAAA", ct_b64: b64(plaintext) });
  ctx.pqDmOpen = async (_secret, ek, _nonce, ct) => (unb64(ek) === MY_KYBER ? unb64(ct) : null);
  const sock = fakeSocket();
  vm.runInContext(`(s, me) => {
    ws = s; myKey = me; myName = 'Me_1'; activeChannel = 'general';
    myDilithiumPublicHex = me; myDilithiumSecret = new Uint8Array(4);
    myKyberPublicBase64 = '${MY_KYBER}'; myKyberSecret = new Uint8Array(4);
  }`, ctx)(sock, ME);
  const handle = async (msg) => { await vm.runInContext("handleMessage", ctx)(msg); await settle(); };
  // The relay's challenge names the server; the member list carries names and DM keys.
  await handle({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  const store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await store.init(ME, "localhost"), "the store loads");
  store.setPassServer(SERVER);
  const users = [[ME, "Me_1"], [ANN, "Ann"], [BEN, "Ben"], [CY, "Cy"]].map(([k, name]) => ({ public_key: k, name, role: "", kyber_public: kyberOf(k), online: true }));
  await handle({ type: "full_user_list", users });
  await settle();
  sock.sent.length = 0;
  state.appended.length = 0;
  const fn = (name) => vm.runInContext(name, ctx);
  const el = (id) => ctx.document.getElementById(id);
  return { ctx, sock, store, state, appended: state.appended, notified, swNotes, chimes, handle, fn, el };
}

// Everything readable in a recorded element and its children.
function textOf(e) {
  if (!e) return "";
  const own = [e.textContent, typeof e.innerHTML === "string" ? e.innerHTML : ""].filter((s) => typeof s === "string").join(" ");
  return [own, ...(e.children || []).map(textOf)].join(" ");
}
const shown = (appended) => appended.map(textOf).join("\n");

// What a sealed dm_put carries (the stand-in seal is base64 of the plaintext).
function opened(put) {
  const env = JSON.parse(put.content);
  return { sealedTo: unb64(env.ek_ct_b64), inner: JSON.parse(unb64(env.ct_b64)) };
}

// A DM from `from` to `to` (signed with the stand-in signer), sealed to my DM key: what my mailbox
// holds for a message to me, or for a self-copy of one I sent.
function envelope(from, to, text, ts = 1760000000000, extra = {}) {
  const sig = b64(`hum/dm/v2\n${from}\n${to}\n${ts}\n${text}`);
  const inner = JSON.stringify({ v: 2, from, to, ts, text, sig, ...extra });
  return JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(inner) });
}
const dmEnvelope = (from, text, ts) => envelope(from, ME, text, ts);
// A pass `issuer` gave `grantee` on this server (the stand-in signature is the signed words).
function passFrom(issuer, grantee = ME, serial = "0123456789abcdef0123456789abcdef", may = MAY) {
  return fp.friendPassJson(serial, may, b64(fp.friendPassPreimage(SERVER, issuer, grantee, serial, may)));
}
const chat = (from, from_name, content, timestamp) => ({ type: "chat", from, from_name, content, timestamp, channel: "general" });
const drawnFrom = (appended, key) => appended.filter((e) => e && e.dataset && e.dataset.from === key);

test("the note markers are the desktop app's, and a note is exactly a marker and a key", (t) => {
  // The desktop app declares its markers beside CTL_FOLLOW in src/net/dm_pq.rs (10d). It is being
  // built in parallel: until its constants are in this checkout, the spec's own text is the
  // reference, and the test says so.
  const rust = fs.readFileSync(path.join(ROOT, "src", "net", "dm_pq.rs"), "utf8");
  const consts = {};
  for (const m of rust.matchAll(/pub\s+const\s+(\w+)\s*:\s*&str\s*=\s*"(\[\[hum:[^"]*)"/g)) consts[m[1]] = m[2];
  const unblockName = Object.keys(consts).find((n) => /UNBLOCK/.test(n));
  const blockName = Object.keys(consts).find((n) => /BLOCK/.test(n) && !/UNBLOCK/.test(n));
  let want;
  if (blockName || unblockName) {
    assert.ok(blockName && unblockName, "src/net/dm_pq.rs declares both a block and an unblock marker");
    want = { block: consts[blockName], unblock: consts[unblockName], from: `src/net/dm_pq.rs (${blockName}, ${unblockName})` };
  } else {
    const spec = fs.readFileSync(path.join(ROOT, "docs", "design", "blocking-and-safe-mode.md"), "utf8");
    const tenD = spec.slice(spec.indexOf("## 10d."));
    const m = tenD.match(/`(\[\[hum:[a-z]+:v1\]\])<key>` and `(\[\[hum:[a-z]+:v1\]\])<key>`/);
    assert.ok(m, "the spec's 10d names the two notes");
    want = { block: m[1], unblock: m[2], from: "docs/design/blocking-and-safe-mode.md 10d" };
    t.diagnostic("src/net/dm_pq.rs does not declare the block markers in this checkout yet: compared with the spec's text instead");
  }
  assert.equal(block.CTL_BLOCK, want.block, `the block marker is ${want.from}'s`);
  assert.equal(block.CTL_UNBLOCK, want.unblock, `the unblock marker is ${want.from}'s`);
  assert.equal(block.CTL_BLOCK, "[[hum:block:v1]]");
  assert.equal(block.CTL_UNBLOCK, "[[hum:unblock:v1]]");

  // A note is the marker and the key, nothing else.
  assert.equal(block.blockNoteText("block", ANN), "[[hum:block:v1]]" + ANN);
  assert.equal(block.blockNoteText("unblock", ANN.toUpperCase()), "[[hum:unblock:v1]]" + ANN, "the key as the list keeps it, lowercase");
  assert.equal(block.blockNoteText("block", "Ann"), null, "a name is not a key");
  assert.equal(block.blockNoteText("mute", ANN), null);
  assert.deepEqual(block.blockNoteParse("[[hum:block:v1]]" + ANN), { action: "block", key: ANN });
  assert.deepEqual(block.blockNoteParse("[[hum:unblock:v1]]" + ANN), { action: "unblock", key: ANN });
  for (const bad of ["[[hum:block:v1]]", "[[hum:block:v1]] " + ANN, "[[hum:block:v1]]" + ANN + "\n", "[[hum:block:v1]]Ann", "[[hum:block]]" + ANN, "hello"]) {
    assert.equal(block.blockNoteParse(bad), null, JSON.stringify(bad));
  }
  assert.ok(block.isBlockNoteText("[[hum:block:v1]]Ann"), "but anything that starts with a marker is still never shown");
  // Only a note from me to me counts, and never one naming me.
  const note = (from, to, key) => ({ from, to, text: block.blockNoteText("block", key) });
  assert.deepEqual(block.blockNoteFromSelf(note(ME, ME, ANN), ME), { action: "block", key: ANN });
  assert.equal(block.blockNoteFromSelf(note(ANN, ME, CY), ME), null, "from someone else");
  assert.equal(block.blockNoteFromSelf(note(ME, ANN, CY), ME), null, "my own message to someone else");
  assert.equal(block.blockNoteFromSelf({ from: ME, to: ME, text: block.CTL_BLOCK + ME }, ME), null, "naming myself");
});

test("Block withdraws the passes, unfollows, and tells only my own devices", async () => {
  const { ctx, sock, store, appended, fn } = await loadChat();
  const S1 = "11".repeat(16);
  const S2 = "22".repeat(16);
  store.recordPassSent(ANN, S1, MAY);
  store.recordPassSent(ANN, S2, "call," + MAY);
  store.setFollowing(ANN, true);
  store.setFollower(ANN, true);
  vm.runInContext("(k) => { myFollowing.add(k); myFollowers.add(k); }", ctx)(ANN);
  const before = Date.now();

  assert.equal(await fn("blockKey")(ANN), true);
  assert.ok(store.isBlocked(ANN), "on the list");
  const [entry] = store.blockedList();
  assert.equal(entry.key, ANN, "by key");
  assert.ok(entry.ts >= before && entry.ts <= Date.now(), "with the date");

  const revoked = sock.sent.filter((m) => m.type === "cert_revoke").map((m) => m.serial).sort();
  assert.deepEqual(revoked, [S1, S2], "every pass I gave them is withdrawn");
  assert.equal(store.certSentTo(ANN), false);
  assert.equal(store.following.has(ANN), false, "unfollowed");
  assert.equal(vm.runInContext("(k) => myFollowing.has(k)", ctx)(ANN), false);

  assert.ok(!sock.sent.some((m) => m.to === ANN), "nothing is sent to them");
  const puts = sock.sent.filter((m) => m.type === "dm_put");
  assert.equal(puts.length, 1, "exactly one note");
  assert.equal(puts[0].to, ME, "to my own mailbox");
  assert.ok(!("friend_cert" in puts[0]) && !("contact_request" in puts[0]));
  const { sealedTo, inner } = opened(puts[0]);
  assert.equal(sealedTo, MY_KYBER, "sealed to my own DM key");
  assert.deepEqual([inner.v, inner.from, inner.to, inner.text], [2, ME, ME, "[[hum:block:v1]]" + ANN], "from me, to me, the marker and their key");
  assert.equal(unb64(inner.sig), `hum/dm/v2\n${ME}\n${ME}\n${inner.ts}\n${inner.text}`, "signed like any DM");
  assert.equal(typeof inner.pad, "string", "padded like any DM");
  const plainLen = unb64(JSON.parse(puts[0].content).ct_b64).length;
  assert.ok(Array.from(vm.runInContext("DM_PAD_BUCKETS", ctx)).some((n) => plainLen <= n && plainLen > n - 12), "to just under a bucket");
  assert.equal(sock.sent.length, 3, "two withdrawals and the note, nothing else");
  assert.ok(shown(appended).includes(block.BLOCKED_LINE), "the one line");
  assert.equal(block.BLOCKED_LINE, "Blocked. You will not see anything from them. They are not told.");

  // Again: nothing more is sent. Myself, or something that is not a key: refused.
  sock.sent.length = 0;
  assert.equal(await fn("blockKey")(ANN), true);
  assert.equal(await fn("blockKey")(ME), false);
  assert.equal(await fn("blockKey")("Ann"), false);
  assert.equal(sock.sent.length, 0);
  assert.ok(shown(appended).includes("You can't block yourself."));
  // And no pass is ever minted for them again, even as a mutual follow.
  store.setFollowing(ANN, true);
  assert.deepEqual(store.friendsWithoutPass(), [], "the pass sweep skips them");
});

test("a blocked key's DM is not stored or notified", async () => {
  const { ctx, sock, store, appended, notified, handle, fn } = await loadChat();
  // "Anyone" may message me, so the only thing that stops Ben is the block.
  await handle({ type: "reach_settings", settings: { message: "anyone", call: "chosen", trade: "friends" } });
  await fn("blockKey")(BEN);
  sock.sent.length = 0;
  appended.length = 0;

  await handle({ type: "dm_new", id: 41, content: dmEnvelope(BEN, "live words from Ben") });
  assert.deepEqual(store.conversation(BEN), [], "not stored");
  assert.equal(store.highWater, 41, "but taken from the mailbox, so it is not fetched again");
  await handle({ type: "dm_new", id: 42, content: dmEnvelope(BEN, CTL_FOLLOW, 5) });
  assert.equal(store.followers.has(BEN), false, "their follow notice is dropped");
  await handle({ type: "dm_new", id: 43, content: envelope(BEN, ME, CTL_FRIEND_CERT, 6, { cert: passFrom(BEN) }) });
  assert.equal(store.certFor(BEN), null, "and so is their pass");
  // The page's own mailbox page: it carries the ref of the page's own fetch (10o O1).
  await handle({ type: "dm_batch", ref: fn("sendDmFetch")(store.highWater), messages: [
    { id: 44, content: dmEnvelope(BEN, "mailbox words from Ben", 7) },
    { id: 45, content: dmEnvelope(CY, "hello from Cy", 8) },
  ], done: true });
  assert.deepEqual(store.conversation(BEN), [], "from the mailbox: not stored");
  assert.equal(store.conversation(CY).length, 1, "while Cy's is");
  await fn("onDCMessage")({ data: JSON.stringify({ type: "p2p_dm", env: dmEnvelope(BEN, "direct words from Ben", 9) }) }, BEN);
  await settle();
  assert.deepEqual(store.conversation(BEN), [], "over a direct channel: not stored");

  const everything = [shown(appended), JSON.stringify(notified)].join("\n");
  assert.ok(!/from Ben|is now following/.test(everything), "and nothing of it is shown or notified");
  assert.ok(notified.every((args) => !String(args[0]).includes("Ben")), "no notification names them");
  assert.deepEqual(sock.sent.filter((m) => m.to === BEN), [], "nothing goes back to them");
  assert.equal(ctx.peerData[BEN].display_name, "Ben");
});

test("a blocked key's contact request is dropped", async () => {
  const { sock, store, notified, handle, fn } = await loadChat();
  await fn("blockKey")(BEN);
  await handle({ type: "dm_new", id: 51, content: dmEnvelope(BEN, reach.contactRequestText("Ben", passFrom(BEN))) });
  await handle({ type: "dm_new", id: 52, content: dmEnvelope(BEN, "a stranger's words under friends-only") });
  assert.deepEqual(store.contactRequestList(), [], "never listed, as a request or as a refused DM");
  assert.equal(notified.length, 0, "never notified");
  assert.equal(fn("receiveContactRequest")({ key: BEN, ts: 1 }), false, "and the Requests list itself refuses them");

  // Block instead of Ignore, on a request already listed.
  await handle({ type: "dm_new", id: 53, content: dmEnvelope(CY, reach.contactRequestText("Cy", passFrom(CY))) });
  assert.deepEqual(store.contactRequestList().map((r) => r.key), [CY]);
  assert.ok(fn("contactRequestsHtml")(store.contactRequestList()).includes(`data-req-block="${CY}"`), "the request offers Block");
  sock.sent.length = 0;
  assert.equal(await fn("blockContactRequest")(CY), true);
  assert.ok(store.isBlocked(CY));
  assert.deepEqual(store.contactRequestList(), [], "and it is gone");
  assert.deepEqual(sock.sent.filter((m) => m.to === CY), [], "they are told nothing");
});

test("a blocked key's channel post is hidden, and so is the rest of what they do in public", async () => {
  const { ctx, store, appended, notified, handle, fn, el } = await loadChat();
  await handle(chat(BEN, "Ben", "first post", 1));
  const first = drawnFrom(appended, BEN);
  assert.equal(first.length, 1, "before: drawn");
  assert.equal(notified.length, 1, "and notified");
  await handle({ type: "reaction", target_from: CY, target_timestamp: 5, emoji: "+1", from: BEN, from_name: "Ben" });
  await handle({ type: "reaction", target_from: CY, target_timestamp: 5, emoji: "+1", from: ANN, from_name: "Ann" });
  const badge = ctx.document.querySelector(`.reactions[data-from="${CY}"][data-ts="5"]`);
  assert.ok(badge.innerHTML.includes('<span class="count">2</span>'), "two reactions before");

  await fn("blockKey")(BEN);
  assert.equal(first[0].style.display, "none", "the post already on screen is hidden");
  assert.ok(badge.innerHTML.includes('<span class="count">1</span>'), "and their reaction no longer counted");
  notified.length = 0;
  // What is drawn from here on (the first post stays in the list, so Unblock can show it again).
  const mark = appended.length;
  const drawnSince = (key) => drawnFrom(appended.slice(mark), key);

  await handle(chat(BEN, "Ben", "second post", 2));
  await handle({ type: "reaction", target_from: CY, target_timestamp: 5, emoji: "heart", from: BEN, from_name: "Ben" });
  await handle({ type: "typing", from: BEN, from_name: "Ben" });
  assert.deepEqual(drawnSince(BEN), [], "a new post is not drawn");
  assert.equal(notified.length, 0, "or notified (a mention in it too)");
  assert.ok(!badge.innerHTML.includes("heart"), "a new reaction is not shown");
  assert.deepEqual(Object.keys(vm.runInContext("typingNames", ctx)), [], "nor their typing");
  // History, group messages (chat-groups-p2p.js draws through the same addChatMessage) and replies.
  fn("addChatMessage")("Ben", "group words", 3, BEN, true, false, null, null);
  assert.deepEqual(drawnSince(BEN), [], "nor their group message");
  fn("addChatMessage")("Cy", "replying", 4, CY, false, false, { from: BEN, from_name: "Ben", content: "the words Ben wrote", timestamp: 1 }, null);
  const reply = drawnSince(CY)[0];
  assert.ok(reply && !textOf(reply).includes("the words Ben wrote") && textOf(reply).includes("Someone you blocked"), "nor their words quoted in Cy's reply");
  vm.runInContext("(k) => { currentThread = { from: k, timestamp: 4, author: 'Cy', body: 'thread' }; }", ctx)(CY);
  fn("renderThreadMessages")([{ from: BEN, from_name: "Ben", content: "Ben in the thread", timestamp: 6 }, { from: ANN, from_name: "Ann", content: "Ann in the thread", timestamp: 7 }]);
  const thread = el("thread-panel-messages").innerHTML;
  assert.ok(!thread.includes("Ben in the thread") && thread.includes("Ann in the thread"), "nor their thread replies");
  // The member list (as chat-voice-rooms.js draws it) marks them, and no one else.
  fn("updateUserList")([{ public_key: BEN, name: "Ben", role: "", online: true }, { public_key: CY, name: "Cy", role: "", online: true }]);
  const list = el("peer-list").innerHTML;
  const row = (k) => { const at = list.indexOf(`data-pubkey="${k}"`); return at < 0 ? "" : list.slice(at, list.indexOf("</div>", at)); };
  assert.ok(row(BEN).includes("line-through") && row(BEN).includes("block-indicator"), "the member list marks them");
  assert.ok(row(CY) && !row(CY).includes("line-through") && !row(CY).includes("block-indicator"), "and no one else");

  // Unblock: the hidden post shows again.
  await fn("unblockKey")(BEN);
  assert.equal(first[0].style.display, "", "Unblock shows it again");
  await handle(chat(BEN, "Ben", "third post", 8));
  assert.equal(drawnSince(BEN).length, 1, "and their next post is drawn");
  assert.equal(store.isBlocked(BEN), false);
});

test("a ring from a blocked key sends nothing back, and their direct offer is not answered", async () => {
  const { ctx, sock, state, swNotes, chimes, handle, fn } = await loadChat();
  // Calls from anyone, so only the block keeps Ben out (the call setting has its own test below).
  await handle({ type: "reach_settings", settings: { message: "friends", call: "anyone", trade: "friends" } });
  await fn("blockKey")(BEN);
  sock.sent.length = 0;
  const ring = (from) => ({ type: "voice_call", from, from_name: "x", to: ME, action: "ring" });
  state.hidden = true; // the tab is in the background: a ring would also go to the system tray

  await handle(ring(BEN));
  assert.equal(vm.runInContext("callState", ctx), "idle", "no incoming-call screen");
  assert.equal(chimes.length, 0, "no chime");
  assert.equal(swNotes.length, 0, "no system notification");
  assert.deepEqual(sock.sent, [], "and nothing back");

  // Busy: nothing goes back to anyone (BUG-177, 2026-10-10: an automatic reject told a caller
  // who cannot see us online that we were there), so a blocked caller and Cy alike learn
  // nothing; Cy's ring leaves a missed-call line here instead. Seen red with the old
  // automatic reject put back: "while busy, nothing goes back to Cy either".
  vm.runInContext("callState = 'in-call'; callPeerKey = null", ctx);
  await handle(ring(BEN));
  assert.deepEqual(sock.sent, [], "even while busy, no reject goes to them");
  await handle(ring(CY));
  assert.deepEqual(sock.sent, [], "while busy, nothing goes back to Cy either");
  assert.equal(swNotes.length, 1, "and Cy's ring notifies");
  vm.runInContext("callState = 'idle'; callPeerKey = null", ctx);
  await handle(ring(CY));
  assert.equal(vm.runInContext("callState", ctx), "ringing-in", "Cy's ring rings");

  // A direct-connection offer: not answered, even from a contact. (Since step E, 2026-10-09,
  // only my own devices are answered at all, so Ann's, a contact not blocked, is not either;
  // scripts/tests/p2p-direct-offers.test.js.)
  sock.sent.length = 0;
  vm.runInContext("(a, b) => { p2pContacts[a] = { name: 'Ben' }; p2pContacts[b] = { name: 'Ann' }; }", ctx)(BEN, ANN);
  const offer = (from) => ({ type: "webrtc_signal", from, to: ME, signal_type: "dc_offer", data: JSON.stringify({ type: "offer", sdp: "v=0 x" }) });
  assert.equal(fn("mayAnswerDirectOffer")(BEN), false, "the direct-offer gate refuses them");
  await handle(offer(BEN));
  await handle(offer(ANN));
  await handle(offer(ME));
  const answers = (k) => sock.sent.filter((m) => m.type === "webrtc_signal" && m.signal_type === "dc_answer" && m.to === k);
  assert.deepEqual(answers(BEN), [], "their offer is not answered");
  assert.deepEqual(answers(ANN), [], "nor Ann's: direct links are my own devices only");
  assert.equal(answers(ME).length, 1, "while my own other device's is");
});

test("the notes round-trip between my devices, and a note to anyone else is ignored", async () => {
  const a = await loadChat();
  const b = await loadChat();
  const S3 = "33".repeat(16);
  b.store.recordPassSent(ANN, S3, MAY);
  b.store.setFollowing(ANN, true);

  // Device A blocks Ann; the note reaches device B from its mailbox, live.
  await a.fn("blockKey")(ANN);
  const note = a.sock.sent.find((m) => m.type === "dm_put");
  const noteTs = opened(note).inner.ts;
  await b.handle({ type: "dm_new", id: 61, content: note.content });
  assert.ok(b.store.isBlocked(ANN), "device B blocks her too");
  assert.equal(b.store.blockedList()[0].ts, noteTs, "with the date device A wrote");
  assert.deepEqual(b.sock.sent.filter((m) => m.type === "cert_revoke").map((m) => m.serial), [S3], "and withdraws the pass B gave her");
  assert.equal(b.store.following.has(ANN), false);
  assert.deepEqual(b.sock.sent.filter((m) => m.type !== "cert_revoke"), [], "sending nothing else, to anyone");
  assert.deepEqual(b.store.conversation(ME), [], "the note is not a message");
  // Its own copy coming back to device A changes nothing.
  a.sock.sent.length = 0;
  await a.handle({ type: "dm_new", id: 61, content: note.content });
  assert.ok(a.store.isBlocked(ANN));
  assert.deepEqual(a.sock.sent, []);

  // Device A unblocks; B follows. Through the mailbox (dm_batch) this time.
  a.sock.sent.length = 0;
  await a.fn("unblockKey")(ANN);
  const unnote = a.sock.sent.find((m) => m.type === "dm_put");
  assert.equal(opened(unnote).inner.text, "[[hum:unblock:v1]]" + ANN);
  await b.handle({ type: "dm_batch", ref: b.fn("sendDmFetch")(b.store.highWater), messages: [{ id: 62, content: unnote.content }], done: true });
  assert.equal(b.store.isBlocked(ANN), false, "device B unblocks her");

  // Notes addressed to anyone but me are ignored, and never shown or stored.
  b.appended.length = 0;
  b.notified.length = 0;
  const ignored = [
    envelope(ANN, ME, block.blockNoteText("block", CY), 70), // Ann's "note" to me
    envelope(ME, ANN, block.blockNoteText("block", CY), 71), // my own message to Ann (its self-copy)
    envelope(ME, ME, block.blockNoteText("block", ME), 72), // a note naming myself
    envelope(ME, ME, "[[hum:block:v1]]not-a-key", 73),       // not a key
  ];
  let id = 70;
  for (const content of ignored) await b.handle({ type: "dm_new", id: ++id, content });
  assert.equal(b.store.isBlocked(CY), false, "no one is blocked by them");
  assert.deepEqual(b.store.blockedList(), []);
  assert.deepEqual([b.store.conversation(ANN), b.store.conversation(ME)], [[], []], "none is stored");
  assert.ok(!shown(b.appended).includes("hum:block") && b.notified.length === 0, "none is shown");
});

test("Unblock takes them off the list, tells my devices, and does not follow them again", async () => {
  const { ctx, sock, store, appended, fn } = await loadChat();
  store.recordPassSent(ANN, "44".repeat(16), MAY);
  store.setFollowing(ANN, true);
  store.storeCertFrom(ANN, passFrom(ANN));
  await fn("blockKey")(ANN);
  sock.sent.length = 0;

  assert.equal(await fn("unblockKey")(ANN), true);
  assert.equal(store.isBlocked(ANN), false, "off the list");
  assert.deepEqual(store.blockedList(), []);
  assert.deepEqual(sock.sent.map((m) => [m.type, m.to]), [["dm_put", ME]], "one note, to my own mailbox, nothing else");
  assert.equal(opened(sock.sent[0]).inner.text, "[[hum:unblock:v1]]" + ANN);
  assert.equal(store.following.has(ANN), false, "not followed again");
  assert.equal(vm.runInContext("(k) => myFollowing.has(k)", ctx)(ANN), false);
  assert.equal(store.certSentTo(ANN), false, "and no pass given");
  assert.ok(shown(appended).includes(block.UNBLOCKED_LINE));
  assert.equal(store.certFor(ANN), passFrom(ANN), "the pass she gave me is hers to withdraw, so it stays");
  sock.sent.length = 0;
  assert.equal(await fn("unblockKey")(ANN), true, "unblocking someone not blocked");
  assert.deepEqual(sock.sent, [], "sends nothing");
});

test("Settings > Safety > Blocked people, the commands, the menus, and a Block made offline", async () => {
  const { ctx, sock, store, appended, fn, el } = await loadChat();
  const model = fn("safetyModel");
  const html = fn("safetyPanelHtml");
  assert.deepEqual(model().blocked, []);
  assert.ok(html(model()).includes("Blocked people") && html(model()).includes("Nobody is blocked."));

  await fn("blockKey")(STRANGER); // not in the member list
  await fn("blockKey")(ANN);
  const m = model();
  const shortKey = fn("shortKey");
  assert.deepEqual(m.blocked.map((b) => [b.key, b.name]).sort(), [[ANN, "Ann"], [STRANGER, shortKey(STRANGER)]].sort(),
    "by the member list's name, or a short key");
  assert.ok(m.blocked.every((b, i) => i === 0 || m.blocked[i - 1].ts >= b.ts), "newest first");
  const page = html(m);
  for (const b of m.blocked) {
    assert.equal(b.date, fn("blockDateLabel")(store.blocked[b.key].ts), "with the date blocked");
    assert.ok(page.includes(`data-unblock="${b.key}"`) && page.includes(`Blocked ${b.date}`), "each row has the date and Unblock");
  }
  assert.ok(!page.includes("Nobody is blocked."));

  // /block <name> and /unblock <name>, through the composer.
  const input = el("msg-input");
  const send = async (text) => { input.value = text; await fn("sendMessage")(); await new Promise((r) => setImmediate(r)); };
  await send("/block cy");
  assert.ok(store.isBlocked(CY), "/block takes the member list's name, letter case aside");
  await send("/unblock Cy");
  assert.equal(store.isBlocked(CY), false, "/unblock too");
  await send("/block Nobody_Here");
  assert.ok(shown(appended).includes('No one called "Nobody_Here" is in the member list.'));
  await send(`/unblock ${shortKey(STRANGER)}`);
  assert.equal(store.isBlocked(STRANGER), false, "/unblock takes the short key a blocked person is listed under");
  await send("/blocklist");
  assert.ok(shown(appended).includes("Blocked: Ann (since "), "/blocklist lists them");

  // The message and member list menu: Block beside Report, Unblock (and no Follow) while blocked.
  const ev = { preventDefault() {}, stopPropagation() {}, clientX: 10, clientY: 10 };
  fn("showUserContextMenu")(ev, "Ben", BEN);
  let menu = el("user-context-menu").innerHTML;
  assert.ok(menu.indexOf("blockFromCtx()") > 0 && menu.indexOf("blockFromCtx()") < menu.indexOf("reportUser()"), "Block, then Report");
  fn("blockFromCtx")();
  await settle();
  assert.ok(store.isBlocked(BEN), "the menu blocks by key");
  fn("showUserContextMenu")(ev, "Ben", BEN);
  menu = el("user-context-menu").innerHTML;
  assert.ok(menu.includes("unblockFromCtx()") && !menu.includes("followFromCtx(true)"), "Unblock, and no Follow, while blocked");

  // The DM conversation header.
  vm.runInContext("(k) => { activeDmPartner = k; activeDmPartnerName = 'Cy'; }", ctx)(CY);
  fn("renderDmHeader")();
  assert.ok(el("channel-header").innerHTML.includes("toggleBlockActiveDm()") && el("channel-header").innerHTML.includes(">Block</button>"), "the DM header offers Block");
  fn("toggleBlockActiveDm")();
  await settle();
  assert.ok(store.isBlocked(CY), "which blocks them");
  assert.ok(el("channel-header").innerHTML.includes(">Unblock</button>"), "and then offers Unblock");

  // Offline: the list changes at once; the note goes on the next connection.
  sock.sent.length = 0;
  sock.readyState = 3;
  const madeAt = Date.now();
  await fn("unblockKey")(CY);
  assert.equal(store.isBlocked(CY), false);
  assert.deepEqual(sock.sent, [], "nothing sent while offline");
  assert.deepEqual(store.blockNotesPending.map(({ action, key }) => ({ action, key })), [{ action: "unblock", key: CY }], "kept until it can go");
  // With the time it was made (10n): the note is signed with it when it goes, however late.
  const at = store.blockNotesPending[0].at;
  assert.ok(Number.isFinite(at) && at >= madeAt, "kept with the time it was made");
  sock.readyState = 1;
  fn("onBlockListLoaded")();
  await settle();
  assert.deepEqual(sock.sent.map((p) => opened(p).inner.text), ["[[hum:unblock:v1]]" + CY], "sent on reconnect");
  assert.equal(opened(sock.sent[0]).inner.ts, at, "signed with the time it was made, not the time it went");
  assert.deepEqual(store.blockNotesPending, []);
});

test("web's old list of names is gone", () => {
  const files = [];
  const walk = (dir) => {
    for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, ent.name);
      if (ent.isDirectory()) { if (ent.name !== "vendor" && ent.name !== "node_modules") walk(p); }
      else if (/\.(js|html)$/.test(ent.name)) files.push(p);
    }
  };
  walk(WEB);
  const offenders = [];
  for (const f of files) {
    const s = fs.readFileSync(f, "utf8");
    if (/humanity_blocks?\b|humanity_blocked\b|\bfunction (isBlocked|blockUser|unblockUser|getBlockList)\s*\(|\bisBlocked\(\s*(name|u\.name|author)\b/.test(s)) offenders.push(path.relative(WEB, f));
  }
  assert.deepEqual(offenders, [], "no name-based block list or its localStorage key remains in web/");
});

// A ring my own call setting refuses is ignored here too (the relay checks first; this is for a
// server that passes one on anyway), the way a blocked caller's is, the desktop app's
// engine/call_relay.rs on_ring. Seen red 2026-10-10 without ringIgnored in handleVoiceCallMessage:
// "a stranger's ring shows no incoming-call screen under the defaults".
test("a ring my call setting refuses shows nothing and sends nothing back; one it allows rings", async () => {
  const { ctx, sock, state, swNotes, chimes, store, handle } = await loadChat();
  const ring = (from) => ({ type: "voice_call", from, from_name: "x", to: ME, action: "ring" });
  const idle = () => vm.runInContext("callState = 'idle'; callPeerKey = null", ctx);
  state.hidden = true;
  sock.sent.length = 0;
  store.recordPassSent(ANN, "11".repeat(16), "invite,message,trade,voice_message");
  store.recordPassSent(BEN, "22".repeat(16), "call,invite,message,trade,voice_message");

  await handle(ring(CY));
  assert.equal(vm.runInContext("callState", ctx), "idle", "a stranger's ring shows no incoming-call screen under the defaults");
  await handle(ring(ANN));
  assert.equal(vm.runInContext("callState", ctx), "idle", "nor a friend's whose pass does not name calls");
  assert.equal(chimes.length, 0, "no chime");
  assert.equal(swNotes.length, 0, "no system notification");
  assert.deepEqual(sock.sent, [], "and nothing back");

  await handle(ring(BEN));
  assert.equal(vm.runInContext("callState", ctx), "ringing-in", "a friend whose pass names calls rings");
  idle();
  await handle({ type: "reach_settings", settings: { message: "friends", call: "anyone", trade: "friends" } });
  await handle(ring(CY));
  assert.equal(vm.runInContext("callState", ctx), "ringing-in", "under Anyone a stranger rings");
});

// 10m R8 (2026-10-10, docs/design/blocking-and-safe-mode.md): group membership in the app's own
// check is the server's call. Under "Friends and people in my groups" a ring the server let
// through rings, whatever this page's own list of groups says: the relay checked membership
// against its own records, and this page's list loads on connect and after my own group changes,
// so someone who joined a group since was dropped. The other audiences do not look at groups.
// Seen red 2026-10-10 against web/ as at bf8c4c582 (HOS_WEB_DIR): "a ring from someone who joined
// my group since rings" (callState stayed idle).
test("10m R8: under Groups, a ring the server let through rings whatever this page's group list says", async () => {
  const { ctx, sock, handle } = await loadChat();
  const ring = (from) => ({ type: "voice_call", from, from_name: "x", to: ME, action: "ring" });
  await handle({ type: "reach_settings", settings: { message: "friends", call: "groups", trade: "friends" } });
  // This page loaded its groups before Cy joined one of them.
  ctx._p2pGroups = [{ group_id: "g1", name: "Hikers", members: [ME, ANN] }];
  sock.sent.length = 0;
  await handle(ring(CY));
  assert.equal(vm.runInContext("callState", ctx), "ringing-in", "a ring from someone who joined my group since rings");
  vm.runInContext("callState = 'idle'; callPeerKey = null", ctx);
  // Under People I choose, groups are not looked at: a stranger's ring stays quiet.
  await handle({ type: "reach_settings", settings: { message: "friends", call: "chosen", trade: "friends" } });
  await handle(ring(CY));
  assert.equal(vm.runInContext("callState", ctx), "idle", "under People I choose a stranger's ring shows nothing");
});
