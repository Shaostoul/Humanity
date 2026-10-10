// The choice for each friend syncs as its own note, in the web chat client (10n, 2026-10-10,
// docs/design/blocking-and-safe-mode.md section 10n). The desktop app builds the same rules from
// the same spec (src/net/dm_pq.rs CTL_CHOICE).
//
// Run: node --test scripts/tests/choice-web.test.js   (in `just rig-tests`)
//
// The page scripts run as they do in the browser, in one shared global scope per page (node:vm),
// with the DOM replaced by a stub, as in reach-web.test.js: the real friend-pass.js, reach.js,
// block.js, crypto.js, chat-dm-store.js (over a stand-in IndexedDB), app.js, chat-dms.js,
// chat-social.js and chat-privacy.js. Two pages loaded with the same identity are two of my
// devices: each has its own store, and the test plays the relay between them (my mailbox, the
// answers to pass puts, and `cert_revoked`, which the relay sends to every socket of mine). Only
// the Dilithium and Kyber primitives are stand-ins: "signing" returns the signed words and
// "checking" compares them; "sealing" base64s the plaintext and names the key it was sealed to.
//
// What it proves (each rule, then the spec's two-device sequences):
//  0. The note's bytes and its reading, against the spec's test vector, and the unusable forms.
//  N1. A tick keeps the choice with its time and sends the note to my own mailbox only, before
//      that friend's withdrawals and pass; made offline it waits, across a reload, and goes on
//      the next connection before them.
//  N2. A note from my other device is applied once (by its signature), the newest wins, at an
//      equal time the larger `may` text; an older one changes nothing; one about someone I
//      blocked is ignored; one from anyone else is dropped unread; none is ever shown or stored.
//  N3. Unfollow and Block clear the choice; an Unfollow made offline waits (both copies) and goes
//      on the next connection, and my other device unfollows them too.
//  N4. A choice that takes something away withdraws at once every pass beyond it, standing or on
//      its way; the sweep sends a pass carrying the choice to a friend who holds none and has
//      none on its way, and nothing more once one stands.
//  N6. A choice note clears the "changed on my other device" mark; while marked the ticks are
//      empty and a tick gives exactly what is ticked; a marked person no longer a mutual follow
//      is not listed.
//  N7. On connect, nothing is swept (no note, withdrawal or pass) until the mailbox was read.
//  N9. Unfollow drops a contact request still waiting for its answer.
//  Sequences: an untick made offline on one device, the other opened later; an untick whose new
//  pass is refused, the other device online and then offline; an older echo read after a newer
//  one; an Unfollow made offline (N3); a tick on a marked friend (N6).
//  10o, the parity review of 10n: O1 a page counts only mailbox pages carrying its own fetch's ref
//    (another device's last page starts nothing), pages from its own last id, and a live DM
//    during its fetch does not move the read position, so no row is skipped; O2 an unanswered
//    pass makes no one owed one; O3 a waiting Unfollow is void once I follow them again (the
//    echo of my own follow clears it; the flush checks) or block them; O4 Block drops a waiting
//    choice note; O5 every path that sends withdrawals sends a waiting choice note first; O6 a
//    cleared choice is the empty may; O7 a note about someone I blocked is not recorded as seen,
//    and my own contact request's echo does not follow them again while its pass is withdrawn.
// (N5, every pass's self-copy held until dm_put_ok again, and the N4 rule for echoes, are in
// reach-web.test.js beside the 10l and 10m tests they replaced. N8, the scratch pad, is in
// private-files-web.test.js beside the other scratch pad tests.)
//
// Red first: see the list at the end of this file.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const fp = require(path.join(WEB, "shared", "friend-pass.js"));
const reach = require(path.join(WEB, "shared", "reach.js"));

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

