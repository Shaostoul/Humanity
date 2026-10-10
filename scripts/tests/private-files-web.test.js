// Files in private conversations are encrypted too, in the web chat client (10k, 2026-10-10,
// docs/design/blocking-and-safe-mode.md). The desktop app is held to the same section separately.
//
// Run: node --test scripts/tests/private-files-web.test.js   (in `just rig-tests`)
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with the
// DOM replaced by a stub that keeps what is written to it and the listeners a script adds (as in
// group-report-web.test.js and protected-web.test.js): the real bip39-english.js, events.js,
// friend-pass.js, reach.js, block.js, report.js, group-report.js, warnings.js, protected.js,
// crypto.js, chat-dm-store.js (over a stand-in IndexedDB), app.js, chat-messages.js, chat-dms.js,
// chat-social.js, chat-groups-p2p.js, chat-ui.js, chat-voice-rooms.js, chat-voice-calls.js,
// chat-profile.js, chat-privacy.js, chat-reports.js, chat-warnings.js, chat-protected.js,
// chat-onboarding.js and chat-p2p.js, in index.html's order.
//
// What is REAL: the file encryption (WebCrypto AES-256-GCM, the page's own pqEncryptFile and
// pqDecryptFile), the group's signed objects and its messages' encryption (pq-object.js and the
// vendored noble bundle, with real keys for Ann, Ben, Cy and Dan; a group message the page sends
// is opened here with the group's key to read what it carried), the recovery-phrase guard (from a
// fixed seed through bip39-english.js) and the protected setup's PIN. The files are real `File`s
// and the uploads real `FormData`, so the bytes a test reads are the bytes that would travel. What
// is stood in: the DM layer (a DM's "signature" is the signed words and "sealing" base64s the
// plaintext under the recipient's key name, so a test reads exactly what was sealed), and the
// server, which keeps each upload's bytes and serves them back at the address it answered.
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves (10k's Proof list, the web items, and the rules around them):
//  1. A pasted image in a DM is encrypted on the device, uploaded as ciphertext with
//     `encrypted=1`, and sent sealed as a [[hum:file:v1]] marker whose key opens the upload back to
//     the picture; no plain upload, no public post, and its address is in no frame in the clear.
//  2. A picked file in a P2P group: the same, with the marker inside the group's encrypted message
//     (opened here with the group's key), and my own copy drawn as a card, never as its text.
//  3. A pasted image in a P2P group: the same, and my own copy is shown inline.
//  4. A public channel still uploads the plain file (its own bytes, no `encrypted=1`) and posts
//     its address.
//  5. A marker in a group message is drawn as a card, never as its text: a friend's picture is
//     fetched, opened and shown inline at once; a picture from someone who is not a friend waits
//     for "Image (click to load)" (nothing is fetched before the click), in a group and in a DM;
//     any other file is a card with its name, size and Save, opened only when Save is pressed.
//  6. A tampered key, altered ciphertext, a missing upload and an address off this server each
//     give "This file could not be opened.", never the ciphertext, and the last is never fetched.
//  7. With the protected setup on, a group file from someone who is not a friend is not shown at
//     all (the preset's line) and never fetched; a friend's still is.
//  8. The guards stay on every encrypted path: a file whose name holds four words of my recovery
//     phrase is not uploaded or sent, nor is one over 6 MB (in a DM and in a group).
//  9. A file whose conversation is left while it uploads is not sent anywhere: a group's marker
//     never lands in the public channel opened meanwhile, and the send path refuses a marker in a
//     public channel whatever the timing.
//
// Red first: see the end of this file for each deliberate break and the assertion it tripped.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const P = require(path.join(WEB, "shared", "protected.js"));
const W = require(path.join(WEB, "shared", "warnings.js"));
const report = require(path.join(WEB, "shared", "report.js"));

const SPEC = fs.readFileSync(path.join(ROOT, "docs", "design", "blocking-and-safe-mode.md"), "utf8");
// 10k with its line wrapping undone (a sentence may wrap in the source).
const TEN_K = SPEC.slice(SPEC.indexOf("## 10k."), SPEC.indexOf("## 11.")).replace(/\s+/g, " ");
const SHIPPED_PRESETS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "gui", "safety_presets.json"), "utf8"));
const SHIPPED_WARNINGS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "warnings.json"), "utf8"));
const SHIPPED_REASONS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "report_reasons.json"), "utf8"));
const PRESET = P.protectedPresetFrom(SHIPPED_PRESETS);
const clone = (x) => JSON.parse(JSON.stringify(x));

const hex = (u8) => Buffer.from(u8).toString("hex");
const b64 = (s) => Buffer.from(s, "utf8").toString("base64");
const unb64 = (s) => Buffer.from(s, "base64").toString("utf8");
const escHtml = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const sameBytes = (a, b) => Buffer.from(a).equals(Buffer.from(b));

const FILE_MARKER = "[[hum:file:v1]]";
const FAILED = "This file could not be opened.";
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const T0 = 1760000000000;
const SEED = Uint8Array.from({ length: 32 }, (_, i) => (i * 7 + 3) & 0xff);
const PIN = "90817263";
const MAY = "invite,message,trade,voice_message";
const CHANNELS = [
  { id: "general", name: "general", read_only: false },
  { id: "announcements", name: "announcements", read_only: true },
];

// Some bytes that are plainly a PNG, and some that are plainly a PDF.
const PNG = Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, ...Buffer.from("a picture of a dog on a beach, pixel by pixel")]);
const PDF = Uint8Array.from(Buffer.from("%PDF-1.7 the meeting notes for Saturday, page one"));
const picture = (name = "image.png", bytes = PNG) => new File([bytes], name, { type: "image/png" });
const pdf = (name = "notes.pdf", bytes = PDF) => new File([bytes], name, { type: "application/pdf" });

