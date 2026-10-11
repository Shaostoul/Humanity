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
// 10. 10m R10, the scratch pad stays on this device: no typing indicator while it is open; a
//     /command's rows are private rows; a Pin, reaction, edit or delete there never reaches the
//     socket; a reply made there is kept on the note as a quote and the reply bar clears; its
//     header, channel description and row tooltip say where a note and a file are kept.
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
// Files in messages (2026-10-11): "Accept files from: Anyone I talk to" kept in this browser. The
// click-to-load tests below are about the pictures rule, which a non-friend's file reaches only when
// it is accepted: under the default (Friends) it first waits for Accept (the tests at the end).
const ACCEPT_ANYONE = { hos_files_in_messages: JSON.stringify({ accept: "anyone" }) };
const MAY = "invite,message,trade,voice_message";
const CHANNELS = [
  { id: "general", name: "general", read_only: false },
  { id: "announcements", name: "announcements", read_only: true },
];

// Some bytes that are plainly a PNG, and some that are plainly a PDF.
// Since 2026-10-11 every file is cleaned on the device before it goes (/shared/upload-clean.js), and a
// picture the cleaning cannot walk is one that "could not be checked": so this PNG is a real one
// (signature, IHDR, an IDAT holding the words, IEND, with real CRCs), with nothing hidden in it, so
// the cleaning reads it through and sends it exactly as it is.
function pngCrc(b) {
  let c = 0xffffffff;
  for (const x of b) { c ^= x; for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xedb88320 : c >>> 1; }
  return (~c) >>> 0;
}
function pngChunk(name, data) {
  const body = Buffer.concat([Buffer.from(name, "latin1"), Buffer.from(data)]);
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(pngCrc(body));
  return Buffer.concat([len, body, crc]);
}
const PNG = Uint8Array.from(Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  pngChunk("IHDR", [0, 0, 0, 4, 0, 0, 0, 4, 8, 6, 0, 0, 0]),
  pngChunk("IDAT", Buffer.from("a picture of a dog on a beach, pixel by pixel")),
  pngChunk("IEND", []),
]));
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
//
// The honest page (`state.honest`, 10m, as group-report-web.test.js): an element's querySelector
// for a class its HTML does not hold answers null, the way a browser does, so a row control that
// is not drawn cannot be hooked by mistake (BUG-180: a fake that answered every lookup hid a
// crash). And `state.absent` names ids that are not on the page until a script gives an element
// that id (getElementById answers null for them until then).
function fakeDom(state) {
  const all = [];
  const absent = state.absent || new Set();
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
      if (state.honest && typeof own.innerHTML === "string" && /^\.[\w-]+$/.test(sel)) {
        const cls = sel.slice(1).replace(/-/g, "\\-");
        if (!new RegExp(`class="(?:[^"]*\\s)?${cls}(?:\\s[^"]*)?"`).test(own.innerHTML)) return null;
      }
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
        // An id that is not on the page until a script gives it to an element (state.absent).
        if (prop === "id" && absent.has(String(v))) byId.set(String(v), p);
        t[prop] = v;
        return true;
      },
    });
    all.push(p);
    return p;
  }
  const byId = new Map();
  const kept = (id) => {
    if (!byId.has(id)) {
      if (absent.has(id)) return null;
      byId.set(id, el("#" + id));
    }
    return byId.get(id);
  };
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
  // Any other store is made when it is first asked for (kept-files.js keeps `files` and `index`).
  const of = (name) => (stores[name] = stores[name] || new Map());
  const req = (result) => {
    const r = { result: undefined, onsuccess: null, onerror: null };
    setImmediate(() => { r.result = result; if (r.onsuccess) r.onsuccess(); });
    return r;
  };
  const objectStore = (name) => ({
    get: (k) => req(of(name).get(k)),
    put: (v) => { of(name).set(v.k ?? v.scope, v); return req(undefined); },
    delete: (k) => { of(name).delete(k); return req(undefined); },
    clear: () => { of(name).clear(); return req(undefined); },
    getAll: () => req([...of(name).values()]),
    getAllKeys: () => req([...of(name).keys()]),
    index: () => ({ getAll: (scope) => req([...of(name).values()].filter((v) => v.scope === scope)) }),
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
  // opts.honest and opts.absentIds: the honest page (fakeDom). opts.timers: an array that collects
  // every setTimeout the page asks for, {cb, ms}, none of which runs by itself (a test may run one,
  // such as chat-ui.js initUnifiedLeftSidebar, which the page queues as it loads).
  const state = { appended: [], honest: !!opts.honest, absent: new Set(opts.absentIds || []) };
  // opts.stored: what this browser's localStorage held before the page loaded ({key: text}).
  const local = fakeStorage();
  for (const [k, v] of Object.entries(opts.stored || {})) local.setItem(k, v);
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
    localStorage: local,
    sessionStorage: fakeStorage(),
    indexedDB: opts.indexedDB || fakeIndexedDB(),
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
    setTimeout: (cb, ms) => { if (opts.timers) opts.timers.push({ cb, ms }); return 0; },
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
  run("shared/strip-metadata.js");
  run("shared/upload-clean.js");
  run("shared/files-in-messages.js");
  run("shared/kept-files.js");
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
  const storeKey = opts.storeKey || await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
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
    myUploadToken = 'tok-test'; // this session's upload token, as the peer list gives it
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
  const page = await loadChat(fx.ann, { stored: ACCEPT_ANYONE });
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
  const page = await loadChat(fx.ann, { stored: ACCEPT_ANYONE });
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

// Since 2026-10-11 (Files in messages): with the setup on, "Show pictures without asking" is always
// Nobody, so a friend's picture, never hidden, waits for the click too.
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
  assert.ok(benRow.innerHTML.includes("Image (click to load)"), "a friend's picture waits for the click (the setup is always Nobody)");
  assert.equal(gets(page, benPic.meta.url), 0, "and is not fetched before it");
  await cardBody(benRow).querySelector(".enc-attach-load").onclick({ stopPropagation() {} });
  await settle();
  const pic = await shownPicture(page, cardBody(benRow));
  assert.ok(pic && sameBytes(pic.bytes, PNG), "after the click, a friend's picture shows");
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
  assert.ok(page.systemLines.some((l) => l.includes("a private conversation can be up to 6 MB")), "and says a private conversation takes up to 6 MB");

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
  assert.ok(page.systemLines.some((l) => l.includes("a private conversation can be up to 6 MB")), "and says a private conversation takes up to 6 MB");
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
// have been kept there and shown to everyone). A group row keeps Reply only (Pin for me went too,
// in the batch review of 2026-10-10: it kept the words under the public channel's pins), and
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

// The batch review (2026-10-10): a picture loads by itself only from the desktop app's friend, a
// mutual follow who also holds a pass from me. A mutual follow alone (a pass not given yet, or
// held back by the protected setup) waits for the click like anyone else. Seen red against web/
// as at a3ca8f167 (HOS_WEB_DIR): "a mutual follow holding no pass from me: Image (click to load)".
test("a picture from a mutual follow who holds no pass from me waits for the click; with the pass it shows at once", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann, { stored: ACCEPT_ANYONE });
  // Ben: a mutual follow, but no pass from me yet.
  page.store.setFollowing(fx.ben.key, true);
  page.store.setFollower(fx.ben.key, true);
  page.set("(k) => { myFollowing.add(k); myFollowers.add(k); }", fx.ben.key);
  const benPic = await fileInGroup(page, fx.ben, { name: "dog.png", mime: "image/png", bytes: PNG, at: T0 + 300 });
  page.relay.log = [fx.hello, benPic.msg];
  await openGroup(page);
  const benRow = rowsFrom(page, fx.ben).find((r) => r.innerHTML.includes("enc-attach"));
  assert.ok(benRow, "Ben's picture is drawn as a card");
  assert.ok(benRow.innerHTML.includes("Image (click to load)"), "a mutual follow holding no pass from me: Image (click to load)");
  assert.equal(gets(page, benPic.meta.url), 0, "nothing fetched before the click");
  // With the pass, a friend: the next drawing shows it at once.
  page.store.recordPassSent(fx.ben.key, "11".repeat(16), MAY);
  assert.equal(page.fn("privateFileFromFriend")(fx.ben.key), true, "a mutual follow holding my pass is a friend");
  assert.equal(page.fn("privateFileFromFriend")(fx.cy.key), false, "someone else is not");
});

