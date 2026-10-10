// Warnings on messages and the recovery-phrase guard in the web chat client (step F, 2026-10-10,
// docs/design/blocking-and-safe-mode.md 6.3 and 10g), mirroring the desktop app.
//
// Run: node --test scripts/tests/warnings-web.test.js   (in `just rig-tests`)
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with the
// DOM replaced by a stub that keeps what is written to it, as in block-web.test.js: the real
// bip39-english.js, friend-pass.js, reach.js, block.js, warnings.js, crypto.js, chat-dm-store.js
// (over a stand-in IndexedDB), app.js, chat-messages.js, chat-dms.js, chat-social.js,
// chat-groups-p2p.js, chat-ui.js, chat-voice-rooms.js, chat-voice-calls.js, chat-profile.js,
// chat-privacy.js, chat-warnings.js and chat-p2p.js, in index.html's order. The stub's elements
// also answer querySelector with a kept element per selector (".msg-main", ".body"), so what is
// put under a message can be read back. Only these are stand-ins: the Dilithium and Kyber
// primitives ("signing" returns the signed words, "sealing" base64s the plaintext and names the
// key), and the two modules chat-groups-p2p.js imports (/shared/pq-object.js and the noble
// bundle), whose stand-ins pass a group message's words through unchanged; the test rewrites that
// file's `import(` to reach them, because node:vm cannot import without a flag. The real
// primitives are covered by scripts/pq-kat.mjs and the relay's tests. HOS_WEB_DIR points the test
// at another copy of web/ (used to see each test red). The warnings file and the shared cases are
// always the shipped ones, data/safety/warnings.json and scripts/tests/fixtures/warning-cases.json.
//
// What it proves (10g's Proof list, the web items, and the rest of 10g's web surface):
//  1. The shared cases: this matcher, over every case in the fixture and against the shipped
//     warnings file, shows exactly each case's `expect` (the desktop app's Rust test runs the same
//     file, so the two cannot drift).
//  2. The matcher on 10g's examples ("I'm an admin!!", "admin" never inside "administer"),
//     Unicode text (Cyrillic, a curly apostrophe, an accent that is a near miss), punctuation
//     inside a phrase ("can't lose"), near misses, the file's order, and the phrase-run rule.
//  3. Friends and strangers: a mutual follow is a friend; following them only, or them following
//     me only, is a stranger; each entry shows only for the audiences in its `applies_to`.
//  4. Under a received direct message: each matching entry once, in the file's order, with its
//     title, explanation, advice and "Got it", which hides that one for that message (also when
//     the conversation is drawn again); never on my own messages; not on public channel posts.
//  5. Under a P2P group message, from the group's history and from a live one, by the same rule.
//  6. The link line: a stranger's direct message with a link is drawn with its links held and the
//     line "<name> is not your friend. Links open only when you choose."; Open lets them open;
//     a friend's links, my own and a message without a link get no line.
//  7. The switch: Settings > Safety, "Warnings on messages", On by default; Off hides every
//     warning (those on screen and those drawn after) and is kept, encrypted, in the local store
//     with the block list; On shows them again.
//  8. The guard: my phrase is derived in the page from the identity (the same words the backup
//     screen shows); a run of 4 of its words in order stops a post, a reply, a DM, a group
//     message, a thread reply, an edit, a contact request's name and a profile field, with 10g's
//     sentence, nothing sent and the text kept; 3 words, or 4 out of order, go; the whole phrase
//     is stopped; a locked identity sends as before.
//  9. Every other send path awaits the guard before it sends, and index.html loads both scripts.
// 10. Messages drawn before the warnings file arrived get their warnings when it does, and a file
//     that cannot be read is not asked for again in a loop.
//
// Red first, 2026-10-10: each mutation made in a copy of web/, this test run against it with
// HOS_WEB_DIR, and seen failing with the assertion named (all the others still passing unless
// said):
//  1: warnings.js warningNormalize without `.toLowerCase()`: "shared case 3 ("I'm an admin. ..."
//     from a stranger) shows exactly its expect" (six tests failed with it, 2 to 5 and 8 too).
//  2: warnings.js NOT_LETTER_OR_DIGIT as /[^a-z0-9]+/g: "Cyrillic letters are letters" (and,
//     on the 17-case fixture, test 1's shared case 17);
//     containsWords without the surrounding spaces: "admin never matches inside administer".
//     And (same day, after 10g was corrected) NOT_LETTER_OR_DIGIT as the first version's
//     /[^\p{L}\p{N}]+/gu, which splits a word at a combining mark Rust keeps in it: "shared case 17
//     ("gift<U+0902> card" from a stranger) shows exactly its expect" in test 1 (seen on the
//     merged fixture before the class was changed), and "an Alphabetic combining mark is part of
//     the word, as in Rust" in test 2.
//  3: warningSenderIsFriend using `store.following.has(key)` (one-way): "following them only is
//     not a friend".
//  4: drawMessageWarnings without the warningsDismissed check: "Got it holds when the
//     conversation is drawn again"; addMessageSafetyLines without its myKey check AND
//     addDmMessage's `received` without `!isMe` (either one alone still keeps my own messages
//     clear, so both were taken out): "never on my own messages".
//  5: the groupMessageWarnings call taken out of _renderP2pMessages: "a group's history shows its
//     warnings (a stranger)"; out of handleP2pGroupObj: "and so does a live group message".
//  6: addDmMessage drawing `bodyHtml` instead of the held copy: "the link is drawn held"; the
//     Open handler not putting the body back: "Open lets the link open".
//  7: chat-dm-store.js _persistMeta without `warningsOn`: "the switch is kept in the store";
//     setMessageWarningsOn without applyWarningsSwitchToView(): "Off hides the warnings on screen".
//  8: the guard call taken out of chat-ui.js sendComposedContent: "a DM with 4 words of the
//     phrase is not sent" (and test 9's "sendComposedContent awaits the guard before it sends");
//     PHRASE_RUN_MIN set to 3: "3 words go" (and test 2's 3 == 4); recoveryPhraseIn answering
//     true when there is no phrase: "a locked identity sends".
//  9: the guard call taken out of chat-voice-rooms.js createVoiceRoom: "chat/chat-voice-rooms.js:
//     async function createVoiceRoom awaits the guard before it sends".
// 10: loadMessageWarnings without the waiting list's replay: "drawn before the file arrived, the
//     warning comes when it does"; without the MESSAGE_WARNINGS_RETRY_MS wait: "a failed file is
//     not asked for again at once".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const nodeCrypto = require("node:crypto");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const W = require(path.join(WEB, "shared", "warnings.js"));
const SHIPPED_WARNINGS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "warnings.json"), "utf8"));
const CASES = JSON.parse(fs.readFileSync(path.join(__dirname, "fixtures", "warning-cases.json"), "utf8"));