/** Encrypt as 10k says (a fresh AES-256-GCM key, a 12-byte nonce), here, with Node's own WebCrypto. */
async function seal(bytes) {
  const raw = globalThis.crypto.getRandomValues(new Uint8Array(32));
  const iv = globalThis.crypto.getRandomValues(new Uint8Array(12));
  const key = await globalThis.crypto.subtle.importKey("raw", raw, { name: "AES-GCM" }, false, ["encrypt"]);
  const ct = new Uint8Array(await globalThis.crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, bytes));
  return { ct, k: Buffer.from(raw).toString("base64"), n: Buffer.from(iv).toString("base64") };
}
async function unseal(ct, k, n) {
  const key = await globalThis.crypto.subtle.importKey("raw", Buffer.from(k, "base64"), { name: "AES-GCM" }, false, ["decrypt"]);
  return new Uint8Array(await globalThis.crypto.subtle.decrypt({ name: "AES-GCM", iv: Buffer.from(n, "base64") }, key, ct));
}
/** The marker text, built the way both clients build it (the marker, then base64 of the JSON). */
const markerText = (meta) => FILE_MARKER + Buffer.from(JSON.stringify(meta), "utf8").toString("base64");
const markerMeta = (text) => (typeof text === "string" && text.startsWith(FILE_MARKER) ? JSON.parse(unb64(text.slice(FILE_MARKER.length))) : null);

// ── The real group modules and the fixture (as group-report-web.test.js) ───