// The scratch pad is local only by its label, and the desktop app has kept it so since v0.1477.0
// (src/gui/pages/chat/attach_send.rs `Destination::Scratchpad`, chat.rs `is_private_channel`):
// a file there is encrypted with its marker kept in this browser, and its rows offer nothing
// that would send their words to the server. Seen red 2026-10-10 against the web before this
// (HOS_WEB_DIR): "uploaded with encrypted=1" (the plain file went up); with the rows left public,
// "a scratch pad row offers no React".
test("the scratch pad: a file is encrypted and its marker kept in this browser; its rows offer no server action", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  page.fn("switchChannel")("__scratch__");
  await settle();
  page.sock.sent.length = 0;
  page.sock.raw.length = 0;
  page.state.appended.length = 0;

  await paste(page, picture("image.png"));
  const up = oneEncryptedUpload(page, PNG);
  assert.equal(page.sock.raw.length, 0, "nothing goes on the socket");
  assert.equal(page.posted.length, 0, "nor is anything posted");
  const kept = page.store.scratchNotes();
  assert.equal(kept.length, 1, "the scratch pad keeps one entry");
  const meta = markerMeta(kept[0].content);
  assert.ok(meta, "its marker");
  await markerOpensUpload(meta, up, PNG);

  const row = page.state.appended.find((e) => e && e.dataset && e.dataset.from === fx.ann.key);
  assert.ok(row && row.innerHTML.includes("enc-attach") && !row.innerHTML.includes(FILE_MARKER) && !row.innerHTML.includes(meta.k), "drawn as a card, not the marker's text");
  const pic = await shownPicture(page, cardBody(row));
  assert.ok(pic && sameBytes(pic.bytes, PNG), "showing the picture");

  page.el("msg-input").value = "a note to myself";
  await page.fn("sendMessage")();
  await settle();
  assert.equal(page.sock.raw.length, 0, "a typed note goes nowhere either");
  const rows = page.state.appended.filter((e) => e && e.dataset && e.dataset.from === fx.ann.key);
  assert.equal(rows.length, 2, "both rows are drawn");
  for (const r of rows) {
    const html = r.innerHTML;
    assert.ok(!html.includes('class="react-btn"'), "a scratch pad row offers no React");
    assert.ok(!html.includes('class="edit-btn"'), "nor Edit");
    assert.ok(!html.includes('class="pin-btn"'), "nor a server Pin");
    assert.ok(!html.includes('class="delete-btn"'), "nor a server Delete");
  }

  // Opening it again draws the kept entries the same way.
  page.fn("switchChannel")("general");
  await settle();
  page.state.appended.length = 0;
  page.fn("switchChannel")("__scratch__");
  await settle();
  const again = page.state.appended.filter((e) => e && e.dataset && e.dataset.from === fx.ann.key);
  assert.equal(again.length, 2, "both entries again");
  assert.ok(again.every((r) => !r.innerHTML.includes('class="react-btn"') && !r.innerHTML.includes(FILE_MARKER)), "as private rows, the file as a card");
});

// ── 10m R10: the scratch pad stays on this device (2026-10-10) ──────────
// docs/design/blocking-and-safe-mode.md 10m R10, the web items: no typing indicator while it is
// open; the rows a /command writes are private rows; the send-time check counts it as a private
// view (a Pin, reaction, edit or delete never reaches the socket); a reply made there is kept on
// the note as a quote and the reply bar clears; its header, channel description and row tooltip
// say where a note and a file are kept. These tests use the honest page (fakeDom): a row control
// that is not drawn is not there. Each was run against web/ as at bf8c4c582 (HOS_WEB_DIR) and
// seen failing there, with the assertion named in the list after them.

const SCRATCH_NOTE = "Notes stay in this browser. A file is stored encrypted on the server, and only this browser has its key.";

/** Open the scratch pad on a fresh page, with nothing on the socket or drawn yet. */
async function openScratchPad(page) {
  page.fn("switchChannel")("__scratch__");
  await settle();
  page.sock.sent.length = 0;
  page.sock.raw.length = 0;
  page.state.appended.length = 0;
}
/** Type `text` into the composer and send it the way the Send button does. */
async function typeAndSend(page, text) {
  page.el("msg-input").value = text;
  await page.fn("sendMessage")();
  await settle();
}
/** The first element under `root` (itself included) whose class names `cls`. */
function findByClass(root, cls) {
  if (!root) return null;
  if (String(root.className || "").split(/\s+/).includes(cls)) return root;
  for (const c of root.children || []) { const f = findByClass(c, cls); if (f) return f; }
  return null;
}
const ROW_SERVER_ACTIONS = ["react-btn", "edit-btn", "pin-btn", "mypin-btn", "delete-btn"];

test("10m R10: no typing indicator while the scratch pad is open", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann, { honest: true });
  await openScratchPad(page);
  const keydown = page.el("msg-input").listeners.keydown || [];
  assert.ok(keydown.length, "the composer listens for keys");
  const press = () => { for (const fn of keydown) fn({ key: "a", shiftKey: false, preventDefault() {}, target: page.el("msg-input") }); };
  press();
  press();
  assert.deepEqual(page.sock.sent.filter((m) => m.type === "typing"), [], "no typing indicator goes from the scratch pad");
  page.fn("switchChannel")("general");
  await settle();
  press();
  assert.equal(page.sock.sent.filter((m) => m.type === "typing").length, 1, "in a channel it still goes");
});

test("10m R10: the rows a /command writes in the scratch pad are private rows", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann, { honest: true });
  // As a moderator, so a public row would offer a server Pin and Delete too.
  page.set("(k) => { peerData[k] = Object.assign({}, peerData[k], { role: 'mod' }); }", fx.ann.key);
  await openScratchPad(page);
  for (const cmd of ["/echo hello there", "/time", "/md **bold** text"]) await typeAndSend(page, cmd);
  const rows = page.state.appended.filter((e) => e && e.dataset && e.dataset.from === "__system__");
  assert.equal(rows.length, 3, "each command writes a row");
  for (const row of rows) {
    for (const cls of ROW_SERVER_ACTIONS) {
      assert.ok(!row.innerHTML.includes(`class="${cls}"`), `a command's row offers no ${cls}`);
    }
    assert.ok(row.innerHTML.includes('class="reply-btn"'), "Reply stays");
  }
  assert.equal(page.sock.raw.length, 0, "and nothing went on the socket");
});

test("10m R10: in the scratch pad a Pin, a reaction, an edit or a delete never reaches the socket", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann, { honest: true });
  page.set("(k) => { peerData[k] = Object.assign({}, peerData[k], { role: 'mod' }); }", fx.ann.key);
  await openScratchPad(page);
  page.fn("pinMessageFromUI")(fx.ann.key, "Ann", "a note to myself", T0);
  page.fn("sendReaction")(fx.ann.key, T0, "👍");
  // An edit, pressed through its own Save button.
  const row = page.ctx.document.createElement("div");
  page.fn("startEditMode")(row, "a note to myself", fx.ann.key, T0);
  const save = findByClass(row.querySelector(".body"), "edit-save");
  assert.ok(save && typeof save.onclick === "function", "the edit has a Save button");
  const area = findByClass(row.querySelector(".body"), "edit-area");
  const textarea = area && (area.children || []).find((c) => c.tag === "textarea");
  if (textarea) textarea.value = "a changed note";
  await save.onclick({ stopPropagation() {} });
  // A delete, from a row drawn without the private options (the /command rows used to be).
  page.fn("addChatMessage")("Ann", "an old note", T0 + 5, fx.ann.key, false, false, null, null);
  const drawn = page.state.appended[page.state.appended.length - 1];
  const del = drawn.querySelector(".delete-btn");
  assert.ok(del, "that row has a Delete button");
  for (const fn of del.listeners.click || []) fn({ stopPropagation() {} });
  await settle();
  const leaked = page.sock.sent.filter((m) => ["pin_request", "reaction", "edit", "delete"].includes(m.type));
  assert.deepEqual(leaked, [], "none of them reaches the socket");
});

test("10m R10: a reply made in the scratch pad is kept on the note as a quote, and the reply bar clears", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann, { honest: true });
  await openScratchPad(page);
  await typeAndSend(page, "buy seeds for the garden");
  const first = page.state.appended.find((e) => e && e.dataset && e.dataset.from === fx.ann.key);
  const kept1 = page.store.scratchNotes();
  assert.equal(kept1.length, 1);
  page.fn("setReplyTarget")("Ann", kept1[0].content, fx.ann.key, kept1[0].timestamp, first);
  assert.equal(page.el("reply-bar").style.display, "flex", "the reply bar shows");

  await typeAndSend(page, "done, and the trowel too");
  const kept = page.store.scratchNotes();
  assert.equal(kept.length, 2, "the answer is kept");
  assert.deepEqual(kept[1].replyTo, { from: fx.ann.key, from_name: "Ann", content: "buy seeds for the garden", timestamp: kept1[0].timestamp },
    "with the note it answers kept on it as a quote");
  assert.equal(page.fn("replyTarget"), null, "the reply is cleared");
  assert.equal(page.el("reply-bar").style.display, "none", "and the reply bar hides");
  const answer = page.state.appended.filter((e) => e && e.dataset && e.dataset.from === fx.ann.key)[1];
  assert.ok(answer && answer.innerHTML.includes('class="reply-indicator"'), "the answer is drawn as a reply");
  assert.ok(answer.innerHTML.includes("buy seeds for the garden"), "quoting the note");
  assert.equal(page.sock.raw.length, 0, "nothing went on the socket");

  // Opening the scratch pad again draws it as a reply still.
  page.fn("switchChannel")("general");
  await settle();
  page.state.appended.length = 0;
  page.fn("switchChannel")("__scratch__");
  await settle();
  const again = page.state.appended.filter((e) => e && e.dataset && e.dataset.from === fx.ann.key);
  assert.equal(again.length, 2);
  assert.ok(again[1].innerHTML.includes('class="reply-indicator"') && again[1].innerHTML.includes("buy seeds for the garden"), "the quote is kept with the note");
  assert.ok(!again[0].innerHTML.includes('class="reply-indicator"'), "and only on the answer");
});

