// A report about a group reaches the group's creator, in the web chat client (10j, 2026-10-10,
// docs/design/blocking-and-safe-mode.md). The web half is built first; the desktop app matches it.
//
// Run: node --test scripts/tests/group-report-web.test.js   (in `just rig-tests`)
//
// The pure rules are web/shared/group-report.js, required here as a CommonJS module. The page
// scripts run as they do in the browser, in one shared global scope (node:vm), with the DOM
// replaced by a stub that keeps what is written to it and the listeners a script adds (as in
// report-web.test.js and warnings-web.test.js): the real events.js, friend-pass.js, reach.js,
// block.js, report.js, group-report.js, crypto.js, chat-dm-store.js (over a stand-in IndexedDB),
// app.js, chat-messages.js, chat-dms.js, chat-social.js, chat-groups-p2p.js, chat-ui.js,
// chat-voice-rooms.js, chat-voice-calls.js, chat-voice-modal.js, chat-privacy.js, chat-reports.js
// and chat-p2p.js, in index.html's order.
//
// The group's signed objects are REAL: pq-object.js and the vendored noble bundle (ML-DSA-65,
// BLAKE3) build them, with real keys for Ann, Ben, Cy and Dan, and the group's messages are really
// encrypted under its key, so the creator's check runs the real signature check over the real
// bytes. chat-groups-p2p.js imports both modules; node:vm cannot import without a flag, so the test
// rewrites that file's `import(` to reach the same two modules. Only the DM layer is stood in, as
// in the other web tests: a DM's "signature" is the signed words and "sealing" base64s the
// plaintext under the recipient's key name, so a test reads exactly what was sealed. The group's
// key is sealed the same stand-in way. HOS_WEB_DIR points the test at another copy of web/ (used to
// see each test red).
//
// What it proves (10j's client Proof list, the web items):
//  1. The words are 10j's: the marker, the JSON's fields in 10j's order, the dm_put flag, the
//     limits, the dialog's choices and the two sentences, the line before sending, the badges, the
//     section title and the three actions, each read out of 10j's own text.
//  2. The dialog's choices: all three with the creator as the default; only the admins, with 10j's
//     sentence, when I created the group or the person reported did (and when the creator cannot
//     be found out); none while finding.
//  3. The marker's exact shape: the text is the marker then the JSON exactly, a file is never in
//     it, 20 items and 16 KB pass and one more does not, on the way out and on the way in.
//  4. The creator's checks against their own copy, over real signed objects: found (with the
//     signer), not found, altered text, wrong sender, wrong time, a stored forgery whose signature
//     does not check, and an id the server put on other bytes.
//  5. The creator keeps only a report about a group they hold as its creator, from someone in it,
//     addressed to them, and not about themselves.
//  6. In the page, the reporter: a group message's menu opens the dialog on that message; the
//     creator is found from the group's own signed record; "Send this report to" offers the three
//     with the creator ticked and "The group's creator will see that you sent this."; Send sends
//     ONE dm_put to the creator flagged group_report, sealed, whose inner text is exactly the
//     marker and the JSON (the item's id the message's signed-object id), no self-copy and no
//     report_v2; Both sends both; the admins alone sends report_v2 only; the two admins-only cases
//     say their sentence; a refusal says the report did not reach the creator instead of offering
//     a contact request; a group message with a file sends no item.
//  7. In the page, the creator: the report arrives as a DM from someone who is not a friend (so it
//     is ingested before the reach screen), is checked against the creator's copy, is listed under
//     "Reports about your groups" with its badges and a count on the group, is never stored as a
//     message nor sent to any server, and stays in the local store across a reload; one about a
//     group not held, from someone not in it, or about the creator is dropped.
//  8. The three actions: Remove them from the group posts a new group key sealed to everyone else
//     (them left out although the server still lists them), then the signed remove (the same
//     builder a leave uses) with them as the subject, and says Removed; a key that cannot be made
//     removes no one; Block them blocks them; Dismiss takes the report off this device and the
//     count off the group.
//
// Red first: see the end of this file for each deliberate break and the assertion it tripped.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const gr = require(path.join(WEB, "shared", "group-report.js"));
const report = require(path.join(WEB, "shared", "report.js"));

const SPEC = fs.readFileSync(path.join(ROOT, "docs", "design", "blocking-and-safe-mode.md"), "utf8");
// 10j with its line wrapping undone (a sentence may wrap in the source).
const TEN_J = SPEC.slice(SPEC.indexOf("## 10j."), SPEC.indexOf("## 11.")).replace(/\s+/g, " ");
const REASONS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "report_reasons.json"), "utf8"));

const hex = (u8) => Buffer.from(u8).toString("hex");
const b64 = (s) => Buffer.from(s, "utf8").toString("base64");
const unb64 = (s) => Buffer.from(s, "base64").toString("utf8");
const html = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");
const FILE_TEXT = "[[hum:file:v1]]eyJrZXkiOiJTRUNSRVQifQ==";

// ── The real group modules and the fixture ──────────────────────────────

