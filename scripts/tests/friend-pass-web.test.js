// The web chat client gives, keeps and withdraws friendship passes (v2, 2026-10-09,
// docs/design/blocking-and-safe-mode.md 10b), the way native src/engine/dm.rs does.
//
// Run: node --test scripts/tests/friend-pass-web.test.js   (in `just rig-tests`)
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with
// the DOM replaced by a stub, as in p2p-direct-offers.test.js: the real friend-pass.js,
// crypto.js, chat-dm-store.js (over a stand-in IndexedDB), app.js, chat-social.js and
// chat-voice-calls.js. Only the Dilithium and Kyber primitives are stand-ins: "signing" returns
// the signed words themselves and "checking" compares them, so a check passes exactly when the
// checker rebuilt the words the minter signed. The real signatures are covered by
// scripts/pq-kat.mjs and the relay's tests.
//
// What it proves:
//  1. On the member list, the client gives a pass to every mutual follow that has none (and to
//     nobody else), minted for the server its challenge named, with a fresh serial and the
//     defaults (no calls); a second list mints nothing more.
//  2. Unfollowing withdraws: `cert_revoke {serial}` goes out, is resent on every member list
//     until the relay answers `cert_revoked`, and stops then.
//  3. A pass from a friend is checked against this server and kept; their unfollow drops it; the
//     echo of a pass we gave from another device records its serial.
//  4. A DM, a call ring and a trade note carry the pass we hold (the ring here).
//  5. A new server identity voids every pass held or given.
//  6. 10l (2026-10-10): a pass put carries a ref and counts as given only after the relay's
//     `dm_put_ok` (only then does the self-copy go); refused or unanswered for 30 seconds,
//     nothing is recorded or withdrawn and the next member list sends it again; once one is
//     taken, the one that went unanswered (perhaps standing) is withdrawn and the refused one is
//     not; Unfollow withdraws a pass still on its way, and its answer records nothing. Every
//     test answers the relay's way, since a pass counts as given only after that.
//
// Red first, 2026-10-09: with `withdrawPassesTo(peer)` taken out of setFollowLocal's unfollow,
// "unfollowing withdraws the pass" failed (no cert_revoke sent); with the `cert_revoked` case
// removed from app.js handleMessage, "the answer stops the resending" failed.

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

const ME = "a1".repeat(32);
const ANN = "b2".repeat(32);
const BEN = "c3".repeat(32);
const CY = "d4".repeat(32);
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const CTL_FOLLOW = "[[hum:follow]]";
const CTL_UNFOLLOW = "[[hum:unfollow]]";
const CTL_FRIEND_CERT = "[[hum:friend-cert]]";

async function settle() {
  for (let i = 0; i < 12; i++) await new Promise((r) => setImmediate(r));
}