// ── The stub page ────────────────────────────────────────────────────────

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

const escapeText = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
// Selectors nothing matches until the page makes one (startEditMode asks whether it is editing).
const NOTHING_YET = new Set([".edit-area", ".edited-marker", ".block-indicator"]);

// A document whose elements keep what is written to them: text, HTML, children, handlers, style,
// a class list, and one kept element per selector they are asked for.
function fakeDom(state) {
  const all = [];
  function el(tag) {
    const classes = new Set();
    const own = {
      tag, children: [], parts: new Map(), style: {}, dataset: {}, attrs: {},
      className: "", textContent: "", value: "", disabled: false, checked: false, removed: false,
    };
    own.classList = {
      add: (c) => classes.add(c), remove: (c) => classes.delete(c), contains: (c) => classes.has(c),
      toggle: (c) => (classes.has(c) ? (classes.delete(c), false) : (classes.add(c), true)),
    };
    own.appendChild = (c) => { own.children.push(c); return c; };
    own.setAttribute = (k, v) => { own.attrs[k] = String(v); };
    own.getAttribute = (k) => (k in own.attrs ? own.attrs[k] : null);
    own.querySelector = (sel) => {
      if (NOTHING_YET.has(sel)) return null;
      if (!own.parts.has(sel)) own.parts.set(sel, el(sel));
      return own.parts.get(sel);
    };
    own.querySelectorAll = () => [];
    own.addEventListener = () => {};
    own.remove = () => { own.removed = true; };
    own.focus = () => {};
    const p = new Proxy(own, {
      get(t, prop) {
        if (prop === "then") return undefined;
        if (prop === "innerHTML") return "innerHTML" in t ? t.innerHTML : escapeText(t.textContent);
        if (prop in t) return t[prop];
        return anything();
      },
      set(t, prop, v) { t[prop] = v; return true; },
    });
    all.push(p);
    return p;
  }
  const byId = new Map();
  const kept = (id) => { if (!byId.has(id)) byId.set(id, el("#" + id)); return byId.get(id); };
  const document = new Proxy({}, {
    get(_t, prop) {
      if (prop === "createElement") return el;
      if (prop === "getElementById") return kept;
      if (prop === "body") return kept("__body");
      if (prop === "querySelector") return () => anything();
      if (prop === "querySelectorAll") {
        return (sel) => {
          if (sel === ".msg-warning") return all.filter((e) => String(e.className).split(/\s+/).includes("msg-warning"));
          if (sel === ".message[data-from]") return state.appended.filter((e) => e && e.dataset && e.dataset.from);
          return [];
        };
      }
      if (prop === "hidden") return false;
      return anything();
    },
    set: () => true,
  });
  return document;
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

// Just enough IndexedDB for chat-dm-store.js, shared across loads so a second load of the same
// identity finds what the first kept (requests answer on the next turn).
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
  return { open: () => req(db), stores };
}

function fakeSocket() {
  return { readyState: 1, sent: [], send(s) { this.sent.push(JSON.parse(s)); }, close() {} };
}

function FakePeerConnection() {
  this.setRemoteDescription = async () => {};
  this.setLocalDescription = async () => {};
  this.createAnswer = async () => ({ type: "answer", sdp: "v=0" });
  this.createOffer = async () => ({ type: "offer", sdp: "v=0" });
  this.createDataChannel = () => ({ readyState: "connecting", send() {} });
  this.addTrack = () => {};
  this.addIceCandidate = async () => {};
  this.close = () => {};
}

const ME = "a1".repeat(32);
const ANN = "b2".repeat(32);
const BEN = "c3".repeat(32);
const CY = "d4".repeat(32);
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const MY_KYBER = "my-kyber";
const kyberOf = (k) => "kyber-" + k.slice(0, 4);
const GROUP = "g1";
// The stand-in BLAKE3 keeps the first 32 bytes, so a key's group fingerprint is its first 32 hex digits.
const fpOf = (k) => k.slice(0, 32);

// The identity's seed, and its phrase worked out here the BIP39 way (independently of crypto.js).
const SEED = Uint8Array.from({ length: 32 }, (_, i) => (i * 7 + 3) & 0xff);

// The page's real asynchronous work is WebCrypto and the stand-in IndexedDB; counting the WebCrypto
// calls in flight lets settle() wait until the page is idle (as block-web.test.js does).
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
const bytesToString = (u) => { let s = ""; for (let i = 0; i < u.length; i++) s += String.fromCharCode(u[i]); return Buffer.from(s, "latin1").toString("utf8"); };
const ok = (json) => Promise.resolve({ ok: true, status: 200, json: async () => json, text: async () => JSON.stringify(json) });

// The two modules chat-groups-p2p.js imports. A group message's payload here is JSON {epoch, text}
// (base64 in the log), opened as it is; building one keeps the words, so a test can read them.
function fakeGroupModules() {
  let built = 0;
  const obj = {
    parseGroupMsgPayload: (payload) => {
      const j = payload && typeof payload === "object" && !ArrayBuffer.isView(payload) && "epoch" in payload ? payload : JSON.parse(bytesToString(payload));
      return { epoch: j.epoch, nonce: "n", ct: j.text };
    },
    aesGcmDecrypt: async (_key, _nonce, ct) => ct,
    openGroupEpochKey: async () => ({ epoch: 1, epochKey: "K" }),
    verifyObjectSubmission: async (sub) => ({ ok: true, objectId: sub.id, authorPubHex: sub.author, payload: sub.payload, createdAt: sub.created_at }),
    buildGroupMsgV1: async ({ plaintext }) => ({ objectId: "built-" + (++built), submission: { object_type: "group_msg_v1", plaintext } }),
    groupSharesHistory: () => false,
  };
  const noble = {
    blake3: {
      create: () => {
        let d = new Uint8Array(0);
        return { update(x) { d = x; return this; }, digest() { const o = new Uint8Array(32); o.set(d.slice(0, 32)); return o; } };
      },
    },
  };
  return { "/shared/pq-object.js": obj, "/shared/vendor/noble-pq.bundle.js": noble };
}