let MODS = null;
async function modules() {
  if (MODS) return MODS;
  const asUrl = (src) => "data:text/javascript;base64," + Buffer.from(src).toString("base64");
  const cborUrl = asUrl(fs.readFileSync(path.join(WEB, "shared", "canonical-cbor.js"), "utf8"));
  const objSrc = fs.readFileSync(path.join(WEB, "shared", "pq-object.js"), "utf8")
    .replace(/(['"])\.\/canonical-cbor\.js\1/, JSON.stringify(cborUrl));
  const obj = await import(asUrl(objSrc));
  const noble = await import(asUrl(fs.readFileSync(path.join(WEB, "shared", "vendor", "noble-pq.bundle.js"))));
  const blake3 = (b) => noble.blake3.create({ dkLen: 32 }).update(b).digest();
  MODS = { obj, noble, blake3 };
  return MODS;
}

const kyberOf = (k) => "kyber-" + k.slice(0, 16);

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
  const dmSeal = async (pub, plaintext) => ({ ek_ct_b64: b64(pub), nonce_b64: "AAAA", ct_b64: b64(plaintext) });
  // Ben's group; its key (epoch 1) sealed to Ann, Ben and Cy.
  const group = await obj.buildGroupV1({ name: "Hikers", authorPublicKey: ben.pub, sign: signer(ben), blake3, createdAt: T0 });
  const G = group.objectId;
  const K = new Uint8Array(32).fill(7);
  const epoch = await obj.buildGroupEpochKeyV1({
    groupId: G, epoch: 1, epochKey: K,
    members: [ann, ben, cy].map((p) => ({ fp: fp(p.key), kyber_public: kyberOf(p.key) })),
    seal: dmSeal, authorPublicKey: ben.pub, sign: signer(ben), blake3, createdAt: T0 + 100,
  });
  /** A message in the group from `p`, served the way the server lists it. */
  const groupMsg = async (p, text, at) => {
    const built = await obj.buildGroupMsgV1({ groupId: G, epoch: 1, epochKey: K, plaintext: text, authorPublicKey: p.pub, sign: signer(p), blake3, createdAt: at });
    return { object_id: built.objectId, author_fp: fp(p.key), received_at: 1, ...built.submission };
  };
  const hello = await groupMsg(ben, "welcome, everyone", T0 + 200);
  FX = {
    m, ann, ben, cy, dan, fp, dmSeal, G, K, groupMsg, hello,
    groupServed: { object_id: G, author_fp: fp(ben.key), received_at: 1, ...group.submission },
    epochServed: { object_id: epoch.objectId, author_fp: fp(ben.key), received_at: 1, ...epoch.submission },
  };
  return FX;
}

/** What a group message the page posted carries, opened with the group's key. */
async function groupText(fx, submission) {
  const { obj } = fx.m;
  const parsed = obj.parseGroupMsgPayload(Buffer.from(submission.payload_b64, "base64"));
  assert.ok(parsed, "the posted object is a group message");
  return obj.aesGcmDecrypt(fx.K, parsed.nonce, parsed.ct);
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
// script adds, and one kept element per selector they are asked for (so a file card's body, and
// the buttons in it, can be read and pressed).
function fakeDom(state) {
  const all = [];
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
    own.after = () => {};
    const p = new Proxy(own, {
      get(t, prop) {
        if (prop === "then") return undefined;
        if (prop === "innerHTML") return "innerHTML" in t ? t.innerHTML : escapeText(t.textContent);
        if (prop in t) return t[prop];
        return anything();
      },
      set(t, prop, v) {
        // Writing the body afresh drops what was put in it before (an image replaced by the line).
        if (prop === "innerHTML") t.children = [];
        t[prop] = v;
        return true;
      },
    });
    all.push(p);
    return p;
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

// The page's asynchronous work is WebCrypto (its own file encryption, the store, the PIN's
// 600,000 rounds, and pq-object.js's group-key work, which runs in this realm), the stand-in
// IndexedDB, the uploads (read from their FormData) and resolved fetches. settle() waits until no
// WebCrypto call and no upload is in flight for a run of turns; a PBKDF2 run outlasts any fixed
// number of turns, so the bound is time.
let cryptoInFlight = 0;
let uploadsInFlight = 0;
const subtle = globalThis.crypto.subtle;
for (const name of ["encrypt", "decrypt", "sign", "verify", "digest", "importKey", "exportKey", "generateKey", "deriveKey", "deriveBits"]) {
  const orig = subtle[name];
  if (typeof orig !== "function" || orig.__counted) continue;
  const counted = (...args) => {
    cryptoInFlight++;
    return Promise.resolve(orig.apply(subtle, args)).finally(() => { cryptoInFlight--; });
  };
  counted.__counted = true;
  Object.defineProperty(subtle, name, { configurable: true, writable: true, value: counted });
}
const countedCrypto = {
  subtle,
  getRandomValues: (a) => globalThis.crypto.getRandomValues(a),
  randomUUID: () => globalThis.crypto.randomUUID(),
};

async function settle() {
  const until = Date.now() + 30000;
  let idle = 0;
  while (idle < 40 && Date.now() < until) {
    await new Promise((r) => setImmediate(r));
    idle = cryptoInFlight === 0 && uploadsInFlight === 0 ? idle + 1 : 0;
  }
}

/** Wait (by turns, never a fixed delay) until `pred` holds; fail naming `what` if it never does. */
async function waitFor(pred, what) {
  const until = Date.now() + 30000;
  while (Date.now() < until) {
    if (pred()) return;
    await new Promise((r) => setImmediate(r));
  }
  assert.fail("never happened: " + what);
}

const answer = (status, { json, bytes, text } = {}) => Promise.resolve({
  ok: status >= 200 && status < 300,
  status,
  json: async () => json,
  text: async () => (text != null ? text : JSON.stringify(json == null ? {} : json)),
  arrayBuffer: async () => {
    const b = bytes || new Uint8Array(0);
    return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
  },
});
const ok = (json) => answer(200, { json });

// A DM's words start "hum/" (and a pass's, an identify's): those are stood in. Anything else
// signed here is a signed object's canonical bytes (a CBOR map), signed for real.
const isWords = (b) => b.length >= 4 && b[0] === 0x68 && b[1] === 0x75 && b[2] === 0x6d && b[3] === 0x2f;

/**
 * The chat page as `me`. `opts.log`: the group's messages the server lists. The stand-in server
 * serves the fixture's group, its key, its members (Ann, Ben, Cy) and its messages, keeps what is
 * posted to /api/v2/objects, and keeps each upload's bytes, serving them back at the address it
 * answered (`files`, which a test may also fill or alter).
 */
async function loadChat(me, opts = {}) {
  const fx = await fixture();
  const state = { appended: [] };
  const posted = [];
  const fetched = [];   // every fetch, "METHOD url"
  const uploads = [];   // { url, query, name, type, bytes } for each POST /api/upload
  const files = new Map(); // address -> the bytes served there
  const blobs = new Map(); // object URL -> the Blob the page made it from
  const notices = [];
  const systemLines = [];
  const relay = { log: opts.log || [fx.hello], hold: null };
  const G = fx.G;
  let uploadCount = 0;
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  class TestURL extends URL {
    static createObjectURL(blob) { const u = "blob:test/" + (blobs.size + 1); blobs.set(u, blob); return u; }
    static revokeObjectURL() {}
  }
  const upload = async (u, init) => {
    uploadsInFlight++;
    try {
      const form = init.body;
      assert.ok(form instanceof FormData, "an upload is a form");
      const file = form.get("file");
      assert.ok(file instanceof Blob, "with the file's bytes in it, not text");
      const bytes = new Uint8Array(await file.arrayBuffer());
      const query = u.slice(u.indexOf("?") + 1);
      const encrypted = /(^|&)encrypted=1(&|$)/.test(query);
      const ext = encrypted ? "enc" : (String(file.name || "").split(".").pop() || "bin");
      const url = `/uploads/u${++uploadCount}.${ext}`;
      uploads.push({ url, query, name: file.name, type: file.type, bytes });
      if (relay.hold) await relay.hold;
      files.set(url, bytes);
      return { url };
    } finally {
      uploadsInFlight--;
    }
  };
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDom(state),
    navigator: new Proxy({}, { get: (_t, p) => (p === "serviceWorker" ? serviceWorker : anything()), has: () => true }),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", origin: "https://localhost", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    indexedDB: fakeIndexedDB(),
    fetch: (url, init) => {
      const u = String(url);
      const method = (init && init.method) || "GET";
      fetched.push(method + " " + u);
      if (u.startsWith("/api/upload") && method === "POST") return upload(u, init).then((json) => ok(json));
      if (u.startsWith("/uploads/")) return files.has(u) ? answer(200, { bytes: files.get(u) }) : answer(404, { text: "not found" });
      if (u.startsWith("/api/federation/servers")) return ok([]);
      if (u === P.PROTECTED_PRESETS_URL) return ok(clone(SHIPPED_PRESETS));
      if (u === W.WARNINGS_URL) return ok(clone(SHIPPED_WARNINGS));
      if (u.startsWith(report.REPORT_REASONS_URL)) return ok(clone(SHIPPED_REASONS));
      if (u === "/api/v2/objects" && method === "POST") { posted.push(JSON.parse(init.body)); return ok({ ok: true }); }
      if (u.startsWith("/api/v2/groups?pubkey=")) return ok({ groups: [{ group_id: G, name: "Hikers", members: [fx.ann.key, fx.ben.key, fx.cy.key], is_creator: false }] });
      if (u === `/api/v2/objects/${G}`) return ok(fx.groupServed);
      if (u.startsWith("/api/v2/objects/")) return answer(404, { json: { error: "not found" } });
      if (u === `/api/v2/groups/${G}/members`) return ok({ members: [fx.ann, fx.ben, fx.cy].map((p) => ({ pubkey: p.key, kyber_public: kyberOf(p.key) })) });
      if (u === `/api/v2/groups/${G}/epochs`) return ok({ epochs: [fx.epochServed] });
      if (u === `/api/v2/groups/${G}/epoch`) return ok(fx.epochServed);
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
    URL: TestURL,
    Blob,
    File,
    FormData,
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
  run("chat/bip39-english.js");
  run("shared/friend-pass.js");
  run("shared/reach.js");
  run("shared/block.js");
  run("shared/report.js");
  run("shared/group-report.js");
  run("shared/warnings.js");
  run("shared/protected.js");
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
  run("chat/chat-reports.js");
  run("chat/chat-warnings.js");
  run("chat/chat-protected.js");
  run("chat/chat-onboarding.js");
  run("chat/chat-p2p.js");
  for (const name of ["hosIcon", "holdToConfirm", "updateStats"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  ctx.holdConfirm = async () => true;
  ctx.appendMessage = (el) => { state.appended.push(el); };
  ctx.notifyNewMessage = () => {};
  ctx.playNotificationChime = () => {};
  ctx.sendSWNotification = () => {};
  ctx.addNotice = (text) => { notices.push(String(text)); };
  ctx.addSystemMessage = (text) => { systemLines.push(String(text)); };
  const storeKey = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => storeKey;
  const { noble } = fx.m;
  ctx.pqSignMessage = async (secret, bytes) => (isWords(bytes) ? new Uint8Array(bytes) : noble.ml_dsa65.sign(bytes, secret));
  ctx.pqVerifyMessage = async (pk, bytes, sig) => (sig.length === 3309 ? noble.ml_dsa65.verify(sig, bytes, pk) : Buffer.from(bytes).equals(Buffer.from(sig)));
  ctx.pqDmSeal = fx.dmSeal;
  ctx.pqDmOpen = async (_secret, ek, _nonce, ct) => (unb64(ek) === kyberOf(me.key) ? unb64(ct) : null);
  ctx.pqBlake3 = async (bytes) => fx.m.blake3(bytes);
  const sock = fakeSocket();
  vm.runInContext(`(s, me, name, sk, kyber, seed) => {
    ws = s; myKey = me; myName = name; activeChannel = 'general'; identityConfirmed = true;
    myDilithiumPublicHex = me; myDilithiumSecret = sk;
    myKyberPublicBase64 = kyber; myKyberSecret = new Uint8Array(4);
    myIdentity = { publicKeyHex: me, seed32: seed, canSign: true };
    dmFetchSent = true;
  }`, ctx)(sock, me.key, me.name, me.sk, kyberOf(me.key), new Uint8Array(SEED));
  const handle = async (msg) => { await vm.runInContext("handleMessage", ctx)(msg); await settle(); };
  await handle({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  const store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await store.init(me.key, "localhost"), "the store loads");
  store.setPassServer(SERVER);
  const users = [fx.ann, fx.ben, fx.cy, fx.dan].map((p) => ({ public_key: p.key, name: p.name, role: "", kyber_public: kyberOf(p.key), online: true }));
  await handle({ type: "full_user_list", users });
  await handle({ type: "reach_settings", settings: { message: "friends", call: "chosen", trade: "friends" } });
  vm.runInContext("updateChannelList", ctx)(clone(CHANNELS));
  await settle();
  sock.sent.length = 0;
  sock.raw.length = 0;
  state.appended.length = 0;
  fetched.length = 0;
  const fn = (name) => vm.runInContext(name, ctx);
  const set = (code, ...args) => vm.runInContext(code, ctx)(...args);
  const page = {
    ctx, fx, sock, store, state, relay, posted, fetched, uploads, files, blobs, notices, systemLines, handle, fn, set,
    el: (id) => ctx.document.getElementById(id),
    /** Hold every upload until release() (to leave the conversation while one is on its way). */
    holdUploads() {
      let release;
      relay.hold = new Promise((r) => { release = r; });
      return () => { relay.hold = null; release(); };
    },
  };
  return page;
}

// ── Helpers ──────────────────────────────────────────────────────────────

/** A friend: a mutual follow holding a pass from me (the protected setup's test needs the pass too). */
function befriend(page, key) {
  page.store.setFollowing(key, true);
  page.store.setFollower(key, true);
  page.store.recordPassSent(key, "11".repeat(16), MAY);
  page.set("(k) => { myFollowing.add(k); myFollowers.add(k); }", key);
}

/** Paste `file` into the composer, through the listener the page put on it. */
async function paste(page, file) {
  const listeners = page.el("msg-input").listeners.paste || [];
  assert.ok(listeners.length, "the composer takes a paste");
  const ev = { clipboardData: { items: [{ type: file.type, getAsFile: () => file }] }, preventDefault() {} };
  for (const fn of listeners) await fn(ev);
  await settle();
}

/** Pick `file` with the attach button. */
async function pick(page, file) {
  await page.fn("handleFileAttachment")({ target: { files: [file], value: "C:\\fakepath\\" + file.name } });
  await settle();
}

/** Open the group and wait until its messages are drawn. */
async function openGroup(page) {
  page.fn("openP2pGroup")(page.fx.G, "Hikers");
  await waitFor(() => page.state.appended.some((e) => e && e.dataset && e.dataset.groupObjectId), "the group's messages are drawn");
  await waitFor(() => page.fn("activeP2pGroup") && page.fn("activeP2pGroup").epochKey, "the group's key is open");
  await settle();
}

/** The group's rows from `who`, drawn from the server's log. */
const rowsFrom = (page, who) => page.state.appended.filter((e) => e && e.dataset && e.dataset.from === who.key && e.dataset.groupObjectId);
/** A file card's body: the element the page writes the picture, the Save button or the line into. */
const cardBody = (row) => row.querySelector(".enc-attach").querySelector(".enc-attach-body");
const gets = (page, url) => page.fetched.filter((f) => f === "GET " + url).length;

/** The picture shown in a card body, and the bytes behind it. */
async function shownPicture(page, body) {
  const img = (body.children || []).find((c) => c.tag === "img");
  if (!img) return null;
  const blob = page.blobs.get(img.src);
  assert.ok(blob, "the picture is one the page opened");
  return { img, bytes: new Uint8Array(await blob.arrayBuffer()), type: blob.type };
}

/** The inner text sealed in a dm_put (the stand-in seal base64s it), and who it was sealed to. */
function opened(put) {
  const env = JSON.parse(put.content);
  return { sealedTo: unb64(env.ek_ct_b64), inner: JSON.parse(unb64(env.ct_b64)) };
}

/** No frame on the socket and nothing posted carries this address in the clear. */
function addressNowhereInTheClear(page, url) {
  assert.ok(!page.sock.raw.some((r) => r.includes(url)), "the upload's address is in no frame on the socket");
  assert.ok(!JSON.stringify(page.posted).includes(url), "nor in anything posted");
  assert.equal(page.sock.sent.filter((m) => m.type === "chat").length, 0, "nothing is posted in the public channel");
}

/** One upload, made as ciphertext with `encrypted=1`, whose bytes are not the file's. */
function oneEncryptedUpload(page, plain) {
  assert.equal(page.uploads.length, 1, "one upload");
  const up = page.uploads[0];
  assert.ok(/(^|&)encrypted=1(&|$)/.test(up.query), "uploaded with encrypted=1");
  assert.ok(!sameBytes(up.bytes, plain), "the uploaded bytes are not the file's bytes");
  assert.ok(!Buffer.from(up.bytes).includes(Buffer.from(plain.slice(0, 12))), "and hold no run of them");
  assert.equal(up.name, "attachment.enc", "and its name is not the file's");
  return up;
}

/** The marker's key opens the upload back to the file's bytes. */
async function markerOpensUpload(meta, up, plain) {
  assert.equal(meta.url, up.url, "the marker names the upload");
  assert.ok(sameBytes(await unseal(up.bytes, meta.k, meta.n), plain), "and its key opens it to the file's bytes");
}

// ── 0. The words are 10k's ──────────────────────────────────────────────

test("the failure line and the marker are 10k's", async () => {
  assert.ok(TEN_K.includes(`says "${FAILED}"`), "10k gives the failure line");
  assert.ok(TEN_K.includes("`[[hum:file:v1]]` marker"), "and the marker");
  assert.ok(TEN_K.includes("`encrypted=1`"), "and the upload's flag");
  const page = await loadChat((await fixture()).ann);
  assert.equal(page.fn("PRIVATE_FILE_FAILED"), FAILED, "the page says 10k's line");
  assert.equal(page.fn("FILE_MARKER"), FILE_MARKER, "and builds 10k's marker");
});

// ── 1. A pasted image in a DM ───────────────────────────────────────────

test("a pasted image in a DM is encrypted, uploaded with encrypted=1 and sent sealed as a marker", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  page.sock.sent.length = 0;
  page.sock.raw.length = 0;
  page.state.appended.length = 0;

  await paste(page, picture("image.png"));
  const up = oneEncryptedUpload(page, PNG);
  const puts = page.sock.sent.filter((m) => m.type === "dm_put");
  const toBen = puts.filter((p) => opened(p).sealedTo === kyberOf(fx.ben.key));
  assert.equal(toBen.length, 1, "one sealed DM to Ben");
  assert.equal(puts.length, 2, "and my own copy (sealed to me)");
  const inner = opened(toBen[0]).inner;
  assert.ok(inner.text.startsWith(FILE_MARKER), "what was sealed is a file marker");
  const meta = markerMeta(inner.text);
  assert.deepEqual([meta.name, meta.mime, meta.size], ["image.png", "image/png", PNG.length], "naming the picture");
  await markerOpensUpload(meta, up, PNG);
  addressNowhereInTheClear(page, up.url);

  // My own copy is a card with the picture in it, never the marker's text.
  const row = page.state.appended.find((e) => e && e.dataset && e.dataset.from === fx.ann.key);
  assert.ok(row && row.innerHTML.includes("enc-attach") && !row.innerHTML.includes(FILE_MARKER) && !row.innerHTML.includes(meta.k), "my copy: a card, not the text");
  const pic = await shownPicture(page, cardBody(row));
  assert.ok(pic && sameBytes(pic.bytes, PNG), "showing the picture");
});

// ── 2 and 3. A picked file and a pasted image in a group ────────────────

test("a picked file in a group is encrypted, uploaded with encrypted=1 and sent inside the group's encrypted message", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  await openGroup(page);
  page.sock.sent.length = 0;
  page.sock.raw.length = 0;
  page.posted.length = 0;
  page.state.appended.length = 0;

  await pick(page, pdf("notes.pdf"));
  const up = oneEncryptedUpload(page, PDF);
  const msgs = page.posted.filter((o) => o.object_type === "group_msg_v1");
  assert.equal(msgs.length, 1, "one group message posted");
  const text = await groupText(fx, msgs[0]);
  assert.ok(text && text.startsWith(FILE_MARKER), "carrying a file marker, inside the group's encryption");
  const meta = markerMeta(text);
  assert.deepEqual([meta.name, meta.mime, meta.size], ["notes.pdf", "application/pdf", PDF.length]);
  await markerOpensUpload(meta, up, PDF);
  addressNowhereInTheClear(page, up.url);
  assert.equal(page.sock.sent.filter((m) => m.type === "dm_put").length, 0, "and no DM");

  // My own copy: a card with its name, size and Save, never the marker's text.
  const row = page.state.appended.find((e) => e && e.dataset && e.dataset.from === fx.ann.key);
  assert.ok(row, "my message is drawn");
  assert.ok(row.innerHTML.includes("enc-attach") && row.innerHTML.includes("notes.pdf") && row.innerHTML.includes(">Save<"), "a card with its name and Save");
  assert.ok(!row.innerHTML.includes(FILE_MARKER) && !row.innerHTML.includes(meta.k), "never the marker's text or its key");
});

test("a pasted image in a group is encrypted, uploaded with encrypted=1 and sent as a marker", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  await openGroup(page);
  page.sock.sent.length = 0;
  page.sock.raw.length = 0;
  page.posted.length = 0;
  page.state.appended.length = 0;

  await paste(page, picture("image.png"));
  const up = oneEncryptedUpload(page, PNG);
  const msgs = page.posted.filter((o) => o.object_type === "group_msg_v1");
  assert.equal(msgs.length, 1, "one group message posted");
  const meta = markerMeta(await groupText(fx, msgs[0]));
  assert.ok(meta, "carrying a file marker");
  await markerOpensUpload(meta, up, PNG);
  addressNowhereInTheClear(page, up.url);

  // My own picture is shown inline.
  const row = page.state.appended.find((e) => e && e.dataset && e.dataset.from === fx.ann.key);
  const pic = await shownPicture(page, cardBody(row));
  assert.ok(pic && sameBytes(pic.bytes, PNG) && pic.type === "image/png", "my copy shows the picture");
});

// ── 4. A public channel ─────────────────────────────────────────────────

test("a public channel still uploads the plain file and posts its address", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  await paste(page, picture("image.png"));
  assert.equal(page.uploads.length, 1, "one upload");
  const up = page.uploads[0];
  assert.ok(!/encrypted=1/.test(up.query), "not marked encrypted");
  assert.ok(sameBytes(up.bytes, PNG), "the file's own bytes");
  const posts = page.sock.sent.filter((m) => m.type === "chat");
  assert.equal(posts.length, 1, "one public post");
  assert.equal(posts[0].content, up.url, "its address");
  assert.equal(posts[0].channel, "general");
  assert.ok(!posts[0].content.includes(FILE_MARKER), "no marker");

  await pick(page, pdf("notes.pdf"));
  assert.equal(page.uploads.length, 2);
  assert.ok(!/encrypted=1/.test(page.uploads[1].query) && sameBytes(page.uploads[1].bytes, PDF), "a picked file too");
  assert.equal(page.sock.sent.filter((m) => m.type === "chat").pop().content, page.uploads[1].url);
});

// ── 5. A marker in a group message ──────────────────────────────────────

/** Put an encrypted file on the server and a group message from `who` carrying its marker. */
async function fileInGroup(page, who, { name, mime, bytes, at, meta: change }) {
  const s = await seal(bytes);
  const url = `/uploads/from-${who.name.toLowerCase()}-${at}.enc`;
  page.files.set(url, s.ct);
  const meta = Object.assign({ url, k: s.k, n: s.n, name, mime, size: bytes.length }, change || {});
  const msg = await page.fx.groupMsg(who, markerText(meta), at);
  return { meta, msg, ct: s.ct };
}

test("a marker in a group message: a friend's picture shows inline, a non-friend's waits for a click, a file has Save", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  befriend(page, fx.ben.key);
  const benPic = await fileInGroup(page, fx.ben, { name: "dog.png", mime: "image/png", bytes: PNG, at: T0 + 300 });
  const cyPic = await fileInGroup(page, fx.cy, { name: "cat.png", mime: "image/png", bytes: PNG, at: T0 + 400 });
  const cyPdf = await fileInGroup(page, fx.cy, { name: "notes.pdf", mime: "application/pdf", bytes: PDF, at: T0 + 500 });
  page.relay.log = [fx.hello, benPic.msg, cyPic.msg, cyPdf.msg];
  await openGroup(page);

  // Each is drawn as a card, never as its text.
  const [, benRow] = rowsFrom(page, fx.ben);
  const [cyPicRow, cyPdfRow] = rowsFrom(page, fx.cy);
  for (const [row, f] of [[benRow, benPic], [cyPicRow, cyPic], [cyPdfRow, cyPdf]]) {
    assert.ok(row && row.innerHTML.includes("enc-attach"), `${f.meta.name}: a card`);
    assert.ok(row.innerHTML.includes(escHtml(f.meta.name)), `${f.meta.name}: with its name`);
    assert.ok(!row.innerHTML.includes(FILE_MARKER) && !row.innerHTML.includes(f.meta.k), `${f.meta.name}: never the marker's text or key`);
  }

  // A friend's picture: fetched, opened and shown at once.
  const benShown = await shownPicture(page, cardBody(benRow));
  assert.ok(benShown && sameBytes(benShown.bytes, PNG), "a friend's picture shows inline, opened to its bytes");
  assert.equal(gets(page, benPic.meta.url), 1, "fetched once");

  // A non-friend's picture: nothing fetched until "Image (click to load)" is pressed.
  assert.ok(cyPicRow.innerHTML.includes("Image (click to load)"), "a non-friend's picture says Image (click to load)");
  assert.equal(gets(page, cyPic.meta.url), 0, "and is not fetched before the click");
  assert.equal(await shownPicture(page, cardBody(cyPicRow)), null, "nor shown");
  const load = cardBody(cyPicRow).querySelector(".enc-attach-load");
  assert.equal(typeof load.onclick, "function", "the click loads it");
  await load.onclick({ stopPropagation() {} });
  await settle();
  const cyShown = await shownPicture(page, cardBody(cyPicRow));
  assert.ok(cyShown && sameBytes(cyShown.bytes, PNG), "after the click: shown, opened to its bytes");
  assert.equal(gets(page, cyPic.meta.url), 1);

  // Any other file: its name, its size and Save; opened only when Save is pressed.
  assert.ok(cyPdfRow.innerHTML.includes(">Save<") && cyPdfRow.innerHTML.includes(`${PDF.length} B`), "a file: its size and Save");
  assert.equal(gets(page, cyPdf.meta.url), 0, "not fetched before Save");
  const save = cardBody(cyPdfRow).querySelector(".enc-attach-dl");
  await save.onclick({ stopPropagation() {} });
  await settle();
  assert.equal(gets(page, cyPdf.meta.url), 1, "Save fetches it");
  const link = page.el("__body").children.find((c) => c.tag === "a" && c.download === "notes.pdf");
  assert.ok(link, "and saves it under its name");
  assert.ok(sameBytes(new Uint8Array(await page.blobs.get(link.href).arrayBuffer()), PDF), "opened to its bytes");
});

test("a non-friend's DM picture waits for the click (the same rule as a group's)", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  const s = await seal(PNG);
  page.files.set("/uploads/dm-cy.enc", s.ct);
  const meta = { url: "/uploads/dm-cy.enc", k: s.k, n: s.n, name: "cat.png", mime: "image/png", size: PNG.length };
  page.fn("addDmMessage")("Cy", markerText(meta), T0 + 600, fx.cy.key, fx.ann.key, true);
  await settle();
  const row = page.state.appended[0];
  assert.ok(row.innerHTML.includes("Image (click to load)"), "Image (click to load)");
  assert.equal(gets(page, meta.url), 0, "nothing fetched before the click");
  await cardBody(row).querySelector(".enc-attach-load").onclick({ stopPropagation() {} });
  await settle();
  const pic = await shownPicture(page, cardBody(row));
  assert.ok(pic && sameBytes(pic.bytes, PNG), "after the click: shown");
});

// ── 6. A file that cannot be opened ─────────────────────────────────────

test("a tampered key, altered ciphertext, a missing upload or an address off this server say the file could not be opened", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  befriend(page, fx.ben.key);
  const other = await seal(PNG); // a key that is not this file's
  const wrongKey = await fileInGroup(page, fx.ben, { name: "a.png", mime: "image/png", bytes: PNG, at: T0 + 300, meta: { k: other.k } });
  const altered = await fileInGroup(page, fx.ben, { name: "b.png", mime: "image/png", bytes: PNG, at: T0 + 310 });
  const bad = new Uint8Array(altered.ct);
  bad[3] ^= 0x01;
  page.files.set(altered.meta.url, bad);
  const missing = await fileInGroup(page, fx.ben, { name: "c.png", mime: "image/png", bytes: PNG, at: T0 + 320 });
  page.files.delete(missing.meta.url);
  const elsewhere = await fileInGroup(page, fx.ben, { name: "d.png", mime: "image/png", bytes: PNG, at: T0 + 330, meta: { url: "https://elsewhere.example/d.enc" } });
  const wrongFile = await fileInGroup(page, fx.ben, { name: "e.pdf", mime: "application/pdf", bytes: PDF, at: T0 + 340, meta: { k: other.k } });
  page.relay.log = [fx.hello, wrongKey.msg, altered.msg, missing.msg, elsewhere.msg, wrongFile.msg];
  await openGroup(page);
  const rows = rowsFrom(page, fx.ben).slice(1); // after "welcome, everyone"
  assert.equal(rows.length, 5);
  const line = `<div class="enc-attach-failed">${FAILED}</div>`;
  for (const [i, what] of ["a wrong key", "altered ciphertext", "a missing upload", "an address off this server"].entries()) {
    const body = cardBody(rows[i]);
    assert.equal(body.innerHTML, line, `${what}: the line`);
    assert.equal(await shownPicture(page, body), null, `${what}: no picture`);
  }
  assert.equal(page.fetched.filter((f) => f.includes("elsewhere.example")).length, 0, "an address off this server is never fetched");
  for (const f of [wrongKey, altered]) {
    const shownText = rows.map((r) => r.innerHTML + cardBody(r).innerHTML).join(" ");
    assert.ok(!shownText.includes(Buffer.from(f.ct).toString("base64").slice(0, 16)), "the ciphertext is never shown");
  }
  // A file's Save with a wrong key: the same line, nothing saved.
  const before = page.el("__body").children.length;
  await cardBody(rows[4]).querySelector(".enc-attach-dl").onclick({ stopPropagation() {} });
  await settle();
  assert.equal(cardBody(rows[4]).innerHTML, line, "Save with a wrong key: the line");
  assert.equal(page.el("__body").children.length, before, "and nothing is saved");
});

// ── 7. The protected setup ──────────────────────────────────────────────

async function turnOn(page) {
  assert.equal(page.fn("openProtectedSetup")(), true, "the setup's steps open");
  assert.equal(page.fn("protectedSetupContinue")(), true, "step 1 (read) to step 2");
  assert.equal(await page.fn("protectedSetupChoosePin")(PIN, PIN), true, "step 2: the PIN twice");
  assert.equal(page.fn("protectedSetupApply")(), true, "step 4: apply");
  await settle();
}

test("with the protected setup on, a group file from someone who is not a friend is not shown or fetched; a friend's is", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  befriend(page, fx.ben.key);
  await page.fn("loadProtectedPreset")();
  await turnOn(page);
  const benPic = await fileInGroup(page, fx.ben, { name: "dog.png", mime: "image/png", bytes: PNG, at: T0 + 300 });
  const cyPic = await fileInGroup(page, fx.cy, { name: "cat.png", mime: "image/png", bytes: PNG, at: T0 + 400 });
  const cyPdf = await fileInGroup(page, fx.cy, { name: "notes.pdf", mime: "application/pdf", bytes: PDF, at: T0 + 500 });
  page.relay.log = [fx.hello, benPic.msg, cyPic.msg, cyPdf.msg];
  page.fetched.length = 0;
  await openGroup(page);
  const line = escHtml(PRESET.picture_hidden_line);
  for (const row of rowsFrom(page, fx.cy)) {
    assert.ok(row.innerHTML.includes(line), "a non-friend's file: the preset's line");
    assert.ok(!row.innerHTML.includes("enc-attach") && !row.innerHTML.includes("click to load"), "no card, nothing to click");
    assert.ok(!row.innerHTML.includes(FILE_MARKER), "and not the text");
  }
  assert.equal(gets(page, cyPic.meta.url) + gets(page, cyPdf.meta.url), 0, "never fetched");
  const [, benRow] = rowsFrom(page, fx.ben);
  assert.ok(!benRow.innerHTML.includes(line), "a friend's is not hidden");
  const pic = await shownPicture(page, cardBody(benRow));
  assert.ok(pic && sameBytes(pic.bytes, PNG), "a friend's picture still shows");
});