let MODS = null;
async function modules() {
  if (MODS) return MODS;
  // ESM in .js files under a package.json with no "type": imported from their text, the CBOR
  // module's relative import pointed at its own data URL.
  const asUrl = (src) => "data:text/javascript;base64," + Buffer.from(src).toString("base64");
  const cborUrl = asUrl(fs.readFileSync(path.join(WEB, "shared", "canonical-cbor.js"), "utf8"));
  const objSrc = fs.readFileSync(path.join(WEB, "shared", "pq-object.js"), "utf8")
    .replace(/(['"])\.\/canonical-cbor\.js\1/, JSON.stringify(cborUrl));
  const obj = await import(asUrl(objSrc));
  const cbor = await import(cborUrl);
  const noble = await import(asUrl(fs.readFileSync(path.join(WEB, "shared", "vendor", "noble-pq.bundle.js"))));
  const blake3 = (b) => noble.blake3.create({ dkLen: 32 }).update(b).digest();
  const verify = async (pk, msg, sig) => noble.ml_dsa65.verify(sig, msg, pk);
  MODS = { obj, cbor, noble, blake3, verify };
  return MODS;
}

const kyberOf = (k) => "kyber-" + k.slice(0, 16);
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const T0 = 1760000000000;

let FX = null;
async function fixture() {
  if (FX) return FX;
  const m = await modules();
  const { obj, noble, blake3 } = m;
  const person = (n, name) => {
    const kp = noble.ml_dsa65.keygen(new Uint8Array(32).fill(n));
    return { name, pub: kp.publicKey, sk: kp.secretKey, key: hex(kp.publicKey) };
  };
  const ann = person(1, "Ann");
  const ben = person(2, "Ben");
  const cy = person(3, "Cy");
  const dan = person(4, "Dan");
  const signer = (p) => async (bytes) => noble.ml_dsa65.sign(bytes, p.sk);
  const fp = (key) => hex(blake3(Buffer.from(key, "hex"))).slice(0, 32);
  const served = (built, author) => ({ object_id: built.objectId, author_fp: fp(author.key), received_at: 1, ...built.submission });
  const seal = async (pub, plaintext) => ({ ek_ct_b64: b64(pub), nonce_b64: "AAAA", ct_b64: b64(plaintext) });

  // Ben's group; its key (epoch 1) sealed to Ann, Ben and Cy.
  const group = await obj.buildGroupV1({ name: "Hikers", authorPublicKey: ben.pub, sign: signer(ben), blake3, createdAt: T0 });
  const G = group.objectId;
  const K = new Uint8Array(32).fill(7);
  const epoch = await obj.buildGroupEpochKeyV1({
    groupId: G, epoch: 1, epochKey: K,
    members: [ann, ben, cy].map((p) => ({ fp: fp(p.key), kyber_public: kyberOf(p.key) })),
    seal, authorPublicKey: ben.pub, sign: signer(ben), blake3, createdAt: T0 + 100,
  });
  const msg = (p, text, at) => obj.buildGroupMsgV1({ groupId: G, epoch: 1, epochKey: K, plaintext: text, authorPublicKey: p.pub, sign: signer(p), blake3, createdAt: at });
  const m1 = await msg(cy, "you are all idiots", T0 + 500);
  const m2 = await msg(ann, "please stop", T0 + 600);
  const m3 = await msg(ben, "calm down everyone", T0 + 700);
  // A forgery a server could hold: words Ann encrypted and signed, with Cy named as the author. Its
  // id is the hash of exactly these bytes, so only the signature check can tell.
  const forgedBase = await msg(ann, "I will hurt you", T0 + 800);
  const forgedSub = { ...forgedBase.submission, author_public_key_b64: Buffer.from(cy.pub).toString("base64") };
  const forgedId = (await obj.verifyObjectSubmission(forgedSub, { blake3, pqVerify: async () => true })).objectId;
  const forged = { object_id: forgedId, author_fp: fp(cy.key), received_at: 1, ...forgedSub };
  // A server that puts Cy's message id on Ann's message.
  const mislabelled = { ...served(m2, ann), object_id: m1.objectId };
  // And on another message of Cy's with the same words at the same moment: genuinely Cy's, but not
  // the message the report names (its own id is the hash of its own bytes).
  const m1twin = await msg(cy, "you are all idiots", T0 + 500);
  const twinRelabelled = { ...served(m1twin, cy), object_id: m1.objectId };

  const log = [served(m1, cy), served(m2, ann), served(m3, ben), forged];
  FX = {
    m, ann, ben, cy, dan, fp, signer, seal, G, K,
    groupServed: { object_id: G, author_fp: fp(ben.key), received_at: 1, ...group.submission },
    epochServed: { object_id: epoch.objectId, author_fp: fp(ben.key), received_at: 1, ...epoch.submission },
    m1, m2, m3, forged, forgedId, mislabelled, twinRelabelled, log,
  };
  return FX;
}

/** The creator's check, in the open, over a log: the real signature check and the group key. */
async function check(fx, rep, log) {
  const { obj, blake3, verify } = fx.m;
  return gr.groupReportCheck(rep, log, {
    verify: (o) => obj.verifyObjectSubmission(o, { blake3, pqVerify: verify }),
    open: async (payload) => {
      const p = obj.parseGroupMsgPayload(payload);
      return p && p.epoch === 1 ? obj.aesGcmDecrypt(fx.K, p.nonce, p.ct) : null;
    },
  });
}

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
const NOTHING_YET = new Set([".edit-area", ".edited-marker", ".block-indicator"]);

// A document whose elements keep what is written to them, their class list, the listeners a
// script adds, and one kept element per selector they are asked for (so the author name a message
// row hooks its menu to can be clicked).
function fakeDom(state) {
  function el(tag) {
    const classes = new Set();
    const own = {
      tag, children: [], parts: new Map(), style: {}, dataset: {}, attrs: {}, listeners: {},
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
    own.addEventListener = (type, fn) => { (own.listeners[type] = own.listeners[type] || []).push(fn); };
    own.remove = () => { own.removed = true; };
    own.focus = () => {};
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
  const byId = new Map();
  const kept = (id) => { if (!byId.has(id)) byId.set(id, el("#" + id)); return byId.get(id); };
  return new Proxy({}, {
    get(_t, prop) {
      if (prop === "createElement") return el;
      if (prop === "getElementById") return kept;
      if (prop === "body") return kept("__body");
      if (prop === "querySelector") return () => anything();
      if (prop === "querySelectorAll") {
        return (sel) => (sel === ".message[data-from]" ? state.appended.filter((e) => e && e.dataset && e.dataset.from) : []);
      }
      if (prop === "hidden") return false;
      return anything();
    },
    set: () => true,
  });
}

function fakeStorage() {
  const m = new Map();
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k), clear: () => m.clear() };
}

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
  return { readyState: 1, sent: [], raw: [], send(s) { this.raw.push(s); this.sent.push(JSON.parse(s)); }, close() {} };
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

// The page's asynchronous work is WebCrypto (its own, and pq-object.js's group-key decryption,
// which runs in this realm), the stand-in IndexedDB and resolved fetches. Not every frame handler
// returns its promise (chat-social.js's wrapper does not), so settle() waits until no WebCrypto
// call is in flight for a run of turns. The counting is on this realm's SubtleCrypto itself, so
// the page's calls and pq-object.js's are both seen.
let cryptoInFlight = 0;
const subtle = globalThis.crypto.subtle;
for (const name of ["encrypt", "decrypt", "sign", "verify", "digest", "importKey", "exportKey", "generateKey", "deriveKey", "deriveBits"]) {
  const orig = subtle[name];
  if (typeof orig !== "function") continue;
  Object.defineProperty(subtle, name, {
    configurable: true,
    writable: true,
    value: (...args) => {
      cryptoInFlight++;
      return Promise.resolve(orig.apply(subtle, args)).finally(() => { cryptoInFlight--; });
    },
  });
}
const countedCrypto = {
  subtle,
  getRandomValues: (a) => globalThis.crypto.getRandomValues(a),
  randomUUID: () => globalThis.crypto.randomUUID(),
};

async function settle() {
  let idle = 0;
  for (let i = 0; i < 20000 && idle < 40; i++) {
    await new Promise((r) => setImmediate(r));
    idle = cryptoInFlight === 0 ? idle + 1 : 0;
  }
}

/** Wait (by turns, never by the clock) until `pred` holds; fail naming `what` if it never does. */
async function waitFor(pred, what) {
  for (let i = 0; i < 20000; i++) {
    if (pred()) return;
    await new Promise((r) => setImmediate(r));
  }
  assert.fail("never happened: " + what);
}

const ok = (json) => Promise.resolve({ ok: true, status: 200, json: async () => json });
const notFound = () => Promise.resolve({ ok: false, status: 404, json: async () => ({ error: "not found" }) });

// A DM's words start "hum/" (and a pass's, an identify's, a report's): those are stood in. Anything
// else signed here is a signed object's canonical bytes (a CBOR map), signed for real.
const isWords = (b) => b.length >= 4 && b[0] === 0x68 && b[1] === 0x75 && b[2] === 0x6d && b[3] === 0x2f;

/**
 * The chat page as `me`. `opts.groups`: what /api/v2/groups lists for me. The stand-in server
 * serves the fixture's group, its key, its members (Ann, Ben, Cy) and its messages, and keeps what
 * is posted to /api/v2/objects.
 */
async function loadChat(me, opts = {}) {
  const fx = await fixture();
  const state = { appended: [] };
  const posted = [];
  const fetched = [];
  const relay = {
    groups: opts.groups || [],
    members: [fx.ann, fx.ben, fx.cy].map((p) => ({ pubkey: p.key, kyber_public: kyberOf(p.key) })),
    log: opts.log || fx.log,
    epoch: fx.epochServed,
  };
  const G = fx.G;
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  const idb = opts.indexedDB || fakeIndexedDB();
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDom(state),
    navigator: new Proxy({}, { get: (_t, p) => (p === "serviceWorker" ? serviceWorker : anything()), has: () => true }),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", origin: "https://localhost", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    indexedDB: idb,
    fetch: (url, init) => {
      const u = String(url);
      const method = (init && init.method) || "GET";
      fetched.push(method + " " + u);
      if (u.startsWith("/api/federation/servers")) return ok([]);
      if (u.startsWith(report.REPORT_REASONS_URL)) return ok(JSON.parse(JSON.stringify(REASONS)));
      if (u === "/api/v2/objects" && method === "POST") { posted.push(JSON.parse(init.body)); return ok({ ok: true }); }
      if (u.startsWith("/api/v2/groups?pubkey=")) return ok({ groups: JSON.parse(JSON.stringify(relay.groups)) });
      if (u === `/api/v2/objects/${G}`) return ok(fx.groupServed);
      if (u.startsWith("/api/v2/objects/")) return notFound();
      if (u === `/api/v2/groups/${G}/members`) return relay.membersDown ? Promise.resolve({ ok: false, status: 503, json: async () => ({}) }) : ok({ members: relay.members });
      if (u === `/api/v2/groups/${G}/epochs`) return ok({ epochs: [relay.epoch] });
      if (u === `/api/v2/groups/${G}/epoch`) return ok(relay.epoch);
      if (u === `/api/v2/groups/${G}/messages`) return ok({ messages: relay.log });
      return Promise.reject(new Error("no network in tests: " + u));
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
  const mods = { "/shared/pq-object.js": fx.m.obj, "/shared/vendor/noble-pq.bundle.js": fx.m.noble };
  ctx.__testImport = async (spec) => {
    if (!mods[spec]) throw new Error("no module " + spec);
    return mods[spec];
  };
  vm.createContext(ctx);
  const run = (rel, rewrite) => {
    let src = fs.readFileSync(path.join(WEB, rel), "utf8");
    if (rewrite) src = rewrite(src);
    vm.runInContext(src, ctx, { filename: rel });
  };
  // In index.html's order.
  run("shared/events.js");
  run("shared/friend-pass.js");
  run("shared/reach.js");
  run("shared/block.js");
  run("shared/report.js");
  run("shared/group-report.js");
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
  run("chat/chat-voice-modal.js");
  run("chat/chat-privacy.js");
  run("chat/chat-reports.js");
  run("chat/chat-p2p.js");
  for (const name of ["hosIcon", "holdToConfirm", "updateStats"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  const confirms = [];
  ctx.holdConfirm = async (message) => { confirms.push(String(message)); return true; };
  ctx.appendMessage = (el) => { state.appended.push(el); };
  ctx.notifyNewMessage = () => {};
  ctx.playNotificationChime = () => {};
  ctx.sendSWNotification = () => {};
  const key = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => opts.storeKey || key;
  const { noble } = fx.m;
  ctx.pqSignMessage = async (secret, bytes) => (isWords(bytes) ? new Uint8Array(bytes) : noble.ml_dsa65.sign(bytes, secret));
  ctx.pqVerifyMessage = async (pk, bytes, sig) => (sig.length === 3309 ? noble.ml_dsa65.verify(sig, bytes, pk) : Buffer.from(bytes).equals(Buffer.from(sig)));
  ctx.pqDmSeal = fx.seal;
  ctx.pqDmOpen = async (_secret, ek, _nonce, ct) => (unb64(ek) === kyberOf(me.key) ? unb64(ct) : null);
  ctx.pqBlake3 = async (bytes) => fx.m.blake3(bytes);
  const sock = fakeSocket();
  vm.runInContext(`(s, me, name, sk, kyber) => {
    ws = s; myKey = me; myName = name; activeChannel = 'general';
    myDilithiumPublicHex = me; myDilithiumSecret = sk;
    myKyberPublicBase64 = kyber; myKyberSecret = new Uint8Array(4);
  }`, ctx)(sock, me.key, me.name, me.sk, kyberOf(me.key));
  const handle = async (msg) => { await vm.runInContext("handleMessage", ctx)(msg); await settle(); };
  await handle({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  const store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await store.init(me.key, "localhost"), "the store loads");
  store.setPassServer(SERVER);
  const users = [fx.ann, fx.ben, fx.cy, fx.dan].map((p) => ({ public_key: p.key, name: p.name, role: "", kyber_public: kyberOf(p.key), online: true }));
  await handle({ type: "full_user_list", users });
  // The safe default: only friends may message me. Nobody here is anyone's friend.
  await handle({ type: "reach_settings", settings: { message: "friends", call: "chosen", trade: "friends" } });
  await settle();
  sock.sent.length = 0;
  sock.raw.length = 0;
  state.appended.length = 0;
  const fn = (name) => vm.runInContext(name, ctx);
  const el = (id) => ctx.document.getElementById(id);
  const dialog = () => vm.runInContext("reportDialog", ctx);
  return { ctx, fx, sock, store, state, relay, posted, fetched, confirms, idb, handle, fn, el, dialog };
}

function textOf(e) {
  if (!e) return "";
  const own = [e.textContent, typeof e.innerHTML === "string" ? e.innerHTML : ""].filter((s) => typeof s === "string").join(" ");
  return [own, ...(e.children || []).map(textOf)].join(" ");
}
const shown = (appended) => appended.map(textOf).join("\n");

// A DM from `from` to `to`, signed with the stand-in signer and sealed to the recipient's DM key.
function envelope(from, to, text, ts) {
  const sig = b64(`hum/dm/v2\n${from}\n${to}\n${ts}\n${text}`);
  const inner = JSON.stringify({ v: 2, from, to, ts, text, sig });
  return JSON.stringify({ v: 2, ek_ct_b64: b64(kyberOf(to)), nonce_b64: "AAAA", ct_b64: b64(inner) });
}
/** The inner payload sealed in a dm_put's content (the stand-in seal base64s it). */
const unsealed = (put) => JSON.parse(unb64(JSON.parse(put.content).ct_b64));

const GROUP_AS = (fx, me, isCreator) => [{ group_id: fx.G, name: "Hikers", members: [fx.ann.key, fx.ben.key, fx.cy.key], is_creator: isCreator }];

/** Open the group, wait for its messages, and press the author of `who`'s message: the Report dialog opens on it. */
async function reportFromGroup(page, who) {
  const { fn, state, fx } = page;
  fn("openP2pGroup")(fx.G, "Hikers");
  await waitFor(() => state.appended.some((e) => e && e.dataset && e.dataset.from === who.key && e.dataset.groupObjectId), "the group's messages are drawn");
  const row = state.appended.find((e) => e && e.dataset && e.dataset.from === who.key && e.dataset.groupObjectId);
  const author = row.querySelector(".author");
  assert.ok(author.listeners.click && author.listeners.click.length, "the author's name opens the menu");
  author.listeners.click[0]({ stopPropagation() {}, preventDefault() {}, clientX: 10, clientY: 10 });
  fn("reportUser")();
  await waitFor(() => page.dialog() && page.dialog().finding === false && page.dialog().reasons, "the dialog finds the creator and the reasons");
  return row;
}

// ── 1. The words are 10j's ──────────────────────────────────────────────

test("the words, the marker, the fields, the flag and the limits are 10j's", () => {
  const quoted = (s) => assert.ok(TEN_J.includes(s), `10j says ${JSON.stringify(s)}`);
  const marker = TEN_J.match(/the marker `([^`]+)`/);
  assert.ok(marker, "10j gives the marker");
  assert.equal(gr.GROUP_REPORT_MARKER, marker[1], "the marker is 10j's");
  // The JSON's fields, in 10j's order, at both levels.
  const shape = TEN_J.match(/`(\{"group_id"[^`]*\})`/);
  assert.ok(shape, "10j gives the JSON");
  const [top, inner] = shape[1].split('"items":');
  assert.deepEqual([...top.matchAll(/"(\w+)"/g)].map((x) => x[1]), ["group_id", "group_name", "target", "reason", "note"]);
  assert.deepEqual([...inner.matchAll(/"(\w+)"/g)].map((x) => x[1]), ["id", "from", "ts", "text"]);
  quoted('`"group_report": true` on the `dm_put`');
  quoted("At most 20 items and 16 KB of inner text");
  assert.equal(gr.GROUP_REPORT_MAX_ITEMS, 20);
  assert.equal(gr.GROUP_REPORT_MAX_BYTES, 16 * 1024);
  // The dialog.
  quoted(`a choice "${gr.GROUP_REPORT_SEND_TO}", with "${gr.GROUP_REPORT_DESTINATION_LABELS.creator}" (default), "${gr.GROUP_REPORT_DESTINATION_LABELS.admins}", and "${gr.GROUP_REPORT_DESTINATION_LABELS.both}"`);
  assert.deepEqual(gr.GROUP_REPORT_DESTINATIONS, ["creator", "admins", "both"]);
  quoted(`("${gr.GROUP_REPORT_YOU_CREATED}" or "${gr.GROUP_REPORT_THEY_CREATED}")`);
  quoted(`the dialog says so before sending: "${gr.GROUP_REPORT_CREATOR_SEES}"`);
  // The creator's side.
  quoted(`"${gr.groupReportFoundBadge("<name>")}"; anything else "${gr.GROUP_REPORT_NOT_FOUND}"`);
  quoted(`under "${gr.GROUP_REPORTS_TITLE}"`);
  const a = gr.GROUP_REPORT_ACTION_LABELS;
  quoted(`the actions "${a.remove}" (the existing remove path), "${a.block}", and "${a.dismiss}"`);
});

// ── 2. The dialog's choices ─────────────────────────────────────────────

test("the dialog's choices: three with the creator first, or only the admins with 10j's sentence", () => {
  const ME = "a1".repeat(32), THEM = "b2".repeat(32), CREATOR = "c3".repeat(32);
  assert.deepEqual(gr.groupReportChoices({ me: ME, target: THEM, creator: CREATOR }),
    { choices: ["creator", "admins", "both"], chosen: "creator", line: null, finding: false }, "all three, the creator by default");
  assert.deepEqual(gr.groupReportChoices({ me: ME, target: THEM, creator: ME.toUpperCase() }),
    { choices: ["admins"], chosen: "admins", line: gr.GROUP_REPORT_YOU_CREATED, finding: false }, "I created the group: only the admins");
  assert.deepEqual(gr.groupReportChoices({ me: ME, target: CREATOR, creator: CREATOR }),
    { choices: ["admins"], chosen: "admins", line: gr.GROUP_REPORT_THEY_CREATED, finding: false }, "they created the group: only the admins");
  assert.deepEqual(gr.groupReportChoices({ me: ME, target: THEM, creator: null }),
    { choices: ["admins"], chosen: "admins", line: gr.GROUP_REPORT_CREATOR_UNKNOWN, finding: false }, "the creator cannot be found out: only the admins");
  assert.deepEqual(gr.groupReportChoices({ me: ME, target: THEM, finding: true }).choices, [], "nothing to choose while finding");
  assert.ok(gr.groupReportToCreator("creator") && gr.groupReportToCreator("both") && !gr.groupReportToCreator("admins"));
  assert.ok(gr.groupReportToAdmins("admins") && gr.groupReportToAdmins("both") && !gr.groupReportToAdmins("creator"));
});

// ── 3. The marker, the limits, no file ──────────────────────────────────

test("the marker's exact shape, 20 items and 16 KB, and never a file", () => {
  const G = "ab".repeat(32), T = "cd".repeat(32), A = "ef".repeat(32);
  const item = (n, text) => ({ id: n.toString(16).padStart(64, "0"), from: T, ts: T0 + n, text: text == null ? `words ${n}` : text });
  const base = { group_id: G, group_name: "Hikers", target: T, reason: "harassment", note: "  it keeps happening  " };

  const built = gr.groupReportText({ ...base, items: [item(1, 'He said "no"\\ then\nleft \u{1F600}')] });
  assert.ok(built.text, built.error);
  const want = `[[hum:group-report:v1]]{"group_id":"${G}","group_name":"Hikers","target":"${T}","reason":"harassment","note":"it keeps happening",`
    + `"items":[{"id":"${item(1).id}","from":"${T}","ts":${T0 + 1},"text":${JSON.stringify('He said "no"\\ then\nleft \u{1F600}')}}]}`;
  assert.equal(built.text, want, "the marker, then the JSON exactly in 10j's order");
  assert.deepEqual(gr.groupReportParse(built.text), { ...base, note: "it keeps happening", items: [item(1, 'He said "no"\\ then\nleft \u{1F600}')] }, "and it reads back");
  assert.equal(gr.isGroupReportText("[[hum:contact-request:v1]]{}"), false);

  // 20 items pass; one more does not, built or read.
  const twenty = Array.from({ length: 20 }, (_, i) => item(i + 1));
  assert.ok(gr.groupReportText({ ...base, items: twenty }).text, "20 items pass");
  const tooMany = gr.groupReportText({ ...base, items: [...twenty, item(21)] });
  assert.ok(!tooMany.text && /At most 20/.test(tooMany.error), "21 items are refused");
  const twentyOne = gr.GROUP_REPORT_MARKER + JSON.stringify({ ...base, note: "", items: [...twenty, item(21)] });
  assert.equal(gr.groupReportParse(twentyOne), null, "and a received report with 21 is not read");

  // 16 KB of text exactly passes; one byte more does not.
  const overhead = gr.groupReportText({ ...base, note: "", items: [item(1, "")] }).text.length;
  const fill = (n) => gr.groupReportText({ ...base, note: "", items: [item(1, "x".repeat(n))] });
  const exact = fill(16384 - overhead);
  assert.equal(Buffer.byteLength(exact.text, "utf8"), 16384, "a report of exactly 16 KB");
  assert.ok(exact.text, "16 KB passes");
  const over = fill(16384 - overhead + 1);
  assert.ok(!over.text && /16 KB/.test(over.error), "16 KB and one byte is refused");
  assert.equal(gr.groupReportParse(gr.GROUP_REPORT_MARKER + JSON.stringify({ ...base, note: "", items: [item(1, "x".repeat(16384 - overhead + 1))] })), null, "and is not read");
  // Bytes, not characters: a 4-byte emoji counts four.
  const emoji = gr.groupReportText({ ...base, note: "", items: [item(1, "\u{1F600}".repeat(Math.ceil((16384 - overhead) / 4) + 1))] });
  assert.ok(!emoji.text, "the limit counts UTF-8 bytes");

  // A file: never an item, never in a built report, never read.
  assert.equal(gr.groupReportItem(item(1).id, T, T0, FILE_TEXT), null, "a group message with a file is never an item");
  assert.equal(gr.groupReportItem(item(1).id, T, T0, "see " + FILE_TEXT.replace("v1", "v2")), null, "nor with another version of the marker");
  const forced = gr.groupReportText({ ...base, items: [item(1, FILE_TEXT)] });
  assert.ok(!forced.text && forced.error === report.REPORT_NO_FILES, "the builder refuses a file item put there by hand");
  const inNote = gr.groupReportText({ ...base, note: "look " + FILE_TEXT, items: [] });
  assert.ok(!inNote.text && inNote.error === report.REPORT_NO_FILES, "and a file in the note");
  assert.equal(gr.groupReportParse(gr.GROUP_REPORT_MARKER + JSON.stringify({ ...base, note: "", items: [item(1, FILE_TEXT)] })), null, "a received report with a file is not read");

  // What a report must name.
  assert.ok(gr.groupReportText({ ...base, group_id: "nope", items: [] }).error);
  assert.ok(gr.groupReportText({ ...base, target: "Ben", items: [] }).error);
  assert.ok(gr.groupReportText({ ...base, reason: "", items: [] }).error);
  assert.ok(gr.groupReportText({ ...base, items: [{ from: T, ts: 1, text: "no id" }] }).error, "an item without its id");
  void A;
});

// ── 4. The creator's checks, over real signed objects ───────────────────

test("the creator's checks against their own copy: found, not found, altered, wrong sender, wrong time, bad signature", async () => {
  const fx = await fixture();
  const { cy, ann, m1, G } = fx;
  const real = await fx.m.obj.verifyObjectSubmission(fx.forged, { blake3: fx.m.blake3, pqVerify: fx.m.verify });
  assert.equal(real.ok, false, "the stored forgery's signature does not check");
  const rep = {
    group_id: G, group_name: "Hikers", target: cy.key, reason: "threats", note: "",
    items: [
      { id: m1.objectId, from: cy.key, ts: T0 + 500, text: "you are all idiots" },          // found
      { id: "ef".repeat(32), from: cy.key, ts: T0 + 500, text: "you are all idiots" },       // not in the copy
      { id: m1.objectId, from: cy.key, ts: T0 + 500, text: "you are all IDIOTS!!" },         // altered text
      { id: m1.objectId, from: ann.key, ts: T0 + 500, text: "you are all idiots" },          // wrong sender
      { id: m1.objectId, from: cy.key, ts: T0 + 501, text: "you are all idiots" },           // wrong time
      { id: fx.forgedId, from: cy.key, ts: T0 + 800, text: "I will hurt you" },              // signature does not check
    ],
  };
  const out = await check(fx, rep, fx.log);
  assert.deepEqual(out.map((it) => it.found), [true, false, false, false, false, false],
    "found; not found; altered text; wrong sender; wrong time; bad signature");
  assert.equal(out[0].signer, cy.key, "a found item names who signed it");
  assert.deepEqual(gr.groupReportItemBadge(out[0], "Cy"), { found: true, text: "Found in your copy of the group, signed by Cy" });
  assert.deepEqual(gr.groupReportItemBadge(out[1], "Cy"), { found: false, text: "Not found in your copy" });
  // A server that puts Cy's id on Ann's bytes: the id is the hash of the bytes, so it is not Cy's.
  const relabelled = await check(fx, { ...rep, items: [rep.items[0]] }, [fx.mislabelled]);
  assert.equal(relabelled[0].found, false, "an id the server put on other bytes is not found");
  // Even on Cy's own identical message: the item names one message, and this is another.
  const twin = await check(fx, { ...rep, items: [rep.items[0]] }, [fx.twinRelabelled]);
  assert.equal(twin[0].found, false, "an id the server put on a twin message is not found");
  // Another group's message is not this group's.
  const elsewhere = await check(fx, { ...rep, group_id: "12".repeat(32), items: [rep.items[0]] }, fx.log);
  assert.equal(elsewhere[0].found, false, "a message of another group is not found in this one");
  // No copy at all: every item shown, none found, none dropped.
  assert.deepEqual((await check(fx, rep, [])).map((it) => it.found), [false, false, false, false, false, false]);
});

// ── 5. Which reports the creator keeps ──────────────────────────────────

test("a report is kept only about a group I hold as its creator, from someone in it, to me, not about me", () => {
  const ME = "a1".repeat(32), ANN = "b2".repeat(32), CY = "d4".repeat(32), DAN = "e5".repeat(32);
  const G = "ab".repeat(32);
  const groups = [{ group_id: G, name: "Hikers", members: [ME, ANN, CY], is_creator: true }];
  const rep = { group_id: G, group_name: "Hikers", target: CY, reason: "spam", note: "", items: [] };
  const verdict = (over) => gr.groupReportAccepts({ me: ME, from: ANN, to: ME, report: rep, groups, ...over });
  assert.equal(verdict({}).ok, true, "kept");
  assert.equal(verdict({}).group.name, "Hikers");
  assert.equal(verdict({ report: { ...rep, group_id: "cd".repeat(32) } }).why, "not_held", "a group I do not hold");
  assert.equal(verdict({ groups: [] }).why, "not_held", "no groups at all");
  assert.equal(verdict({ groups: [{ ...groups[0], is_creator: false }] }).why, "not_creator", "a group I am in but did not create");
  assert.equal(verdict({ from: DAN }).why, "reporter_not_in_group", "from someone not in it");
  assert.equal(verdict({ to: ANN }).why, "not_to_me", "addressed to someone else");
  assert.equal(verdict({ report: { ...rep, target: ME } }).why, "about_me", "about me");
  assert.equal(gr.groupReportCount([{ group_id: G }, { group_id: G }, { group_id: "cd".repeat(32) }], G), 2, "the count on the group");
});

// ── 6. The reporter, in the page ────────────────────────────────────────

test("the reporter: Send this report to, the creator by default, one sealed flagged DM with exactly the marker", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann, { groups: GROUP_AS(fx, fx.ann, false) });
  const { sock, fn, el, dialog, posted } = page;
  await reportFromGroup(page, fx.cy);
  const st = dialog();
  assert.equal(st.context, "group");
  assert.equal(st.target, fx.cy.key);
  assert.deepEqual(JSON.parse(JSON.stringify(st.group)), { id: fx.G, name: "Hikers" }, "the dialog knows the group");
  assert.equal(st.creator, fx.ben.key, "the creator, from the group's own signed record");
  assert.deepEqual(JSON.parse(JSON.stringify(st.groupItem)), { id: fx.m1.objectId, from: fx.cy.key, ts: T0 + 500, text: "you are all idiots" },
    "the item names the message by its signed-object id");
  assert.equal(st.sendTo, "creator", "the creator is the default");
  let card = el("report-card").innerHTML;
  assert.ok(card.includes(html(gr.GROUP_REPORT_SEND_TO)), "Send this report to");
  for (const d of ["creator", "admins", "both"]) assert.ok(card.includes(`data-report-send-to="${d}"`) && card.includes(html(gr.GROUP_REPORT_DESTINATION_LABELS[d])), `offers ${d}`);
  assert.ok(/data-report-send-to="creator" checked/.test(card), "the creator ticked");
  assert.ok(card.includes(html(gr.GROUP_REPORT_CREATOR_SEES)), "The group's creator will see that you sent this.");
  assert.ok(card.includes(html("This goes to the group's creator.")), "the first line says who reads it");

  fn("reportDialogChoose")("harassment");
  fn("reportDialogSetNote")("he keeps doing this");
  assert.equal(await fn("submitReportDialog")(), true);
  await settle();
  const puts = sock.sent.filter((m) => m.type === "dm_put");
  assert.equal(puts.length, 1, "one dm_put: no self-copy");
  const [put] = puts;
  assert.equal(put.to, fx.ben.key, "to the group's creator");
  assert.equal(put.group_report, true, "flagged group_report");
  assert.deepEqual(Object.keys(put), ["type", "to", "content", "group_report"]);
  assert.equal(sock.sent.filter((m) => m.type === "report_v2").length, 0, "nothing to the admins");
  const inner = unsealed(put);
  assert.equal(inner.from, fx.ann.key);
  assert.equal(inner.to, fx.ben.key);
  const want = `[[hum:group-report:v1]]{"group_id":"${fx.G}","group_name":"Hikers","target":"${fx.cy.key}","reason":"harassment","note":"he keeps doing this",`
    + `"items":[{"id":"${fx.m1.objectId}","from":"${fx.cy.key}","ts":${T0 + 500},"text":"you are all idiots"}]}`;
  assert.equal(inner.text, want, "the sealed inner text is exactly the marker and the JSON");
  assert.equal(inner.sig, b64(`hum/dm/v2\n${fx.ann.key}\n${fx.ben.key}\n${inner.ts}\n${want}`), "signed as any DM");
  assert.ok(!sock.raw.some((s) => s.includes("group-report")), "nothing readable travels: only the sealed envelope");
  assert.ok(shown(page.state.appended).includes(gr.GROUP_REPORT_SENT_LINE), "Report sent to the group's creator.");
  assert.equal(dialog(), null, "the dialog closes");
  assert.deepEqual(posted, [], "nothing posted as a group object");

  // The creator's server refuses it: told about the report, not offered a contact request.
  page.state.appended.length = 0;
  await page.handle({ type: "reach_refused", kind: "message", to: fx.ben.key });
  assert.ok(shown(page.state.appended).includes(gr.GROUP_REPORT_REFUSED_LINE), "the report did not reach the creator");
  assert.ok(!page.state.appended.some((e) => String(e.className).includes("reach-refused")), "and no contact request is offered for it");
  // An ordinary refusal later is the contact-request offer again.
  await page.handle({ type: "reach_refused", kind: "message", to: fx.ben.key });
  assert.ok(page.state.appended.some((e) => String(e.className).includes("reach-refused")), "an ordinary refusal is unchanged");
});

test("the reporter: Both sends both, the admins alone send report_v2 only", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann, { groups: GROUP_AS(fx, fx.ann, false) });
  const { sock, fn, el, dialog } = page;
  await reportFromGroup(page, fx.cy);
  assert.equal(fn("reportDialogSendTo")("both"), true);
  assert.ok(el("report-card").innerHTML.includes(html(gr.GROUP_REPORT_CREATOR_SEES)), "Both: the creator will see it");
  assert.ok(el("report-card").innerHTML.includes(html(report.REPORT_GROUP_UNPROVEN)), "and the admins' line about group words");
  fn("reportDialogChoose")("spam");
  assert.equal(await fn("submitReportDialog")(), true);
  await settle();
  assert.deepEqual(sock.sent.map((m) => m.type), ["dm_put", "report_v2"], "both");
  assert.equal(sock.sent[0].group_report, true);
  const v2 = sock.sent[1];
  assert.equal(v2.context, "group");
  assert.deepEqual(v2.evidence, [{ kind: "group_text", from: fx.cy.key, ts: T0 + 500, text: "you are all idiots" }], "the admins get the words seen, unproven");

  sock.sent.length = 0;
  await reportFromGroup(page, fx.cy);
  assert.equal(fn("reportDialogSendTo")("admins"), true);
  const card = el("report-card").innerHTML;
  assert.ok(!card.includes(html(gr.GROUP_REPORT_CREATOR_SEES)), "the admins alone: no line about the creator");
  assert.ok(card.includes(html("This goes to this server's admins and moderators.")));
  fn("reportDialogChoose")("spam");
  assert.equal(await fn("submitReportDialog")(), true);
  await settle();
  assert.deepEqual(sock.sent.map((m) => m.type), ["report_v2"], "only the admins");
  assert.equal(fn("reportDialogSendTo")("creator"), false, "nothing to choose with no dialog open");
  void dialog;
});

test("the reporter: only the admins when I created the group, or the person reported did", async () => {
  const fx = await fixture();
  // Ann reports Ben's message: Ben created the group.
  const ann = await loadChat(fx.ann, { groups: GROUP_AS(fx, fx.ann, false) });
  await reportFromGroup(ann, fx.ben);
  assert.deepEqual(Array.from(JSON.parse(JSON.stringify(ann.fn("reportGroupChoices")(ann.dialog()))).choices), ["admins"]);
  assert.equal(ann.dialog().sendTo, "admins");
  let card = ann.el("report-card").innerHTML;
  assert.ok(card.includes(html(gr.GROUP_REPORT_THEY_CREATED)), "The person you are reporting created this group...");
  assert.ok(!card.includes('data-report-send-to="creator"') && !card.includes(html(gr.GROUP_REPORT_CREATOR_SEES)), "no creator to choose");
  assert.equal(ann.fn("reportDialogSendTo")("creator"), false, "and it cannot be chosen");
  ann.fn("reportDialogChoose")("spam");
  assert.equal(await ann.fn("submitReportDialog")(), true);
  await settle();
  assert.deepEqual(ann.sock.sent.map((m) => m.type), ["report_v2"], "it goes to the admins");

  // Ben, the creator, reports Cy's message.
  const ben = await loadChat(fx.ben, { groups: GROUP_AS(fx, fx.ben, true) });
  await reportFromGroup(ben, fx.cy);
  assert.equal(ben.dialog().sendTo, "admins");
  card = ben.el("report-card").innerHTML;
  assert.ok(card.includes(html(gr.GROUP_REPORT_YOU_CREATED)), "You created this group: remove them...");
  assert.ok(!card.includes('data-report-send-to="creator"'), "no creator to choose");
});

test("the reporter: a group message with a file sends no item to the creator", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann, { groups: GROUP_AS(fx, fx.ann, false) });
  const { fn, sock, dialog } = page;
  await fn("openReportDialog")({ target: fx.cy.key, name: "Cy", context: "group",
    message: { timestamp: T0 + 900, text: FILE_TEXT, group: true, groupId: fx.G, groupName: "Hikers", id: "aa".repeat(32) } });
  await waitFor(() => dialog() && dialog().finding === false && dialog().reasons, "the dialog is ready");
  assert.equal(dialog().groupItem, null, "a message with a file is no item");
  fn("reportDialogChoose")("other");
  fn("reportDialogSetNote")("they posted something awful");
  assert.equal(await fn("submitReportDialog")(), true);
  await settle();
  const [put] = sock.sent.filter((m) => m.type === "dm_put");
  assert.ok(put && put.group_report === true, "the report goes to the creator");
  const text = unsealed(put).text;
  assert.deepEqual(gr.groupReportParse(text).items, [], "no item");
  assert.ok(!text.includes("hum:file") && !sock.raw.some((s) => s.includes("SECRET")), "the file's marker and key go nowhere");
});

// ── 7 and 8. The creator, in the page ───────────────────────────────────

/** Ann's report to Ben about Cy, as Ann's client sends it, with the items given. */
function reportText(fx, items, over = {}) {
  return gr.groupReportText({ group_id: fx.G, group_name: "Totally not Hikers", target: fx.cy.key, reason: "threats", note: "please look", items, ...over }).text;
}

test("the creator: checked against my copy, listed with a count on the group, never a message, never sent anywhere", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ben, { groups: GROUP_AS(fx, fx.ben, true) });
  const { handle, store, sock, fn, el, posted, fetched, state } = page;
  const items = [
    { id: fx.m1.objectId, from: fx.cy.key, ts: T0 + 500, text: "you are all idiots" },
    { id: fx.m1.objectId, from: fx.cy.key, ts: T0 + 500, text: "I know where you live" },
    // The forgery the server holds, whose signature does not check.
    { id: fx.forgedId, from: fx.cy.key, ts: T0 + 800, text: "I will hurt you" },
  ];
  const text = reportText(fx, items);
  fetched.length = 0;
  await handle({ type: "dm_new", id: 11, content: envelope(fx.ann.key, fx.ben.key, text, T0 + 5000) });
  const list = store.groupReportList();
  assert.equal(list.length, 1, "kept, although Ann is not my friend (the reach screen comes after)");
  const rec = list[0];
  assert.deepEqual([rec.from, rec.target, rec.group_id, rec.reason, rec.note], [fx.ann.key, fx.cy.key, fx.G, "threats", "please look"]);
  assert.equal(rec.group_name, "Hikers", "the group's name as my own list has it, not as the report claims");
  assert.deepEqual(rec.items.map((it) => [it.found, it.signer]), [[true, fx.cy.key], [false, ""], [false, ""]],
    "found; altered text not found; the forgery's signature does not check in my copy");
  assert.equal(store.conversation(fx.ann.key).length, 0, "never stored as a message");
  assert.deepEqual(Object.keys(store.contactRequests), [], "and not turned into a contact request");
  assert.deepEqual(sock.sent, [], "nothing sent to the server");
  assert.deepEqual(posted, [], "nothing posted");
  assert.ok(fetched.every((f) => f.startsWith("GET ")), "only reads: " + fetched.join(", "));
  assert.ok(shown(state.appended).includes(html(gr.groupReportArrivedLine("Ann", "Cy", "Hikers"))) || shown(state.appended).includes(gr.groupReportArrivedLine("Ann", "Cy", "Hikers")), "I am told");

  // Settings > Safety.
  fn("openSafetyPanel")();
  const safety = el("safety-card").innerHTML;
  assert.ok(safety.includes(html(gr.GROUP_REPORTS_TITLE)), "Reports about your groups");
  assert.ok(safety.includes("Cy: Threats of violence"), "the person reported and the reason's label");
  assert.ok(safety.includes("In Hikers") && safety.includes("reported by Ann"), "the group and who reported");
  assert.ok(safety.includes("please look"), "the note");
  assert.ok(safety.includes(html("Found in your copy of the group, signed by Cy")), "the found badge");
  assert.ok(safety.includes(html("Not found in your copy")), "the not-found badge, shown, not dropped");
  for (const a of Object.values(gr.GROUP_REPORT_ACTION_LABELS)) assert.ok(safety.includes(html(a)), `the action ${a}`);
  // The count on the group.
  fn("renderGroupList")();
  const groupsTab = el("tab-groups").innerHTML;
  assert.ok(/data-group-reports="[0-9a-f]{64}"[^>]*>1<\/span>/.test(groupsTab), "a count of 1 on the group");
  assert.ok(groupsTab.includes(html(gr.groupReportCountTitle(1))));

  // The same report handed over again (the mailbox, a refetch): kept once.
  await handle({ type: "dm_batch", messages: [{ id: 11, content: envelope(fx.ann.key, fx.ben.key, text, T0 + 5000) }], done: true });
  assert.equal(store.groupReportList().length, 1, "kept once");

  // Kept on this device: a reload finds it.
  const again = await loadChat(fx.ben, { groups: GROUP_AS(fx, fx.ben, true), indexedDB: page.idb, storeKey: await page.ctx.getDmStoreKey() });
  assert.equal(again.store.groupReportList().length, 1, "the local store keeps it across a reload");
  assert.equal(again.store.groupReportList()[0].items[0].found, true);
});