test("10m R10: the scratch pad's header, channel description and row tooltip say where a note and a file are kept", async () => {
  const fx = await fixture();
  const timers = [];
  const page = await loadChat(fx.ann, { honest: true, timers, absentIds: ["unified-scratch-row"] });
  page.systemLines.length = 0;
  page.fn("switchChannel")("__scratch__");
  await settle();
  const header = page.el("channel-header").innerHTML;
  assert.ok(header.includes(SCRATCH_NOTE), "the header says it");
  const description = page.fn("SCRATCH_PAD_CHANNEL").description;
  assert.ok(description.startsWith(SCRATCH_NOTE), "and the channel description");
  assert.ok(page.systemLines.some((l) => l.includes(SCRATCH_NOTE)), "and the empty pad's line");
  // The scratch pad's row at the top of the left rail, which the page draws once it has loaded.
  const init = timers.find((t) => t.cb && t.cb.name === "initUnifiedLeftSidebar");
  assert.ok(init, "the page queues the left rail");
  init.cb();
  const row = page.el("unified-scratch-row");
  assert.ok(row, "the scratch pad's row is drawn");
  assert.equal(row.title, "Your scratch pad. " + SCRATCH_NOTE, "and its tooltip says it too");
  for (const said of [header, description, row.title]) {
    assert.ok(!/Nothing sent|Local only/i.test(said), "no longer claiming nothing is sent");
    assert.ok(!said.includes("\u2014") && !/\bseed\b/i.test(said), "plain words, no em dash");
  }
});

// Red first, 2026-10-10 (10m R10), each against web/ as at bf8c4c582 through HOS_WEB_DIR:
//  "no typing indicator goes from the scratch pad" (one went on every key, throttled);
//  "a command's row offers no react-btn" (the /echo, /time and /md rows were public rows);
//  "none of them reaches the socket" (a reaction, a Pin request, an edit and a delete went);
//  "with the note it answers kept on it as a quote" (the note was kept with no reply);
//  "the header says it" (it said "Local only. Nothing sent to anyone.").

// ── 10n N8: the scratch pad moves into the encrypted DM store (2026-10-10) ──
// docs/design/blocking-and-safe-mode.md 10n N8: its notes are kept in the encrypted DM store, for
// this identity on this server, newest 500, as the desktop app keeps its own; the old
// localStorage key `hos_scratch_msgs`, which held file keys in the clear and was shared by every
// identity in the browser, is removed (not read). While the store is still loading nothing is
// kept and the words stay in the composer.
test("10n N8: the scratch pad's notes are kept in the encrypted DM store, per identity and server, newest 500", async () => {
  const fx = await fixture();
  const OLD_NOTE = "the old pad's note, with a file's key in the clear";
  const page = await loadChat(fx.ann, {
    honest: true,
    stored: { hos_scratch_msgs: JSON.stringify([{ from_name: "Ann", from: fx.ann.key, content: OLD_NOTE, timestamp: T0 }]) },
  });
  assert.equal(page.ctx.localStorage.getItem("hos_scratch_msgs"), null, "the old key is gone from this browser");
  await openScratchPad(page);
  const drawn = () => page.systemLines.concat(page.state.appended.map((e) => (e && e.innerHTML) || "")).join("\n");
  assert.ok(!drawn().includes(OLD_NOTE), "and its notes are not read");

  const NOTE = "bulbs go in after the first frost";
  await typeAndSend(page, NOTE);
  assert.deepEqual(page.store.scratchNotes().map((n) => n.content), [NOTE], "the note is kept in the store");
  assert.equal(page.ctx.localStorage.getItem("hos_scratch_msgs"), null, "and not in localStorage");
  await settle();
  const rec = page.ctx.indexedDB.stores.meta.get(`${page.store.scope}:scratch`);
  assert.ok(rec && rec.box && Array.isArray(rec.box.ct), "in its own encrypted record beside the meta box");
  assert.ok(!Buffer.from(rec.box.ct).toString("latin1").includes("bulbs") && !JSON.stringify(rec).includes("bulbs"),
    "which does not hold the words in the clear");

  // Across a reload, for this identity on this server only.
  assert.ok(await page.store.init(fx.ann.key, "localhost"));
  assert.deepEqual(page.store.scratchNotes().map((n) => n.content), [NOTE], "kept across a reload");
  assert.ok(await page.store.init(fx.ann.key, "elsewhere.example"));
  assert.deepEqual(page.store.scratchNotes(), [], "another server's pad is its own");
  assert.ok(await page.store.init(fx.ben.key, "localhost"));
  assert.deepEqual(page.store.scratchNotes(), [], "and another identity's");
  assert.ok(await page.store.init(fx.ann.key, "localhost"));
  assert.deepEqual(page.store.scratchNotes().map((n) => n.content), [NOTE], "and back again");

  // The newest 500.
  for (let i = 0; i < 505; i++) page.store.addScratchNote({ content: `note ${i}`, timestamp: T0 + i });
  await settle();
  const kept = page.store.scratchNotes();
  assert.equal(kept.length, 500, "the newest 500 are kept");
  assert.equal(kept[0].content, "note 5", "the oldest go first");
  assert.ok(await page.store.init(fx.ann.key, "localhost"));
  assert.equal(page.store.scratchNotes().length, 500, "and so after a reload");

  // While the store is still loading, nothing is kept and the words stay in the composer.
  const key = page.store._key;
  page.store._key = null;
  page.systemLines.length = 0;
  page.el("msg-input").value = "a note too early";
  await page.fn("sendMessage")();
  assert.equal(page.el("msg-input").value, "a note too early", "the words stay in the composer");
  assert.ok(page.systemLines.some((l) => l.includes("still loading")), "and the page says the pad is still loading");
  page.store._key = key;
  assert.ok(!page.store.scratchNotes().some((n) => n.content === "a note too early"), "nothing was kept");
});

// Red first, 2026-10-10 (10n N8), against web/ as at 91ac73d78 through HOS_WEB_DIR: "the old key is
// gone from this browser" (it stayed, with its note readable). Then one break at a time in a copy of
// the fixed web/: the removeItem line taken out of app.js: "the old key is gone from this browser";
// app.js scratchPadKeep writing to localStorage again: "the note is kept in the store"; the splice
// taken out of chat-dm-store.js addScratchNote: "the newest 500 are kept"; the record kept unencrypted
// (its box the plain list): "which does not hold the words in the clear".

// A server's room takes what MY limit on that server allows, as its peer list says
// (window.myUploadLimitBytes, 2026-10-10); a private conversation stays at 6 MB. Seen red
// 2026-10-10 against the web before this: "and says so" (the old words named a flat 6 MB max);
// with that line removed, "a 10 MB picture goes into a room within my 300 MB limit".
test("a room takes a file up to my own limit on the server; with no limit said, 6 MB", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  const MB = 1024 * 1024;
  const ten = new Uint8Array(10 * MB);
  ten.set(PNG, 0);
  await paste(page, picture("big.png", ten));
  assert.equal(page.uploads.length, 0, "with no limit said, the 6 MB fallback holds");
  assert.ok(page.systemLines.some((l) => l.includes("over your 6 MB limit on this server")), "and says so");

  page.set("(b) => { window.myUploadLimitBytes = b; }", 300 * MB);
  await paste(page, picture("big.png", ten));
  assert.equal(page.uploads.length, 1, "a 10 MB picture goes into a room within my 300 MB limit");
  assert.ok(!/encrypted=1/.test(page.uploads[0].query), "as a room's plain upload");
});

// The largest file in a private conversation is my role's, as my own peer-list entry says
// (`private_limit_mb`, docs/design/media-showcase.md follow-up B2, 2026-10-11): 6 MB only until
// the server has said it, and my room limit never applies to it, since the server holds an
// encrypted upload to the private limit instead. Seen red 2026-10-11 with attachmentLimit's
// private branch capping the server's limit at 6 MB again, and (alone) with app.js not keeping
// `private_limit_mb`: both "a 7 MB picture goes into a DM within my 10 MB private limit".
test("a private conversation takes a file up to my role's private limit once the server says it", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  const MB = 1024 * 1024;
  const seven = new Uint8Array(7 * MB);
  seven.set(PNG, 0);
  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  await paste(page, picture("big.png", seven));
  assert.equal(page.uploads.length, 0, "with no private limit said, 6 MB holds");
  assert.ok(page.systemLines.some((l) => l.includes("a private conversation can be up to 6 MB for your role on this server")), "and says so");
  // My peer-list entry: a 10 MB private limit, and a room limit of 1 MB that does not apply here.
  page.fn("updatePeerList")([{ public_key: fx.ann.key, private_limit_mb: 10, upload_limit_mb: 1 }]);
  await paste(page, picture("big.png", seven));
  assert.equal(page.uploads.length, 1, "a 7 MB picture goes into a DM within my 10 MB private limit");
  assert.ok(/(^|&)encrypted=1(&|$)/.test(page.uploads[0].query), "encrypted, as every private file is");
  const eleven = new Uint8Array(11 * MB);
  eleven.set(PNG, 0);
  await paste(page, picture("bigger.png", eleven));
  assert.equal(page.uploads.length, 1, "over it, nothing is uploaded");
  assert.ok(page.systemLines.some((l) => l.includes("a private conversation can be up to 10 MB for your role on this server")), "and the words name my limit");
});