// ── 8. The guards on every encrypted path ───────────────────────────────

test("a file named with my recovery phrase or over 6 MB is not uploaded or sent, in a DM or a group", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  const phrase = (await page.fn("mnemonicFromSeed")(new Uint8Array(SEED))).split(" ");
  const phraseName = phrase.slice(3, 7).join("-") + ".png";
  const big = new Uint8Array(6 * 1024 * 1024 + 1);
  big.set(PNG);

  const nothingWent = (where) => {
    assert.equal(page.uploads.length, 0, `${where}: nothing uploaded`);
    assert.equal(page.sock.sent.filter((m) => m.type === "dm_put" || m.type === "chat").length, 0, `${where}: nothing sent`);
    assert.equal(page.posted.filter((o) => o.object_type === "group_msg_v1").length, 0, `${where}: nothing posted`);
  };
  const guardSaid = () => page.state.appended.some((e) => e && String(e.className).includes("phrase-guard-stop") && String(e.textContent).startsWith("The file was not sent."));

  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  page.sock.sent.length = 0;
  page.state.appended.length = 0;
  await paste(page, picture(phraseName));
  nothingWent("a DM, the phrase in the name");
  assert.ok(guardSaid(), "the guard says the file was not sent");
  await paste(page, picture("big.png", big));
  nothingWent("a DM, over 6 MB");
  assert.ok(page.systemLines.some((l) => l.includes("over the 6 MB max")), "and says it is over 6 MB");

  page.systemLines.length = 0;
  await openGroup(page);
  page.sock.sent.length = 0;
  page.posted.length = 0;
  page.state.appended.length = 0;
  await pick(page, picture(phraseName));
  nothingWent("a group, the phrase in the name");
  assert.ok(guardSaid(), "the guard says the file was not sent");
  await paste(page, picture("big.png", big));
  nothingWent("a group, over 6 MB");
  assert.ok(page.systemLines.some((l) => l.includes("over the 6 MB max")), "and says it is over 6 MB");
});