test("the creator: a report about a group not held, from someone not in it, or about me is dropped", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ben, { groups: GROUP_AS(fx, fx.ben, true) });
  const { handle, store, sock } = page;
  const item = [{ id: fx.m1.objectId, from: fx.cy.key, ts: T0 + 500, text: "you are all idiots" }];
  // A group I do not hold.
  await handle({ type: "dm_new", id: 21, content: envelope(fx.ann.key, fx.ben.key, reportText(fx, item, { group_id: "12".repeat(32) }), T0 + 1) });
  assert.equal(store.groupReportList().length, 0, "a group I do not hold");
  // From Dan, who is not in it.
  await handle({ type: "dm_new", id: 22, content: envelope(fx.dan.key, fx.ben.key, reportText(fx, item), T0 + 2) });
  assert.equal(store.groupReportList().length, 0, "from someone not in it");
  // About me.
  await handle({ type: "dm_new", id: 23, content: envelope(fx.ann.key, fx.ben.key, reportText(fx, [], { target: fx.ben.key }), T0 + 3) });
  assert.equal(store.groupReportList().length, 0, "about me");
  // Not the creator: Ben's list says he only joined it.
  page.relay.groups = GROUP_AS(fx, fx.ben, false);
  await handle({ type: "dm_new", id: 24, content: envelope(fx.ann.key, fx.ben.key, reportText(fx, item), T0 + 4) });
  assert.equal(store.groupReportList().length, 0, "a group I did not create");
  // None of them became a message or a request, and nothing was sent.
  assert.equal(store.conversation(fx.ann.key).length + store.conversation(fx.dan.key).length, 0);
  assert.deepEqual(Object.keys(store.contactRequests), []);
  assert.deepEqual(sock.sent, []);
  // The same report from Ann about the group I hold is kept.
  page.relay.groups = GROUP_AS(fx, fx.ben, true);
  await handle({ type: "dm_new", id: 25, content: envelope(fx.ann.key, fx.ben.key, reportText(fx, item), T0 + 5) });
  assert.equal(store.groupReportList().length, 1, "kept");
});