// Every upload carries this session's token; before the server has sent one nothing is uploaded
// and the person is told why (the upload review, 2026-10-11: a bare key proved nothing about the
// sender and the server no longer takes it). Seen red 2026-10-11 against the web before this: "no
// upload is made without a token" (it fell back to ?key=).
test("no upload goes without this session's token, and every one carries it", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  page.set("() => { myUploadToken = ''; }");
  await paste(page, picture("image.png"));
  assert.equal(page.uploads.length, 0, "no upload is made without a token");
  assert.ok(page.systemLines.some((l) => l.includes("has not finished signing you in yet")), "and the person is told why");
  page.set("() => { myUploadToken = 'tok-later'; }");
  await paste(page, picture("image.png"));
  assert.equal(page.uploads.length, 1);
  assert.ok(/(^|&)token=tok-later(&|$)/.test(page.uploads[0].query) && !/(^|&)key=/.test(page.uploads[0].query), "carrying the token, never the key: " + page.uploads[0].query);
});

// ── Round 3, review 7: what a browser can hold (docs/design/media-showcase.md) ──
//
// A private file is fetched whole and opened at once (WebCrypto's AES-GCM), so a browser holds it
// about twice while it opens. Pictures are shown in place up to 16 MB, any file is saved up to
// 512 MB, and a larger one is a card saying to open it in the desktop app; an opened picture is
// not downloaded again when its conversation is drawn again; and the sender lets each copy go as
// soon as the next exists.

// Seen red 2026-10-11 with privateFileWay ignoring the size (any picture shown in place, any file
// offered Save): "a friend's 20 MB picture is not fetched by itself" (it was, at once); with the
// server's length check taken out of openPrivateFile: "a marker that says less than its file
// is not opened" (the 17 MB picture was shown).
test("review 7: a picture over 16 MB is a Save card, over 512 MB a card naming the desktop app, and the server's length is held to the marker's", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  befriend(page, fx.ben.key);
  const MB = 1024 * 1024;
  const big = await fileInGroup(page, fx.ben, { name: "big.png", mime: "image/png", bytes: PNG, at: T0 + 300, meta: { size: 20 * MB } });
  const huge = await fileInGroup(page, fx.ben, { name: "huge.mov", mime: "video/quicktime", bytes: PDF, at: T0 + 310, meta: { size: 600 * MB } });
  const small = await fileInGroup(page, fx.ben, { name: "dog.png", mime: "image/png", bytes: PNG, at: T0 + 320 });
  // A marker that says less than its file: a real 17 MB picture on the server, sealed with the
  // marker's key, "45 bytes" in the marker.
  const seventeen = new Uint8Array(17 * MB);
  seventeen.set(PNG, 0);
  const lying = await fileInGroup(page, fx.ben, { name: "tiny.png", mime: "image/png", bytes: seventeen, at: T0 + 330, meta: { size: PNG.length } });
  page.relay.log = [fx.hello, big.msg, huge.msg, small.msg, lying.msg];
  await openGroup(page);
  const [, bigRow, hugeRow, smallRow, lyingRow] = rowsFrom(page, fx.ben);

  // A friend's picture over 16 MB: a Save card with its note, nothing fetched by itself.
  assert.equal(gets(page, big.meta.url), 0, "a friend's 20 MB picture is not fetched by itself");
  assert.ok(bigRow.innerHTML.includes(">Save<") && bigRow.innerHTML.includes("enc-attach-note"), "a card with Save and a note");
  assert.equal(cardBody(bigRow).querySelector(".enc-attach-note").textContent, "This picture is 20.0 MB, larger than is shown here (up to 16 MB). Save it to see it.");
  assert.equal(await shownPicture(page, cardBody(bigRow)), null, "not shown in place");
  await cardBody(bigRow).querySelector(".enc-attach-dl").onclick({ stopPropagation() {} });
  await settle();
  assert.equal(gets(page, big.meta.url), 1, "Save fetches it");
  assert.ok(page.el("__body").children.find((c) => c.tag === "a" && c.download === "big.png"), "and saves it");

  // Over 512 MB: the desktop app's card; nothing to press, nothing fetched.
  assert.ok(!hugeRow.innerHTML.includes(">Save<") && !hugeRow.innerHTML.includes("click to load"), "nothing to press over 512 MB");
  assert.equal(cardBody(hugeRow).querySelector(".enc-attach-note").textContent, "This file is 600.0 MB, more than a browser can open (up to 512 MB). Open it in the HumanityOS desktop app.");
  assert.equal(gets(page, huge.meta.url), 0, "never fetched");

  // Within 16 MB, a friend's picture still shows at once.
  const shown = await shownPicture(page, cardBody(smallRow));
  assert.ok(shown && sameBytes(shown.bytes, PNG), "a friend's small picture shows at once");

  // The server's length is held to the marker's limit: not opened, and nothing shown.
  assert.equal(cardBody(lyingRow).innerHTML, `<div class="enc-attach-failed">${FAILED}</div>`, "a marker that says less than its file is not opened");
  assert.equal(await shownPicture(page, cardBody(lyingRow)), null);
});

// Seen red 2026-10-11 with show() calling openPrivateFile again instead of openPrivatePicture (no
// cache): "the same opened picture / 'blob:test/2' !== 'blob:test/1'" (it was downloaded and
// opened again).
test("review 7: a picture opened once is not downloaded again when its conversation is drawn again", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  befriend(page, fx.ben.key);
  const s = await seal(PNG);
  page.files.set("/uploads/dm-ben.enc", s.ct);
  const meta = { url: "/uploads/dm-ben.enc", k: s.k, n: s.n, name: "dog.png", mime: "image/png", size: PNG.length };
  page.fn("addDmMessage")("Ben", markerText(meta), T0 + 600, fx.ben.key, fx.ann.key, true);
  await settle();
  const first = await shownPicture(page, cardBody(page.state.appended[0]));
  assert.ok(first && sameBytes(first.bytes, PNG), "shown");
  // The conversation drawn again: the same message, a new row.
  page.fn("addDmMessage")("Ben", markerText(meta), T0 + 600, fx.ben.key, fx.ann.key, true);
  await settle();
  const again = await shownPicture(page, cardBody(page.state.appended[1]));
  assert.ok(again, "shown again");
  assert.equal(again.img.src, first.img.src, "the same opened picture");
  assert.equal(gets(page, meta.url), 1, "downloaded once");
});

// Seen red 2026-10-11 with `bytes = null;` taken out of sendEncryptedAttachment: "the plain
// bytes are let go before the upload"; and with `enc.cipherBytes = null;` taken out: "and the
// ciphertext once the upload's Blob holds it".
test("review 7: the sender lets the plain bytes go before the upload, and the ciphertext once the upload holds its own copy", async () => {
  require("node:v8").setFlagsFromString("--expose-gc");
  const gc = require("node:vm").runInNewContext("gc");
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  // What the page encrypts and what encrypting gives back, watched without being held.
  const refs = {};
  const encrypt = page.ctx.pqEncryptFile;
  page.ctx.pqEncryptFile = async (b) => {
    refs.plain = new WeakRef(b.buffer);
    const out = await encrypt(b);
    refs.cipher = new WeakRef(out.cipherBytes.buffer);
    return out;
  };
  const release = page.holdUploads();
  const listeners = page.el("msg-input").listeners.paste || [];
  const big = new Uint8Array(2 * 1024 * 1024);
  big.set(PNG, 0);
  const file = picture("photo.png", big);
  const sending = listeners[0]({ clipboardData: { items: [{ type: file.type, getAsFile: () => file }] }, preventDefault() {} });
  await waitFor(() => page.uploads.length === 1, "the upload is on its way");
  // A WeakRef keeps its target until the turn that made or read it ends; then a full collection.
  await new Promise((r) => setImmediate(r));
  gc();
  await new Promise((r) => setImmediate(r));
  gc();
  assert.equal(refs.plain.deref(), undefined, "the plain bytes are let go before the upload");
  assert.equal(refs.cipher.deref(), undefined, "and the ciphertext once the upload's Blob holds it");
  release();
  await sending;
  await settle();
  assert.equal(page.sock.sent.filter((m) => m.type === "dm_put").length, 2, "and it is sent as ever");
});

// ── Round 4, W4-4: the opened-picture cache keeps its bound (docs/design/media-showcase.md) ──
//
// An opened picture is kept for the page, at most 128 MB of them, the oldest let go first. Its
// place in the cache was set when it was ASKED for (a placeholder while it opens), so a picture that
// finished opening after later ones sat before them, the sweep stopped at it, and nothing was let
// go: the review's 20 pictures finishing in reverse kept 320 MB. On finishing, an entry is now
// taken out and put back, so it is the newest.