// ── 9. Leaving the conversation while it uploads ────────────────────────

test("a group's file is not sent anywhere when another conversation is opened while it uploads", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  await openGroup(page);
  page.sock.sent.length = 0;
  page.posted.length = 0;
  const release = page.holdUploads();
  const sending = page.fn("handleFileAttachment")({ target: { files: [picture("image.png")], value: "x" } });
  await waitFor(() => page.uploads.length === 1, "the upload has started");
  page.fn("switchChannel")("general");
  release();
  await sending;
  await settle();
  assert.ok(/(^|&)encrypted=1(&|$)/.test(page.uploads[0].query), "it was uploaded encrypted");
  assert.equal(page.sock.sent.filter((m) => m.type === "chat").length, 0, "nothing is posted in the public channel opened meanwhile");
  assert.equal(page.posted.filter((o) => o.object_type === "group_msg_v1").length, 0, "nor in the group left");
  assert.ok(page.systemLines.includes("The file was not sent because another conversation was opened while it uploaded."), "and it says so");

  // And whatever the timing, a marker is never posted in a public channel: the send path itself
  // refuses one there.
  page.systemLines.length = 0;
  const marker = markerText({ url: page.uploads[0].url, k: "a2V5", n: "bm9uY2U=", name: "image.png", mime: "image/png", size: PNG.length });
  assert.equal(await page.fn("sendComposedContent")(marker), false, "a marker in a public channel is refused");
  assert.equal(page.sock.sent.filter((m) => m.type === "chat").length, 0, "and not posted");
  assert.ok(page.systemLines.includes("The file was not sent."), "with a line saying so");
});