// An element that keeps what is written to it (text, children) and absorbs the rest.
function fakeElement(tag) {
  const own = { tag, children: [], style: {}, dataset: {}, className: "", textContent: "", disabled: false };
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
// The ids the chat page's own HTML has (web/chat/index.html): only those are "there".
const PAGE_IDS = new Set([...fs.readFileSync(path.join(WEB, "chat", "index.html"), "utf8").matchAll(/\bid="([^"]+)"/g)].map((m) => m[1]));
// The page's document. A lookup by id answers "not there" (null), the way a browser does, for any
// element the page's HTML does not have.
function fakeDocument() {
  const kept = new Map();
  const present = PAGE_IDS;
  return new Proxy({}, {
    get(_t, prop) {
      if (prop === "createElement") return fakeElement;
      if (prop === "getElementById") {
        return (id) => {
          if (!present.has(id)) return null;
          if (!kept.has(id)) kept.set(id, fakeElement("#" + id));
          return kept.get(id);
        };
      }
      if (prop === "querySelectorAll") return () => [];
      if (prop === "querySelector") return () => null;
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

const ME = "a1".repeat(32);
const ANN = "b2".repeat(32);
const BEN = "c3".repeat(32);
const CY = "d4".repeat(32);
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const CTL_FOLLOW = "[[hum:follow]]";
const CTL_UNFOLLOW = "[[hum:unfollow]]";
const CTL_FRIEND_CERT = "[[hum:friend-cert]]";
const MY_KYBER = "my-kyber";
const kyberOf = (k) => "kyber-" + k.slice(0, 4);
const DEFAULT_MAY = "invite,message,trade,voice_message";
const WITH_CALL = "call,invite,message,trade,voice_message";
const NO_TRADE = "invite,message,voice_message";
const NO_TICKS = { message: false, call: false, trade: false };

// Wait until the page is idle: its asynchronous work is WebCrypto (the store hashes and encrypts
// every record) and the stand-in IndexedDB; settle() waits for 12 idle turns in a row.
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
const memberUsers = () => [[ANN, "Ann"], [BEN, "Ben"], [CY, "Cy"]].map(([k, name]) => ({ public_key: k, name, role: "", kyber_public: kyberOf(k) }));

/**
 * One of my devices: the chat page with my identity, its own store and socket. `dev.sock` is the
 * page's socket as it is now (a reconnect opens a new one).
 */
async function loadDevice() {
  const appended = [];
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDocument(),
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
  run("shared/events.js");
  run("shared/friend-pass.js");
  run("shared/reach.js");
  run("shared/block.js");
  run("chat/crypto.js");
  run("chat/chat-dm-store.js");
  run("chat/app.js");
  run("chat/chat-dms.js");
  for (const name of ["updateStats", "renderServerList", "renderGroupList", "hosIcon", "playNotificationChime", "renderPresenceSidebarForActiveContext", "roleBadge", "isBlocked", "generateIdenticon", "switchSidebarTab", "renderChannelList", "isMobile", "setStatus", "updateChannelHeader", "updateInputForChannel", "updateChannelList"]) {
    ctx[name] = () => "";
  }
  ctx.notifyNewMessage = () => {};
  run("chat/chat-social.js");
  run("chat/chat-privacy.js");
  ctx.appendMessage = (el) => { appended.push(el); };
  const key = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => key;
  ctx.pqSignMessage = async (_secret, bytes) => new Uint8Array(bytes);
  ctx.pqVerifyMessage = async (_pk, bytes, sig) => Buffer.from(bytes).equals(Buffer.from(sig));
  ctx.pqDmSeal = async (pub, plaintext) => ({ ek_ct_b64: b64(pub), nonce_b64: "AAAA", ct_b64: b64(plaintext) });
  ctx.pqDmOpen = async (_secret, ek, _nonce, ct) => (unb64(ek) === MY_KYBER ? unb64(ct) : null);
  vm.runInContext(`(s, me) => {
    ws = s; myKey = me; myName = 'Me_1';
    myDilithiumPublicHex = me; myDilithiumSecret = new Uint8Array(4);
    myKyberPublicBase64 = '${MY_KYBER}'; myKyberSecret = new Uint8Array(4);
  }`, ctx)(fakeSocket(), ME);
  const handle = (msg) => vm.runInContext("handleMessage", ctx)(msg);
  await handle({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  const store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await store.init(ME, "localhost"), "the store loads");
  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  const dev = {
    ctx, store, appended, handle,
    fn: (name) => vm.runInContext(name, ctx),
    get sock() { return vm.runInContext("ws", ctx); },
  };
  dev.sock.sent.length = 0;
  return dev;
}

/** A mutual follow, as the page and its store know one. */
function befriend(dev, key) {
  dev.store.setFollowing(key, true);
  dev.store.setFollower(key, true);
  vm.runInContext("(k) => { myFollowing.add(k); myFollowers.add(k); }", dev.ctx)(key);
}

/** My choice for `peer`, as a note from my other device sets it (no note is queued here). */
function chose(dev, peer, may, at = 1) {
  dev.store.applyChoiceNote(peer, may, at);
  assert.equal(dev.store.choiceMay(peer), may, "set up: the choice");
}

// What a sealed dm_put carries (the stand-in seal is base64 of the plaintext).
function opened(put) {
  const env = JSON.parse(put.content);
  return { sealedTo: unb64(env.ek_ct_b64), inner: JSON.parse(unb64(env.ct_b64)) };
}
/** A DM from me to `to` as one of my devices seals it to my own key (a self-copy, or a note). */
function selfEnvelope(to, text, cert, ts) {
  const sig = b64(`hum/dm/v2\n${ME}\n${to}\n${ts}\n${text}`);
  const inner = JSON.stringify({ v: 2, from: ME, to, ts, text, sig, cert });
  return JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(inner) });
}
/** A DM from `from` to me. */
function envelopeFrom(from, text, ts) {
  const sig = b64(`hum/dm/v2\n${from}\n${ME}\n${ts}\n${text}`);
  const inner = JSON.stringify({ v: 2, from, to: ME, ts, text, sig });
  return JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(inner) });
}
/** A choice note from my other device, made at `at`. */
const choiceNote = (peer, may, at) => selfEnvelope(ME, fp.choiceNoteText(peer, may), undefined, at);
/** My pass to `peer` as its JSON (the stand-in signature is the signed words). */
const passJson = (peer, serial, may) => fp.friendPassJson(serial, may, b64(fp.friendPassPreimage(SERVER, ME, peer, serial, may)));

const textOf = (put) => opened(put).inner.text;
const puts = (dev) => dev.sock.sent.filter((m) => m.type === "dm_put");
const toMe = (dev) => puts(dev).filter((m) => m.to === ME);
const notesOf = (dev) => toMe(dev).filter((m) => fp.isChoiceNoteText(textOf(m)));
const passesTo = (dev, peer) => puts(dev).filter((m) => m.to === peer && textOf(m) === CTL_FRIEND_CERT);
const passOf = (put) => fp.friendPassParse(opened(put).inner.cert);
const revokes = (dev) => dev.sock.sent.filter((m) => m.type === "cert_revoke").map((m) => m.serial);
const at = (dev, m) => dev.sock.sent.indexOf(m);

let nextId = 500;
/** Hand my other device what reached my mailbox, live (`dm_new`), in order. */
async function live(dev, list) {
  for (const p of list) {
    await dev.handle({ type: "dm_new", id: ++nextId, content: p.content || p });
  }
  await settle();
}
/** The relay's answer to a pass put. */
async function answer(dev, put, type = "dm_put_ok", reason = "rate") {
  await dev.handle(type === "dm_put_ok" ? { type, ref: put.ref } : { type, ref: put.ref, reason });
  await settle();
}
/** The relay confirms every withdrawal `from` asked for, to each of my devices online. */
async function confirm(from, devices) {
  for (const serial of new Set(revokes(from))) {
    for (const d of devices) await d.handle({ type: "cert_revoked", to: ME, serial });
  }
  await settle();
}
async function memberList(dev) {
  await dev.handle({ type: "full_user_list", users: memberUsers() });
  await settle();
}

/** The page's latest mailbox fetch (`dm_fetch`), the one its next page answers. */
const lastFetch = (dev) => dev.sock.sent.filter((m) => m.type === "dm_fetch").pop();

/**
 * The relay's answer to the page's latest fetch, from `mailbox` (rows {id, content} in id order):
 * the rows after its after_id, at most `pageSize`, carrying its ref (10o O1; the relay echoes a
 * fetch's ref on its page, and every device of mine receives every page).
 */
async function serveFetch(dev, mailbox, pageSize = 200) {
  const f = lastFetch(dev);
  assert.ok(f, "the page asked for its mailbox");
  const after = mailbox.filter((r) => r.id > (Number(f.after_id) || 0));
  await dev.handle({ type: "dm_batch", ref: f.ref, messages: after.slice(0, pageSize), done: after.length <= pageSize });
  await settle();
  return f;
}

/**
 * The page starts connecting again, the way app.js does it: a new socket, the challenge, the
 * channel list (which loads the store and asks for the mailbox) and the member list. The mailbox
 * is not answered yet. Returns the new socket.
 */
async function reconnectStart(dev) {
  dev.sock.readyState = 3;
  dev.fn("openSocket")();
  const sock = dev.sock;
  if (typeof sock.onopen === "function") sock.onopen();
  await dev.handle({ type: "identify_challenge", nonce: "cd", server_did: SERVER });
  await dev.handle({ type: "channel_list", channels: [] });
  await settle();
  assert.ok(sock.sent.some((m) => m.type === "dm_fetch"), "the page asks for its mailbox");
  await memberList(dev);
  return sock;
}

/**
 * The page connects again (reconnectStart), and then the mailbox comes, on the page answering its
 * own fetch (`mailbox`: what reached it meanwhile). Returns what the page sent before it came.
 */
async function reconnect(dev, mailbox = []) {
  const sock = await reconnectStart(dev);
  const early = sock.sent.filter((m) => !["identify", "identify_response", "dm_fetch"].includes(m.type));
  await dev.handle({ type: "dm_batch", ref: lastFetch(dev).ref, messages: mailbox.map((p) => ({ id: ++nextId, content: p.content || p })), done: true });
  await settle();
  return early;
}

// ── 0. The note ─────────────────────────────────────────────────────────

test("10n: the note's bytes and its reading, against the spec's test vector", () => {
  assert.equal(fp.CTL_CHOICE, "[[hum:choice:v1]]");
  const vector = "[[hum:choice:v1]]ab12/invite,message,trade,voice_message";
  assert.equal(fp.choiceNoteText("ab12", "invite,message,trade,voice_message"), vector, "the spec's vector, byte for byte");
  assert.equal(fp.choiceNoteText("ab12", ["voice_message", "trade", "message", "invite"]), vector, "from the words in any order");
  assert.deepEqual(fp.choiceNoteParse(vector), { key: "ab12", may: "invite,message,trade,voice_message" }, "and read back");
  assert.equal(fp.choiceNoteText("ab12", ["invite"]), "[[hum:choice:v1]]ab12/invite", "invite alone when nothing is ticked");
  for (const bad of [
    "[[hum:choice:v1]]ab12/",
    "[[hum:choice:v1]]/message",
    "[[hum:choice:v1]]ab 12/message",
    "[[hum:choice:v1]]ab12/message,teleport",
    "[[hum:choice:v1]]AB12/message",
    "[[hum:choice:v1]]ab12/message,invite",
    "[[hum:choice:v1]]ab12",
    "[[hum:choice:v2]]ab12/message",
  ]) {
    assert.equal(fp.choiceNoteParse(bad), null, `not a usable note: ${bad}`);
    if (bad.startsWith(fp.CTL_CHOICE)) assert.ok(fp.isChoiceNoteText(bad), `but never shown either: ${bad}`);
  }
  assert.equal(fp.choiceNoteText("ab12", []), null, "no words, no note");
  assert.equal(fp.choiceNoteText("ab12", ["teleport"]), null, "a word outside the pass's five, no note");
  // Only a note from me, to me, about someone else counts.
  const inner = (from, to, text) => ({ from, to, text });
  assert.deepEqual(fp.choiceNoteFromSelf(inner(ME, ME, fp.choiceNoteText(ANN, NO_TRADE)), ME), { key: ANN, may: NO_TRADE });
  assert.equal(fp.choiceNoteFromSelf(inner(ANN, ME, fp.choiceNoteText(BEN, NO_TRADE)), ME), null, "from anyone else");
  assert.equal(fp.choiceNoteFromSelf(inner(ME, ANN, fp.choiceNoteText(BEN, NO_TRADE)), ME), null, "to anyone else");
  assert.equal(fp.choiceNoteFromSelf(inner(ME, ME, fp.choiceNoteText(ME, NO_TRADE)), ME), null, "about myself");
  // Which choice wins.
  assert.equal(fp.choiceWins("invite", 2, { may: WITH_CALL, at: 1 }), true, "the newer");
  assert.equal(fp.choiceWins(WITH_CALL, 1, { may: "invite", at: 2 }), false, "not the older");
  assert.equal(fp.choiceWins(NO_TRADE, 5, { may: WITH_CALL, at: 5 }), true, "at an equal time, the larger text");
  assert.equal(fp.choiceWins(WITH_CALL, 5, { may: NO_TRADE, at: 5 }), false);
  assert.equal(fp.choiceWins("invite", 5, { may: "", at: 5 }), true, "a cleared choice is the smallest");
  // The desktop app's marker, once its half has landed (built from the same spec in parallel).
  const rust = fs.readFileSync(path.join(ROOT, "src", "net", "dm_pq.rs"), "utf8");
  const m = rust.match(/pub const CTL_CHOICE:\s*&str\s*=\s*"([^"]*)"/);
  if (m) assert.equal(m[1], fp.CTL_CHOICE, "the desktop app's CTL_CHOICE is the same marker");
});

// ── N1 ─────────────────────────────────────────────────────────────────

test("10n N1: a tick keeps the choice and sends its note to my own mailbox first, then the withdrawals and the pass", async () => {
  const a = await loadDevice();
  befriend(a, ANN);
  const OLD = "11".repeat(16);
  chose(a, ANN, WITH_CALL);
  a.store.recordPassSent(ANN, OLD, WITH_CALL);
  const t0 = Date.now();
  assert.equal(await a.fn("setFriendTick")(ANN, "call", false), true, "Call unticked");
  await settle();
  const [note] = notesOf(a);
  assert.ok(note, "a choice note goes to my own mailbox");
  assert.equal(notesOf(a).length, 1, "one");
  assert.equal(opened(note).sealedTo, MY_KYBER, "sealed to my own key");
  const inner = opened(note).inner;
  assert.deepEqual([inner.from, inner.to], [ME, ME], "from me, to me");
  assert.equal(inner.text, fp.choiceNoteText(ANN, DEFAULT_MAY), "naming her and what she may do now");
  assert.ok(inner.ts >= t0, "signed with when the choice was made");
  assert.deepEqual(a.store.choiceOf(ANN), { may: DEFAULT_MAY, at: inner.ts }, "and kept with that time");
  const revoke = a.sock.sent.find((m) => m.type === "cert_revoke" && m.serial === OLD);
  const [pass] = passesTo(a, ANN);
  assert.ok(revoke && pass, "the pass allowing calls is withdrawn and a new one goes");
  assert.ok(at(a, note) < at(a, revoke) && at(a, revoke) < at(a, pass), "the note first, then the withdrawal, then the pass");
  assert.deepEqual(puts(a).filter((m) => m.to !== ME && m.to !== ANN), [], "nobody else is sent anything");
  // The choice stays when the pass carrying it is taken (until 10n it was cleared then).
  await answer(a, pass);
  assert.deepEqual(a.store.choiceOf(ANN), { may: DEFAULT_MAY, at: inner.ts }, "the choice stays once its pass is taken");
  // Its echo, coming back to this page from my mailbox, is not applied again.
  a.sock.sent.length = 0;
  await live(a, [note]);
  assert.deepEqual(a.sock.sent, [], "my own note coming back changes nothing");
  assert.deepEqual(a.store.conversation(ME), [], "and is not a message");
});

// ── N2 ─────────────────────────────────────────────────────────────────

test("10n N2: a note from my other device is applied once; the newest wins, at an equal time the larger text; never shown", async () => {
  const b = await loadDevice();
  for (const k of [ANN, BEN, CY]) befriend(b, k);
  const P = "22".repeat(16);
  b.store.recordPassSent(ANN, P, DEFAULT_MAY);
  const ticks = () => b.fn("friendTicks")(ANN);

  const n1 = choiceNote(ANN, WITH_CALL, 2000);
  await live(b, [n1]);
  assert.deepEqual(ticks(), { message: true, call: true, trade: true }, "a note from my other device becomes my choice");
  await live(b, [choiceNote(ANN, "invite", 1000)]);
  assert.deepEqual(ticks(), { message: true, call: true, trade: true }, "an older one changes nothing");
  await live(b, [choiceNote(ANN, NO_TRADE, 2000)]);
  assert.deepEqual(ticks(), { message: true, call: false, trade: false }, "at an equal time the larger may text wins");
  await live(b, [choiceNote(ANN, WITH_CALL, 2000)]);
  assert.deepEqual(ticks(), { message: true, call: false, trade: false }, "whichever order the two arrive in");
  assert.equal(b.store.choiceMay(ANN), NO_TRADE);

  // Applied once: a note delivered again does not clear a mark set since (N6 clears it the first time).
  b.store.recordPassSent(BEN, "33".repeat(16), DEFAULT_MAY);
  const nb = choiceNote(BEN, DEFAULT_MAY, 3000);
  await live(b, [nb]);
  await b.handle({ type: "cert_revoked", to: ME, serial: "33".repeat(16) });
  await settle();
  assert.ok(b.store.passChangedOnOtherDevice(BEN), "Ben is marked: my other device withdrew his pass");
  await live(b, [nb]);
  assert.ok(b.store.passChangedOnOtherDevice(BEN), "the same note delivered again is not applied again");

  // About someone I blocked: ignored. From anyone else: dropped unread.
  await b.fn("blockKey")(CY);
  await settle();
  await live(b, [choiceNote(CY, WITH_CALL, 9000000000000)]);
  assert.equal(b.store.choiceOf(CY), null, "a note about someone I blocked is ignored");
  b.appended.length = 0;
  await live(b, [envelopeFrom(ANN, fp.choiceNoteText(BEN, WITH_CALL), 4000)]);
  assert.notEqual(b.store.choiceMay(BEN), WITH_CALL, "a choice note from anyone else is dropped unread");
  for (const k of [ME, ANN, BEN]) assert.deepEqual(b.store.conversation(k), [], "no note is stored as a message");
  assert.equal(b.appended.length, 0, "nor shown");
});

// ── N3 ─────────────────────────────────────────────────────────────────

test("10n N3: an Unfollow made offline waits, both copies, and goes on the next connection; my other device unfollows too", async () => {
  const a = await loadDevice();
  const b = await loadDevice();
  const P = "44".repeat(16);
  for (const d of [a, b]) {
    befriend(d, ANN);
    chose(d, ANN, WITH_CALL);
    d.store.recordPassSent(ANN, P, WITH_CALL);
  }

  // Device A, offline, unfollows Ann.
  a.sock.readyState = 3;
  a.sock.sent.length = 0;
  await a.fn("setFollowLocal")(ANN, false);
  await settle();
  assert.deepEqual(a.sock.sent, [], "nothing is sent while offline");
  assert.equal(a.store.following.has(ANN), false, "the unfollow takes effect here at once");
  assert.equal(a.store.choiceOf(ANN), null, "and my choice for her is cleared");
  assert.deepEqual(a.store.unfollowsPending.map((u) => u.peer), [ANN], "the Unfollow waits");
  const madeAt = a.store.unfollowsPending[0].at;
  assert.ok(await a.store.init(ME, "localhost"));
  assert.deepEqual(a.store.unfollowsPending.map((u) => u.peer), [ANN], "across a reload");

  // On the next connection both copies go, before the withdrawal.
  await reconnect(a);
  const toAnn = puts(a).find((m) => m.to === ANN && textOf(m) === CTL_UNFOLLOW);
  const toMine = toMe(a).find((m) => textOf(m) === CTL_UNFOLLOW && opened(m).inner.to === ANN);
  assert.ok(toAnn, "the Unfollow goes to her");
  assert.ok(toMine, "and to my own mailbox, for my other devices");
  assert.equal(opened(toMine).inner.ts, madeAt, "signed with when it was made");
  const revoke = a.sock.sent.find((m) => m.type === "cert_revoke" && m.serial === P);
  assert.ok(revoke && at(a, toMine) < at(a, revoke), "before the withdrawal of the pass she held");
  assert.deepEqual(a.store.unfollowsPending, [], "and it waits no more");

  // Device B hears it from my mailbox: it unfollows her too, withdraws, and clears the choice.
  await live(b, [toMine]);
  assert.equal(b.store.following.has(ANN), false, "my other device unfollows her too");
  assert.deepEqual(revokes(b), [P], "withdraws the pass it knew of");
  assert.equal(b.store.choiceOf(ANN), null, "and clears my choice for her");

  // Block clears the choice as well, on both devices, and a note about her after it is ignored.
  for (const d of [a, b]) { befriend(d, BEN); chose(d, BEN, WITH_CALL, 5); }
  a.sock.sent.length = 0;
  await a.fn("blockKey")(BEN);
  await settle();
  assert.equal(a.store.choiceOf(BEN), null, "Block clears it here");
  await live(b, toMe(a));
  assert.ok(b.store.isBlocked(BEN), "my other device blocks him from its note");
  assert.equal(b.store.choiceOf(BEN), null, "and clears my choice for him");
});

// ── N4 ─────────────────────────────────────────────────────────────────

test("10n N4: a choice taking something away withdraws every pass beyond it at once; the sweep gives one carrying it", async () => {
  const b = await loadDevice();
  for (const k of [ANN, CY]) befriend(b, k);
  const P = "55".repeat(16);
  chose(b, ANN, WITH_CALL);
  b.store.recordPassSent(ANN, P, WITH_CALL);
  // Cy: a pass of mine on its way to him, carrying Call.
  chose(b, CY, WITH_CALL);
  await memberList(b);
  const [onWay] = passesTo(b, CY);
  assert.ok(onWay && passOf(onWay).may === WITH_CALL, "set up: a pass to Cy is on its way");
  assert.deepEqual(passesTo(b, ANN), [], "and none is owed to Ann: hers carries my choice");
  b.sock.sent.length = 0;

  // My other device unticks Call for both.
  await live(b, [choiceNote(ANN, DEFAULT_MAY, 5000), choiceNote(CY, DEFAULT_MAY, 5000)]);
  // (Each withdrawal is sent again until the relay confirms it, so a serial may appear twice.)
  assert.deepEqual([...new Set(revokes(b))].sort(), [P, passOf(onWay).serial].sort(), "every pass allowing calls is withdrawn at once, standing or on its way");
  assert.deepEqual(passesTo(b, ANN).concat(passesTo(b, CY)), [], "the sweep, not the note, sends the new ones");
  assert.equal(b.fn("passPutInFlight")(CY), false, "the one on its way no longer holds the way");

  // The sweep sends each a pass carrying the choice.
  await memberList(b);
  const [toAnn] = passesTo(b, ANN);
  const [toCy] = passesTo(b, CY);
  assert.ok(toAnn && passOf(toAnn).may === DEFAULT_MAY, "Ann gets a pass carrying it");
  assert.ok(toCy && passOf(toCy).may === DEFAULT_MAY, "and so does Cy");
  // The withdrawn pass's late answer records nothing.
  await answer(b, onWay);
  assert.ok(!(b.store.certsSent[CY] || []).some((p) => p.serial === passOf(onWay).serial), "the withdrawn pass is never recorded");
  await answer(b, toAnn);
  await answer(b, toCy);
  b.sock.sent.length = 0;
  await memberList(b);
  assert.deepEqual(puts(b), [], "once a pass carrying it stands, nothing more is owed");
  // No choice at all: the defaults.
  befriend(b, BEN);
  await memberList(b);
  const [toBen] = passesTo(b, BEN);
  assert.ok(toBen && passOf(toBen).may === DEFAULT_MAY, "with no choice stored, the pass carries the defaults");
  await answer(b, toBen);
  // A choice that ADDS something (Call, from my other device): his pass stands until one carrying
  // it is taken, which replaces it (10l).
  b.sock.sent.length = 0;
  await live(b, [choiceNote(BEN, WITH_CALL, 6000)]);
  assert.deepEqual(revokes(b).filter((s) => s === passOf(toBen).serial), [], "an added tick withdraws nothing");
  await memberList(b);
  const [withCall] = passesTo(b, BEN);
  assert.ok(withCall && passOf(withCall).may === WITH_CALL, "a friend whose pass does not carry the choice is owed one that does");
  assert.ok(!revokes(b).includes(passOf(toBen).serial), "and keeps the old one meanwhile");
  await answer(b, withCall);
  assert.ok(revokes(b).includes(passOf(toBen).serial), "which goes once the new one is taken");
  assert.deepEqual(b.store.certsSent[BEN], [{ serial: passOf(withCall).serial, may: WITH_CALL }], "and the new one is the record");
});

// ── N6 ─────────────────────────────────────────────────────────────────

test("10n N6: a choice note clears the mark; a marked friend's ticks are empty; a marked one no longer a mutual follow is not listed", async () => {
  const b = await loadDevice();
  for (const k of [ANN, BEN]) befriend(b, k);
  const P = "66".repeat(16);
  b.store.recordPassSent(BEN, P, DEFAULT_MAY);
  await b.handle({ type: "cert_revoked", to: ME, serial: P });
  await settle();
  assert.ok(b.store.passChangedOnOtherDevice(BEN), "set up: Ben is marked");
  const row = (k) => b.fn("safetyModel")().chosen.find((c) => c.key === k);
  assert.deepEqual(row(BEN).ticks, NO_TICKS, "a marked friend's ticks are drawn empty");
  assert.equal(row(BEN).updating, true, "and he is drawn as updating his pass");
  await live(b, [choiceNote(BEN, NO_TRADE, 7000)]);
  assert.equal(b.store.passChangedOnOtherDevice(BEN), false, "my other device's choice note clears the mark");
  assert.deepEqual(b.fn("friendTicks")(BEN), { message: true, call: false, trade: false }, "and his ticks are that choice");
  await memberList(b);
  const [toBen] = passesTo(b, BEN);
  assert.ok(toBen && passOf(toBen).may === NO_TRADE, "and the sweep may give him a pass carrying it");

  // Cy: followed but not following me back, with a choice, and marked: not listed.
  b.store.setFollowing(CY, true);
  chose(b, CY, WITH_CALL);
  const Q = "77".repeat(16);
  b.store.recordPassSent(CY, Q, WITH_CALL);
  assert.ok(row(CY), "set up: Cy is listed while my pass stands");
  await b.handle({ type: "cert_revoked", to: ME, serial: Q });
  await settle();
  assert.ok(b.store.passChangedOnOtherDevice(CY), "set up: Cy is marked");
  assert.equal(row(CY), undefined, "a marked person who is not a mutual follow is not listed");
});

// ── N7 ─────────────────────────────────────────────────────────────────

test("10n N7: on connect nothing is swept until the mailbox was read, so my notes arrive first", async () => {
  const a = await loadDevice();
  const b = await loadDevice();
  for (const d of [a, b]) befriend(d, BEN);
  // Device A gives Ben a pass without Trade and says so; device B, offline, has heard nothing.
  b.sock.readyState = 3;
  a.sock.sent.length = 0;
  await memberList(a);
  await answer(a, passesTo(a, BEN)[0]);
  assert.equal(await a.fn("setFriendTick")(BEN, "trade", false), true);
  await settle();
  await answer(a, passesTo(a, BEN)[1]);
  const mailbox = toMe(a);
  assert.ok(mailbox.some((m) => fp.isChoiceNoteText(textOf(m))), "set up: my mailbox holds the note");

  // B connects: the member list comes before the mailbox.
  const early = await reconnect(b, mailbox);
  assert.deepEqual(early, [], "nothing is sent before the mailbox was read: no note, no withdrawal, no pass");
  assert.deepEqual(passesTo(b, BEN), [], "and after it nothing is owed: B heard the choice and the pass carrying it");
  assert.deepEqual(b.fn("friendTicks")(BEN), { message: true, call: false, trade: false }, "B's ticks are A's choice");
});

// ── N9 ─────────────────────────────────────────────────────────────────

test("10n N9: Unfollow drops a contact request still waiting for its answer", async () => {
  const a = await loadDevice();
  const answers = [];
  assert.equal(await a.fn("sendContactRequest")(ANN, { onAnswer: (x) => answers.push(x) }), true);
  await settle();
  const [req] = puts(a).filter((m) => m.to === ANN && m.contact_request === true);
  assert.ok(req && a.fn("passPutInFlight")(ANN), "set up: a request on its way");
  await a.fn("setFollowLocal")(ANN, false);
  await settle();
  assert.equal(a.fn("passPutInFlight")(ANN), false, "Unfollow drops the request waiting for its answer");
  assert.ok(answers.some((x) => x && x.taken === false), "and its button hears it was not sent");
  await answer(a, req);
  assert.deepEqual(a.store.certsSent[ANN] || [], [], "its late answer records nothing");
  assert.equal(a.store.following.has(ANN), false, "and does not follow her");
});

// ── The spec's two-device sequences ──────────────────────────────────────

test("10n sequence: an untick made offline on one device, the other opened later", async () => {
  const a = await loadDevice();
  const b = await loadDevice();
  const P = "88".repeat(16);
  for (const d of [a, b]) {
    befriend(d, ANN);
    chose(d, ANN, WITH_CALL);
    d.store.recordPassSent(ANN, P, WITH_CALL);
  }
  // A, offline: Call unticked. Kept, and waiting, across a reload.
  a.sock.readyState = 3;
  a.sock.sent.length = 0;
  assert.equal(await a.fn("setFriendTick")(ANN, "call", false), true, "a tick made offline is kept");
  await settle();
  assert.deepEqual(a.sock.sent, [], "nothing is sent while offline");
  assert.deepEqual(a.store.choiceNotesPending.map((n) => [n.peer, n.may]), [[ANN, DEFAULT_MAY]], "its note waits");
  assert.ok(await a.store.init(ME, "localhost"));
  assert.deepEqual(a.store.choiceNotesPending.map((n) => [n.peer, n.may]), [[ANN, DEFAULT_MAY]], "across a reload");
  assert.equal(a.store.choiceMay(ANN), DEFAULT_MAY, "and so does the choice");

  // A connects: the note goes first, then the withdrawal, then the pass carrying the choice.
  await reconnect(a);
  const [note] = notesOf(a);
  const revoke = a.sock.sent.find((m) => m.type === "cert_revoke" && m.serial === P);
  const [pass] = passesTo(a, ANN);
  assert.ok(note && revoke && pass, "the note, the withdrawal and the new pass go");
  assert.ok(at(a, note) < at(a, revoke) && at(a, revoke) < at(a, pass), "in that order");
  assert.equal(passOf(pass).may, DEFAULT_MAY, "the pass without calls");
  await answer(a, pass);
  assert.deepEqual(a.store.choiceNotesPending, [], "the note waits no more");

  // B, opened later, reads my mailbox: it learns the choice, takes back the pass allowing calls
  // and records A's pass; it gives nothing that carries Call.
  const early = await reconnect(b, toMe(a));
  assert.deepEqual(early, [], "B sends nothing before it read the mailbox");
  assert.equal(b.store.choiceMay(ANN), DEFAULT_MAY, "B's choice is A's");
  assert.ok(revokes(b).includes(P), "B withdraws the pass allowing calls too");
  assert.deepEqual(b.store.certsSent[ANN].map((p) => p.serial), [passOf(pass).serial], "and records A's new pass");
  assert.ok(!passesTo(b, ANN).some((m) => passOf(m).may.includes("call")), "B never gives her Call back");
});

test("10n sequence: an untick whose new pass is refused, the other device online and then offline", async () => {
  const a = await loadDevice();
  const b = await loadDevice();
  const P = "99".repeat(16);
  for (const d of [a, b]) {
    befriend(d, ANN);
    chose(d, ANN, WITH_CALL);
    d.store.recordPassSent(ANN, P, WITH_CALL);
  }
  a.sock.sent.length = 0;
  assert.equal(await a.fn("setFriendTick")(ANN, "call", false), true);
  await settle();
  // B is online: the note reaches it live, and B takes back the pass allowing calls at once.
  await live(b, toMe(a));
  assert.equal(b.store.choiceMay(ANN), DEFAULT_MAY, "B hears the choice from its note");
  assert.ok(revokes(b).includes(P), "and withdraws the pass allowing calls at once");
  // A's new pass is refused: its self-copy never goes, so B never records it.
  const [refused] = passesTo(a, ANN);
  await answer(a, refused, "dm_put_refused", "rate");
  assert.equal(toMe(a).filter((m) => textOf(m) === CTL_FRIEND_CERT).length, 0, "a refused pass's self-copy never goes");
  assert.ok(!(b.store.certsSent[ANN] || []).some((p) => p.serial === passOf(refused).serial), "B does not count it as given");
  // A goes offline. B's next sweep gives her the pass A could not: the choice, without calls.
  a.sock.readyState = 3;
  await confirm(a, [b]);
  b.sock.sent.length = 0;
  await memberList(b);
  const [fromB] = passesTo(b, ANN);
  assert.ok(fromB, "my other device sends her a pass carrying the choice");
  assert.equal(passOf(fromB).may, DEFAULT_MAY, "without calls");
});

test("10n sequence: an older echo read after a newer one", async () => {
  const a = await loadDevice();
  const b = await loadDevice();
  for (const d of [a, b]) befriend(d, ANN);
  await memberList(a);
  await answer(a, passesTo(a, ANN)[0]);
  // A ticks Call (note 1, pass Q1), then unticks it (note 2, Q1 withdrawn, pass Q2): both taken.
  assert.equal(await a.fn("setFriendTick")(ANN, "call", true), true);
  await settle();
  const q1 = passesTo(a, ANN)[1];
  await answer(a, q1);
  assert.equal(await a.fn("setFriendTick")(ANN, "call", false), true);
  await settle();
  const q2 = passesTo(a, ANN)[2];
  await answer(a, q2);
  assert.ok(revokes(a).includes(passOf(q1).serial), "set up: A withdrew Q1");
  const mine = toMe(a);
  const [n1, n2] = mine.filter((m) => fp.isChoiceNoteText(textOf(m)));
  const echo = (q) => mine.find((m) => textOf(m) === CTL_FRIEND_CERT && passOf(m).serial === passOf(q).serial);
  assert.ok(n1 && n2 && echo(q1) && echo(q2), "set up: two notes and two echoes in my mailbox");

  // B reads them newest first: the newer note, the older note, the newer echo, the older echo.
  await live(b, [n2, n1, echo(q2), echo(q1)]);
  assert.equal(b.store.choiceMay(ANN), DEFAULT_MAY, "the older note does not bring Call back");
  assert.deepEqual(b.fn("friendTicks")(ANN), { message: true, call: false, trade: true }, "B's ticks are the newest choice");
  assert.ok((b.store.certsSent[ANN] || []).some((p) => p.serial === passOf(q2).serial), "the newer pass stands");
  assert.ok(!(b.store.certsSent[ANN] || []).some((p) => p.serial === passOf(q1).serial), "the older echo, allowing calls, never stands");
  assert.ok(revokes(b).includes(passOf(q1).serial), "and is withdrawn at once");
  assert.ok(!revokes(b).includes(passOf(q2).serial), "while the newer one is not withdrawn for it");
});

test("10n sequence: a tick on a marked friend gives exactly what is ticked, and my other device follows", async () => {
  const a = await loadDevice();
  const b = await loadDevice();
  const P = "ab".repeat(16);
  for (const d of [a, b]) {
    befriend(d, BEN);
    d.store.recordPassSent(BEN, P, DEFAULT_MAY);
  }
  // A withdrew Ben's pass while B could not hear why (say a note lost to an older client):
  // B is marked, and draws no tick as given.
  await b.handle({ type: "cert_revoked", to: ME, serial: P });
  await settle();
  assert.ok(b.store.passChangedOnOtherDevice(BEN), "set up: Ben is marked on B");
  assert.deepEqual(b.fn("friendTicks")(BEN), NO_TICKS, "his ticks are empty");
  b.sock.sent.length = 0;
  assert.equal(await b.fn("setFriendTick")(BEN, "message", true), true, "a tick on him is taken");
  await settle();
  assert.equal(b.store.passChangedOnOtherDevice(BEN), false, "and clears the mark");
  assert.equal(b.store.choiceMay(BEN), NO_TRADE, "the choice is exactly what is ticked: Message, not the defaults' Trade");
  const [toBen] = passesTo(b, BEN);
  assert.ok(toBen && passOf(toBen).may === NO_TRADE, "the pass carries it");
  // A hears it from B's note, and its own passes follow.
  a.sock.sent.length = 0;
  await live(a, notesOf(b));
  assert.equal(a.store.choiceMay(BEN), NO_TRADE, "my other device takes the same choice");
  assert.ok(revokes(a).includes(P), "and withdraws its pass allowing trade");
});

// ── 10o: the parity review of 10n ────────────────────────────────────────

test("10o O1: a device counts only its own mailbox pages; another device's last page starts nothing", async () => {
  const a = await loadDevice();
  const b = await loadDevice();
  for (const d of [a, b]) befriend(d, BEN);
  // My mailbox while both were away: two DMs I sent Ann from a third device (their self-copies),
  // then that device's choice for Ben.
  const row = (content) => ({ id: ++nextId, content });
  const mailbox = [
    row(selfEnvelope(ANN, "one", undefined, 1001)),
    row(selfEnvelope(ANN, "two", undefined, 1002)),
    row(choiceNote(BEN, NO_TRADE, 1003)),
  ];
  const hw = b.store.highWater;
  // Both pages of mine connect; each asks for the mailbox with its own ref.
  await reconnectStart(a);
  await reconnectStart(b);
  const fa = lastFetch(a);
  const fb = lastFetch(b);
  const unread = () => b.fn("dmMailboxUnread");
  const sentNothing = (why) => assert.deepEqual(b.sock.sent.filter((m) => m.type === "dm_put" || m.type === "cert_revoke"), [], why);

  // Device A's last page reaches B first (the relay sends every page to every device of mine).
  await b.handle({ type: "dm_batch", ref: fa.ref, messages: mailbox, done: true });
  await settle();
  assert.equal(unread(), true, "another device's last page does not count as my mailbox read");
  sentNothing("and starts no sweep");
  assert.equal(b.store.highWater, hw, "nor moves my read position");
  assert.deepEqual(b.store.conversation(ANN), [], "nothing of it is taken in");
  assert.equal(b.store.choiceOf(BEN), null, "not even my choice: my own fetch brings it");
  // A page carrying no ref is not mine either.
  await b.handle({ type: "dm_batch", messages: mailbox, done: true });
  await settle();
  assert.equal(unread(), true, "a page with no ref is not mine");
  assert.equal(b.store.highWater, hw);
  // (The wire: every fetch carries a ref of 1 to 64 [A-Za-z0-9_-], each page its own.)
  for (const f of [fa, fb]) assert.ok(typeof f.ref === "string" && /^[A-Za-z0-9_-]{1,64}$/.test(f.ref), "every fetch carries a ref");
  assert.notEqual(fa.ref, fb.ref, "each page its own");

  // A live DM during my fetch (the mailbox is longer than a page): handled, the position unmoved.
  const liveRow = row(selfEnvelope(CY, "live", undefined, 1004));
  mailbox.push(liveRow);
  await b.handle({ type: "dm_new", id: liveRow.id, content: liveRow.content });
  await settle();
  assert.equal(b.store.conversation(CY).length, 1, "a live DM during my fetch is handled as usual");
  assert.equal(b.store.highWater, hw, "but does not move my read position");

  // My own first page (two rows a page here).
  await serveFetch(b, mailbox, 2);
  assert.equal(b.store.conversation(ANN).length, 2, "my own page is taken in");
  assert.equal(b.store.highWater, mailbox[1].id, "and moves my read position to its last row");
  assert.equal(unread(), true, "not the last page: the mailbox is not read yet");
  sentNothing("so still no sweep");
  const fb2 = lastFetch(b);
  assert.notEqual(fb2, fb, "the next page is asked for");
  assert.notEqual(fb2.ref, fb.ref, "with a fresh ref");
  assert.equal(fb2.after_id, mailbox[1].id, "from the last id of my own page, not the live DM's");

  // My own last page.
  await serveFetch(b, mailbox, 2);
  assert.equal(b.store.choiceMay(BEN), NO_TRADE, "the row past the page boundary is read: my choice for Ben");
  assert.equal(b.store.conversation(CY).length, 1, "the live DM fetched again is kept once");
  assert.equal(b.store.highWater, liveRow.id, "my read position is the last row now");
  assert.equal(unread(), false, "my own last page: the mailbox is read");
  const [toBen] = passesTo(b, BEN);
  assert.ok(toBen && passOf(toBen).may === NO_TRADE, "only now the sweep runs, and gives Ben a pass carrying my choice");
});

test("10o O1: a live DM during a fetch of more than one page makes it skip no row", async () => {
  const b = await loadDevice();
  befriend(b, BEN);
  const row = (content) => ({ id: ++nextId, content });
  const mailbox = [
    row(selfEnvelope(ANN, "one", undefined, 2001)),
    row(selfEnvelope(ANN, "two", undefined, 2002)),
    row(choiceNote(BEN, NO_TRADE, 2003)),
  ];
  await reconnectStart(b);
  // A live DM arrives after my fetch went and before its first page; its row lies past the
  // choice note, which my second page holds.
  const liveRow = row(selfEnvelope(CY, "live", undefined, 2004));
  mailbox.push(liveRow);
  await b.handle({ type: "dm_new", id: liveRow.id, content: liveRow.content });
  await settle();
  await serveFetch(b, mailbox, 2);
  if (b.fn("dmMailboxUnread")) await serveFetch(b, mailbox, 2);
  assert.equal(b.store.choiceMay(BEN), NO_TRADE, "the choice note before the live DM is read, not skipped");
  assert.equal(b.store.conversation(ANN).length, 2);
  assert.equal(b.store.conversation(CY).length, 1);
});

test("10o O2: a pass sent and never answered makes no one owed an ordinary pass", async () => {
  const a = await loadDevice();
  const waits = [];
  a.ctx.setTimeout = (cb, ms) => { if (ms === 30000) waits.push(cb); return 0; };
  befriend(a, BEN);
  // A contact request to Cy, whom I do not follow, that the server never answers.
  assert.equal(await a.fn("sendContactRequest")(CY), true);
  await settle();
  const [req] = puts(a).filter((m) => m.to === CY && m.contact_request === true);
  assert.ok(req && waits.length, "set up: a contact request to Cy is on its way");
  waits[waits.length - 1]();
  await settle();
  assert.equal(a.fn("passPutInFlight")(CY), false, "set up: thirty seconds without an answer");
  assert.equal(a.store.following.has(CY), false, "set up: so I do not follow Cy");
  a.sock.sent.length = 0;
  await memberList(a);
  assert.deepEqual(passesTo(a, CY), [], "the sweep gives Cy no ordinary pass: the unanswered request makes no one owed");
  assert.ok(!a.store.passesOwed().includes(CY), "Cy is not owed one");
  assert.equal(passesTo(a, BEN).length, 1, "while a mutual follow is given one");
});

test("10o O3: the echo of my own follow clears a waiting Unfollow, and it is never sent", async () => {
  const a = await loadDevice();
  befriend(a, ANN);
  a.sock.readyState = 3;
  await a.fn("setFollowLocal")(ANN, false);
  await settle();
  assert.deepEqual(a.store.unfollowsPending.map((u) => u.peer), [ANN], "set up: the Unfollow made offline waits");
  // My other device followed her again; the echo of that follow is in my mailbox.
  await reconnectStart(a);
  const echo = selfEnvelope(ANN, CTL_FOLLOW, undefined, Date.now() + 1000);
  await a.handle({ type: "dm_batch", ref: lastFetch(a).ref, messages: [{ id: ++nextId, content: echo }], done: false });
  await settle();
  assert.ok(a.store.following.has(ANN), "I follow her again, from my other device");
  assert.deepEqual(a.store.unfollowsPending, [], "the echo of my own follow clears the waiting Unfollow");
  await serveFetch(a, []);
  assert.deepEqual(puts(a).filter((m) => textOf(m) === CTL_UNFOLLOW), [], "and no Unfollow is sent, to her or to my mailbox");
  assert.ok(a.store.following.has(ANN), "I still follow her");
});

test("10o O3: a waiting Unfollow of someone I follow again by the time it would go is dropped, not sent", async () => {
  const a = await loadDevice();
  befriend(a, ANN);
  a.sock.readyState = 3;
  await a.fn("setFollowLocal")(ANN, false);
  await settle();
  assert.deepEqual(a.store.unfollowsPending.map((u) => u.peer), [ANN], "set up: the Unfollow waits");
  // My other device sent her a contact request meanwhile: its echo makes me follow her again
  // (and does not itself clear the Unfollow; the check when it would go does).
  const pass = passJson(ANN, "c1".repeat(16), DEFAULT_MAY);
  const request = selfEnvelope(ANN, reach.contactRequestText("Me_1", pass), undefined, Date.now() + 1000);
  await reconnect(a, [request]);
  assert.ok(a.store.following.has(ANN), "set up: I follow her again");
  assert.deepEqual(puts(a).filter((m) => textOf(m) === CTL_UNFOLLOW), [], "the waiting Unfollow is not sent");
  assert.deepEqual(a.store.unfollowsPending, [], "it is dropped");
});

test("10o O3, O4: Block drops a waiting Unfollow and a waiting choice note; nothing about them goes", async () => {
  const a = await loadDevice();
  for (const k of [ANN, BEN]) befriend(a, k);
  chose(a, BEN, WITH_CALL);
  a.sock.readyState = 3;
  await a.fn("setFollowLocal")(ANN, false);
  assert.equal(await a.fn("setFriendTick")(BEN, "call", false), true);
  await settle();
  assert.deepEqual(a.store.unfollowsPending.map((u) => u.peer), [ANN], "set up: an Unfollow of Ann waits");
  assert.deepEqual(a.store.choiceNotesPending.map((n) => n.peer), [BEN], "set up: a choice note about Ben waits");
  await a.fn("blockKey")(ANN);
  await a.fn("blockKey")(BEN);
  await settle();
  assert.deepEqual(a.store.unfollowsPending, [], "O3: Block clears the waiting Unfollow");
  assert.deepEqual(a.store.choiceNotesPending, [], "O4: Block drops the waiting choice note");
  assert.equal(a.store.passChoice[BEN].may, "", "O6: the cleared choice is kept as the empty may, the smallest");
  await reconnect(a);
  assert.deepEqual(puts(a).filter((m) => m.to === ANN || m.to === BEN), [], "nothing is ever sent to them");
  assert.deepEqual(toMe(a).filter((m) => textOf(m) === CTL_UNFOLLOW), [], "no Unfollow goes to my mailbox");
  assert.deepEqual(notesOf(a), [], "nor any choice note");
  for (const k of [ANN, BEN]) assert.ok(toMe(a).some((m) => textOf(m) === "[[hum:block:v1]]" + k), "the block notes go, and say it");
});

// O5 (the web's rule, kept): every path that sends withdrawals sends a waiting choice note first.
for (const [name, trigger] of [
  ["Block", async (a) => { await a.fn("blockKey")(BEN); }],
  ["a choice note's arrival", async (a) => { await live(a, [choiceNote(BEN, DEFAULT_MAY, Date.now() + 5000)]); }],
  ["an echo of my own pass", async (a) => { await live(a, [selfEnvelope(BEN, CTL_FRIEND_CERT, passJson(BEN, "e5".repeat(16), WITH_CALL), Date.now() + 5000)]); }],
  ["cert_revoked from my other device", async (a) => { await a.handle({ type: "cert_revoked", to: ME, serial: "f6".repeat(16) }); }],
]) {
  test(`10o O5: a waiting choice note goes before any withdrawal (${name})`, async () => {
    const a = await loadDevice();
    for (const k of [ANN, BEN]) befriend(a, k);
    const P = "a7".repeat(16);
    chose(a, ANN, WITH_CALL);
    a.store.recordPassSent(ANN, P, WITH_CALL);
    // Ben: my choice the defaults, or Call when the trigger takes it away; his pass on record.
    if (name === "a choice note's arrival") chose(a, BEN, WITH_CALL);
    a.store.recordPassSent(BEN, "f6".repeat(16), name === "a choice note's arrival" ? WITH_CALL : DEFAULT_MAY);
    // Offline: Call unticked for Ann. Its note and the withdrawal of her pass wait.
    a.sock.readyState = 3;
    assert.equal(await a.fn("setFriendTick")(ANN, "call", false), true);
    await settle();
    assert.equal(a.store.choiceNotesPending.length, 1, "set up: the note waits");
    // Connected, the mailbox not read yet (so the sweep has not run): the trigger.
    await reconnectStart(a);
    assert.deepEqual(a.sock.sent.filter((m) => m.type === "dm_put" || m.type === "cert_revoke"), [], "set up: nothing went yet");
    await trigger(a);
    await settle();
    const [note] = notesOf(a);
    const revoke = a.sock.sent.find((m) => m.type === "cert_revoke");
    assert.ok(note && revoke, `${name}: the note and a withdrawal go`);
    assert.ok(at(a, note) < at(a, revoke), `${name}: the waiting choice note goes before any withdrawal`);
  });
}

test("10o O7: a choice note about someone I blocked is not recorded as seen; after Unblock it can apply", async () => {
  const b = await loadDevice();
  befriend(b, CY);
  await b.fn("blockKey")(CY);
  await settle();
  const note = choiceNote(CY, NO_TRADE, Date.now() + 1000);
  await live(b, [note]);
  assert.equal(b.store.choiceOf(CY), null, "set up: ignored while Cy is blocked");
  await b.fn("unblockKey")(CY);
  await settle();
  await live(b, [note]);
  assert.equal(b.store.choiceMay(CY), NO_TRADE, "the same note delivered again after Unblock applies");
});

test("10o O7: the echo of my own contact request does not bring the follow back while its pass is being withdrawn", async () => {
  const a = await loadDevice();
  assert.equal(await a.fn("sendContactRequest")(ANN), true);
  await settle();
  const [req] = puts(a).filter((m) => m.to === ANN && m.contact_request === true);
  await answer(a, req);
  const echo = toMe(a).find((m) => reach.isContactRequestText(textOf(m)));
  assert.ok(echo && a.store.following.has(ANN), "set up: the request was taken, I follow Ann, and its self-copy went");
  await a.fn("setFollowLocal")(ANN, false);
  await settle();
  const serial = fp.friendPassParse(reach.contactRequestParse(textOf(echo)).pass).serial;
  assert.ok(a.store.withdrawalsPending.includes(serial), "set up: its pass is being withdrawn");
  // The self-copy comes back to this page (late, or fetched again) before the relay confirms.
  await live(a, [echo]);
  assert.equal(a.store.following.has(ANN), false, "the echo does not follow her again");
  assert.equal(a.fn("isFollowing")(ANN), false, "not in the member list's marks either");
});

// Seen red, 2026-10-10 (10o). Against web/ as at d283cffc5 (`git archive d283cffc5 web`, through
// HOS_WEB_DIR), each of these failed on the assertion named:
//  O1 sequence: "another device's last page does not count as my mailbox read" (any last page
//    cleared it and started the sweep). O1 skip: "the choice note before the live DM is read, not
//    skipped" (the live DM moved the read position, and the next page was asked for from it).
//  O2: "the sweep gives Cy no ordinary pass: the unanswered request makes no one owed".
//  O3 echo: "the echo of my own follow clears the waiting Unfollow". O3 flush: "the waiting
//    Unfollow is not sent". O3/O4 Block: "O3: Block clears the waiting Unfollow".
//  O7 note: "the same note delivered again after Unblock applies". O7 request: "the echo does not
//    follow her again".
// O4, O5 and O6 were already the web's rules, so they pass there; each was broken in a copy of the
// finished web/ instead: chat-dm-store.js clearChoice keeping choiceNotesPending: "O4: Block
// drops the waiting choice note"; sendPendingWithdrawals sending the withdrawals and then the
// waiting notes: all four O5 cases, "the waiting choice note goes before any withdrawal"; the
// clear storing the defaults' text: "O6: the cleared choice is kept as the empty may, the
// smallest". And the finished code broken one rule at a time: the ref check taken out of
// handleDmBatch: "another device's last page does not count as my mailbox read"; dm_new moving
// the read position during a fetch: "but does not move my read position"; that and paging from
// the read position again: the skip test, "the choice note before the live DM is read, not
// skipped"; sendQueuedSelfNotes without its check: "the waiting Unfollow is not sent";
// blockLocally without dropQueuedUnfollow (the check kept): "O3: Block clears the waiting
// Unfollow"; the follow echo without it: "the echo of my own follow clears the waiting Unfollow".

// Red first, 2026-10-10. Against web/ as at 91ac73d78 (before 10n) through HOS_WEB_DIR every test
// above fails: N9 on its assertion "Unfollow drops the request waiting for its answer", the rest
// on the 10n names that do not exist there (choiceNoteText, applyChoiceNote, choiceMay). So each
// rule was also broken one at a time in a copy of the finished web/ (HOS_WEB_DIR), and seen
// failing on the assertion named:
//  N1: makeChoice queueing no note: "a choice note goes to my own mailbox" and, in the offline
//    sequence, "its note waits"; the note flushed after the withdrawal and the pass (online):
//    "the note first, then the withdrawal, then the pass"; the queued note sent only at the end
//    of the sweep, with sendPendingWithdrawals not sending it first: "in that order".
//  N2: no first-sight check in ingestChoiceNote: "the same note delivered again is not applied
//    again"; every note winning: "an older one changes nothing" and, in the older-echo sequence,
//    "the older note does not bring Call back"; the smaller text winning a tie: "at an equal time
//    the larger may text wins"; the blocked check taken out: "a note about someone I blocked is
//    ignored"; choiceNoteFromSelf checking only `to`: "a choice note from anyone else is dropped
//    unread".
//  N3: an offline Unfollow not queued: "the Unfollow waits"; my own Unfollow's echo clearing the
//    choice as of an old time: "and clears my choice for her".
//  N4: a received choice not acting on the passes: "every pass allowing calls is withdrawn at
//    once, standing or on its way"; a pass on its way left in place: "the one on its way no
//    longer holds the way"; passOwed counting any standing pass: "a friend whose pass does not
//    carry the choice is owed one that does"; passTaken never replacing: "which goes once the
//    new one is taken"; an echo beyond the choice recorded as standing: "the older echo,
//    allowing calls, never stands" (and reach-web.test.js's N4 test, "an echo beyond my choice is
//    withdrawn at once, and nothing else").
//  N6: applyChoiceNote leaving the mark: "my other device's choice note clears the mark";
//    passFriends listing a marked one-way follow by their choice: "a marked person who is not a
//    mutual follow is not listed".
//  N7: the mailbox gate taken out of sweepFriendPasses: "nothing is sent before the mailbox was
//    read: no note, no withdrawal, no pass".
//  N9: withdrawPassesTo keeping a contact request's put: "Unfollow drops the request waiting for
//    its answer".
//  Sequences: a pass's self-copy sent at once again (10m R2's way): "a refused pass's self-copy
//    never goes"; ingestChoiceNote applying nothing: "my other device takes the same choice".