test("the creator's three actions: Remove them from the group, Block them, Dismiss", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ben, { groups: GROUP_AS(fx, fx.ben, true) });
  const { handle, store, fn, el, posted, confirms } = page;
  const { obj, blake3, verify, cbor } = fx.m;
  await handle({ type: "dm_new", id: 31, content: envelope(fx.ann.key, fx.ben.key, reportText(fx, [{ id: fx.m1.objectId, from: fx.cy.key, ts: T0 + 500, text: "you are all idiots" }]), T0 + 9) });
  const [rec] = store.groupReportList();
  assert.ok(rec, "kept");

  // Remove them from the group, while the new key cannot be made: no one is removed.
  page.relay.membersDown = true;
  assert.equal(await fn("groupReportRemove")(rec.id), false, "a key that cannot be made removes no one");
  await settle();
  assert.deepEqual(posted, [], "nothing posted when the new key cannot be made");
  assert.ok(shown(page.state.appended).includes("Could not remove Cy from Hikers: a new group key could not be made."), "and it says so");
  assert.equal(store.groupReportList()[0].removed, false);
  page.relay.membersDown = false;

  // Remove them from the group.
  confirms.length = 0;
  assert.equal(await fn("groupReportRemove")(rec.id), true);
  await settle();
  assert.equal(confirms.length, 1, "confirmed first");
  assert.ok(confirms[0].includes("Cy") && confirms[0].includes("Hikers"));
  assert.deepEqual(posted.map((p) => p.object_type), ["group_epoch_key_v1", "group_member_v1"], "a new group key, then the remove");
  const removal = await obj.verifyObjectSubmission(posted[1], { blake3, pqVerify: verify });
  assert.ok(removal.ok, "signed by me, for real");
  assert.equal(removal.authorPubHex, fx.ben.key);
  assert.deepEqual(removal.references, [fx.G]);
  const what = cbor.decodeCanonicalCbor(removal.payload);
  assert.equal(what.action, "remove");
  assert.equal(hex(what.subject), fx.cy.key, "with Cy as the subject");
  const rekey = await obj.verifyObjectSubmission(posted[0], { blake3, pqVerify: verify });
  assert.ok(rekey.ok && rekey.authorPubHex === fx.ben.key, "the new key, signed by me");
  const sealedTo = obj.parseGroupEpochKeyPayload(rekey.payload);
  assert.equal(sealedTo.epoch, 2, "the next epoch");
  assert.deepEqual(sealedTo.recipients.map((r) => r.fp).sort(), [fx.fp(fx.ann.key), fx.fp(fx.ben.key)].sort(),
    "sealed to everyone but Cy, though the server still lists Cy");
  assert.equal(store.groupReportList()[0].removed, true);
  fn("openSafetyPanel")();
  let safety = el("safety-card").innerHTML;
  assert.ok(safety.includes(html(gr.GROUP_REPORT_REMOVED)), "Removed from the group.");
  assert.ok(!safety.includes(`data-group-report-remove=`), "and no second Remove");

  // Block them.
  assert.equal(await fn("groupReportBlock")(rec.id), true);
  await settle();
  assert.equal(store.isBlocked(fx.cy.key), true, "Cy is blocked");
  safety = el("safety-card").innerHTML;
  assert.ok(safety.includes(html(gr.GROUP_REPORT_BLOCKED)), "You have blocked them.");
  assert.ok(!safety.includes(`data-group-report-block=`), "and no second Block");
  assert.equal(store.groupReportList().length, 1, "the report stays until dismissed");

  // Dismiss.
  assert.equal(fn("groupReportDismiss")(rec.id), true);
  await settle();
  assert.equal(store.groupReportList().length, 0, "gone from this device");
  safety = el("safety-card").innerHTML;
  assert.ok(safety.includes(html(gr.GROUP_REPORTS_NONE)), "No reports about your groups.");
  fn("renderGroupList")();
  assert.ok(!el("tab-groups").innerHTML.includes("data-group-reports="), "and no count on the group");
  const reloaded = await loadChat(fx.ben, { groups: GROUP_AS(fx, fx.ben, true), indexedDB: page.idb, storeKey: await page.ctx.getDmStoreKey() });
  assert.equal(reloaded.store.groupReportList().length, 0, "dismissed for good");
});