// ── Red first ────────────────────────────────────────────────────────────
// 2026-10-10: each break made in a copy of web/ (chat/ and shared/), this file run against it with
// HOS_WEB_DIR, and seen failing with the assertion named; every other test passed.
//  B1 chat-messages.js, the paste handler back to `uploadImage` then sendComposedContent(url) (as
//     before 10k): test 1 and test 3 "uploaded with encrypted=1".
//  B2 chat-messages.js, privateConversationNow() seeing only a DM (a group taken for a public
//     channel, as before 10k): tests 2 and 3 "uploaded with encrypted=1", test 9 "it was uploaded
//     encrypted".
//  B3 chat-ui.js, sendComposedContent without its P2P group branch: tests 2 and 3 "one group
//     message posted" (the marker went to the public channel instead).
//  B4 chat-messages.js, the file's own bytes put in the encrypted upload's form in place of its
//     ciphertext: tests 1, 2 and 3 "the uploaded bytes are not the file's bytes".
//  B5 chat-messages.js, sendAttachment always taking the encrypted path: test 4 "one upload".
//  B6 chat-groups-p2p.js, addGroupMessage without { privateFiles: true }: test 2 "a card with its
//     name and Save", test 3 "my copy shows the picture", test 5 "dog.png: a card", test 6 "a
//     wrong key: the line", test 7 "a non-friend's file: the preset's line".
//  B7 chat-dms.js, openPrivateFile handing back the fetched bytes when they do not open: test 6
//     "a wrong key: the line".
//  B8 chat-dms.js, privateFileFromFriend true for everyone: test 5 "a non-friend's picture says
//     Image (click to load)", and the DM test "Image (click to load)".
//  B9 chat-dms.js, privateFileHidden always false: test 7 "a non-friend's file: the preset's line".
//  B10 chat-messages.js, sendEncryptedAttachment without the recovery-phrase guard: test 8 "a DM,
//     the phrase in the name: nothing uploaded".
//  B11 chat-messages.js, sendEncryptedAttachment without the 6 MB check: test 8 "a DM, over 6 MB:
//     nothing uploaded".
//  B12 chat-messages.js, the conversation not checked again after the upload: test 9 "and it says
//     so". Before B14's refusal existed, the same break failed at "nothing is posted in the public
//     channel opened meanwhile": the group's marker, and with it the file's key, went out in public.
//  B13 chat-dms.js, openPrivateFile following any address: test 6 "an address off this server is
//     never fetched".
//  B14 chat-ui.js, sendComposedContent without its refusal of a marker in a public channel: test 9
//     "a marker in a public channel is refused".
//  B15 chat-dms.js, the failure line in other words ("Attachment unavailable."): test 0 "the page
//     says 10k's line", test 6 "a wrong key: the line".