// Seen red 2026-10-11 with the `privateFilesOpened.delete(key);` before the set taken out (round
// 3's): "held to 128 MB however they finish / 335544320 bytes kept".
test("W4-4: pictures that finish opening out of order are still let go oldest first, so the cache keeps its 128 MB bound", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  const MB = 1024 * 1024;
  // The fetch and the opening stood in for: each picture is 16 MB and finishes when the test says.
  const finish = new Map();
  let opens = 0;
  page.ctx.openPrivateFile = (meta) => { opens++; return new Promise((r) => finish.set(meta.url, () => r({ size: 16 * MB }))); };
  const metas = Array.from({ length: 20 }, (_, i) => ({ url: "/uploads/p" + i + ".enc", k: "k" + i, n: "n", name: "p" + i + ".png", mime: "image/png", size: 16 * MB }));
  const open = page.fn("openPrivatePicture");
  const shown = metas.map((m) => open(m));
  await settle();
  assert.equal(opens, 20, "all twenty asked for");
  // They finish in reverse: the last asked for first.
  for (const m of metas.slice().reverse()) {
    finish.get(m.url)();
    await settle();
  }
  const urls = await Promise.all(shown);
  assert.ok(urls.every((u) => typeof u === "string" && u.startsWith("blob:")), "each one shown");
  const bytes = page.fn("privateOpenedBytes");
  assert.ok(bytes <= 128 * MB, "held to 128 MB however they finish / " + bytes + " bytes kept");
  const kept = Array.from(page.fn("privateFilesOpened").keys());
  assert.equal(kept.length, 8, "eight 16 MB pictures");
  const keyOf = (m) => m.url + "\n" + m.k;
  assert.deepEqual(kept, metas.slice(0, 8).reverse().map(keyOf), "the last eight to finish, oldest first");
  // One shown again is not opened again.
  const again = await open(metas[0]);
  assert.equal(again, urls[0], "the newest is kept");
  assert.equal(opens, 20, "and not opened again");
});

// ══ Files on the sender's device and in messages (2026-10-11) ══════════════════════════════
// docs/design/media-showcase.md, "Files on the sender's device and in messages, exact", parts 1
// to 4, the chat page's side: every file is cleaned on this device before it is uploaded or
// encrypted, with the attach step and its switch (part 1); a file that could not be checked
// (part 2); Files in messages (part 3); a copy of each private file opened, kept here (part 4).
// The cleaning, the rules and the kept-file store are tested on their own in
// upload-clean-web.test.js, files-in-messages-web.test.js and kept-files-web.test.js; these hold
// the chat page to them. The red runs are listed at the end of this section.

const S = require(path.join(WEB, "shared", "strip-metadata.js"));
const UC = require(path.join(WEB, "shared", "upload-clean.js"));
const FIM = require(path.join(WEB, "shared", "files-in-messages.js"));
const B = require("./file-bytes.js");
const MARKER_FIXTURE = JSON.parse(fs.readFileSync(path.join(__dirname, "fixtures", "file-marker-unchecked.json"), "utf8"));

const PHOTO = B.phoneJpeg();
const PHOTO_CLEAN = S.stripFile(PHOTO, "IMG_0001.jpg").bytes;
const photo = (name = "IMG_0001.jpg") => new File([PHOTO], name, { type: "image/jpeg" });
// A video cut before its movie index: where it was filmed cannot be checked.
const CUT_VIDEO = B.movie("mdatFirst", B.ftypMp4(), B.everything()).subarray(0, 300);
const cutVideo = () => new File([CUT_VIDEO], "clip.mp4", { type: "video/mp4" });
const heicPhoto = () => new File([B.heic()], "IMG_0002.HEIC", { type: "image/heic" });

/** Every element under `el` (the stand-in keeps children in `children`). */
function everyUnder(el, out = []) {
  for (const c of (el && el.children) || []) { out.push(c); everyUnder(c, out); }
  return out;
}
/** The attach step on screen now (the newest one put on the page and not taken off), or null. */
function attachStep(page) {
  const all = page.el("__body").children.filter((c) => c && c.id === "attach-step-overlay" && !c.removed);
  return all.length ? all[all.length - 1] : null;
}
/** The step's parts, by what they are. */
function stepParts(step) {
  const els = everyUnder(step);
  const byClass = (cls) => els.find((e) => String(e.className).split(/\s+/).includes(cls)) || null;
  const byStep = (what) => els.find((e) => e.dataset && e.dataset.attachStep === what) || null;
  return {
    summary: byClass("attach-step-summary"), said: byClass("attach-step-said"), off: byClass("attach-step-off"),
    recipient: byClass("attach-step-recipient"), protectedLine: byClass("attach-step-protected"),
    details: els.filter((e) => e.tag === "li").map((e) => e.textContent),
    remove: byStep("remove"), send: byStep("send"), cancel: byStep("cancel"),
    switchLabel: (byClass("attach-step-switch") ? everyUnder(byClass("attach-step-switch")).map((e) => e.textContent).join("") : ""),
  };
}
/**
 * Start sending `file` through the composer's paste (not awaited), and wait for its attach step.
 * The send's promise comes back inside an object ({going}): handed back bare, awaiting this would
 * wait for the whole send, which waits for a button on the step.
 */
async function pasteToStep(page, file) {
  const listeners = page.el("msg-input").listeners.paste || [];
  const ev = { clipboardData: { items: [{ type: file.type, getAsFile: () => file }] }, preventDefault() {} };
  const going = (async () => { for (const fn of listeners) await fn(ev); })();
  await waitFor(() => attachStep(page), "the attach step is shown");
  return { going };
}
/** The same through the attach button. */
async function pickToStep(page, file) {
  const going = page.fn("handleFileAttachment")({ target: { files: [file], value: "x" } });
  await waitFor(() => attachStep(page), "the attach step is shown");
  return { going };
}
/** Press one of the step's buttons, then let the send finish. */
async function press(page, going, which) {
  const p = stepParts(attachStep(page));
  p[which].onclick();
  await going;
  await settle();
}
const hasQuery = (up, k) => new RegExp("(^|&)" + k + "(&|$)").test(up.query);

// ── Part 1: cleaned on this device ──────────────────────────────────────

test("files: a phone photo pasted in a room shows the attach step, switch on; Send uploads the cleaned photo, no keep_metadata", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  const { going } = await pasteToStep(page, photo());
  const p = stepParts(attachStep(page));
  assert.equal(p.summary.textContent, "This photo says where it was taken and which camera took it.", "what it holds");
  assert.ok(p.details.includes("Where it was taken"), "with the details in plain words");
  assert.ok(p.remove && p.remove.checked === true, "the switch, on by default");
  assert.ok(p.switchLabel.includes("Remove location and camera details"), "with its label");
  assert.equal(page.uploads.length, 0, "nothing uploaded while the step is open");
  await press(page, going, "send");
  assert.equal(page.uploads.length, 1, "one upload");
  const up = page.uploads[0];
  assert.ok(sameBytes(up.bytes, PHOTO_CLEAN), "the cleaned photo, the strip's own bytes");
  assert.ok(!B.contains(up.bytes, "SN-0042-SECRET"), "the camera's serial number gone");
  assert.ok(!hasQuery(up, "keep_metadata=1"), "no keep_metadata");
  assert.equal(up.name, "IMG_0001.jpg", "under its own name");
  assert.equal(page.ctx.localStorage.getItem(UC.REMOVE_DETAILS_KEY), "1", "the switch remembered as the last choice");
  assert.equal(page.sock.sent.filter((m) => m.type === "chat").pop().content, up.url, "and its address posted");
});

test("files: with the switch off the photo goes as it was with keep_metadata=1, and the next step starts off", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  let { going } = await pasteToStep(page, photo());
  let p = stepParts(attachStep(page));
  p.remove.checked = false;
  p.remove.onchange();
  assert.equal(p.off.textContent, UC.WORDS.switchOffLine, "it says everything goes");
  await press(page, going, "send");
  const up = page.uploads[0];
  assert.ok(sameBytes(up.bytes, PHOTO), "the photo exactly as picked");
  assert.ok(hasQuery(up, "keep_metadata=1"), "with keep_metadata=1, so the server keeps it too");
  assert.equal(page.ctx.localStorage.getItem(UC.REMOVE_DETAILS_KEY), "0", "remembered");
  ({ going } = await pasteToStep(page, photo("IMG_0003.jpg")));
  p = stepParts(attachStep(page));
  assert.equal(p.remove.checked, false, "the next attach step starts as last chosen");
  await press(page, going, "cancel");
  assert.equal(page.uploads.length, 1, "Cancel sends nothing");
});