// Red first, 2026-10-10. Each break made alone in a fresh copy of web/, this test run against it
// with HOS_WEB_DIR, and seen failing with the assertion named (passing again on the real web/):
//  group-report.js
//   - groupReportChoices without the "I created it" case: "I created the group: only the admins"
//     (and the page's admins-only test); without the "they created it" case: "they created the
//     group: only the admins"; choosing 'admins' by default: "all three, the creator by default",
//     "the creator is the default".
//   - groupReportText writing `target` before `group_name`: "the marker, then the JSON exactly in
//     10j's order" and, in the page, "the sealed inner text is exactly the marker and the JSON".
//   - the 21-item refusal off by one: "21 items are refused"; in groupReportParse: "and a received
//     report with 21 is not read". The 16 KB limit off by one: "16 KB and one byte is refused";
//     counted in characters, not bytes: "the limit counts UTF-8 bytes".
//   - groupReportItem without the file rule: "a group message with a file is never an item" (and
//     the page's "a message with a file is no item"); groupReportText without it: "the builder
//     refuses a file item put there by hand".
//   - groupReportCheck not comparing the text (only that it opened): "found; not found; altered
//     text; wrong sender; wrong time; bad signature" (and the page's list); not comparing the
//     sender: the same; not comparing the time: the same; not requiring the id to be the hash of
//     the bytes: "an id the server put on a twin message is not found".
//   - groupReportAccepts finding any group I created instead of the one named: "a group I do not
//     hold"; without the member check: "from someone not in it"; without the creator check: "a
//     group I am in but did not create" (and the page's "a group I did not create"); without the
//     about-me check: "about me".
//  crypto.js: pqBuildGroupReport not setting group_report: "flagged group_report".
//  app.js: the menu not taking the row's object id: "the item names the message by its
//    signed-object id"; ingestGroupReport removed from dm_new (so the reach screen takes it as a
//    request): "kept, although Ann is not my friend (the reach screen comes after)".
//  chat-groups-p2p.js: the row not given its object id: "never happened: the group's messages are
//    drawn"; the creator's copy checked without the signature (pqVerify always true): "found;
//    altered text not found; the forgery's signature does not check in my copy"; the new key
//    skipped: "a key that cannot be made removes no one"; its failure ignored: the same; the
//    remove posted before the key: "nothing posted when the new key cannot be made"; the new key
//    sealed to the person removed too: "sealed to everyone but Cy, though the server still lists
//    Cy".
//  chat-reports.js: a self-copy sent with the report: "one dm_put: no self-copy"; Both sending only
//    to the creator: "both"; the line before sending left out: "The group's creator will see that
//    you sent this."; the refusal not caught: "the report did not reach the creator"; the group's
//    name taken from the report: "the group's name as my own list has it, not as the report
//    claims"; Remove naming the reporter: "with Cy as the subject"; Block naming the reporter: "Cy
//    is blocked"; Dismiss keeping the report: "gone from this device".
//  chat-dm-store.js: the reports left out of what is saved: "the local store keeps it across a
//    reload".
//  chat-social.js: the count left off the group: "a count of 1 on the group".
// One break was not caught by the pure test alone, and why: groupReportCheck without its
// `v.ok` test still finds nothing for a bad signature, because pq-object.js's verifier returns no
// object id for one; the signature check is proven in the page instead (above).