// BUG-178 (2026-10-10): a group's row offered the server's React, Edit, Pin and Delete, and
// Edit and Pin sent the group message's text to the server in the clear (an admin's pin would
// have been kept there and shown to everyone). A group row keeps Reply and Pin for me only, and
// nothing that would send a group message's text or reactions goes out while a group is open.
// Seen red with the privateRow test taken out of addChatMessage: "a group row offers no React".
test("a group's row offers no server reaction, edit, pin or delete, and none is sent", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  const mine = await page.fx.groupMsg(fx.ann, "a private word", T0 + 300);
  page.relay.log = [fx.hello, mine];
  await openGroup(page);
  const rows = page.state.appended.filter((e) => e && e.dataset && e.dataset.groupObjectId);
  assert.ok(rows.length >= 2, "the group's rows are drawn");
  for (const row of rows) {
    const html = row.innerHTML;
    assert.ok(!html.includes('class="react-btn"'), "a group row offers no React");
    assert.ok(!html.includes('class="edit-btn"'), "nor Edit");
    assert.ok(!html.includes('class="pin-btn"'), "nor a server Pin");
    assert.ok(!html.includes('class="delete-btn"'), "nor a server Delete");
    assert.ok(html.includes('class="reply-btn"'), "Reply stays");
  }
  page.sock.sent.length = 0;
  page.fn("pinMessageFromUI")(fx.ann.key, fx.ann.name, "a private word", T0 + 300);
  page.fn("sendReaction")(fx.ann.key, T0 + 300, "👍");
  await settle();
  const leaked = page.sock.sent.filter((m) => m.type === "pin_request" || m.type === "reaction" || m.type === "edit");
  assert.deepEqual(leaked, [], "nothing that carries the group's text or reactions goes to the server");
});