test("files: in a DM the photo is cleaned before it is encrypted; the sealed upload opens to the cleaned bytes", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  page.sock.sent.length = 0;
  const { going } = await pasteToStep(page, photo());
  await press(page, going, "send");
  const up = oneEncryptedUpload(page, PHOTO);
  const toBen = page.sock.sent.filter((m) => m.type === "dm_put").find((x) => opened(x).sealedTo === kyberOf(fx.ben.key));
  const meta = markerMeta(opened(toBen).inner.text);
  assert.ok(sameBytes(await unseal(up.bytes, meta.k, meta.n), PHOTO_CLEAN), "opens to the cleaned photo");
  assert.equal(meta.size, PHOTO_CLEAN.length, "its size is the cleaned photo's");
  assert.ok(!("unchecked" in meta), "a file that was checked says nothing about it");
});

// ── Part 2: a file that could not be checked ────────────────────────────

test("files: a video cut before its movie is not sent to a room (Send off, the words), and goes with keep_metadata=1 once kept by choice", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  let { going } = await pickToStep(page, cutVideo());
  let p = stepParts(attachStep(page));
  assert.equal(p.send.disabled, true, "Send is off");
  assert.ok(p.said.textContent.includes(UC.WORDS.publicRefused) && p.said.textContent.includes(UC.WORDS.publicKeepHint), "and the words say why and how: " + p.said.textContent);
  p.send.onclick();
  await settle();
  assert.equal(page.uploads.length, 0, "pressing it does nothing");
  await press(page, going, "cancel");
  assert.equal(page.uploads.length, 0, "nothing uploaded");

  ({ going } = await pickToStep(page, cutVideo()));
  p = stepParts(attachStep(page));
  p.remove.checked = false;
  p.remove.onchange();
  assert.equal(p.send.disabled, false, "kept by choice, it may go");
  await press(page, going, "send");
  assert.equal(page.uploads.length, 1);
  assert.ok(sameBytes(page.uploads[0].bytes, CUT_VIDEO) && hasQuery(page.uploads[0], "keep_metadata=1"), "as picked, with keep_metadata=1");
});

test("files: in a DM it asks; Send anyway marks the marker unchecked, and the recipient's card says so", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  page.sock.sent.length = 0;
  let { going } = await pickToStep(page, cutVideo());
  let p = stepParts(attachStep(page));
  assert.equal(p.said.textContent, UC.WORDS.askPrivate, "the spec's question");
  assert.equal(p.send.textContent, "Send anyway");
  await press(page, going, "cancel");
  assert.equal(page.uploads.length, 0, "Cancel: nothing uploaded");

  ({ going } = await pickToStep(page, cutVideo()));
  await press(page, going, "send");
  const up = oneEncryptedUpload(page, CUT_VIDEO);
  const toBen = page.sock.sent.filter((m) => m.type === "dm_put").find((x) => opened(x).sealedTo === kyberOf(fx.ben.key));
  const text = opened(toBen).inner.text;
  const meta = markerMeta(text);
  assert.equal(meta.unchecked, true, "the marker says unchecked");
  assert.deepEqual(Object.keys(meta), Object.keys(MARKER_FIXTURE.built[0].meta), "its fields in the shared fixture's order (the desktop app's)");
  // What could be cleaned before the walk stopped was (the XMP box at the top of the file), the rest
  // goes as it is: the strip's own bytes for the cut file.
  const partly = S.stripFile(CUT_VIDEO, "clip.mp4");
  assert.ok(partly.unfinished && partly.changed, "test setup: the strip cleans part of it and stops");
  assert.ok(sameBytes(await unseal(up.bytes, meta.k, meta.n), partly.bytes), "the file as far as it could be cleaned");

  // Ben's page draws it with the line under its name.
  const ben = await loadChat(fx.ben);
  ben.files.set(meta.url, up.bytes);
  ben.fn("addDmMessage")("Ann", text, T0 + 900, fx.ann.key, fx.ben.key, true);
  await settle();
  const row = ben.state.appended[0];
  assert.ok(row.innerHTML.includes("enc-attach-unchecked"), "the card has the line");
  assert.equal(row.querySelector(".enc-attach").querySelector(".enc-attach-unchecked").textContent, UC.WORDS.recipientLine, "in the spec's words");
});

test("files: a HEIC photo, which the website cannot clean yet: to a room it goes to the server, which cleans it; in a DM the HEIC question", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  await page.fn("handleFileAttachment")({ target: { files: [heicPhoto()], value: "x" } });
  await settle();
  assert.equal(attachStep(page), null, "no step in a room");
  assert.equal(page.uploads.length, 1, "sent to the room's server, which cleans HEIC photos itself");
  assert.ok(!hasQuery(page.uploads[0], "keep_metadata=1"), "without keep_metadata, so the server strips it");
  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  const { going } = await pickToStep(page, heicPhoto());
  const p = stepParts(attachStep(page));
  assert.equal(p.said.textContent, UC.WORDS.askPrivateHeif, "in a DM, asked: the server never opens a private file");
  await press(page, going, "cancel");
  assert.equal(page.uploads.length, 1, "nothing more sent");
});

test("files: with the protected setup on, no switch, a photo is always cleaned, and an unchecked file goes nowhere, a DM included", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann, { stored: { [UC.REMOVE_DETAILS_KEY]: "0" } });
  await page.fn("loadProtectedPreset")();
  await turnOn(page);
  // Nothing to decide: cleaned and sent at once, the switch's "keep" ignored.
  await paste(page, photo());
  assert.equal(page.uploads.length, 1, "sent with no step");
  assert.ok(sameBytes(page.uploads[0].bytes, PHOTO_CLEAN) && !hasQuery(page.uploads[0], "keep_metadata=1"), "cleaned, whatever the switch last said");

  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  const { going } = await pickToStep(page, cutVideo());
  const p = stepParts(attachStep(page));
  assert.equal(p.remove, null, "no switch");
  assert.equal(p.protectedLine.textContent, UC.WORDS.protectedLine, "the protected setup's line instead");
  assert.equal(p.send.disabled, true, "and it cannot go");
  assert.ok(p.said.textContent.includes(UC.WORDS.protectedRefused), "saying why");
  await press(page, going, "cancel");
  assert.equal(page.uploads.length, 1, "nothing more uploaded");
});

// ── The shared marker fixture (with the desktop app) ────────────────────

test("files: the unchecked marker is built and read exactly as the shared fixture says", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ben);
  assert.equal(page.fn("FILE_MARKER"), MARKER_FIXTURE.marker);
  for (const b of MARKER_FIXTURE.built) {
    assert.equal(page.fn("pqBuildFileMarker")(b.meta), b.text, `${b.name}: built byte for byte`);
    assert.equal(JSON.stringify(b.meta), b.json, `${b.name}: its JSON`);
  }
  for (const r of MARKER_FIXTURE.read) {
    const meta = page.fn("pqParseFileMarker")(r.text);
    assert.ok(meta, `${r.name}: read`);
    assert.equal(meta.unchecked === true, r.unchecked, `${r.name}: unchecked ${r.unchecked}`);
  }
  // A received fixture marker is drawn with the line; a checked one without it.
  page.fn("addDmMessage")("Ann", MARKER_FIXTURE.built[0].text, T0 + 1, fx.ann.key, fx.ben.key, true);
  page.fn("addDmMessage")("Ann", MARKER_FIXTURE.built[1].text, T0 + 2, fx.ann.key, fx.ben.key, true);
  await settle();
  const [unchecked, checked] = page.state.appended;
  assert.ok(unchecked.innerHTML.includes("enc-attach-unchecked"), "the unchecked one has the line");
  assert.ok(!checked.innerHTML.includes("enc-attach-unchecked"), "the checked one has none");
});

// ── Part 3: Files in messages ───────────────────────────────────────────

test("files: by default a non-friend's file waits for Accept, nothing fetched; Accept opens that picture; a document's Accept offers Save", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  befriend(page, fx.ben.key);
  const benPic = await fileInGroup(page, fx.ben, { name: "dog.png", mime: "image/png", bytes: PNG, at: T0 + 300 });
  const cyPic = await fileInGroup(page, fx.cy, { name: "cat.png", mime: "image/png", bytes: PNG, at: T0 + 400 });
  const cyPdf = await fileInGroup(page, fx.cy, { name: "notes.pdf", mime: "application/pdf", bytes: PDF, at: T0 + 500 });
  page.relay.log = [fx.hello, benPic.msg, cyPic.msg, cyPdf.msg];
  await openGroup(page);
  const [, benRow] = rowsFrom(page, fx.ben);
  const [cyPicRow, cyPdfRow] = rowsFrom(page, fx.cy);
  assert.ok(await shownPicture(page, cardBody(benRow)), "a friend's picture shows at once");

  for (const [row, f] of [[cyPicRow, cyPic], [cyPdfRow, cyPdf]]) {
    assert.ok(row.innerHTML.includes("enc-attach-accept") && !row.innerHTML.includes("click to load") && !row.innerHTML.includes(">Save<"), `${f.meta.name}: only Accept`);
    assert.equal(cardBody(row).querySelector(".enc-attach-ask").textContent, "Cy sent a file. You accept files only from friends. Accept this one?", `${f.meta.name}: the spec's words`);
    assert.equal(gets(page, f.meta.url), 0, `${f.meta.name}: nothing fetched`);
  }
  await cardBody(cyPicRow).querySelector(".enc-attach-accept").onclick({ stopPropagation() {} });
  await settle();
  const shown = await shownPicture(page, cardBody(cyPicRow));
  assert.ok(shown && sameBytes(shown.bytes, PNG), "Accept: that picture fetched and shown");
  assert.equal(gets(page, cyPic.meta.url), 1);

  await cardBody(cyPdfRow).querySelector(".enc-attach-accept").onclick({ stopPropagation() {} });
  await settle();
  assert.ok(cardBody(cyPdfRow).innerHTML.includes(">Save<"), "Accept on a document: its Save");
  assert.equal(gets(page, cyPdf.meta.url), 0, "fetched only when Save is pressed");
});