async function loadChat(opts = {}) {
  const state = { appended: [] };
  const fetches = [];
  const posted = [];
  const groupLog = [];
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  const idb = opts.indexedDB || fakeIndexedDB();
  const warningsFetch = opts.warningsFetch || (() => ok(SHIPPED_WARNINGS));
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDom(state),
    navigator: new Proxy({}, { get: (_t, p) => (p === "serviceWorker" ? serviceWorker : anything()), has: () => true }),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", origin: "https://localhost", reload() {} },
    localStorage: opts.localStorage || fakeStorage(),
    sessionStorage: fakeStorage(),
    indexedDB: idb,
    fetch: (url, init) => {
      const u = String(url);
      fetches.push(u);
      if (u.startsWith("/api/federation/servers")) return ok([]);
      if (u === W.WARNINGS_URL) return warningsFetch();
      if (u === "/api/v2/objects" && init && init.method === "POST") { posted.push(JSON.parse(init.body)); return ok({ ok: true }); }
      if (u === `/api/v2/groups/${GROUP}/members`) return ok({ members: [{ pubkey: ME }, { pubkey: ANN }, { pubkey: CY }] });
      if (u === `/api/v2/groups/${GROUP}/epochs`) return ok({ epochs: [{ payload_b64: "AA==" }] });
      if (u === `/api/v2/groups/${GROUP}/messages`) return ok({ messages: groupLog });
      return Promise.reject(new Error("no network in tests"));
    },
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
  const modules = fakeGroupModules();
  ctx.__testImport = async (spec) => {
    if (!modules[spec]) throw new Error("no module " + spec);
    return modules[spec];
  };
  vm.createContext(ctx);
  const run = (rel, rewrite) => {
    let src = fs.readFileSync(path.join(WEB, rel), "utf8");
    if (rewrite) src = rewrite(src);
    vm.runInContext(src, ctx, { filename: rel });
  };
  // In index.html's order.
  run("shared/events.js");
  run("chat/bip39-english.js");
  run("shared/friend-pass.js");
  run("shared/reach.js");
  run("shared/block.js");
  run("shared/warnings.js");
  run("chat/crypto.js");
  run("chat/chat-dm-store.js");
  run("chat/view/timestampPill.js");
  run("chat/view/messageRow.js");
  run("chat/app.js");
  run("chat/chat-messages.js");
  run("chat/chat-dms.js");
  run("chat/chat-social.js");
  run("chat/chat-groups-p2p.js", (s) => s.replace(/\bimport\(/g, "__testImport("));
  run("chat/chat-ui.js");
  run("chat/chat-voice-rooms.js");
  run("chat/chat-voice-calls.js");
  run("chat/chat-profile.js");
  run("chat/chat-privacy.js");
  run("chat/chat-warnings.js");
  run("chat/chat-p2p.js");
  for (const name of ["hosIcon", "holdToConfirm", "holdConfirm", "updateStats"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  const notices = [];
  ctx.appendMessage = (el) => { state.appended.push(el); };
  ctx.notifyNewMessage = () => {};
  ctx.playNotificationChime = () => {};
  ctx.sendSWNotification = () => {};
  ctx.addNotice = (text) => { notices.push(String(text)); };
  const key = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => opts.storeKey || key;
  ctx.pqSignMessage = async (_secret, bytes) => new Uint8Array(bytes);
  ctx.pqVerifyMessage = async (_pk, bytes, sig) => Buffer.from(bytes).equals(Buffer.from(sig));
  ctx.pqDmSeal = async (pub, plaintext) => ({ ek_ct_b64: b64(pub), nonce_b64: "AAAA", ct_b64: b64(plaintext) });
  ctx.pqDmOpen = async (_secret, ek, _nonce, ct) => (unb64(ek) === MY_KYBER ? unb64(ct) : null);
  const sock = fakeSocket();
  vm.runInContext(`(s, me, seed) => {
    ws = s; myKey = me; myName = 'Me_1'; activeChannel = 'general'; identityConfirmed = true;
    myDilithiumPublicHex = me; myDilithiumSecret = new Uint8Array(4);
    myKyberPublicBase64 = '${MY_KYBER}'; myKyberSecret = new Uint8Array(4);
    myIdentity = { publicKeyHex: me, seed32: seed, canSign: true };
  }`, ctx)(sock, ME, new Uint8Array(SEED));
  const handle = async (msg) => { await vm.runInContext("handleMessage", ctx)(msg); await settle(); };
  await handle({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  const store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await store.init(ME, "localhost"), "the store loads");
  store.setPassServer(SERVER);
  const users = [[ME, "Me_1"], [ANN, "Ann"], [BEN, "Ben"], [CY, "Cy"]].map(([k, name]) => ({ public_key: k, name, role: "", kyber_public: kyberOf(k), online: true }));
  await handle({ type: "full_user_list", users });
  // "Anyone" may message me, so every DM in these tests is drawn (the reach rules are reach-web's).
  await handle({ type: "reach_settings", settings: { message: "anyone", call: "chosen", trade: "friends" } });
  await settle();
  sock.sent.length = 0;
  state.appended.length = 0;
  const fn = (name) => vm.runInContext(name, ctx);
  const el = (id) => ctx.document.getElementById(id);
  const set = (code, ...args) => vm.runInContext(code, ctx)(...args);
  return { ctx, sock, store, state, appended: state.appended, fetches, posted, groupLog, notices, handle, fn, el, set, idb };
}

// ── Reading back what was drawn ──────────────────────────────────────────

function partsOf(e) { return e && e.parts ? [...e.parts.values()] : []; }
function kidsOf(e) { return e && Array.isArray(e.children) ? e.children : []; }
function hidden(e) { return !!(e && (e.removed || (e.style && e.style.display === "none"))); }

// Everything readable in an element and below it; `visible` leaves out what is hidden or removed.
function textOf(e, visible) {
  if (!e || (visible && hidden(e))) return "";
  const own = [e.textContent, typeof e.innerHTML === "string" ? e.innerHTML : ""].filter((s) => typeof s === "string").join(" ");
  return [own, ...kidsOf(e).map((c) => textOf(c, visible)), ...partsOf(e).map((c) => textOf(c, visible))].join(" ");
}

// Every element below `e` (children and kept parts) whose class list holds `cls`.
function findAll(e, cls) {
  const out = [];
  const walk = (x) => {
    if (!x || typeof x !== "object" && typeof x !== "function") return;
    if (String(x.className || "").split(/\s+/).includes(cls)) out.push(x);
    kidsOf(x).forEach(walk);
    partsOf(x).forEach(walk);
  };
  walk(e);
  return out;
}
const warningsUnder = (row) => findAll(row, "msg-warning");
const shownWarningIds = (row) => warningsUnder(row).filter((w) => !hidden(w)).map((w) => w.dataset.warningId);

// A DM from `from` to `to`, signed with the stand-in signer and sealed to my DM key.
function envelope(from, to, text, ts) {
  const sig = b64(`hum/dm/v2\n${from}\n${to}\n${ts}\n${text}`);
  const inner = JSON.stringify({ v: 2, from, to, ts, text, sig });
  return JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(inner) });
}

// Receive a DM from `from` into the open conversation with them, and return its drawn row.
let dmId = 100;
async function receiveDm(chat, from, text, ts) {
  chat.set("(k, n) => { activeDmPartner = k; activeDmPartnerName = n; }", from, from === ANN ? "Ann" : from === BEN ? "Ben" : "Cy");
  const before = chat.appended.length;
  await chat.handle({ type: "dm_new", id: ++dmId, content: envelope(from, ME, text, ts) });
  const rows = chat.appended.slice(before).filter((e) => e && e.dataset && e.dataset.from === from);
  assert.equal(rows.length, 1, `the DM from ${from.slice(0, 4)} is drawn`);
  return rows[0];
}

function makeFriends(chat, key) {
  chat.store.setFollowing(key, true);
  chat.store.setFollower(key, true);
}

// The phrase, the BIP39 way: 256 bits of seed and 8 of SHA-256 checksum, 24 words of 11 bits.
function phraseOf(seed, list) {
  const cs = nodeCrypto.createHash("sha256").update(seed).digest()[0];
  const bits = [];
  for (const b of seed) for (let i = 7; i >= 0; i--) bits.push((b >> i) & 1);
  for (let i = 7; i >= 0; i--) bits.push((cs >> i) & 1);
  const out = [];
  for (let i = 0; i < 24; i++) {
    let x = 0;
    for (let j = 0; j < 11; j++) x = (x << 1) | bits[i * 11 + j];
    out.push(list[x]);
  }
  return out;
}

// ── 1 and 2: the matcher ─────────────────────────────────────────────────

test("the shared cases: exactly each case's expect, against the shipped warnings file", () => {
  const list = W.warningsFrom(SHIPPED_WARNINGS);
  assert.deepEqual(list.map((w) => w.id), SHIPPED_WARNINGS.warnings.map((w) => w.id), "every entry of the shipped file is read, in its order");
  assert.ok(CASES.cases.length >= 17, "the shared cases are there");
  CASES.cases.forEach((c, i) => {
    const got = W.warningMatches(c.text, c.from, list).map((w) => w.id);
    assert.deepEqual(got, c.expect, `shared case ${i + 1} (${JSON.stringify(c.text)} from a ${c.from}) shows exactly its expect`);
  });
});

test("the matcher: 10g's examples, Unicode, punctuation inside a phrase, near misses, the file's order", () => {
  const list = W.warningsFrom(SHIPPED_WARNINGS);
  const ids = (text, who = "strangers") => W.warningMatches(text, who, list).map((w) => w.id);
  // 10g's own examples.
  assert.equal(W.warningNormalize("I'm an admin!!"), "i m an admin");
  assert.equal(W.warningNormalize("i'm an admin"), "i m an admin");
  assert.deepEqual(ids("I'm an admin!!"), ["staff_claim"]);
  const one = (phrase) => W.warningsFrom({ warnings: [{ id: "t", applies_to: ["strangers"], title: "T", explain: "E", advice: "A", any: [phrase] }] });
  assert.deepEqual(W.warningMatches("I administer the garden group.", "strangers", one("admin")), [], "admin never matches inside administer");
  assert.equal(W.warningMatches("Ask an admin.", "strangers", one("admin")).length, 1, "but matches as a word");
  // Unicode text: letters of any script are letters, lower-cased the standard way.
  assert.equal(W.warningNormalize("ОТПРАВЬ Gift-Card!"), "отправь gift card", "Cyrillic letters are letters");
  assert.deepEqual(ids("Отправь мне gift-card, пожалуйста"), ["money"]);
  assert.equal(W.warningMatches("ПОДАРОЧНАЯ КАРТА!!!", "strangers", one("подарочная карта")).length, 1, "a phrase in Cyrillic, upper case in the text");
  assert.deepEqual(ids("I’m an admin"), ["staff_claim"], "a curly apostrophe is punctuation too");
  assert.deepEqual(ids("Ím an admin"), [], "near miss: an accented letter is a different letter");
  assert.equal(W.warningNormalize("½ price, ²nd chance"), "½ price ²nd chance", "number characters are kept");
  // A combining mark that is Alphabetic (a Devanagari vowel sign) stays in its word, as Rust's
  // char::is_alphanumeric keeps it: "giftं" is one word, not "gift" and a separator.
  assert.equal(W.warningNormalize("Giftं card"), "giftं card", "an Alphabetic combining mark is part of the word, as in Rust");
  assert.deepEqual(ids("giftं card"), [], "so it is not the phrase gift card");
  assert.equal(W.warningNormalize("बीमा!"), "बीमा", "a Hindi word with its vowel signs stays whole");
  // Punctuation inside a phrase.
  assert.deepEqual(ids("You CAN'T LOSE."), ["money"], "can't lose");
  assert.deepEqual(ids("you can t lose"), ["money"], "the same words, spaced");
  // Near misses.
  assert.deepEqual(ids("you cant lose"), [], "near miss: cant");
  assert.deepEqual(ids("a giftcard"), [], "near miss: giftcard is one word");
  assert.deepEqual(ids("a gift for you"), [], "near miss: gift alone");
  assert.deepEqual(ids(""), []);
  assert.deepEqual(ids("   !!!   "), []);
  // Each entry once, in the file's order, whatever the text's order.
  assert.deepEqual(ids("Don't tell anyone. Buy a gift card. I'm an admin."), ["money", "staff_claim", "urgency_secrecy"]);
  // A phrase that is all punctuation would match everything: it is left out.
  assert.deepEqual(one("?!"), [], "an entry whose phrases are all punctuation is left out");
  assert.equal(W.warningAudience("stranger"), "strangers");
  assert.equal(W.warningAudience("friend"), "friends");
  assert.equal(W.warningAudience("everyone"), null);

  // The phrase-run rule (the guard's).
  const words = ["adapt", "explain", "ecology", "dinner", "glare", "old"];
  assert.equal(W.PHRASE_RUN_MIN, 4);
  assert.ok(W.phraseRunFound("glare, OLD; ecology-dinner? no: ecology dinner glare old", words), "4 in a row, in order, case and punctuation aside");
  assert.ok(!W.phraseRunFound("ecology dinner glare", words), "3 in a row are not enough");
  assert.ok(!W.phraseRunFound("dinner ecology glare old", words), "4, out of order");
  assert.ok(!W.phraseRunFound("ecology dinner glares old", words), "a word inside another word is not the word");
  assert.ok(W.phraseRunFound(words.join(" "), words), "the whole phrase");
  assert.ok(W.phraseRunFound("x " + words.join(" ") + " y", words.join(" ")), "the phrase as one string");
  assert.ok(!W.phraseRunFound("anything", ["a", "b"]), "a phrase shorter than the run");
});

// ── 3 to 5: warnings under messages ─────────────────────────────────────

test("friends and strangers: a mutual follow is a friend, and each entry shows only for its audiences", async () => {
  const chat = await loadChat();
  makeFriends(chat, ANN);
  chat.store.setFollowing(BEN, true); // I follow Ben; he does not follow me
  chat.store.setFollower(CY, true);   // Cy follows me; I do not follow Cy
  const text = "I'm an admin. Send me a gift card.";
  const ann = await receiveDm(chat, ANN, text, 1000);
  const ben = await receiveDm(chat, BEN, text, 1001);
  const cy = await receiveDm(chat, CY, text, 1002);
  assert.deepEqual(shownWarningIds(ann), ["money"], "a friend: money applies to friends, staff_claim does not");
  assert.deepEqual(shownWarningIds(ben), ["money", "staff_claim"], "following them only is not a friend");
  assert.deepEqual(shownWarningIds(cy), ["money", "staff_claim"], "them following me only is not a friend");
  const telegram = await receiveDm(chat, ANN, "Let's talk on Telegram", 1003);
  assert.deepEqual(shownWarningIds(telegram), [], "a stranger-only entry never shows on a friend's message");
});

test("under a received DM: each match once, in order, with Got it; never mine; not public posts", async () => {
  const chat = await loadChat();
  const row = await receiveDm(chat, BEN, "Your account will be suspended unless you act now. Buy a gift card. I'm an admin.", 2000);
  const boxes = warningsUnder(row);
  assert.deepEqual(boxes.map((b) => b.dataset.warningId), ["money", "staff_claim", "urgency_secrecy"], "each matching entry once, in the file's order");
  const money = SHIPPED_WARNINGS.warnings.find((w) => w.id === "money");
  const box = boxes[0];
  const shown = textOf(box, true);
  for (const words of [money.title, money.explain, money.advice, "Got it"]) {
    assert.ok(shown.includes(escapeText(words)) || shown.includes(words), `the warning shows ${JSON.stringify(words.slice(0, 30))}`);
  }
  const gotIt = findAll(box, "msg-warning-ok")[0];
  assert.ok(gotIt && gotIt.textContent === "Got it" && typeof gotIt.onclick === "function", "with a Got it button");
  // The warnings sit in the message's content column, under its text.
  assert.ok(findAll(row.querySelector(".msg-main"), "msg-warning").length === 3, "in the content column");

  gotIt.onclick({ stopPropagation() {} });
  assert.deepEqual(shownWarningIds(row), ["staff_claim", "urgency_secrecy"], "Got it hides that one");
  // The conversation drawn again: still hidden for that message, and only that one.
  chat.appended.length = 0;
  chat.fn("renderDmConversationFromStore")(BEN);
  const again = chat.appended.filter((e) => e && e.dataset && e.dataset.from === BEN);
  assert.equal(again.length, 1);
  assert.deepEqual(shownWarningIds(again[0]), ["staff_claim", "urgency_secrecy"], "Got it holds when the conversation is drawn again");
  const other = await receiveDm(chat, BEN, "Another gift card, please.", 2001);
  assert.deepEqual(shownWarningIds(other), ["money"], "and another message still shows its own");

  // My own message, from my other device: never.
  chat.set("(k) => { activeDmPartner = k; }", BEN);
  const before = chat.appended.length;
  await chat.handle({ type: "dm_new", id: ++dmId, content: envelope(ME, BEN, "I'm an admin, buy a gift card", 2002) });
  const mine = chat.appended.slice(before).filter((e) => e && e.dataset && e.dataset.from === ME);
  assert.equal(mine.length, 1, "my own message is drawn");
  assert.deepEqual(warningsUnder(mine[0]), [], "never on my own messages");
  // A public channel post: not in this step.
  chat.set("() => { activeDmPartner = null; activeDmPartnerName = ''; }");
  const mark = chat.appended.length;
  await chat.handle({ type: "chat", from: CY, from_name: "Cy", content: "Buy a gift card, I'm an admin", timestamp: 2003, channel: "general" });
  const post = chat.appended.slice(mark).filter((e) => e && e.dataset && e.dataset.from === CY);
  assert.equal(post.length, 1, "the post is drawn");
  assert.deepEqual(warningsUnder(post[0]), [], "not on public channel posts");
});

test("under a P2P group message: from the group's history and live, by the same rule", async () => {
  const chat = await loadChat();
  makeFriends(chat, ANN);
  chat.groupLog.push(
    { author_fp: fpOf(CY), created_at: 3000, payload_b64: b64(JSON.stringify({ epoch: 1, text: "Let's talk on Telegram. I'm an admin." })) },
    { author_fp: fpOf(ANN), created_at: 3001, payload_b64: b64(JSON.stringify({ epoch: 1, text: "Let's talk on Telegram. Buy a gift card." })) },
    { author_fp: fpOf(ME), created_at: 3002, payload_b64: b64(JSON.stringify({ epoch: 1, text: "Buy a gift card, I'm an admin" })) },
  );
  chat.fn("openP2pGroup")(GROUP, "Garden");
  await settle();
  const rowFrom = (k) => chat.appended.filter((e) => e && e.dataset && e.dataset.from === k);
  assert.equal(rowFrom(CY).length, 1, "the group's history is drawn");
  assert.deepEqual(shownWarningIds(rowFrom(CY)[0]), ["staff_claim", "move_elsewhere"], "a group's history shows its warnings (a stranger)");
  assert.deepEqual(shownWarningIds(rowFrom(ANN)[0]), ["money"], "a friend in the group: only what applies to friends");
  assert.deepEqual(warningsUnder(rowFrom(ME)[0]), [], "never on my own");

  // Live, from a member's push.
  const live = { object_type: "group_msg_v1", references: [GROUP], id: "obj-live", author: CY, created_at: 3005, payload: { epoch: 1, text: "Wire transfer now, this is our secret." } };
  await chat.fn("handleP2pGroupObj")({ submission: live }, CY);
  await settle();
  const liveRow = rowFrom(CY).find((e) => String(e.dataset.timestamp) === "3005");
  assert.ok(liveRow, "the live message is drawn");
  assert.deepEqual(shownWarningIds(liveRow), ["money", "urgency_secrecy"], "and so does a live group message");
});

test("a stranger's link is held until Open; a friend's, mine and a message without a link are not", async () => {
  const chat = await loadChat();
  makeFriends(chat, ANN);
  const URL1 = "https://example.org/claim?prize=1";
  const row = await receiveDm(chat, BEN, `Look at this: ${URL1}`, 4000);
  assert.ok(row.innerHTML.includes(`data-held-href="${URL1}"`), "the link is drawn held");
  assert.ok(!/\shref="https:\/\/example\.org/.test(row.innerHTML), "the stranger's link does not open");
  const lines = findAll(row, "msg-link-hold");
  assert.equal(lines.length, 1, "one line under it");
  assert.ok(textOf(lines[0], true).includes("Ben is not your friend. Links open only when you choose."), "10g's line, with their name");
  assert.equal(W.strangerLinkLine("Ben"), "Ben is not your friend. Links open only when you choose.");
  const open = findAll(lines[0], "msg-link-open")[0];
  assert.ok(open && open.textContent === "Open", "with an Open button");
  open.onclick({ stopPropagation() {} });
  const body = row.querySelector(".body");
  assert.ok(/<a href="https:\/\/example\.org\/claim\?prize=1"/.test(body.innerHTML), "Open lets the link open");
  assert.ok(hidden(lines[0]), "and the line goes");
  // Drawn again: it stays opened.
  chat.appended.length = 0;
  chat.fn("renderDmConversationFromStore")(BEN);
  const again = chat.appended.find((e) => e && e.dataset && e.dataset.from === BEN);
  assert.ok(/\shref="https:\/\/example\.org/.test(again.innerHTML) && findAll(again, "msg-link-hold").length === 0, "Open holds when the conversation is drawn again");

  const friend = await receiveDm(chat, ANN, `Recipe: ${URL1}`, 4001);
  assert.ok(/\shref="https:\/\/example\.org/.test(friend.innerHTML) && findAll(friend, "msg-link-hold").length === 0, "a friend's link opens, with no line");
  const plain = await receiveDm(chat, CY, "No links here", 4002);
  assert.equal(findAll(plain, "msg-link-hold").length, 0, "no link, no line");
  const fileLink = await receiveDm(chat, CY, "the notes: /uploads/notes.pdf", 4003);
  assert.ok(fileLink.innerHTML.includes('data-held-href="/uploads/notes.pdf"') && findAll(fileLink, "msg-link-hold").length === 1, "a file on this server is a link too");
  // My own (from another of my devices).
  chat.set("(k) => { activeDmPartner = k; }", CY);
  const before = chat.appended.length;
  await chat.handle({ type: "dm_new", id: ++dmId, content: envelope(ME, CY, `mine: ${URL1}`, 4004) });
  const mine = chat.appended.slice(before).find((e) => e && e.dataset && e.dataset.from === ME);
  assert.ok(/\shref="https:\/\/example\.org/.test(mine.innerHTML) && findAll(mine, "msg-link-hold").length === 0, "my own links are mine");
  // The switch is about the warnings; the link rule stays when it is off.
  chat.fn("setMessageWarningsOn")(false);
  const offRow = await receiveDm(chat, CY, `Again: ${URL1}`, 4005);
  assert.ok(offRow.innerHTML.includes("data-held-href") && findAll(offRow, "msg-link-hold").length === 1, "the link line stays with warnings off");
});

// ── 7: the switch ────────────────────────────────────────────────────────

test("Settings > Safety: Warnings on messages, On by default, kept in the local store, Off hides them all", async () => {
  const idb = fakeIndexedDB();
  const storeKey = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  const chat = await loadChat({ indexedDB: idb, storeKey });
  assert.equal(chat.store.warningsOn, true, "On by default");
  let model = chat.fn("safetyModel")();
  assert.equal(model.warningsOn, true);
  let html = chat.fn("safetyPanelHtml")(model);
  assert.ok(html.includes(">Warnings on messages<"), "Settings > Safety has the switch, by its name");
  assert.ok(/<input type="checkbox" data-warnings-switch[^>]* checked/.test(html), "switched on");
  assert.ok(!/data-warnings-switch[^>]* disabled/.test(html), "and it can be changed once the store is loaded");
  assert.ok(html.includes("Warnings are checked on this device only."), "with 6.5's sentence");

  const first = await receiveDm(chat, BEN, "Buy a gift card", 5000);
  assert.deepEqual(shownWarningIds(first), ["money"]);
  // Off, through the page's own switch.
  chat.fn("openSafetyPanel")();
  const card = chat.el("safety-card");
  const sw = card.querySelector("[data-warnings-switch]");
  assert.equal(typeof sw.onchange, "function", "the switch is wired");
  sw.checked = false;
  sw.onchange();
  await settle();
  assert.equal(chat.store.warningsOn, false);
  assert.deepEqual(shownWarningIds(first), [], "Off hides the warnings on screen");
  const second = await receiveDm(chat, BEN, "Another gift card", 5001);
  assert.deepEqual(shownWarningIds(second), [], "and the ones drawn after");
  assert.equal(warningsUnder(second).length, 1, "(drawn hidden, so On can show them)");
  html = chat.fn("safetyPanelHtml")(chat.fn("safetyModel")());
  assert.ok(!/data-warnings-switch[^>]* checked/.test(html), "the switch shows Off");

  // Kept, encrypted, with the block list: a fresh load of the same identity reads it back.
  await settle();
  const meta = [...idb.stores.meta.values()][0];
  assert.ok(meta && meta.box && !JSON.stringify(meta).includes("warningsOn"), "kept only inside the encrypted box");
  const again = await loadChat({ indexedDB: idb, storeKey });
  assert.equal(again.store.warningsOn, false, "the switch is kept in the store");
  const later = await receiveDm(again, BEN, "gift card", 5002);
  assert.deepEqual(shownWarningIds(later), [], "and honoured after a reload");

  // On again: they show, except one dismissed with Got it.
  chat.fn("setMessageWarningsOn")(true);
  assert.deepEqual(shownWarningIds(first), ["money"], "On shows them again");
  assert.deepEqual(shownWarningIds(second), ["money"], "the one drawn while Off too");
  findAll(first, "msg-warning-ok")[0].onclick({ stopPropagation() {} });
  chat.fn("setMessageWarningsOn")(false);
  chat.fn("setMessageWarningsOn")(true);
  assert.deepEqual(shownWarningIds(first), [], "one dismissed with Got it stays hidden");
  assert.deepEqual(shownWarningIds(second), ["money"], "and only that one");

  // Before the store loads, the switch cannot be changed (there is nowhere to keep it yet).
  await settle(); // the saves above finish first
  chat.set("() => { hosDmStore._db = null; }");
  html = chat.fn("safetyPanelHtml")(chat.fn("safetyModel")());
  assert.ok(/data-warnings-switch[^>]* disabled/.test(html), "disabled until the store loads");
  assert.equal(chat.fn("setMessageWarningsOn")(false), false);
});

// ── 8: the guard ─────────────────────────────────────────────────────────

test("the recovery-phrase guard: 4 words in a row stop every kind of send; 3, or out of order, go; locked sends", async () => {
  const chat = await loadChat();
  const list = chat.ctx.BIP39_ENGLISH;
  const words = phraseOf(SEED, list);
  // The phrase the page derives is the one the backup screen shows.
  assert.deepEqual(await chat.fn("recoveryPhraseWords")(), words, "derived in the page from the identity's seed, the BIP39 way");
  assert.equal(await chat.fn("generateMnemonic")(), words.join(" "), "the backup screen's phrase");
  for (const f of ["here", "are", "my", "words", "send", "thanks", "the", "and", "for"]) assert.ok(!list.includes(f), `${f} is not a phrase word`);
  const four = `Here are my words: ${words[4][0].toUpperCase()}${words[4].slice(1)}, ${words[5].toUpperCase()} ${words[6]}-${words[7]}. Thanks`;
  const three = `Here are my words: ${words[4]} ${words[5]} ${words[6]}, thanks`;
  const shuffled = `${words[4]} ${words[6]} ${words[5]} ${words[7]}`;
  const whole = words.join(" ");
  const SENTENCE = "This is your recovery phrase. Anyone who has it owns your identity and everything in it. Nobody legitimate will ever ask for it. Remove it to send the rest.";
  assert.equal(W.PHRASE_GUARD_SENTENCE, SENTENCE, "10g's sentence");
  const stops = () => chat.appended.filter((e) => String(e.className).includes("phrase-guard-stop"));
  const input = chat.el("msg-input");
  const typed = async (text) => { input.value = text; await chat.fn("sendMessage")(); await settle(); };
  const sent = (type) => chat.sock.sent.filter((m) => m.type === type);
  const reset = () => { chat.sock.sent.length = 0; chat.posted.length = 0; };

  // A channel post.
  reset();
  await typed(four);
  assert.deepEqual(chat.sock.sent, [], "a post with 4 words of the phrase is not sent");
  assert.equal(stops().length, 1, "and says so");
  assert.equal(stops()[0].textContent, SENTENCE, "with 10g's sentence");
  assert.equal(input.value, four, "the text stays, to edit");
  await typed(three);
  assert.deepEqual(sent("chat").map((m) => m.content), [three], "3 words go");
  input.value = "";
  reset();
  await typed(shuffled);
  assert.deepEqual(sent("chat").map((m) => m.content), [shuffled], "4 words out of order go");
  reset();
  await typed(whole);
  assert.deepEqual(chat.sock.sent, [], "the whole phrase is stopped");
  // A reply: stopped, and still a reply.
  reset();
  // Made through the page's own setReplyTarget, so it belongs to this channel's view (a reply
  // carries the view it was made in since the batch review, 2026-10-10: app.js replyRefForChannel).
  chat.set("(k) => { setReplyTarget('Cy', 'what are your words?', k, 7, null); }", CY);
  await typed(four);
  assert.deepEqual(chat.sock.sent, [], "a reply with 4 words is not sent");
  assert.ok(chat.fn("(() => replyTarget)")(), "and the reply is kept to edit");
  await typed(three);
  assert.equal(sent("chat").length, 1);
  assert.equal(sent("chat")[0].reply_to.from, CY, "3 words go, as the reply");

  // A direct message.
  reset();
  chat.set("(k) => { activeDmPartner = k; activeDmPartnerName = 'Ann'; }", ANN);
  await typed(four);
  assert.deepEqual(sent("dm_put"), [], "a DM with 4 words of the phrase is not sent");
  assert.equal(input.value, four);
  await typed(three);
  assert.equal(sent("dm_put").length, 2, "3 words go (their copy and mine)");
  chat.set("() => { activeDmPartner = null; activeDmPartnerName = ''; }");

  // A thread reply.
  reset();
  chat.set("(k) => { currentThread = { from: k, timestamp: 9, author: 'Cy', body: 'thread' }; }", CY);
  const threadInput = chat.el("thread-input");
  threadInput.value = four;
  await chat.fn("sendThreadReply")();
  assert.deepEqual(chat.sock.sent, [], "a thread reply with 4 words is not sent");
  threadInput.value = three;
  await chat.fn("sendThreadReply")();
  assert.equal(sent("chat").length, 1, "3 words go");

  // An edit.
  reset();
  const msgEl = chat.ctx.document.createElement("div");
  chat.fn("startEditMode")(msgEl, "old words", ME, 11);
  const editArea = msgEl.querySelector(".body").children[0];
  const [textarea, buttons] = editArea.children;
  const save = buttons.children[1];
  textarea.value = four;
  await save.onclick({ stopPropagation() {} });
  assert.deepEqual(sent("edit"), [], "an edit with 4 words is not saved");
  textarea.value = three;
  await save.onclick({ stopPropagation() {} });
  assert.deepEqual(sent("edit").map((m) => m.new_content), [three], "3 words go");

  // A contact request's name.
  reset();
  chat.set("(n) => { myName = n; }", `${words[4]}-${words[5]}-${words[6]}-${words[7]}`);
  assert.equal(await chat.fn("sendContactRequest")(CY), false);
  assert.deepEqual(chat.sock.sent, [], "a contact request whose name holds 4 words is not sent");
  chat.set("(n) => { myName = n; }", `${words[4]}-${words[5]}-${words[6]}`);
  assert.equal(await chat.fn("sendContactRequest")(CY), true);
  assert.ok(sent("dm_put").some((m) => m.contact_request === true), "3 words go");
  chat.set("() => { myName = 'Me_1'; }");

  // A profile field.
  reset();
  chat.ctx.localStorage.setItem("humanity_profile", JSON.stringify({ bio: "gardener", socials: { discord: four } }));
  await chat.fn("pushProfileToRelay")();
  assert.deepEqual(sent("profile_update"), [], "a profile field with 4 words is not sent");
  assert.ok(stops().some((e) => e.textContent === "Your profile was not sent to this server. " + SENTENCE), "and the chat says which");
  chat.ctx.localStorage.setItem("humanity_profile", JSON.stringify({ bio: three, socials: {} }));
  await chat.fn("pushProfileToRelay")();
  assert.deepEqual(sent("profile_update").map((m) => m.bio), [three], "3 words go");

  // A P2P group message.
  reset();
  chat.fn("openP2pGroup")(GROUP, "Garden");
  await settle();
  await typed(four);
  assert.deepEqual(chat.posted, [], "a group message with 4 words is not sent");
  assert.equal(input.value, four);
  await typed(three);
  assert.deepEqual(chat.posted.map((s) => s.plaintext), [three], "3 words go");
  chat.fn("closeP2pGroup")();

  // Locked: there is no phrase to derive, so the guard cannot run and the send goes as today.
  reset();
  chat.set("() => { myIdentity = null; }");
  assert.equal(await chat.fn("recoveryPhraseWords")(), null);
  await typed(four);
  assert.deepEqual(sent("chat").map((m) => m.content), [four], "a locked identity sends");
});

// ── 9: every other path ──────────────────────────────────────────────────

test("every other send path awaits the guard before it sends, and index.html loads both scripts", () => {
  const src = (rel) => fs.readFileSync(path.join(WEB, rel), "utf8");
  // [file, where the path starts, what sends]: the guard must come between the two.
  const paths = [
    ["chat/app.js", "async function sendMessage()", "ws.send("],
    ["chat/app.js", "async function connect(", "openSocket("],
    ["chat/app.js", "async function sendChatCommand(", "ws.send("],
    ["chat/app.js", "async function handleSkillVerifyRequest(", "ws.send("],
    ["chat/app.js", "async function sendGameBan(", "ws.send("],
    ["chat/chat-ui.js", "async function sendComposedContent(", "ws.send("],
    // A file's marker in a P2P group (10k) leaves sendComposedContent here.
    ["chat/chat-ui.js", "async function sendComposedContent(", "sendToActiveP2pGroup("],
    ["chat/chat-ui.js", "async function promptAddServer(", "ws.send("],
    ["chat/chat-ui.js", "async function doSearch(", "ws.send("],
    ["chat/chat-groups-p2p.js", "sendMessage = async function ()", "sendToActiveP2pGroup("],
    ["chat/chat-messages.js", "saveBtn.onclick = async", "ws.send("],
    ["chat/chat-messages.js", "async function sendThreadReply(", "ws.send("],
    ["chat/chat-messages.js", "async function uploadImage(", "fetch("],
    ["chat/chat-messages.js", "async function sendEncryptedAttachment(", "fetch("],
    ["chat/chat-p2p.js", "async function sendP2PMessage(", "dc.send("],
    ["chat/chat-privacy.js", "async function sendContactRequest(", "ws.send("],
    ["chat/chat-profile.js", "async function pushProfileToRelay(", "ws.send("],
    ["chat/chat-profile.js", "async function syncSystemProfile(", "fetch("],
    ["chat/chat-reports.js", "async function submitReportDialog(", "ws.send("],
    ["chat/chat-voice-rooms.js", "async function createVoiceRoom(", "ws.send("],
    ["chat/chat-voice-rooms.js", "if (action === 'rename')", "ws.send("],
    ["chat/crypto.js", "async function labelDevice(", "ws.send("],
  ];
  for (const [rel, start, sends] of paths) {
    const s = src(rel);
    const at = s.indexOf(start);
    assert.ok(at >= 0, `${rel}: ${start} is there`);
    const send = s.indexOf(sends, at);
    assert.ok(send > at, `${rel}: ${start} sends with ${sends}`);
    const body = s.slice(at, send);
    assert.ok(/await recoveryPhrase(GuardStops|In)\(/.test(body), `${rel}: ${start.replace(/\(.*$/, "")} awaits the guard before it sends`);
  }
  const html = src("chat/index.html");
  const at = (needle) => html.indexOf(needle);
  assert.ok(at('src="/shared/warnings.js?v=') > 0, "index.html loads /shared/warnings.js");
  assert.ok(at('src="/chat/chat-warnings.js?v=') > 0, "and /chat/chat-warnings.js");
  assert.ok(at('src="/shared/warnings.js?v=') < at('src="/chat/crypto.js?v='), "the matcher before the chat scripts");
  assert.ok(at('src="/chat/chat-privacy.js?v=') < at('src="/chat/chat-warnings.js?v='), "the warnings after the Safety page they draw into");
});

// ── 10: the file arriving late, or not at all ───────────────────────────

test("messages drawn before the warnings file arrived get their warnings when it does; a failed file is not asked for in a loop", async () => {
  let release;
  const gate = new Promise((r) => { release = r; });
  const chat = await loadChat({ warningsFetch: () => gate.then(() => ok(SHIPPED_WARNINGS)) });
  const row = await receiveDm(chat, BEN, "Buy a gift card", 6000);
  assert.deepEqual(warningsUnder(row), [], "nothing yet: the file has not arrived");
  release();
  await settle();
  assert.deepEqual(shownWarningIds(row), ["money"], "drawn before the file arrived, the warning comes when it does");

  const failing = await loadChat({ warningsFetch: () => Promise.reject(new Error("offline")) });
  for (let i = 0; i < 5; i++) await receiveDm(failing, BEN, "Buy a gift card " + i, 6100 + i);
  const asked = failing.fetches.filter((u) => u === W.WARNINGS_URL).length;
  assert.equal(asked, 1, "a failed file is not asked for again at once");
});

// A phrase pasted as a numbered list, the way backup screens show it, is stopped too
// (2026-10-10, found by the desktop build; the same rule as src/net/warnings.rs
// holds_phrase_run). Seen red 2026-10-10 with the number-only filter removed from
// phraseRunFound: "a numbered list".
test("a numbered list of the recovery phrase is stopped", () => {
  const { phraseRunFound } = require(path.join(WEB, "shared", "warnings.js"));
  const phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident " +
    "account accuse achieve acid acoustic acquire across act action actor actress actual";
  assert.ok(phraseRunFound("1. abandon 2. ability 3. able 4. about", phrase), "a numbered list");
  assert.ok(phraseRunFound("5) above\n6) absent\n7) absorb\n8) abstract", phrase), "numbers with brackets, one per line");
  assert.ok(phraseRunFound("#9 absurd #10 abuse #11 access #12 accident", phrase), "numbers with a hash");
  assert.ok(!phraseRunFound("1. abandon 2. ability 3. able", phrase), "three numbered words still go");
  assert.ok(!phraseRunFound("abandon able ability about", phrase), "out of order still goes");
});