async function loadChat() {
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: anything(),
    navigator: anything(),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    indexedDB: fakeIndexedDB(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: () => 0,
    clearTimeout: () => {},
    setInterval: () => 0,
    clearInterval: () => {},
    WebSocket: Object.assign(function () { return fakeSocket(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    Audio: function () { return anything(); },
    addEventListener: () => {},
    removeEventListener: () => {},
    matchMedia: () => anything(),
    requestAnimationFrame: () => 0,
    Notification: anything(),
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
  run("shared/events.js");
  run("shared/friend-pass.js");
  run("chat/crypto.js");
  run("chat/chat-dm-store.js");
  run("chat/app.js");
  for (const name of ["updateStats", "renderServerList", "renderGroupList", "addSystemMessage", "hosIcon", "shortKey", "esc", "playNotificationChime", "renderPresenceSidebarForActiveContext", "roleBadge", "isBlocked", "generateIdenticon"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  run("chat/chat-social.js");
  run("chat/chat-voice-calls.js");
  // The stand-in primitives (see the top of this file), and a DM builder that records the
  // control it carries instead of sealing it.
  const key = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => key;
  ctx.pqSignMessage = async (_secret, bytes) => new Uint8Array(bytes);
  ctx.pqVerifyMessage = async (_pk, bytes, sig) => Buffer.from(bytes).equals(Buffer.from(sig));
  vm.runInContext(`(s, me) => {
    ws = s; myKey = me; myDilithiumPublicHex = me; myDilithiumSecret = new Uint8Array(4);
    pqBuildDmPuts = async (text, peer, ts, opts) => ({
      recipientPut: { type: 'dm_put', to: peer, ctl: text, cert: opts && opts.ctlCert },
      selfPut: { type: 'dm_put', to: myKey, ctl: text },
      inner: {},
    });
  }`, ctx)(fakeSocket(), ME);
  const sock = vm.runInContext("ws", ctx);
  // The relay's challenge names the server; the member list carries the DM keys.
  await vm.runInContext("handleMessage", ctx)({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  assert.ok(await vm.runInContext("hosDmStore", ctx).init(ME, "localhost"), "the store loads");
  return { ctx, sock, store: vm.runInContext("hosDmStore", ctx) };
}

const users = [ANN, BEN, CY].map((k, i) => ({ public_key: k, name: `P${i}`, role: "", kyber_public: "kyber-" + k.slice(0, 4) }));
async function memberList(ctx) {
  await vm.runInContext("handleMessage", ctx)({ type: "full_user_list", users });
  await settle();
}
const sentOf = (sock, type) => sock.sent.filter((m) => m.type === type);
const passesSent = (sock) => sock.sent.filter((m) => m.type === "dm_put" && m.ctl === CTL_FRIEND_CERT && m.to !== ME);

// The relay's answer (10l) to every put that carried a ref and has not been answered yet:
// `dm_put_ok` as a relay that stored them sends, or `dm_put_refused` with `reason`.
const answeredRefs = new Set();
async function answerPuts(ctx, sock, type = "dm_put_ok", reason = "rate") {
  for (const m of sock.sent) {
    if (m.type !== "dm_put" || typeof m.ref !== "string" || answeredRefs.has(m.ref)) continue;
    answeredRefs.add(m.ref);
    await vm.runInContext("handleMessage", ctx)(type === "dm_put_ok" ? { type, ref: m.ref } : { type, ref: m.ref, reason });
  }
  await settle();
}

test("mutual follows get a pass on the member list, once; withdrawals resend until answered", async () => {
  const { ctx, sock, store } = await loadChat();
  const fp = require(path.join(WEB, "shared", "friend-pass.js"));
  for (const k of [ANN, BEN, CY]) store.setFollowing(k, true);
  for (const k of [ANN, BEN]) store.setFollower(k, true); // Cy does not follow back
  await memberList(ctx);
  await answerPuts(ctx, sock); // the relay took them (10l)

  const given = passesSent(sock);
  assert.deepEqual(given.map((m) => m.to).sort(), [ANN, BEN].sort(), "a pass for each mutual follow, none for a one-way follow");
  for (const m of given) {
    const pass = fp.friendPassParse(m.cert);
    assert.ok(pass, "the pass is a v2 pass in canonical form");
    assert.equal(pass.may, "invite,message,trade,voice_message", "the defaults: no calls");
    // The stand-in signature is the signed words: they name this server, me and them.
    assert.equal(Buffer.from(pass.sig, "base64").toString(), fp.friendPassPreimage(SERVER, ME, m.to, pass.serial, pass.may));
    assert.ok(store.certSentTo(m.to));
  }
  const annSerial = fp.friendPassParse(given.find((m) => m.to === ANN).cert).serial;
  assert.notEqual(annSerial, fp.friendPassParse(given.find((m) => m.to === BEN).cert).serial, "a fresh serial for each pass");

  await memberList(ctx);
  assert.equal(passesSent(sock).length, 2, "the next member list mints nothing more");

  // Unfollowing withdraws the pass.
  await vm.runInContext("setFollowLocal", ctx)(ANN, false);
  await settle();
  assert.deepEqual(sentOf(sock, "cert_revoke").map((m) => m.serial), [annSerial], "unfollowing withdraws the pass");
  assert.ok(sock.sent.some((m) => m.type === "dm_put" && m.to === ANN && m.ctl === CTL_UNFOLLOW), "and tells them, as before");
  assert.ok(!store.certSentTo(ANN));

  await memberList(ctx);
  assert.equal(sentOf(sock, "cert_revoke").length, 2, "unanswered, the withdrawal is sent again");
  await vm.runInContext("handleMessage", ctx)({ type: "cert_revoked", to: ME, serial: annSerial });
  await settle();
  await memberList(ctx);
  assert.equal(sentOf(sock, "cert_revoke").length, 2, "the answer stops the resending");
  assert.deepEqual([...store.withdrawalsPending], []);
});

test("a friend's pass is checked, kept, carried and dropped when they unfollow", async () => {
  const { ctx, sock, store } = await loadChat();
  const fp = require(path.join(WEB, "shared", "friend-pass.js"));
  const serial = "00112233445566778899aabbccddeeff";
  const may = "invite,message,trade,voice_message";
  const passFrom = (issuer, server) => fp.friendPassJson(serial, may, Buffer.from(fp.friendPassPreimage(server, issuer, ME, serial, may)).toString("base64"));
  const ingest = (inner) => vm.runInContext("ingestDmControl", ctx)(inner);
  await memberList(ctx); // the store learns which server its passes name

  await ingest({ from: BEN, to: ME, text: CTL_FRIEND_CERT, cert: passFrom(BEN, "did:hum:elsewhere") });
  assert.equal(store.certFor(BEN), null, "a pass given on another server is not kept");
  await ingest({ from: BEN, to: ME, text: CTL_FRIEND_CERT, cert: passFrom(ANN, SERVER) });
  assert.equal(store.certFor(BEN), null, "nor one signed for someone else's words");
  const good = passFrom(BEN, SERVER);
  await ingest({ from: BEN, to: ME, text: CTL_FRIEND_CERT, cert: good });
  assert.equal(store.certFor(BEN), good, "a good pass is kept");

  // The ring carries it.
  vm.runInContext("startCall", ctx)(BEN, "Ben");
  const ring = sock.sent.find((m) => m.type === "voice_call" && m.action === "ring");
  assert.equal(ring && ring.friend_cert, good, "a call ring carries the callee's pass");

  await ingest({ from: BEN, to: ME, text: CTL_UNFOLLOW });
  assert.equal(store.certFor(BEN), null, "their unfollow withdrew it, so it is dropped");

  // Our own pass, echoed from another of our devices: its serial is recorded.
  await ingest({ from: ME, to: CY, text: CTL_FRIEND_CERT, cert: fp.friendPassJson(serial, may, "c2ln") });
  assert.ok(store.certSentTo(CY), "the echo of a pass we gave records it");

  // A new server identity voids it all.
  assert.ok(store.setPassServer("did:hum:another"));
  assert.ok(!store.certSentTo(CY) && store.certFor(BEN) === null);
});

// The relay takes a burst of 8 private messages, then one a second (handlers/dm_rate.rs); a pass
// sent past that is refused while the page counts it as given. A sweep sends at most 6 at once,
// then one per 1.1 s, and only one sweep runs at a time (2026-10-10 batch review). Seen red with
// the pause taken out of sweepFriendPasses: "the rest wait their turn" (no waits recorded).
test("a sweep owing many passes sends six at once, then waits between the rest", async () => {
  const { ctx, sock, store } = await loadChat();
  const many = Array.from({ length: 10 }, (_, i) => (i + 10).toString(16).padStart(2, "0").repeat(32));
  for (const k of many) { store.setFollowing(k, true); store.setFollower(k, true); }
  const waits = [];
  // The 30-second wait for the relay's answer (10l) is not the pacing: it is not run here.
  ctx.setTimeout = (fn, ms) => {
    if (ms >= 30000) return 0;
    waits.push({ ms, sentSoFar: passesSent(sock).length });
    fn();
    return 0;
  };
  const list = many.map((k, i) => ({ public_key: k, name: `M${i}`, role: "", kyber_public: "kyber-" + k.slice(0, 4) }));
  await vm.runInContext("handleMessage", ctx)({ type: "full_user_list", users: list });
  await settle();
  await settle();
  const paced = waits.filter((w) => w.ms >= 1000);
  assert.equal(passesSent(sock).length, 10, "every friend gets a pass");
  assert.equal(paced.length, 4, "the rest wait their turn: one wait before each pass after the sixth");
  assert.equal(paced[0].sentSoFar, 6, "six went at once before the first wait");
});

// ── 10l: a pass counts as given only once the server took it (2026-10-10) ──
// docs/design/blocking-and-safe-mode.md 10l. Every put that gives a pass carries a `ref`; only
// the relay's `dm_put_ok` for it records the pass as given and lets the self-copy go (my other
// devices, and this page's own echo, would otherwise adopt a pass the friend never got). A
// `dm_put_refused`, or 30 seconds with no answer, records nothing and the friend is owed a pass,
// which the next member list sends again. Red first: see the end of this file.

const REF_FORM = /^[A-Za-z0-9_-]{1,64}$/;
const selfCopies = (sock) => sock.sent.filter((m) => m.type === "dm_put" && m.to === ME && m.ctl === CTL_FRIEND_CERT);

test("10l: a first pass is recorded only after dm_put_ok; refused or unanswered, the next member list sends it again", async () => {
  const { ctx, sock, store } = await loadChat();
  const fp = require(path.join(WEB, "shared", "friend-pass.js"));
  const answerWaits = [];
  ctx.setTimeout = (fn, ms) => { if (ms === 30000) answerWaits.push(fn); return 0; };
  const handle = (msg) => vm.runInContext("handleMessage", ctx)(msg);
  store.setFollowing(ANN, true);
  store.setFollower(ANN, true);

  await memberList(ctx);
  let puts = passesSent(sock);
  assert.equal(puts.length, 1, "a pass goes to Ann");
  const first = puts[0];
  assert.equal(store.certSentTo(ANN), false, "not given until the server says so");
  assert.ok(typeof first.ref === "string" && REF_FORM.test(first.ref), "the put carries a ref of 1 to 64 letters, digits, _ or -");
  assert.equal(selfCopies(sock).length, 0, "and my other devices are not told yet");
  await memberList(ctx);
  assert.equal(passesSent(sock).length, 1, "one pass on its way at a time: the next list sends no second");

  // Refused (the burst limit): nothing recorded, nothing withdrawn, nothing told.
  await handle({ type: "dm_put_refused", ref: first.ref, reason: "rate" });
  await settle();
  assert.equal(store.certSentTo(ANN), false, "a refused pass is not given");
  assert.equal(selfCopies(sock).length, 0);
  assert.equal(sentOf(sock, "cert_revoke").length, 0, "and nothing is withdrawn");

  // The next member list sends it again: the defaults, a new serial, a new ref.
  await memberList(ctx);
  puts = passesSent(sock);
  assert.equal(puts.length, 2, "Ann is still owed a pass");
  const second = puts[1];
  assert.equal(fp.friendPassParse(second.cert).may, "invite,message,trade,voice_message", "with the same may");
  assert.notEqual(second.ref, first.ref);
  assert.notEqual(fp.friendPassParse(second.cert).serial, fp.friendPassParse(first.cert).serial);

  // Thirty seconds of silence: nothing recorded or withdrawn, and a late answer changes nothing.
  assert.equal(answerWaits.length, 2, "each put waits 30 seconds for its answer");
  answerWaits[1]();
  await settle();
  assert.equal(store.certSentTo(ANN), false, "an unanswered pass is not given");
  assert.equal(sentOf(sock, "cert_revoke").length, 0, "and nothing is withdrawn");
  await handle({ type: "dm_put_ok", ref: second.ref });
  await settle();
  assert.equal(store.certSentTo(ANN), false, "an answer after the wait records nothing");
  assert.equal(selfCopies(sock).length, 0);

  // Again, and this time the server takes it.
  await memberList(ctx);
  puts = passesSent(sock);
  assert.equal(puts.length, 3, "sent again after the silence");
  const third = puts[2];
  const thirdSerial = fp.friendPassParse(third.cert).serial;
  await handle({ type: "dm_put_ok", ref: third.ref });
  await settle();
  assert.deepEqual(store.certsSent[ANN], [{ serial: thirdSerial, may: "invite,message,trade,voice_message" }], "given once the server took it");
  assert.equal(selfCopies(sock).length, 1, "and my other devices are told now");
  // The unanswered one may be standing on the server: Ann holds the new one, so it is withdrawn.
  // The refused one was never stored, so nothing is sent for it.
  assert.deepEqual(sentOf(sock, "cert_revoke").map((m) => m.serial), [fp.friendPassParse(second.cert).serial],
    "the pass that went unanswered is withdrawn, the refused one is not");
  await memberList(ctx);
  assert.equal(passesSent(sock).length, 3, "nothing more is owed");

  // A put that gives no pass carries no ref.
  await vm.runInContext("setFollowLocal", ctx)(BEN, true);
  const follow = sock.sent.find((m) => m.type === "dm_put" && m.to === BEN && m.ctl === CTL_FOLLOW);
  assert.ok(follow && !("ref" in follow), "a follow notice carries no ref");
});

test("10l: Unfollow while a pass is on its way withdraws it, and its answer records nothing", async () => {
  const { ctx, sock, store } = await loadChat();
  const fp = require(path.join(WEB, "shared", "friend-pass.js"));
  const handle = (msg) => vm.runInContext("handleMessage", ctx)(msg);
  store.setFollowing(ANN, true);
  store.setFollower(ANN, true);
  await memberList(ctx);
  const [put] = passesSent(sock);
  const serial = fp.friendPassParse(put.cert).serial;
  await vm.runInContext("setFollowLocal", ctx)(ANN, false);
  await settle();
  assert.deepEqual(sentOf(sock, "cert_revoke").map((m) => m.serial), [serial], "the pass on its way is withdrawn: the server may store it");
  await handle({ type: "dm_put_ok", ref: put.ref });
  await settle();
  assert.equal(store.certSentTo(ANN), false, "its answer records nothing");
  assert.equal(selfCopies(sock).length, 0, "and my other devices are not told of it");
});

// Red first, 2026-10-10 (10l). Both tests above were run against web/ as at b46441843 (before
// 10l) through HOS_WEB_DIR and seen failing there: "not given until the server says so" (the pass
// was recorded the moment it was sent) and "and my other devices are not told of it" (the
// self-copy went with it). Then one break at a time in a copy of the fixed web/: the self-copy
// sent at once failed "and my other devices are not told yet"; chat-dm-store.js withdrawPassesTo
// leaving passes on their way failed "the pass on its way is withdrawn: the server may store it";
// the 30-second wait settling as taken failed "an unanswered pass is not given".