test("files: Accept files from Nobody asks even for a friend's; Show pictures from Nobody makes a friend's picture wait for the click", async () => {
  const fx = await fixture();
  let page = await loadChat(fx.ann, { stored: { [FIM.FILES_SETTINGS_KEY]: JSON.stringify({ accept: "nobody" }) } });
  befriend(page, fx.ben.key);
  let benPic = await fileInGroup(page, fx.ben, { name: "dog.png", mime: "image/png", bytes: PNG, at: T0 + 300 });
  page.relay.log = [fx.hello, benPic.msg];
  await openGroup(page);
  let [, benRow] = rowsFrom(page, fx.ben);
  assert.equal(cardBody(benRow).querySelector(".enc-attach-ask").textContent, "Ben sent a file. You do not accept files in messages. Accept this one?", "Nobody: a friend's file asks too");
  assert.equal(gets(page, benPic.meta.url), 0);

  page = await loadChat(fx.ann, { stored: { [FIM.FILES_SETTINGS_KEY]: JSON.stringify({ pictures: "nobody" }) } });
  befriend(page, fx.ben.key);
  benPic = await fileInGroup(page, fx.ben, { name: "dog.png", mime: "image/png", bytes: PNG, at: T0 + 300 });
  page.relay.log = [fx.hello, benPic.msg];
  await openGroup(page);
  [, benRow] = rowsFrom(page, fx.ben);
  assert.ok(benRow.innerHTML.includes("Image (click to load)") && !benRow.innerHTML.includes("enc-attach-accept"), "accepted, but the picture waits for the click");
  assert.equal(gets(page, benPic.meta.url), 0, "nothing fetched before it");
});

// ── The profile: only a change made in this browser goes (BUG-195, 2026-10-11) ──
// Every connect used to push the stored profile, so a change made on another device (or the desktop
// app) was undone the next time this browser connected; and the stored profile took the server's
// copy only when it was empty.

test("profile: a connect sends nothing unless it changed here; my own copy from the server is taken in, the public copy of my save is not", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  const ls = page.ctx.localStorage;
  const updates = () => page.sock.sent.filter((m) => m.type === "profile_update");
  const stored = () => JSON.parse(ls.getItem("humanity_profile") || "{}");
  ls.setItem("humanity_profile", JSON.stringify({ bio: "Old bio", socials: { twitter: "old" }, location: "Lisbon", privacy: {} }));
  // The /profile page's drafts: the bio and the x link as last saved, the location being typed.
  ls.setItem("humanity_profile_full_v1", JSON.stringify({ bio: "Old bio", location: "Lisboa (typing)", twitter: "https://x.com/old", "directory-listed": true, "first-name": "Ann" }));
  ls.setItem("humanity_profile_privacy", JSON.stringify({ location: "public" }));
  const page_ = () => JSON.parse(ls.getItem("humanity_profile_full_v1"));
  page.sock.sent.length = 0;
  await page.fn("syncProfileOnConnect")();
  await settle();
  assert.equal(updates().length, 0, "a connect does not push a profile that did not change here");

  // My own copy, changed on another device: taken in, keeping the stored profile's other keys.
  ls.setItem("humanity_profile", JSON.stringify(Object.assign(stored(), { draft_note: "mine" })));
  await page.handle({ type: "profile_data", name: "Ann", bio: "New bio from my phone", socials: '{"discord":"ann","twitter":"annie"}', location: "Porto", privacy: '{"directory":"unlisted","location":"private"}', accepts_files: "friends" });
  assert.deepEqual(
    [stored().bio, stored().location, stored().socials, stored().privacy, stored().draft_note],
    ["New bio from my phone", "Porto", { discord: "ann", twitter: "annie" }, { directory: "unlisted", location: "private" }, "mine"],
    "my own copy is taken in",
  );
  // The /profile page follows it where its fields still hold what was last synced.
  assert.deepEqual(
    [page_().bio, page_().location, page_().twitter, page_().discord, page_()["directory-listed"], page_()["first-name"]],
    ["New bio from my phone", "Lisboa (typing)", "annie", "ann", false, "Ann"],
    "the page follows, keeping what is being typed there and its private fields",
  );
  assert.equal(JSON.parse(ls.getItem("humanity_profile_privacy")).location, "private", "and its privacy lock");
  // The public copy of my save names me too but carries no privacy map (and leaves fields out).
  await page.handle({ type: "profile_data", name: "Ann", bio: "New bio from my phone", socials: "{}", accepts_files: "friends" });
  assert.equal(stored().location, "Porto", "the public copy of my save changes nothing here");
  assert.equal(updates().length, 0, "and nothing went back");

  // A change saved on the /profile page: marked unsent, it goes, and is no longer waiting.
  ls.setItem("humanity_profile_unsent", "1");
  ls.setItem("humanity_profile", JSON.stringify(Object.assign(stored(), { bio: "Typed here" })));
  await page.fn("pushProfileToRelay")();
  assert.deepEqual(updates().map((u) => [u.bio, u.location]), [["Typed here", "Porto"]], "a change made here goes, whole");
  assert.equal(ls.getItem("humanity_profile_unsent"), null, "and no longer waits");

  // A copy from the server while a change of mine waits is not taken in (mine goes first).
  ls.setItem("humanity_profile_unsent", "1");
  ls.setItem("humanity_profile", JSON.stringify(Object.assign(stored(), { bio: "Typed again" })));
  await page.handle({ type: "profile_data", name: "Ann", bio: "Typed here", socials: "{}", location: "Porto", privacy: "{}" });
  assert.equal(stored().bio, "Typed again", "a waiting change of mine is not overwritten");
});

/** Open Safety and find the Files in messages section's controls (the newest drawn). */
function filesSection(page) {
  page.fn("openSafetyPanel")();
  const box = page.el("safety-card").querySelector("[data-files-in-messages]");
  const els = everyUnder(box);
  const last = (what) => els.filter((e) => e.dataset && e.dataset.filesSetting === what).pop() || null;
  return { box, els, accept: last("accept"), pictures: last("pictures"), attach: last("attach"), text: els.map((e) => e.textContent).join(" | ") };
}

test("files: Safety shows the three settings; a change is kept, published as accepts_files, and the attach button follows it", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  let s = filesSection(page);
  assert.ok(s.text.includes(FIM.WORDS.group), "the group, in Safety");
  assert.equal(s.accept.value, "friends", "Accept files from: Friends");
  assert.deepEqual(s.accept.children.map((o) => o.textContent), ["Nobody", "Friends", "Anyone I talk to"]);
  assert.equal(s.pictures.value, "friends", "Show pictures without asking from: Friends");
  assert.equal(s.attach.checked, true, "the attach button: on");

  page.sock.sent.length = 0;
  s.accept.value = "nobody";
  s.accept.onchange();
  await settle();
  assert.equal(FIM.readSettings(page.ctx.localStorage).accept, "nobody", "kept on this device");
  // The relay saves a profile_update whole (every field left out is saved empty): with nothing in
  // the local profile, nothing goes until the server's copy of mine has been heard.
  const updates = () => page.sock.sent.filter((m) => m.type === "profile_update");
  assert.equal(updates().length, 0, "an empty local profile sends nothing before the server's copy is heard");
  // The server's copy of my profile, as it sends it on connect (written on another device): my own
  // copy carries my privacy map.
  await page.handle({ type: "profile_data", name: "Ann", bio: "I grow tomatoes", socials: "{}", pronouns: "she/her", accepts_files: "friends", privacy: "{}" });
  const update = updates().pop();
  assert.ok(update, "then the profile goes to the server");
  assert.equal(update.accepts_files, "nobody", "with accepts_files");
  assert.deepEqual([update.bio, update.pronouns], ["I grow tomatoes", "she/her"], "and the whole profile (the server's copy, filled in here first)");
  assert.equal(page.fn("buildProfileUpdate")(page.fn("loadProfileLocal")()).accepts_files, undefined, "once sent, an update leaves it out (the server keeps its word)");
  // Another device chose Anyone: my own profile comes back with it, and this device takes it.
  await page.handle({ type: "profile_data", name: "Ann", bio: "I grow tomatoes", socials: "{}", accepts_files: "anyone", privacy: "{}" });
  assert.equal(FIM.readSettings(page.ctx.localStorage).accept, "anyone", "a choice made on another device is taken here");

  s = filesSection(page);
  s.attach.checked = false;
  s.attach.onchange();
  assert.equal(FIM.readSettings(page.ctx.localStorage).attach, false);
  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  assert.equal(page.el("attach-btn").style.display, "none", "no attach button in a DM");
  page.uploads.length = 0;
  await paste(page, picture("image.png"));
  assert.equal(page.uploads.length, 0, "and a paste there sends nothing");
  assert.ok(page.systemLines.includes(FIM.WORDS.attachOff), "saying where it is turned off");
  page.fn("switchChannel")("general");
  await settle();
  assert.equal(page.el("attach-btn").style.display, "", "the button is back in a room");
  await paste(page, picture("image.png"));
  assert.equal(page.uploads.length, 1, "where a file still goes");
});

test("files: with the protected setup on, opening Accept files from up needs the PIN, closing it never does; pictures are a line", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  await page.fn("loadProtectedPreset")();
  await turnOn(page);
  let s = filesSection(page);
  assert.equal(s.pictures, null, "no pictures choice");
  assert.ok(s.text.includes(FIM.WORDS.picturesProtected), "a line instead");
  s.accept.value = "anyone";
  s.accept.onchange();
  await settle();
  assert.equal(FIM.readSettings(page.ctx.localStorage).accept, "friends", "not changed before the PIN");
  assert.equal(await page.fn("protectedPinSubmit")(PIN), true, "the PIN");
  await settle();
  assert.equal(FIM.readSettings(page.ctx.localStorage).accept, "anyone", "then changed");
  s = filesSection(page);
  s.accept.value = "nobody";
  s.accept.onchange();
  await settle();
  assert.equal(FIM.readSettings(page.ctx.localStorage).accept, "nobody", "closing needs no PIN");
});

test("files: a DM to someone who accepts files only from friends says so in the attach step; their profile is asked for once", async () => {
  const fx = await fixture();
  const page = await loadChat(fx.ann);
  page.sock.sent.length = 0;
  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  const asks = () => page.sock.sent.filter((m) => m.type === "profile_request");
  assert.deepEqual(asks().map((m) => m.name), ["Ben"], "Ben's profile asked for");
  page.fn("openDmConversation")(fx.ben.key, "Ben");
  await settle();
  assert.equal(asks().length, 1, "once");
  await page.handle({ type: "profile_data", name: "Ben", bio: "", socials: "{}", accepts_files: "friends" });

  // A screenshot with nothing hidden: the step only for the line.
  const { going } = await pasteToStep(page, picture("image.png"));
  const p = stepParts(attachStep(page));
  assert.ok(p.recipient.textContent.startsWith("Ben accepts files only from friends."), "the spec's words: " + p.recipient.textContent);
  assert.equal(p.summary, null, "and nothing about hidden details");
  await press(page, going, "send");
  assert.equal(page.uploads.length, 1, "it still goes");

  // Once Ben counts me a friend (we follow each other and Ben gave me a pass), nothing is said.
  page.store.setFollowing(fx.ben.key, true);
  page.store.setFollower(fx.ben.key, true);
  page.store.storeCertFrom(fx.ben.key, { v: 2, serial: "22".repeat(16), may: MAY, sig: "c2ln" });
  await paste(page, picture("image.png"));
  assert.equal(attachStep(page), null, "no step");
  assert.equal(page.uploads.length, 2, "sent at once");
});

// ── Part 4: a copy of each private file opened ──────────────────────────

test("files: a private file opened is kept here, encrypted, and opens after a reload with the server's copy gone", async () => {
  const fx = await fixture();
  const idb = fakeIndexedDB();
  const storeKey = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  const s = await seal(PNG);
  const meta = { url: "/uploads/dm-ben.enc", k: s.k, n: s.n, name: "dog.png", mime: "image/png", size: PNG.length };

  let page = await loadChat(fx.ann, { indexedDB: idb, storeKey });
  befriend(page, fx.ben.key);
  page.files.set(meta.url, s.ct);
  page.fn("addDmMessage")("Ben", markerText(meta), T0 + 700, fx.ben.key, fx.ann.key, true);
  await settle();
  assert.ok(await shownPicture(page, cardBody(page.state.appended[0])), "shown");
  assert.equal(gets(page, meta.url), 1, "fetched once");
  const kept = [...(idb.stores.files || new Map()).values()];
  assert.equal(kept.length, 1, "one copy kept on this device");
  assert.ok(!Buffer.from(kept[0].ct).includes(Buffer.from("a picture of a dog")), "encrypted");

  // A reload: the same browser storage and identity, and the server no longer has the file.
  page = await loadChat(fx.ann, { indexedDB: idb, storeKey });
  befriend(page, fx.ben.key);
  page.fn("addDmMessage")("Ben", markerText(meta), T0 + 700, fx.ben.key, fx.ann.key, true);
  await settle();
  const pic = await shownPicture(page, cardBody(page.state.appended[0]));
  assert.ok(pic && sameBytes(pic.bytes, PNG), "it still opens");
  assert.equal(gets(page, meta.url), 0, "from the copy here, without asking the server");

  // With the cap at nothing, nothing is kept.
  const idb2 = fakeIndexedDB();
  page = await loadChat(fx.ann, { indexedDB: idb2, storeKey, stored: { hos_kept_files_cap_mb: "0" } });
  befriend(page, fx.ben.key);
  page.files.set(meta.url, s.ct);
  page.fn("addDmMessage")("Ben", markerText(meta), T0 + 700, fx.ben.key, fx.ann.key, true);
  await settle();
  assert.ok(await shownPicture(page, cardBody(page.state.appended[0])), "shown");
  assert.equal((idb2.stores.files || new Map()).size, 0, "a cap of nothing keeps nothing");
});

// ── Red first (this section) ─────────────────────────────────────────────
// 2026-10-11: each break made in a copy of web/ and this file run against it with HOS_WEB_DIR,
// seen failing with the assertion named; every other test passed.
//  P1 chat-messages.js, uploadImage appending the file as picked (`file`) instead of `ready.file`:
//     "files: a phone photo pasted in a room ... the cleaned photo, the strip's own bytes".
//  P2 chat-messages.js, uploadImage without the keep_metadata query: "files: with the switch off
//     ... with keep_metadata=1, so the server keeps it too".
//  P3 chat-messages.js, sendEncryptedAttachment encrypting `file` instead of the cleaned one:
//     "files: in a DM ... opens to the cleaned photo".
//  P4 chat-messages.js, the marker built without `unchecked`: "files: in a DM it asks ... the
//     marker says unchecked".
//  P5 chat-dms.js, the unchecked line left out of privateFileHtml: "files: in a DM it asks ... the
//     card has the line", and the fixture test "the unchecked one has the line".
//  P6 chat-dms.js, privateFileAccepted always true: "files: by default a non-friend's file ...
//     cat.png: only Accept".
//  P7 chat-dms.js, privateFilePictureNow ignoring the pictures setting (friends shown at once):
//     "files: Accept files from Nobody ... accepted, but the picture waits for the click".
//  P8 chat-profile.js, buildProfileUpdate without accepts_files: "files: Safety shows ... with
//     accepts_files".
//  P8b chat-profile.js, an empty local profile pushed for a waiting choice before the server's copy
//     of mine was heard (the guard against a whole-profile save wiping it): "files: Safety shows
//     ... an empty local profile sends nothing before the server's copy is heard".
//  P9 chat-messages.js, sendAttachment not checking the attach setting: "files: Safety shows ...
//     and a paste there sends nothing".
//  P10 chat-privacy.js, chooseFilesSetting without the PIN gate: "files: with the protected setup
//     on ... not changed before the PIN".
//  P11 chat-messages.js, acceptsFilesLine always null: "files: a DM to someone who accepts files
//     only from friends ... the attach step is shown" (never happened).
//  P12 chat-dms.js, openPrivateFile not looking at the kept copy first: "files: a private file
//     opened is kept here ... it still opens".
//  P13 chat-dm-store.js, keptFilePut doing nothing: "files: a private file opened ... one copy
//     kept on this device".
//  P14 chat-profile.js, ownProfileArrived not taking another device's choice: "files: Safety shows
//     ... a choice made on another device is taken here".
//  P15 chat-profile.js, pushProfileToRelay without the unsent check (the push on every connect, as
//     before BUG-195): "profile: a connect sends nothing ... a connect does not push a profile that
//     did not change here".
//  P16 chat-profile.js, adoptOwnProfile not asking for the privacy map: "profile: ... the public
//     copy of my save changes nothing here".
//  P17 chat-profile.js, adoptOwnProfile not checking for a waiting change: "profile: ... a waiting
//     change of mine is not overwritten".
//  P18 chat-profile.js, followOnProfilePage replacing every page field (no untouched check): "the
//     page follows, keeping what is being typed there and its private fields".
// And the tests above this section that this increment changed: "a marker in a group message",
// "a non-friend's DM picture waits for the click" and "a picture from a mutual follow" failed on
// "Image (click to load)" once a non-friend's file waited for Accept, so they now keep "Accept
// files from: Anyone I talk to" (ACCEPT_ANYONE), which is what they test; the protected setup's
// test failed on "a friend's picture still shows" (the setup is always Nobody now) and says so;
// every test that pastes PNG failed on the attach step once the cleaning could not walk the old
// stand-in PNG, so PNG is a real one now.
