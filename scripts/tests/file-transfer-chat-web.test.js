// Device-to-device big files in the web chat (2026-10-11, docs/design/media-showcase.md,
// "Device-to-device big files, exact" and "...: the shapes, exact", parts 3 to 6, the website's
// half): web/chat/chat-file-transfer.js and the lines it changed in chat-messages.js (the attach
// step), chat-social.js (the notes), chat-voice-rooms.js (the file credentials), chat-voice-calls.js
// (the signals), chat-dms.js (the rows) and chat-privacy.js (the direct-connections setting). The
// protocol under it (the pass, the notes' bytes, piece checks, resume, the whole-file check, memory)
// is held in file-transfer-web.test.js.
//
// Run: node --test scripts/tests/file-transfer-chat-web.test.js   (in `just rig-tests`)
//
// Two chat pages run as they do in the browser, each in its own global scope (node:vm), with the
// DOM replaced by a stub that keeps what is written to it (as private-files-web.test.js): Ann, who
// sends, and Ben, who receives (and in one test a second page of Ben's, his other device). The
// real chat scripts load in index.html's order, the new ones included. Stood in: the relay (it
// passes each `dm_put` to the pages of the key it names as `dm_new`, answers a put that carries a
// ref with `dm_put_ok`, refuses a put to someone who takes messages only from friends unless its
// `file_pass` verifies (test 14), forwards `webrtc_signal` without its `file_pass` as the relay does, and
// answers `call_credentials`, or refuses it as a relay without its forwarder does), the DM layer as
// in private-files-web.test.js (a DM's "signature" is its signed words and "sealing" base64s it,
// so a test reads exactly what was sealed), and WebRTC (two stand-in peer connections that join
// when both descriptions are set, whose data channels deliver one message per turn, as bytes made
// in the receiving page's own realm, with a buffered amount and its low-water event). The file
// pass is signed and checked with real Dilithium3 (the vendored noble bundle), and the hashes are
// real BLAKE3. HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves:
//  1. The attach step: a file over my private limit in a DM is offered device to device, with the
//     spec's line ("Files over 6 MB go straight to Ben's device instead of through the server.");
//     nothing is uploaded; Send sends one sealed note, to Ben alone, whose text is the offer note
//     for this file (its BLAKE3, its piece hashes' BLAKE3, 256 KiB pieces) with a pass that
//     verifies (Dilithium3 over hum/file-pass/v1, the server's did:hum, Ann, Ben, the id, an end
//     at most 7 days ahead); Ann's row says "Waiting for Ben to accept". A group still refuses a
//     file over the limit. When Ben's profile says he accepts files only from friends and Ann is
//     not one, the step says his app will not ask him, and Send is off. An offer whose pass is made
//     out to someone else is dropped by Ben's page: no row, no question, nothing sent back.
//  2. Files in messages first: an offer from someone Ben does not accept files from is never
//     asked about (no Accept, nothing sent back) and its row says why; Nobody refuses a friend's
//     too; a friend's offer under Friends asks "Ann wants to send you archive.zip (7 MB). Accept?".
//     The note's text is never shown as a message.
//  3. Decline sends the answer note with accept false (and a copy to Ben's other devices); Ann's
//     row says "Ben declined"; nothing is dialed.
//  4. Accept: the answer note, then Ben dials Ann: credentials asked for exactly as the spec's
//     third request ({file, peer, file_pass}), a relay-only connection with the reply's three
//     fields, one channel labelled file:<id>, the dc_offer carrying the pass; Ann answers with her
//     own credentials, relay-only; the file arrives and is saved (a Blob of exactly the file's
//     bytes) only then; Ben's row says "Saved", Ann's "Sent"; nothing went to the server's disk.
//  5. Cancel from either side: Ann cancels an offer before it is answered (Ben's row says
//     "Cancelled", Accept gone); Ben cancels mid-transfer (Ann's row says "Cancelled", and Ben's
//     pieces are dropped from this device, a piece still being written when he pressed it too).
//  6. Pause from the receiver: Ann's row says "Paused by Ben", nothing more arrives; Resume
//     finishes it.
//  7. A server whose call relay is not set up: "This server is not set up for that yet", no
//     connection made; once it is, Resume connects.
//  8. Resume: after a drop, Ben dials again when Ann is in the member list, asking from the first
//     piece he lacks; after a reload of Ben's page, the same from the pieces kept on this device.
//  9. An offer whose pass has run out: Ben's row says so and asks nothing; Ann's offer past its
//     end says "Ben did not answer in time".
// 10. A file whose whole BLAKE3 is not the one offered is never saved, and its row says it does
//     not match its description.
// 11. Ben's other device: the question there becomes "Answered on your other device" once he
//     accepts here.
// 12. Direct connections: off by default (relay only), a row in Safety > Files in messages; on, a
//     friend's file may go direct (iceTransportPolicy 'all'); with the protected setup on, never.
// 13. Rule 2 of "...: rules settled by the first review" (2026-10-12): two files to Ben (the second
//     accepted while the first moves), and one each way, all over one connection, a channel and an
//     engine each, each file exactly its bytes.
// 14. Rule 4: the receiver's answer (yes and no) and cancel carry the offer's pass on their put, and
//     reach a sender who takes messages only from friends (the stand-in relay checks the pass with
//     real Dilithium3, and refuses the same put without it).
// 15. One click on Accept moves the row on at once and starts the transfer, with the save dialog
//     closed at once (as headless Edge does), still open (the file waits for the place, then goes
//     into it), or absent; Decline, Pause, Resume and Cancel each change the row in the click; a
//     redraw never replaces the Pause button.
//
// Red first: see the end of this file for each deliberate break and the assertion it tripped.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const SPEC = fs.readFileSync(path.join(ROOT, "docs", "design", "media-showcase.md"), "utf8").replace(/\r\n/g, "\n");
// The shapes section alone, up to the next heading (never the As built notes after it, which quote it).
const SHAPES_AT = SPEC.indexOf("## Device-to-device big files: the shapes, exact");
const SHAPES = SPEC.slice(SHAPES_AT, SPEC.indexOf("\n## ", SHAPES_AT + 10)).replace(/\s+/g, " ");
const FT = require(path.join(WEB, "shared", "file-transfer.js"));
const B3 = require(path.join(WEB, "shared", "blake3.js"));

const hex = (u8) => Buffer.from(u8).toString("hex");
const b64 = (s) => Buffer.from(s, "utf8").toString("base64");
const unb64 = (s) => Buffer.from(s, "base64").toString("utf8");
const sameBytes = (a, b) => Buffer.from(a).equals(Buffer.from(b));
const clone = (x) => JSON.parse(JSON.stringify(x));
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const SEED = Uint8Array.from({ length: 32 }, (_, i) => (i * 7 + 3) & 0xff);
const MB = 1024 * 1024;
const MAY = "invite,message,trade,voice_message";
const CHANNELS = [{ id: "general", name: "general", read_only: false }];
const P = require(path.join(WEB, "shared", "protected.js"));
const W = require(path.join(WEB, "shared", "warnings.js"));
const report = require(path.join(WEB, "shared", "report.js"));
const SHIPPED_PRESETS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "gui", "safety_presets.json"), "utf8"));
const SHIPPED_WARNINGS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "warnings.json"), "utf8"));
const SHIPPED_REASONS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "report_reasons.json"), "utf8"));

// ── The real modules: noble (Dilithium3, the pass) and pq-object (loaded by the group script) ──

let MODS = null;
async function modules() {
  if (MODS) return MODS;
  const asUrl = (src) => "data:text/javascript;base64," + Buffer.from(src).toString("base64");
  const cborUrl = asUrl(fs.readFileSync(path.join(WEB, "shared", "canonical-cbor.js"), "utf8"));
  const objSrc = fs.readFileSync(path.join(WEB, "shared", "pq-object.js"), "utf8").replace(/(['"])\.\/canonical-cbor\.js\1/, JSON.stringify(cborUrl));
  const obj = await import(asUrl(objSrc));
  const noble = await import(asUrl(fs.readFileSync(path.join(WEB, "shared", "vendor", "noble-pq.bundle.js"))));
  MODS = { obj, noble };
  return MODS;
}

let PEOPLE = null;
async function people() {
  if (PEOPLE) return PEOPLE;
  const { noble } = await modules();
  const person = (n, name) => {
    const kp = noble.ml_dsa65.keygen(new Uint8Array(32).fill(n));
    return { name, pub: kp.publicKey, sk: kp.secretKey, key: hex(kp.publicKey) };
  };
  PEOPLE = { ann: person(1, "Ann"), ben: person(2, "Ben"), cy: person(3, "Cy") };
  return PEOPLE;
}
const kyberOf = (k) => "kyber-" + k.slice(0, 16);
// The DM layer's stand-in signature is the signed words themselves; the pass is really signed.
const isWords = (b) => b.length >= 4 && b[0] === 0x68 && b[1] === 0x75 && b[2] === 0x6d && b[3] === 0x2f;
const isPassWords = (b) => Buffer.from(b.slice(0, 17)).toString("latin1") === "hum/file-pass/v1\n";

// ── The stub page (as private-files-web.test.js) ───────────────────────────

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
    own.clicks = 0;
    own.click = () => { own.clicks++; };
    const p = new Proxy(own, {
      get(t, prop) {
        if (prop === "then") return undefined;
        if (prop === "innerHTML") return "innerHTML" in t ? t.innerHTML : escapeText(t.textContent);
        if (prop in t) return t[prop];
        return anything();
      },
      set(t, prop, v) {
        if (prop === "innerHTML") t.children = [];
        t[prop] = v;
        return true;
      },
    });
    all.push(p);
    return p;
  }
  const byId = new Map();
  const kept = (id) => {
    if (!byId.has(id)) byId.set(id, el("#" + id));
    return byId.get(id);
  };
  return new Proxy({}, {
    get(_t, prop) {
      if (prop === "createElement") return el;
      if (prop === "getElementById") return kept;
      if (prop === "body") return kept("__body");
      if (prop === "querySelector") return () => anything();
      if (prop === "querySelectorAll") return () => [];
      if (prop === "hidden") return false;
      return anything();
    },
    set: () => true,
  });
}

function fakeStorage(init) {
  const m = new Map(Object.entries(init || {}));
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k), clear: () => m.clear() };
}

/**
 * IndexedDB, enough of it: databases by name, stores with or without a keyPath (an explicit key
 * otherwise), get, put, delete, getAll, and an index's getAll. Shared between a page and its
 * reload (pass the same one).
 */
function fakeIndexedDB() {
  const dbs = new Map();
  // `api.gate`, a promise: while set, a write to the `pieces` store waits for it (a slow disk), so a
  // test can let a piece's write finish after something else happened. `api.pending`: how many wait.
  const api = { dbs, gate: null, pending: 0 };
  const req = (fn, gate) => {
    const r = { result: undefined, error: null, onsuccess: null, onerror: null };
    const go = () => setImmediate(() => {
      try { r.result = fn(); if (r.onsuccess) r.onsuccess({ target: r }); }
      catch (e) { r.error = e; if (r.onerror) r.onerror({ target: r }); }
    });
    if (gate) { api.pending++; gate.then(() => { api.pending--; go(); }); } else go();
    return r;
  };
  const storeApi = (st, name) => ({
    put: (v, key) => req(() => { const k = key !== undefined ? key : (st.keyPath ? v[st.keyPath] : (v.k ?? v.scope)); st.map.set(k, v); return k; }, name === "pieces" ? api.gate : null),
    get: (k) => req(() => st.map.get(k)),
    delete: (k) => req(() => { st.map.delete(k); }),
    clear: () => req(() => { st.map.clear(); }),
    getAll: () => req(() => [...st.map.values()]),
    getAllKeys: () => req(() => [...st.map.keys()]),
    index: (field) => ({ getAll: (val) => req(() => [...st.map.values()].filter((v) => v[field] === val)) }),
  });
  return Object.assign(api, {
    open(name) {
      const r = { result: null, error: null, onsuccess: null, onerror: null, onupgradeneeded: null };
      setImmediate(() => {
        let db = dbs.get(name);
        const fresh = !db;
        if (!db) { db = { stores: new Map() }; dbs.set(name, db); }
        r.result = {
          objectStoreNames: { contains: (n) => db.stores.has(n) },
          createObjectStore: (n, o) => { db.stores.set(n, { keyPath: o && o.keyPath, map: new Map() }); return { createIndex() {} }; },
          transaction: () => ({
            objectStore: (n) => {
              if (!db.stores.has(n)) db.stores.set(n, { keyPath: n === "msgs" ? "k" : (n === "meta" ? "scope" : undefined), map: new Map() });
              return storeApi(db.stores.get(n), n);
            },
          }),
          close() {},
        };
        if (fresh && r.onupgradeneeded) r.onupgradeneeded({ target: r });
        if (r.onsuccess) r.onsuccess({ target: r });
      });
      return r;
    },
    /** The pieces kept for a transfer, by number. */
    pieces(id) {
      const db = dbs.get("humanity_file_transfers");
      const st = db && db.stores.get("pieces");
      if (!st) return [];
      return [...st.map.keys()].filter((k) => String(k).startsWith(id + ":")).map((k) => Number(String(k).split(":")[1])).sort((a, b) => a - b);
    },
  });
}

// The page's asynchronous work: WebCrypto (the DM store), the stand-in IndexedDB and the channels.
let cryptoInFlight = 0;
const subtle = globalThis.crypto.subtle;
for (const name of ["encrypt", "decrypt", "sign", "verify", "digest", "importKey", "exportKey", "generateKey", "deriveKey", "deriveBits"]) {
  const orig = subtle[name];
  if (typeof orig !== "function" || orig.__counted) continue;
  const counted = (...args) => { cryptoInFlight++; return Promise.resolve(orig.apply(subtle, args)).finally(() => { cryptoInFlight--; }); };
  counted.__counted = true;
  Object.defineProperty(subtle, name, { configurable: true, writable: true, value: counted });
}
const countedCrypto = { subtle, getRandomValues: (a) => globalThis.crypto.getRandomValues(a), randomUUID: () => globalThis.crypto.randomUUID() };

async function settle(n = 60) {
  const until = Date.now() + 30000;
  let idle = 0;
  while (idle < n && Date.now() < until) {
    await new Promise((r) => setImmediate(r));
    idle = cryptoInFlight === 0 ? idle + 1 : 0;
  }
}

/** Wait (by turns) until `pred` holds; fail naming `what` if it never does. Each wait here takes well under a second. */
async function waitFor(pred, what) {
  const until = Date.now() + 15000;
  while (Date.now() < until) {
    if (pred()) return;
    await new Promise((r) => setImmediate(r));
  }
  assert.fail("never happened: " + what);
}

const answer = (status, { json, bytes, text } = {}) => Promise.resolve({
  ok: status >= 200 && status < 300, status,
  json: async () => json,
  text: async () => (text !== undefined ? text : JSON.stringify(json)),
  arrayBuffer: async () => (bytes ? bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) : new ArrayBuffer(0)),
});
const ok = (json) => answer(200, { json });

// ── The stand-in WebRTC ────────────────────────────────────────────────────

/**
 * Peer connections that join when both descriptions are set. The offer's SDP names the
 * connection that made it, so the answering one finds it; the channels made on the dialing side
 * appear on the other (ondatachannel), then both open. A channel delivers one message per turn,
 * as bytes made in the receiving page's own realm, keeps a buffered amount, and fires
 * bufferedamountlow when it drops to its threshold.
 */
function webrtc(net) {
  const make = (page) => {
    function FakeChannel(label, pc) {
      this.label = label;
      this.readyState = "connecting";
      this.binaryType = "blob";
      this.bufferedAmount = 0;
      this.bufferedAmountLowThreshold = 0;
      this.onopen = null; this.onmessage = null; this.onclose = null; this.onbufferedamountlow = null;
      this.peer = null;
      this.pc = pc;
      this.queue = [];
      this.sentCount = 0;
    }
    FakeChannel.prototype.send = function (data) {
      if (this.readyState !== "open") throw new Error("channel not open");
      const size = typeof data === "string" ? data.length : data.byteLength;
      this.bufferedAmount += size;
      this.sentCount++;
      net.log.push({ from: page.who, label: this.label, kind: typeof data === "string" ? data : "bin", size });
      const copy = typeof data === "string" ? data : Uint8Array.from(new Uint8Array(data.buffer || data, data.byteOffset || 0, size));
      this.queue.push({ copy, size });
      if (this.queue.length === 1) setImmediate(() => this._pump());
    };
    FakeChannel.prototype._pump = function () {
      const item = this.queue.shift();
      if (!item) return;
      if (this.readyState === "open" && this.peer && this.peer.readyState === "open") {
        let data = item.copy;
        if (typeof data !== "string") {
          if (net.tamper) data = net.tamper(data) || data;
          const there = this.peer.pc.page.makeBytes(data.length);
          there.set(data);
          data = there.buffer;
        }
        if (this.peer.onmessage) this.peer.onmessage({ data });
      }
      const before = this.bufferedAmount;
      this.bufferedAmount -= item.size;
      if (before > this.bufferedAmountLowThreshold && this.bufferedAmount <= this.bufferedAmountLowThreshold && this.onbufferedamountlow) this.onbufferedamountlow();
      if (this.queue.length) setImmediate(() => this._pump());
    };
    FakeChannel.prototype.close = function () {
      if (this.readyState === "closed") return;
      this.readyState = "closed";
      this.queue.length = 0;
      setImmediate(() => { if (this.onclose) this.onclose(); });
      if (this.peer && this.peer.readyState !== "closed") this.peer.close();
    };
    function FakePeerConnection(config) {
      this.config = config;
      this.page = page;
      this.id = ++net.count;
      net.pcs.set(this.id, this);
      net.made.push({ who: page.who, config: clone(config) });
      this.channels = [];
      this.local = null; this.remote = null; this.peer = null;
      this.connectionState = "new";
      this.onicecandidate = null; this.ondatachannel = null; this.onconnectionstatechange = null;
      this.ice = [];
    }
    FakePeerConnection.prototype.createDataChannel = function (label, opts) {
      const ch = new FakeChannel(label, this);
      ch.opts = opts;
      this.channels.push(ch);
      // On a connection already joined (either side), a new channel reaches the other side at once,
      // as a browser's does over the live connection (no new offer needed).
      if (this.joined && this.peer) {
        const there = this.peer;
        ch.delivered = true;
        setImmediate(() => {
          if (this.connectionState === "closed" || there.connectionState === "closed" || ch.readyState === "closed") return;
          const other = new FakeChannel(label, there);
          other.peer = ch; ch.peer = other;
          if (there.ondatachannel) there.ondatachannel({ channel: other });
          other.readyState = "open"; ch.readyState = "open";
          if (other.onopen) other.onopen();
          if (ch.onopen) ch.onopen();
        });
      }
      return ch;
    };
    FakePeerConnection.prototype.createOffer = async function () { return { type: "offer", sdp: "fake-offer " + this.id }; };
    FakePeerConnection.prototype.createAnswer = async function () { return { type: "answer", sdp: "fake-answer " + this.id }; };
    FakePeerConnection.prototype.setLocalDescription = async function (d) {
      this.local = d;
      const self = this;
      setImmediate(() => { if (self.onicecandidate) self.onicecandidate({ candidate: { candidate: "candidate:fake " + self.id, sdpMid: "0" } }); });
      this._join();
    };
    FakePeerConnection.prototype.setRemoteDescription = async function (d) {
      this.remote = d;
      const otherId = Number(String(d.sdp).split(" ")[1]);
      this.peer = net.pcs.get(otherId) || null;
      if (this.peer && !this.peer.peer) this.peer.peer = this;
      this._join();
    };
    FakePeerConnection.prototype.addIceCandidate = async function (c) { this.ice.push(c); };
    FakePeerConnection.prototype._join = function () {
      const a = this;
      const b = this.peer;
      if (!b || !a.local || !a.remote || !b.local || !b.remote || a.joined || net.blocked) return;
      a.joined = b.joined = true;
      const dialer = a.local.type === "offer" ? a : b;
      const answerer = dialer === a ? b : a;
      setImmediate(() => {
        for (const ch of dialer.channels) {
          if (ch.delivered) continue;
          ch.delivered = true;
          const other = new FakeChannel(ch.label, answerer);
          other.peer = ch; ch.peer = other;
          if (answerer.ondatachannel) answerer.ondatachannel({ channel: other });
          other.readyState = "open"; ch.readyState = "open";
          dialer.connectionState = answerer.connectionState = "connected";
          if (other.onopen) other.onopen();
          if (ch.onopen) ch.onopen();
        }
      });
    };
    FakePeerConnection.prototype.close = function () {
      this.connectionState = "closed";
      for (const ch of this.channels) ch.close();
    };
    return FakePeerConnection;
  };
  return { make };
}

// ── The stand-in relay ─────────────────────────────────────────────────────

/** Does `pass` let `grantee` reach `issuer`: a file pass of the issuer's, made out to the grantee, on this server, not ended? Real Dilithium3. */
function passLetsIn(pass, issuer, grantee) {
  const p = FT.filePassParse(pass);
  if (!p || !MODS || !FT.filePassTimeOk(p, Math.floor(Date.now() / 1000))) return false;
  const words = new TextEncoder().encode(FT.filePassPreimage(SERVER, issuer, grantee, p.id, p.expiry));
  try { return MODS.noble.ml_dsa65.verify(Buffer.from(p.sig, "base64"), words, Buffer.from(issuer, "hex")); } catch (e) { return false; }
}

function relayFor() {
  const net = { pcs: new Map(), count: 0, made: [], log: [], tamper: null, blocked: false };
  const relay = {
    pages: [],
    net,
    rtc: webrtc(net),
    forwarder: true,
    // Recipients who take messages only from friends: key -> the keys of their friends. A put to
    // one from anyone else is refused, unless it carries `file_pass`, a pass that verifies with the
    // recipient as its issuer and the sender as its grantee (rule 4 of "Device-to-device big files:
    // rules settled by the first review"; the relay's own check is being built beside this).
    onlyFriends: new Map(),
    refusedPuts: [],       // each put refused that way, {who, msg}
    refuse: null,          // a reason word: every put with a ref is refused with it
    frames: [],            // every frame a page sent, {who, msg}
    signals: [],           // every webrtc_signal as sent, file_pass included
    credentials: [],       // every call_credentials asked
    nextId: 1,
    deliver(page, msg) {
      page.inbox.push(msg);
      if (page.inbox.length === 1) setImmediate(() => page.drain());
    },
    fromPage(page, msg) {
      relay.frames.push({ who: page.who, msg });
      const to = (k) => relay.pages.filter((p) => p.key === k && !p.closed);
      if (msg.type === "dm_put") {
        if (msg.ref && relay.refuse) { relay.deliver(page, { type: "dm_put_refused", ref: msg.ref, reason: relay.refuse }); return; }
        const friends = relay.onlyFriends.get(msg.to);
        if (friends && msg.to !== page.key && !friends.has(page.key) && !passLetsIn(msg.file_pass, msg.to, page.key)) {
          relay.refusedPuts.push({ who: page.who, msg: clone(msg) });
          if (msg.ref) relay.deliver(page, { type: "dm_put_refused", ref: msg.ref, reason: "reach" });
          return;
        }
        const id = relay.nextId++;
        for (const p of to(msg.to)) relay.deliver(p, { type: "dm_new", id, content: msg.content });
        if (msg.ref && msg.to !== page.key) relay.deliver(page, { type: "dm_put_ok", ref: msg.ref });
      } else if (msg.type === "webrtc_signal") {
        relay.signals.push({ who: page.who, msg: clone(msg) });
        const fwd = { type: "webrtc_signal", from: page.key, to: msg.to, signal_type: msg.signal_type, data: msg.data };
        for (const p of to(msg.to)) relay.deliver(p, fwd);
      } else if (msg.type === "call_credentials") {
        relay.credentials.push({ who: page.who, msg: clone(msg) });
        if (!relay.forwarder) relay.deliver(page, { type: "call_credentials", file: msg.file, refused: true });
        else relay.deliver(page, { type: "call_credentials", file: msg.file, urls: ["turn:relay.test:3478?transport=udp"], username: "1760000000:abc-" + page.who, credential: "cred-" + page.who, ttl: 600 });
      }
    },
  };
  return relay;
}

// ── A chat page ────────────────────────────────────────────────────────────

async function loadPage(relay, me, opts = {}) {
  const { noble } = await modules();
  const ppl = await people();
  const state = { appended: [] };
  const local = fakeStorage(opts.stored);
  const blobs = new Map();
  const fetched = [];
  const systemLines = [];
  const notices = [];
  const page = { who: opts.who || me.name, key: me.key, me, state, blobs, fetched, systemLines, notices, inbox: [], closed: false };
  class TestURL extends URL {
    static createObjectURL(blob) { const u = "blob:test/" + page.who + "/" + (blobs.size + 1); blobs.set(u, blob); return u; }
    static revokeObjectURL() {}
  }
  const sock = {
    readyState: 1, sent: [], raw: [],
    send(s) { if (page.closed) return; this.raw.push(s); const m = JSON.parse(s); this.sent.push(m); relay.fromPage(page, m); },
    close() {},
  };
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
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
      if (u.startsWith("/api/federation/servers")) return ok([]);
      if (u === P.PROTECTED_PRESETS_URL) return ok(clone(SHIPPED_PRESETS));
      if (u === W.WARNINGS_URL) return ok(clone(SHIPPED_WARNINGS));
      if (u.startsWith(report.REPORT_REASONS_URL)) return ok(clone(SHIPPED_REASONS));
      if (u.startsWith("/api/v2/groups?pubkey=")) return ok({ groups: [] });
      if (u === "/api/turn-credentials") return ok({ iceServers: [{ urls: ["stun:relay.test:3478"] }] });
      return Promise.reject(new Error("no network in tests: " + u));
    },
    setTimeout: () => 0,
    clearTimeout: () => {},
    setInterval: () => 0,
    clearInterval: () => {},
    WebSocket: Object.assign(function () { return sock; }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
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
  ctx.RTCPeerConnection = relay.rtc.make(page);
  ctx.window = ctx;
  ctx.self = ctx;
  const mods = { "/shared/pq-object.js": (await modules()).obj, "/shared/vendor/noble-pq.bundle.js": noble };
  ctx.__testImport = async (spec) => { if (!mods[spec]) throw new Error("no module " + spec); return mods[spec]; };
  vm.createContext(ctx);
  page.ctx = ctx;
  page.makeBytes = vm.runInContext("(n) => new Uint8Array(n)", ctx);
  const run = (rel, rewrite) => {
    let src = fs.readFileSync(path.join(WEB, rel), "utf8");
    if (rewrite) src = rewrite(src);
    vm.runInContext(src, ctx, { filename: rel });
  };
  // In index.html's order.
  for (const rel of [
    "shared/events.js", "chat/bip39-english.js", "shared/friend-pass.js", "shared/reach.js", "shared/block.js",
    "shared/report.js", "shared/group-report.js", "shared/warnings.js", "shared/protected.js", "shared/strip-metadata.js",
    "shared/upload-clean.js", "shared/files-in-messages.js", "shared/kept-files.js", "shared/blake3.js",
    "shared/file-transfer.js", "shared/profile-changes.js", "chat/crypto.js", "chat/chat-dm-store.js",
    "chat/view/timestampPill.js", "chat/view/messageRow.js", "chat/app.js", "chat/chat-messages.js", "chat/chat-dms.js",
    "chat/chat-social.js",
  ]) run(rel);
  run("chat/chat-groups-p2p.js", (s) => s.replace(/\bimport\(/g, "__testImport("));
  for (const rel of [
    "chat/chat-ui.js", "chat/chat-voice-rooms.js", "chat/chat-voice-calls.js", "chat/chat-profile.js",
    "chat/chat-privacy.js", "chat/chat-reports.js", "chat/chat-warnings.js", "chat/chat-file-transfer.js",
    "chat/chat-protected.js", "chat/chat-onboarding.js", "chat/chat-p2p.js",
  ]) run(rel);
  for (const name of ["hosIcon", "holdToConfirm", "updateStats"]) if (typeof ctx[name] !== "function") ctx[name] = () => "";
  ctx.holdConfirm = async () => true;
  ctx.appendMessage = (el) => { state.appended.push(el); };
  ctx.notifyNewMessage = (who, text) => { notices.push(String(who) + ": " + String(text)); };
  ctx.playNotificationChime = () => {};
  ctx.sendSWNotification = () => {};
  ctx.addNotice = (text) => { notices.push(String(text)); };
  ctx.addSystemMessage = (text) => { systemLines.push(String(text)); };
  const storeKey = opts.storeKey || await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  page.storeKey = storeKey;
  ctx.getDmStoreKey = async () => storeKey;
  ctx.pqSignMessage = async (secret, bytes) => (isWords(bytes) && !isPassWords(bytes) ? new Uint8Array(bytes) : noble.ml_dsa65.sign(bytes, secret));
  ctx.pqVerifyMessage = async (pk, bytes, sig) => (sig.length === 3309 ? noble.ml_dsa65.verify(sig, bytes, pk) : Buffer.from(bytes).equals(Buffer.from(sig)));
  ctx.pqDmSeal = async (pub, plaintext) => ({ ek_ct_b64: b64(pub), nonce_b64: "AAAA", ct_b64: b64(plaintext) });
  ctx.pqDmOpen = async (_secret, ek, _nonce, ct) => (unb64(ek) === kyberOf(me.key) ? unb64(ct) : null);
  ctx.pqBlake3 = async (bytes) => B3.hash(bytes);
  vm.runInContext(`(s, me, name, sk, kyber, seed) => {
    ws = s; myKey = me; myName = name; activeChannel = 'general'; identityConfirmed = true;
    myUploadToken = 'tok-test';
    myDilithiumPublicHex = me; myDilithiumSecret = sk;
    myKyberPublicBase64 = kyber; myKyberSecret = new Uint8Array(4);
    myIdentity = { publicKeyHex: me, seed32: seed, canSign: true };
    dmFetchSent = true;
  }`, ctx)(sock, me.key, me.name, me.sk, kyberOf(me.key), new Uint8Array(SEED));
  page.sock = sock;
  page.handle = async (msg) => { await vm.runInContext("handleMessage", ctx)(msg); };
  page.drain = async () => {
    while (page.inbox.length) {
      const msg = page.inbox[0];
      if (!page.closed) { try { await page.handle(msg); } catch (e) { page.errors = (page.errors || []).concat(String(e && e.stack || e)); } }
      page.inbox.shift();
    }
  };
  await page.handle({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  page.store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await page.store.init(me.key, "localhost"), "the store loads");
  page.store.setPassServer(SERVER);
  relay.pages.push(page);
  const everyone = [ppl.ann, ppl.ben, ppl.cy];
  const online = opts.online || everyone.map((p) => p.key);
  page.userList = async (keys) => {
    const users = everyone.map((p) => ({ public_key: p.key, name: p.name, role: "", kyber_public: kyberOf(p.key), online: keys.includes(p.key) }));
    await page.handle({ type: "full_user_list", users });
    await settle();
  };
  await page.handle({ type: "reach_settings", settings: { message: "anyone", call: "chosen", trade: "friends" } });
  vm.runInContext("updateChannelList", ctx)(clone(CHANNELS));
  await page.userList(online);
  await settle();
  page.fn = (name) => vm.runInContext(name, ctx);
  page.set = (code, ...args) => vm.runInContext(code, ctx)(...args);
  page.el = (id) => ctx.document.getElementById(id);
  return page;
}

// ── Helpers ────────────────────────────────────────────────────────────────

/** Two people friends both ways on this page: a mutual follow, my pass to them, and theirs to me. */
function befriend(page, key) {
  page.store.setFollowing(key, true);
  page.store.setFollower(key, true);
  page.store.recordPassSent(key, "11".repeat(16), MAY);
  page.store.storeCertFrom(key, '{"v":2,"serial":"' + "22".repeat(16) + '","may":"' + MAY + '","sig":"c2ln"}');
  page.set("(k) => { myFollowing.add(k); myFollowers.add(k); }", key);
}

function everyUnder(el, out = []) {
  for (const c of (el && el.children) || []) { out.push(c); everyUnder(c, out); }
  return out;
}
/** The newest row drawn for a transfer (or the only transfer) on this page. */
function rowOf(page, id) {
  const rows = page.state.appended.filter((e) => e && e.dataset && e.dataset.fileTransfer && (!id || e.dataset.fileTransfer === id));
  return rows.length ? rows[rows.length - 1] : null;
}
/**
 * A row's parts as a person sees them. Since 2026-10-12 a row keeps every button it can show and
 * hides those that do not apply (display none), so a redraw never replaces the button under the
 * pointer; a hidden one is not there for a person, so it is not counted here.
 */
function rowParts(row) {
  const els = everyUnder(row);
  const seen = (e) => !e.style || e.style.display !== "none";
  const byClass = (cls) => els.find((e) => String(e.className).split(/\s+/).includes(cls)) || null;
  const buttons = els.filter((e) => e.dataset && e.dataset.ft && seen(e));
  return {
    words: (byClass("ft-state") || {}).textContent,
    name: (byClass("ft-name") || {}).textContent,
    size: (byClass("ft-size") || {}).textContent,
    bar: byClass("ft-bar-fill"),
    note: seen(byClass("ft-note") || {}) ? (byClass("ft-note") || {}).textContent : undefined,
    button: (what) => buttons.find((e) => e.dataset.ft === what) || null,
    buttons: buttons.map((e) => e.dataset.ft),
  };
}
const wordsOf = (page, id) => { const r = rowOf(page, id); return r ? rowParts(r).words : null; };
const transfers = (page) => page.fn("fileTransfers")();
const only = (page) => { const t = transfers(page); assert.equal(t.length, 1, "one transfer on " + page.who); return t[0]; };

function attachStep(page) {
  const all = page.el("__body").children.filter((c) => c && c.id === "attach-step-overlay" && !c.removed);
  return all.length ? all[all.length - 1] : null;
}
function stepParts(step) {
  const els = everyUnder(step);
  const byClass = (cls) => els.filter((e) => String(e.className).split(/\s+/).includes(cls));
  const byStep = (what) => els.find((e) => e.dataset && e.dataset.attachStep === what) || null;
  return { direct: byClass("attach-step-direct").map((e) => e.textContent), send: byStep("send"), cancel: byStep("cancel") };
}

/** Deterministic bytes that are no picture or video, so nothing in them needs cleaning. */
function zipBytes(size, seed = 3) {
  const b = new Uint8Array(size);
  b.set([0x50, 0x4b, 0x03, 0x04]);
  for (let i = 4; i < size; i++) b[i] = (Math.imul(i, 2654435761) >>> 11 ^ seed) & 0xff;
  return b;
}
const zip = (bytes, name = "archive.zip") => new File([bytes], name, { type: "application/zip" });

/** Pick a file in the conversation on screen, wait for the attach step, press Send (or Cancel). */
async function sendFile(page, file, press = "send") {
  const going = page.fn("handleFileAttachment")({ target: { files: [file], value: "x" } });
  await waitFor(() => attachStep(page), "the attach step is shown");
  const parts = stepParts(attachStep(page));
  if (press) {
    const b = parts[press];
    if (!b.disabled) b.onclick();
    else parts.cancel.onclick();
  }
  return { going, parts };
}

/** The sealed text of every put a page sent, opened (the stand-in seal base64s it). */
function putsFrom(page) {
  return page.sock.sent.filter((m) => m.type === "dm_put").map((m) => {
    const env = JSON.parse(m.content);
    return { to: m.to, sealedTo: unb64(env.ek_ct_b64), ref: m.ref, inner: JSON.parse(unb64(env.ct_b64)) };
  });
}

/** Ann and Ben, Ben's conversation with Ann open on his page and hers with him on hers. */
async function pair(opts = {}) {
  const ppl = await people();
  const relay = relayFor();
  const ann = await loadPage(relay, ppl.ann, opts.ann || {});
  const ben = await loadPage(relay, ppl.ben, opts.ben || {});
  if (opts.friends !== false) { befriend(ann, ppl.ben.key); befriend(ben, ppl.ann.key); }
  ann.fn("openDmConversation")(ppl.ben.key, "Ben");
  ben.fn("openDmConversation")(ppl.ann.key, "Ann");
  await settle();
  return { ppl, relay, ann, ben };
}

/** Ann offers `bytes` to Ben through the attach step; returns her transfer once it is offered. */
async function offer(s, bytes, name) {
  const { going } = await sendFile(s.ann, zip(bytes, name));
  await going;
  await waitFor(() => transfers(s.ann).length === 1 && only(s.ann).state !== "preparing", "Ann's offer is made");
  await settle();
  return only(s.ann);
}

const press = async (page, id, what) => {
  const b = rowParts(rowOf(page, id)).button(what);
  assert.ok(b, `${page.who}'s row has ${what}`);
  await b.onclick();
  await settle();
};

// ── 1. The attach step and the offer ───────────────────────────────────────

test("1. a file over my private limit in a DM is offered device to device: the step's line, one sealed note to Ben, a pass that verifies", async () => {
  const s = await pair();
  const { noble } = await modules();
  const bytes = zipBytes(7 * MB + 123);
  const before = Date.now();
  const { going, parts } = await sendFile(s.ann, zip(bytes), null);
  assert.deepEqual(parts.direct.slice(0, 1), ["Files over 6 MB go straight to Ben's device instead of through the server."], "the spec's line");
  assert.ok(SHAPES.includes('("Files over 6 MB go straight to Ann\'s device instead of through the server.")'), "which is the spec's, for Ann");
  assert.ok(parts.direct[1].includes("Keep this page open"), "and that the page must stay open");
  assert.equal(parts.send.disabled, false, "Send is on");
  assert.equal(s.ann.fetched.filter((f) => f.includes("/api/upload")).length, 0, "nothing uploaded");
  parts.send.onclick();
  await going;
  await waitFor(() => transfers(s.ann).length === 1 && only(s.ann).state === "offered", "the offer is made");
  await settle();
  const t = only(s.ann);
  const puts = putsFrom(s.ann);
  assert.equal(puts.length, 1, "one put");
  assert.equal(puts[0].to, s.ppl.ben.key, "to Ben");
  assert.equal(puts[0].sealedTo, kyberOf(s.ppl.ben.key), "sealed to Ben alone: my other devices have no file to send");
  assert.ok(/^file-[0-9a-f]{24}$/.test(puts[0].ref), "with a ref, so the server's answer reaches the row");
  const text = puts[0].inner.text;
  const parsed = FT.offerNoteParse(text);
  assert.ok(parsed, "the note is an offer");
  assert.equal(FT.offerNoteText(parsed), text, "written exactly as the spec's note");
  assert.equal(parsed.name, "archive.zip");
  assert.equal(parsed.size, bytes.length);
  assert.equal(parsed.type, "application/zip");
  assert.equal(parsed.piece, 262144);
  assert.equal(parsed.pieces, Math.ceil(bytes.length / 262144));
  assert.equal(parsed.blake3, hex(B3.hash(bytes)), "the whole file's BLAKE3");
  const list = new Uint8Array(parsed.pieces * 32);
  for (let i = 0; i < parsed.pieces; i++) list.set(B3.hash(bytes.subarray(i * 262144, (i + 1) * 262144)), i * 32);
  assert.equal(parsed.hashes, hex(B3.hash(list)), "the BLAKE3 of the piece hashes, each 32 raw bytes, in order");
  assert.ok(/^[0-9a-f]{32}$/.test(parsed.id) && parsed.pass.id === parsed.id, "a 16-byte id, the pass's too");
  const nowSecs = Math.floor(Date.now() / 1000);
  assert.ok(parsed.pass.expiry > nowSecs && parsed.pass.expiry <= Math.floor(before / 1000) + 7 * 24 * 3600 + 1, "an end at most 7 days ahead");
  const words = FT.filePassPreimage(SERVER, s.ppl.ann.key, s.ppl.ben.key, parsed.id, parsed.pass.expiry);
  const sig = Buffer.from(parsed.pass.sig, "base64");
  assert.ok(noble.ml_dsa65.verify(sig, new TextEncoder().encode(words), s.ppl.ann.pub), "the pass: Ann's Dilithium3 over the spec's words, for Ben, on this server");
  assert.equal(wordsOf(s.ann, t.id), "Waiting for Ben to accept", "Ann's row");
  assert.ok(rowParts(rowOf(s.ann, t.id)).note.includes("keep this page open"), "saying the page must stay open");
  assert.deepEqual(rowParts(rowOf(s.ann, t.id)).buttons, ["cancel"], "Cancel, while it waits");
  assert.equal(s.ann.fetched.filter((f) => f.includes("/api/upload")).length, 0, "and nothing went to the server's disk");

  // A group (or anything not a DM) still refuses a file over the limit, with the limit's words.
  s.ann.systemLines.length = 0;
  s.ann.set("() => { activeDmPartner = null; window.activeP2pGroup = { id: 'g1' }; }");
  await s.ann.fn("sendAttachment")(zip(bytes));
  assert.ok(s.ann.systemLines.some((l) => l.includes("a private conversation can be up to 6 MB")), "a group says the limit");
  assert.equal(transfers(s.ann).length, 1, "and offers nothing");
});

test("1b. when Ben's profile says he accepts files only from friends and Ann is not one, the step says his app will not ask him, and Send is off", async () => {
  const s = await pair({ friends: false });
  // Ben's published choice, as a profile answer gives it.
  s.ann.set("(n) => { profileCache[n] = { name: 'Ben', accepts_files: 'friends' }; }", "ben");
  const { going, parts } = await sendFile(s.ann, zip(zipBytes(7 * MB)), "cancel");
  await going;
  assert.ok(parts.direct.includes("Ben accepts files only from friends, so their app will not ask them about this one."), "the line: " + JSON.stringify(parts.direct));
  assert.equal(parts.send.disabled, true, "Send is off");
  assert.equal(transfers(s.ann).length, 0, "nothing offered");
  assert.equal(putsFrom(s.ann).length, 0, "nothing sent");
});

test("1c. an offer whose pass is not Ben's (made out to someone else) is dropped: no row, no question, nothing back", async () => {
  const s = await pair();
  // Ann's page signs a pass made out to Cy, and offers it to Ben.
  s.ann.set("(cy) => { const real = HosFileTransfer.filePassPreimage; HosFileTransfer.filePassPreimage = (sv, i, g, id, e) => real(sv, i, cy, id, e); }", s.ppl.cy.key);
  await offer(s, zipBytes(7 * MB));
  assert.equal(putsFrom(s.ann).length, 1, "the offer went");
  await settle(200);
  assert.equal(s.relay.frames.filter((f) => f.who === "Ben" && f.msg.type === "dm_put").length, 0, "Ben sends nothing back");
  assert.equal(transfers(s.ben).length, 0, "Ben's page keeps nothing of it");
  assert.equal(rowOf(s.ben), null, "and draws no row");
});

// ── 2. Files in messages first ──────────────────────────────────────────────

test("2. the receiver applies Files in messages before asking: a sender it does not accept is never asked about", async () => {
  // Not friends, Ben's default Friends: no question.
  let s = await pair({ friends: false });
  let t = await offer(s, zipBytes(7 * MB));
  await waitFor(() => transfers(s.ben).length === 1, "Ben's page has the offer");
  await settle();
  let bt = only(s.ben);
  assert.equal(bt.state, "refused");
  assert.equal(wordsOf(s.ben, t.id), "Ann offered you archive.zip (7 MB). You accept files only from friends, so you were not asked.");
  assert.deepEqual(rowParts(rowOf(s.ben, t.id)).buttons, [], "no Accept, no Decline");
  assert.equal(putsFrom(s.ben).length, 0, "nothing sent back (an answer would tell Ann that Ben was online)");
  assert.ok(!s.ben.state.appended.some((e) => String(e.textContent || e.innerHTML || "").includes("[[hum:offer")), "the note's text is never shown as a message");
  assert.equal(s.ben.store.conversation(s.ppl.ann.key).length, 0, "nor kept as one");
  assert.equal(wordsOf(s.ann, t.id), "Waiting for Ben to accept", "Ann still waits");

  // Nobody: a friend's offer is not asked about either.
  s = await pair({ ben: { stored: { hos_files_in_messages: JSON.stringify({ accept: "nobody" }) } } });
  t = await offer(s, zipBytes(7 * MB));
  await waitFor(() => transfers(s.ben).length === 1, "Ben's page has the offer");
  await settle();
  assert.equal(wordsOf(s.ben, t.id), "Ann offered you archive.zip (7 MB). You do not accept files in messages, so you were not asked.");
  assert.deepEqual(rowParts(rowOf(s.ben, t.id)).buttons, []);

  // A friend, under the default Friends: the question.
  s = await pair();
  t = await offer(s, zipBytes(7 * MB));
  await waitFor(() => transfers(s.ben).length === 1, "Ben's page has the offer");
  await settle();
  bt = only(s.ben);
  assert.equal(bt.state, "asked");
  assert.equal(wordsOf(s.ben, t.id), "Ann wants to send you archive.zip (7 MB). Accept?");
  assert.ok(SHAPES.includes('"Ann wants to send you clip.mp4 (2.3 GB). Accept?"'), "the spec's question");
  assert.equal(FT.WORDS.question("Ann", "clip.mp4", 2468000000), "Ann wants to send you clip.mp4 (2.3 GB). Accept?", "word for word");
  assert.deepEqual(rowParts(rowOf(s.ben, t.id)).buttons, ["accept", "decline"]);
  assert.ok(s.ben.notices.some((n) => n.startsWith("Ann: Wants to send you archive.zip")), "and Ben is told");
});

// ── 3. Decline ─────────────────────────────────────────────────────────────

test("3. Decline sends accept false, Ann's row says Ben declined, and nothing is dialed", async () => {
  const s = await pair();
  const t = await offer(s, zipBytes(7 * MB));
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  await press(s.ben, t.id, "decline");
  const puts = putsFrom(s.ben);
  const toAnn = puts.filter((p) => p.sealedTo === kyberOf(s.ppl.ann.key));
  const toMe = puts.filter((p) => p.sealedTo === kyberOf(s.ppl.ben.key));
  assert.equal(toAnn.length, 1, "one note to Ann");
  assert.equal(toAnn[0].inner.text, `[[hum:offer-answer:v1]]{"id":"${t.id}","accept":false}`, "the answer, no");
  assert.equal(toMe.length, 1, "and its copy to my other devices");
  await waitFor(() => wordsOf(s.ann, t.id) === "Ben declined", "Ann's row says so");
  assert.equal(wordsOf(s.ben, t.id), "You declined");
  assert.equal(s.relay.credentials.length, 0, "no credentials asked");
  assert.equal(s.relay.signals.length, 0, "nothing dialed");
  assert.equal(s.relay.net.made.length, 0, "no connection made");
});

// ── 4. Accept and the whole transfer ───────────────────────────────────────

/** Ben accepts and the transfer runs to its end. */
async function acceptAndWait(s, t, until) {
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  await press(s.ben, t.id, "accept");
  await waitFor(until || (() => only(s.ben).state === "saved"), "the transfer ends");
  await settle();
}

test("4. Accept: Ben dials Ann through the server's call relay, with the pass; the file arrives and is saved only then", async () => {
  const s = await pair();
  const bytes = zipBytes(7 * MB + 999, 9);
  const t = await offer(s, bytes);
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  await press(s.ben, t.id, "accept");
  const answerPut = putsFrom(s.ben).find((p) => p.sealedTo === kyberOf(s.ppl.ann.key));
  assert.equal(answerPut.inner.text, `[[hum:offer-answer:v1]]{"id":"${t.id}","accept":true}`, "the answer, yes");
  await waitFor(() => only(s.ben).state === "saved", "the file is saved");
  await settle();
  // Ben's credentials: the spec's third request, exactly.
  const benAsk = s.relay.credentials.find((c) => c.who === "Ben").msg;
  assert.deepEqual(benAsk, { type: "call_credentials", file: t.id, peer: s.ppl.ann.key, file_pass: t.offer.pass }, "{file, peer, file_pass}");
  const annAsk = s.relay.credentials.find((c) => c.who === "Ann").msg;
  assert.deepEqual(annAsk, { type: "call_credentials", file: t.id, peer: s.ppl.ben.key, file_pass: t.offer.pass }, "Ann asks with the same pass");
  // Both connections: the server's call relay only, with the reply's three fields.
  const benPc = s.relay.net.made.find((m) => m.who === "Ben").config;
  assert.deepEqual(benPc, { iceServers: [{ urls: ["turn:relay.test:3478?transport=udp"], username: "1760000000:abc-Ben", credential: "cred-Ben" }], iceTransportPolicy: "relay" });
  const annPc = s.relay.net.made.find((m) => m.who === "Ann").config;
  assert.equal(annPc.iceTransportPolicy, "relay", "Ann's too");
  // The dial: from Ben to Ann, its dc_offer carrying the pass, one channel labelled file:<id>.
  const dial = s.relay.signals.find((x) => x.msg.signal_type === "dc_offer");
  assert.equal(dial.who, "Ben", "the receiver dials");
  assert.equal(dial.msg.to, s.ppl.ann.key);
  assert.deepEqual(dial.msg.file_pass, t.offer.pass, "the dc_offer carries the pass");
  const pc = [...s.relay.net.pcs.values()].find((p) => p.page.who === "Ben");
  assert.deepEqual(pc.channels.map((c) => [c.label, c.opts]), [["file:" + t.id, { ordered: true }]], "one ordered channel, file:<id>");
  assert.ok(s.relay.signals.some((x) => x.who === "Ann" && x.msg.signal_type === "dc_answer"), "Ann answers");
  assert.ok(s.relay.signals.some((x) => x.msg.signal_type === "dc_ice"), "and candidates go both ways");
  // The file: one Blob, exactly its bytes, saved through the browser.
  assert.equal(s.ben.blobs.size, 1, "one Blob made");
  const saved = new Uint8Array(await [...s.ben.blobs.values()][0].arrayBuffer());
  assert.ok(sameBytes(saved, bytes), "exactly the file's bytes");
  assert.equal(wordsOf(s.ben, t.id), "Saved");
  await waitFor(() => wordsOf(s.ann, t.id) === "Sent", "Ann's row says Sent");
  // Every message on the channel at most 64 KiB.
  const big = s.relay.net.log.filter((m) => m.kind === "bin");
  assert.ok(big.length > 0 && big.every((m) => m.size <= 65536), "binary messages at most 64 KiB");
  assert.equal(s.ann.fetched.filter((f) => f.includes("/api/upload")).length + s.ben.fetched.filter((f) => f.includes("/api/upload")).length, 0, "nothing to the server's disk");
  assert.deepEqual(s.ann.errors || [], [], "no errors on Ann's page");
  assert.deepEqual(s.ben.errors || [], [], "nor Ben's");
});

// ── 5. Cancel ──────────────────────────────────────────────────────────────

test("5. cancel from either side: before the answer, and mid-transfer (the receiver's pieces go)", async () => {
  let s = await pair();
  let t = await offer(s, zipBytes(7 * MB));
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  await press(s.ann, t.id, "cancel");
  const note = putsFrom(s.ann).filter((p) => p.inner.text.startsWith("[[hum:offer-cancel:v1]]"));
  assert.deepEqual(note.map((p) => [p.sealedTo, p.inner.text]), [
    [kyberOf(s.ppl.ben.key), `[[hum:offer-cancel:v1]]{"id":"${t.id}"}`],
    [kyberOf(s.ppl.ann.key), `[[hum:offer-cancel:v1]]{"id":"${t.id}"}`],
  ], "the cancel note to Ben, and its copy to my other devices");
  assert.equal(wordsOf(s.ann, t.id), "Cancelled");
  await waitFor(() => wordsOf(s.ben, t.id) === "Cancelled", "Ben's row says so");
  assert.deepEqual(rowParts(rowOf(s.ben, t.id)).buttons, [], "Accept gone");

  // Mid-transfer, Ben cancels.
  const idb = fakeIndexedDB();
  s = await pair({ ben: { indexedDB: idb } });
  t = await offer(s, zipBytes(9 * MB));
  await acceptAndWait(s, t, () => idb.pieces(t.id).length >= 3);
  // A piece is still being written when Cancel is pressed (a slow disk): it must not be left behind.
  let release;
  idb.gate = new Promise((r) => { release = r; });
  await waitFor(() => idb.pending > 0, "a piece is being written");
  await press(s.ben, t.id, "cancel");
  idb.gate = null;
  release();
  assert.equal(wordsOf(s.ben, t.id), "Cancelled");
  await waitFor(() => wordsOf(s.ann, t.id) === "Cancelled", "Ann's row says so");
  await settle();
  assert.deepEqual(idb.pieces(t.id), [], "the pieces Ben had are dropped from his device, the one written after the cancel too");
  assert.ok(putsFrom(s.ben).some((p) => p.inner.text === `[[hum:offer-cancel:v1]]{"id":"${t.id}"}`), "and the note went, so Ann's row says so even if the connection had gone");
  assert.equal(s.ben.blobs.size, 0, "nothing saved");
});

// ── 6. Pause ───────────────────────────────────────────────────────────────

test("6. Ben pauses: Ann's row says Paused by Ben and nothing more arrives; Resume finishes it", async () => {
  const idb = fakeIndexedDB();
  const s = await pair({ ben: { indexedDB: idb } });
  const bytes = zipBytes(12 * MB, 4);
  const t = await offer(s, bytes);
  await acceptAndWait(s, t, () => idb.pieces(t.id).length >= 4);
  await press(s.ben, t.id, "pause");
  assert.equal(wordsOf(s.ben, t.id), "Paused");
  await waitFor(() => wordsOf(s.ann, t.id) === "Paused by Ben", "Ann's row says so");
  await settle(200);
  const kept = idb.pieces(t.id).length;
  const sent = s.relay.net.log.length;
  await settle(300);
  assert.equal(idb.pieces(t.id).length, kept, "nothing more kept while paused");
  assert.equal(s.relay.net.log.length, sent, "nothing more sent");
  assert.deepEqual(rowParts(rowOf(s.ben, t.id)).buttons, ["resume", "cancel"]);
  await press(s.ben, t.id, "resume");
  await waitFor(() => only(s.ben).state === "saved", "it finishes");
  const saved = new Uint8Array(await [...s.ben.blobs.values()][0].arrayBuffer());
  assert.ok(sameBytes(saved, bytes), "the whole file");
});

// ── 7. Not set up ──────────────────────────────────────────────────────────

test("7. a server whose call relay is not set up: the row says so and nothing connects; once it is, Resume goes", async () => {
  const s = await pair();
  s.relay.forwarder = false;
  const t = await offer(s, zipBytes(7 * MB));
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  await press(s.ben, t.id, "accept");
  await waitFor(() => wordsOf(s.ben, t.id) === "This server is not set up for that yet", "Ben's row says so");
  assert.ok(SHAPES.includes('"This server is not set up for that yet"'), "the spec's words");
  assert.equal(s.relay.net.made.length, 0, "no connection made");
  assert.equal(s.relay.signals.length, 0, "nothing dialed");
  assert.deepEqual(rowParts(rowOf(s.ben, t.id)).buttons, ["resume", "cancel"], "Resume to try again");
  s.relay.forwarder = true;
  await press(s.ben, t.id, "resume");
  await waitFor(() => only(s.ben).state === "saved", "it goes");
});

// ── 8. Resume ──────────────────────────────────────────────────────────────

test("8. after a drop, and after a reload, Ben dials again and asks from the first piece he lacks", async () => {
  const idb = fakeIndexedDB();
  const s = await pair({ ben: { indexedDB: idb } });
  const bytes = zipBytes(14 * MB, 6);
  const t = await offer(s, bytes);
  await acceptAndWait(s, t, () => idb.pieces(t.id).length >= 6);
  // The connection drops.
  s.relay.net.blocked = true;
  for (const pc of s.relay.net.pcs.values()) pc.close();
  await settle(100);
  const kept = idb.pieces(t.id).length;
  assert.ok(kept >= 6 && kept < 56, "part of it kept: " + kept);
  assert.notEqual(only(s.ben).state, "saved");
  s.relay.net.blocked = false;
  const gets = () => s.relay.net.log.filter((m) => m.from === "Ben" && m.kind.includes('"t":"get"')).map((m) => JSON.parse(m.kind));
  const asked = gets().length;
  // Ann is in the member list: Ben dials again.
  await s.ben.userList([s.ppl.ann.key, s.ppl.ben.key]);
  await waitFor(() => gets().length > asked, "Ben asks again");
  assert.equal(gets()[asked].from, kept, "from the first piece he lacks");
  await waitFor(() => idb.pieces(t.id).length >= kept + 8, "more arrives");

  // A reload of Ben's page: the transfer, and the pieces kept, are still here.
  s.relay.net.blocked = true;
  for (const pc of s.relay.net.pcs.values()) pc.close();
  await settle(100);
  s.ben.closed = true;
  await settle(50);
  const kept2 = only(s.ben).kept;
  const ben2 = await loadPage(s.relay, s.ppl.ben, { indexedDB: idb, storeKey: s.ben.storeKey, who: "Ben2", online: [] });
  befriend(ben2, s.ppl.ann.key);
  ben2.fn("openDmConversation")(s.ppl.ann.key, "Ann");
  await settle();
  await waitFor(() => transfers(ben2).length === 1, "the transfer is loaded");
  assert.equal(wordsOf(ben2, t.id), "Waiting for Ann to come online", "Ann is not in the member list yet");
  s.relay.net.blocked = false;
  const before = s.relay.net.log.length;
  await ben2.userList([s.ppl.ann.key, s.ppl.ben.key]);
  await waitFor(() => only(ben2).state === "saved", "the file arrives on the reloaded page");
  const afterGets = s.relay.net.log.slice(before).filter((m) => m.from === "Ben2" && m.kind.includes('"t":"get"')).map((m) => JSON.parse(m.kind));
  assert.equal(afterGets[0].from, kept2, "asking from the first piece kept before the reload");
  const saved = new Uint8Array(await [...ben2.blobs.values()][0].arrayBuffer());
  assert.ok(sameBytes(saved, bytes), "the whole file, the pieces from before the reload checked in its BLAKE3");
});

// ── 9. Expiry ──────────────────────────────────────────────────────────────

test("9. an offer whose pass has run out: Ben is not asked; Ann's says Ben did not answer in time", async () => {
  // Ann's clock eight days behind: the offer she makes ran out a day ago by Ben's.
  let s = await pair();
  s.ann.set("(d) => { const real = Date.now; Date.now = () => real() - d; }", 8 * 24 * 3600 * 1000);
  const t = await offer(s, zipBytes(7 * MB));
  await waitFor(() => transfers(s.ben).length === 1, "Ben's page has it");
  await settle();
  assert.equal(only(s.ben).state, "expired");
  assert.equal(wordsOf(s.ben, t.id), "This offer ran out. Ask for it to be sent again.");
  assert.deepEqual(rowParts(rowOf(s.ben, t.id)).buttons, [], "no Accept");

  // Ann's offer, unanswered past its pass's end.
  s = await pair({ friends: false });
  const t2 = await offer(s, zipBytes(7 * MB));
  s.ann.set("(d) => { const real = Date.now; Date.now = () => real() + d; }", 8 * 24 * 3600 * 1000);
  s.ann.fn("expireFileTransfers")();
  await settle();
  assert.equal(wordsOf(s.ann, t2.id), "Ben did not answer in time");
});

// ── 10. The whole file's BLAKE3 ────────────────────────────────────────────

test("10. a file whose whole BLAKE3 is not the one offered is never saved, and the row says it does not match", async () => {
  const idb = fakeIndexedDB();
  const s = await pair({ ben: { indexedDB: idb } });
  // Ann's page describes the file with another file's whole hash (its pieces and list are right).
  s.ann.set("(h) => { const real = HosFileTransfer.hashFile; HosFileTransfer.hashFile = async (f, o) => { const r = await real(f, o); r.blake3 = h; return r; }; }", hex(B3.hash(zipBytes(10))));
  const t = await offer(s, zipBytes(7 * MB));
  await acceptAndWait(s, t, () => ["failed", "saved"].includes(only(s.ben).state));
  assert.equal(only(s.ben).state, "failed");
  assert.equal(wordsOf(s.ben, t.id), "The file Ann offered does not match its description");
  assert.equal(s.ben.blobs.size, 0, "nothing saved");
  assert.deepEqual(idb.pieces(t.id), [], "and the pieces are dropped");
});

// ── 11. Ben's other device ─────────────────────────────────────────────────

test("11. the question on Ben's other device becomes Answered on your other device", async () => {
  const s = await pair();
  const ben2 = await loadPage(s.relay, s.ppl.ben, { who: "Ben2" });
  befriend(ben2, s.ppl.ann.key);
  ben2.fn("openDmConversation")(s.ppl.ann.key, "Ann");
  await settle();
  const t = await offer(s, zipBytes(7 * MB));
  await waitFor(() => transfers(ben2).length === 1 && only(ben2).state === "asked", "asked on the other device too");
  await acceptAndWait(s, t);
  await waitFor(() => wordsOf(ben2, t.id) === "Answered on your other device", "the other device says so");
  assert.deepEqual(rowParts(rowOf(ben2, t.id)).buttons, []);
  assert.equal(s.relay.signals.filter((x) => x.who === "Ben2").length, 0, "and it dials nothing");
});

// ── 12. Direct connections ─────────────────────────────────────────────────

test("12. direct connections: off by default; on, a friend's file may go direct; never with the protected setup on", async () => {
  // The setting's row in Safety > Files in messages, off.
  let s = await pair();
  const box = s.ben.ctx.document.createElement("div");
  const rows = [];
  s.ben.fn("drawFileDirectRow")(box, (label, control, explain) => { rows.push({ label, control, explain }); return s.ben.ctx.document.createElement("div"); });
  assert.equal(rows.length, 1);
  assert.equal(rows[0].label, "Let files from friends go straight between devices");
  assert.equal(rows[0].control.checked, false, "off by default");
  assert.ok(rows[0].explain.includes("only the server sees your network address"));
  // On, between friends: Ben's connection may go direct.
  rows[0].control.checked = true;
  rows[0].control.onchange();
  const t = await offer(s, zipBytes(7 * MB));
  await acceptAndWait(s, t);
  const benPc = s.relay.net.made.find((m) => m.who === "Ben").config;
  assert.equal(benPc.iceTransportPolicy, "all", "Ben's own choice: direct allowed");
  assert.ok(benPc.iceServers.some((x) => JSON.stringify(x.urls).includes("stun:relay.test")), "with this server's own address lookup");
  const annPc = s.relay.net.made.find((m) => m.who === "Ann").config;
  assert.equal(annPc.iceTransportPolicy, "relay", "Ann's setting is off: her address stays hers");

  // The protected setup on: relay only, whatever is kept. (Its state stood in by protectedIsOn alone,
  // so the rest of the setup's rules, tested elsewhere, do not change this transfer.)
  s = await pair({ ben: { stored: { hos_file_direct: "1" } } });
  s.ben.set("() => { window.protectedIsOn = () => true; }");
  const t2 = await offer(s, zipBytes(7 * MB));
  await acceptAndWait(s, t2);
  assert.equal(s.relay.net.made.find((m) => m.who === "Ben").config.iceTransportPolicy, "relay");
});

// ── 13 to 15. The rules settled by the first review (2026-10-12) ───────────
// docs/design/media-showcase.md, "Device-to-device big files: rules settled by the first review":
// rule 2 (one connection, several transfers), rule 4 (answers reach a sender who takes messages
// only from friends), and the Accept click the review's real-browser run found doing nothing.

/** `page` offers `bytes` to the person whose conversation is open; returns the new transfer once offered. */
async function offerFrom(page, bytes, name) {
  const before = new Set(transfers(page).map((t) => t.id));
  const { going } = await sendFile(page, zip(bytes, name));
  await going;
  await waitFor(() => transfers(page).some((t) => !before.has(t.id) && t.state !== "preparing"), page.who + "'s offer is made");
  await settle();
  return transfers(page).find((t) => !before.has(t.id));
}
const transferOf = (page, id) => transfers(page).find((t) => t.id === id);
const savedFiles = async (page) => Promise.all([...page.blobs.values()].map(async (b) => new Uint8Array(await b.arrayBuffer())));

test("13. one connection, several transfers: two files to Ben, and one each way, all arrive over one connection, a channel and an engine each", async () => {
  // a. Ann offers Ben two files. He accepts the first, and the second while the first is moving:
  //    the second opens its own channel on the same connection, and both arrive.
  const idb = fakeIndexedDB();
  let s = await pair({ ben: { indexedDB: idb } });
  const one = zipBytes(9 * MB, 31);
  const two = zipBytes(7 * MB + 77, 32);
  const tOne = await offerFrom(s.ann, one, "one.zip");
  const tTwo = await offerFrom(s.ann, two, "two.zip");
  await waitFor(() => transfers(s.ben).length === 2 && transfers(s.ben).every((t) => t.state === "asked"), "both asked");
  await press(s.ben, tOne.id, "accept");
  await waitFor(() => idb.pieces(tOne.id).length >= 3, "the first is moving");
  await press(s.ben, tTwo.id, "accept");
  await waitFor(() => transfers(s.ben).every((t) => t.state === "saved"), "a. both files to Ben saved");
  assert.equal(s.relay.net.made.filter((m) => m.who === "Ben").length, 1, "one connection on Ben's side");
  assert.equal(s.relay.net.made.filter((m) => m.who === "Ann").length, 1, "and one on Ann's");
  assert.equal(s.relay.signals.filter((x) => x.msg.signal_type === "dc_offer").length, 1, "dialled once");
  const benPc = [...s.relay.net.pcs.values()].find((p) => p.page.who === "Ben");
  assert.deepEqual(benPc.channels.map((c) => c.label), ["file:" + tOne.id, "file:" + tTwo.id], "a channel each, both opened by Ben");
  const got = await savedFiles(s.ben);
  assert.equal(got.length, 2, "two files saved");
  assert.ok(got.some((g) => sameBytes(g, one)) && got.some((g) => sameBytes(g, two)), "each exactly its bytes");
  await waitFor(() => wordsOf(s.ann, tOne.id) === "Sent" && wordsOf(s.ann, tTwo.id) === "Sent", "Ann's rows both say Sent");
  assert.deepEqual([...(s.ann.errors || []), ...(s.ben.errors || [])], [], "no errors");

  // b. One each way: Ann offers Ben a file and Ben offers Ann one. Ben accepts (and dials); then
  //    Ann accepts, and as the receiver she opens her channel on the connection Ben dialled.
  s = await pair();
  const toBen = zipBytes(7 * MB, 33);
  const toAnn = zipBytes(7 * MB + 5, 34);
  const tToBen = await offerFrom(s.ann, toBen, "to-ben.zip");
  const tToAnn = await offerFrom(s.ben, toAnn, "to-ann.zip");
  await waitFor(() => transferOf(s.ben, tToBen.id) && transferOf(s.ben, tToBen.id).state === "asked"
    && transferOf(s.ann, tToAnn.id) && transferOf(s.ann, tToAnn.id).state === "asked", "each asked");
  await press(s.ben, tToBen.id, "accept");
  await waitFor(() => (transferOf(s.ben, tToBen.id).done || 0) >= 2, "Ben's is moving");
  await press(s.ann, tToAnn.id, "accept");
  await waitFor(() => transferOf(s.ben, tToBen.id).state === "saved" && transferOf(s.ann, tToAnn.id).state === "saved", "b. the file each way saved");
  assert.equal(s.relay.net.made.length, 2, "one connection: a peer connection on each side");
  assert.deepEqual(s.relay.signals.filter((x) => x.msg.signal_type === "dc_offer").map((x) => x.who), ["Ben"], "dialled once, by Ben");
  const annPc = [...s.relay.net.pcs.values()].find((p) => p.page.who === "Ann");
  assert.deepEqual(annPc.channels.map((c) => c.label), ["file:" + tToAnn.id], "Ann opened her own transfer's channel on Ben's connection");
  assert.ok(sameBytes((await savedFiles(s.ben))[0], toBen) && sameBytes((await savedFiles(s.ann))[0], toAnn), "each file exactly its bytes");
  await waitFor(() => wordsOf(s.ann, tToBen.id) === "Sent" && wordsOf(s.ben, tToAnn.id) === "Sent", "and each sender's row says Sent");
  assert.deepEqual([...(s.ann.errors || []), ...(s.ben.errors || [])], [], "no errors");
});

test("14. the receiver's answer and cancel carry the offer's file pass on their put, so they reach a sender who takes messages only from friends", async () => {
  // Ann and Ben are not friends; Ben accepts files from anyone he talks to; Ann takes messages only
  // from friends. The server's call relay is off, so only the notes can move Ann's row.
  const s = await pair({ friends: false, ben: { stored: { hos_files_in_messages: JSON.stringify({ accept: "anyone" }) } } });
  s.relay.onlyFriends.set(s.ppl.ann.key, new Set());
  s.relay.forwarder = false;
  const t = await offer(s, zipBytes(7 * MB, 51));
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  await press(s.ben, t.id, "accept");
  const putsTo = (key) => s.ben.sock.sent.filter((m) => m.type === "dm_put" && m.to === key);
  assert.equal(putsTo(s.ppl.ann.key).length, 1, "one put to Ann");
  assert.deepEqual(putsTo(s.ppl.ann.key)[0].file_pass, t.offer.pass, "the answer's put carries the offer's pass, the same object");
  assert.ok(!("file_pass" in putsTo(s.ppl.ben.key)[0]), "its copy to my other devices carries none");
  await waitFor(() => transferOf(s.ann, t.id).state === "accepted", "the answer reached Ann: her row moved on");
  assert.equal(s.relay.refusedPuts.length, 0, "nothing refused");
  // Ben cancels: its put carries the pass too, and Ann's row says so.
  await press(s.ben, t.id, "cancel");
  const cancel = putsTo(s.ppl.ann.key).find((m) => FT.cancelNoteText(t.id) === JSON.parse(unb64(JSON.parse(m.content).ct_b64)).text);
  assert.ok(cancel, "the cancel note went to Ann");
  assert.deepEqual(cancel.file_pass, t.offer.pass, "with the pass");
  await waitFor(() => wordsOf(s.ann, t.id) === "Cancelled", "Ann's row says Cancelled");
  // A second offer, declined: the no reaches her too.
  const t2 = await offerFrom(s.ann, zipBytes(7 * MB + 9, 52), "second.zip");
  await waitFor(() => transferOf(s.ben, t2.id) && transferOf(s.ben, t2.id).state === "asked", "asked again");
  await press(s.ben, t2.id, "decline");
  assert.deepEqual(putsTo(s.ppl.ann.key).pop().file_pass, t2.offer.pass, "the no carries its own offer's pass");
  await waitFor(() => wordsOf(s.ann, t2.id) === "Ben declined", "Ann's row says Ben declined");
  assert.equal(s.relay.refusedPuts.length, 0, "nothing refused");
  // The stand-in holds the rule: the same answer without the pass is refused.
  const bare = clone(putsTo(s.ppl.ann.key)[0]);
  delete bare.file_pass;
  s.relay.fromPage(s.ben, bare);
  assert.equal(s.relay.refusedPuts.length, 1, "without the pass, a stranger's put to Ann is refused");
});

test("15. one click on Accept moves the row on at once and starts the transfer, the save dialog closed, open or absent; Decline, Pause, Resume and Cancel too, and a redraw never replaces a button", async () => {
  // a. A browser that cannot show the save dialog closes it at once: headless Edge, measured
  //    2026-10-12, rejects showSaveFilePicker after a real click with AbortError "The user aborted
  //    a request." That is the review's real-browser run, where Accept did nothing.
  let s = await pair();
  const asked = [];
  s.ben.ctx.showSaveFilePicker = (o) => { asked.push(o); return Promise.reject(Object.assign(new Error("The user aborted a request."), { name: "AbortError" })); };
  let bytes = zipBytes(7 * MB, 61);
  let t = await offer(s, bytes);
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  rowParts(rowOf(s.ben, t.id)).button("accept").onclick(); // one click, nothing awaited
  assert.equal(asked.length, 1, "the save dialog was asked for in the click itself");
  assert.equal(wordsOf(s.ben, t.id), "Connecting", "the row moved on at once");
  assert.deepEqual(rowParts(rowOf(s.ben, t.id)).buttons, ["pause", "cancel"], "Accept and Decline gone at once");
  await waitFor(() => only(s.ben).state === "saved", "the file arrived and was saved");
  assert.equal(s.ben.blobs.size, 1, "through the browser's own download");
  assert.ok(sameBytes((await savedFiles(s.ben))[0], bytes), "exactly its bytes");

  // b. The dialog still open while the file moves: the file waits for the place, then goes into it.
  s = await pair();
  let choose = null;
  s.ben.ctx.showSaveFilePicker = () => new Promise((r) => { choose = r; });
  bytes = zipBytes(7 * MB + 3, 62);
  t = await offer(s, bytes);
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  rowParts(rowOf(s.ben, t.id)).button("accept").onclick();
  assert.equal(wordsOf(s.ben, t.id), "Connecting", "the row moved on at once, the dialog open");
  await waitFor(() => only(s.ben).awaitingPlace === true, "every piece in, waiting for the place");
  assert.equal(wordsOf(s.ben, t.id), "Choose where to save it");
  const written = [];
  choose({ name: "archive.zip", async createWritable() { return { async write(b) { written.push(Buffer.from(await b.arrayBuffer())); }, async close() {} }; } });
  await waitFor(() => only(s.ben).state === "saved", "saved into the place chosen");
  assert.ok(sameBytes(Buffer.concat(written), bytes), "exactly its bytes, written there");
  assert.equal(s.ben.blobs.size, 0, "and no download");

  // c. Decline: one click, the row says so at once.
  s = await pair({ ben: { indexedDB: fakeIndexedDB() } });
  t = await offer(s, zipBytes(7 * MB, 63));
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  rowParts(rowOf(s.ben, t.id)).button("decline").onclick();
  assert.equal(wordsOf(s.ben, t.id), "You declined", "Decline: at once");

  // d. While a file moves, its row is redrawn with every piece; Pause stays the same element (a
  //    browser clicks only the element the button went down and came up on), and Pause, Resume and
  //    Cancel each change the row at once.
  const idb = fakeIndexedDB();
  s = await pair({ ben: { indexedDB: idb } });
  t = await offer(s, zipBytes(14 * MB, 64));
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  rowParts(rowOf(s.ben, t.id)).button("accept").onclick();
  await waitFor(() => idb.pieces(t.id).length >= 2, "moving");
  const row = rowOf(s.ben, t.id);
  const down = rowParts(row).button("pause");
  const widthDown = rowParts(row).bar.style.width;
  await waitFor(() => idb.pieces(t.id).length >= 12 && rowParts(rowOf(s.ben, t.id)).bar.style.width !== widthDown, "redrawn as pieces arrived");
  assert.equal(rowOf(s.ben, t.id), row, "the same row");
  const up = rowParts(rowOf(s.ben, t.id)).button("pause");
  assert.ok(up === down, "the same Pause button after the redraws: a click across them still lands on it");
  up.onclick();
  assert.equal(wordsOf(s.ben, t.id), "Paused", "Pause: at once");
  assert.deepEqual(rowParts(row).buttons, ["resume", "cancel"]);
  rowParts(row).button("resume").onclick();
  assert.equal(wordsOf(s.ben, t.id), "Receiving through the server's call relay", "Resume: at once");
  rowParts(row).button("cancel").onclick();
  assert.equal(wordsOf(s.ben, t.id), "Cancelled", "Cancel: at once");
  await waitFor(() => wordsOf(s.ann, t.id) === "Cancelled", "and Ann's row says so");
  // Each button says what it does in its hint (shell.js's tooltip had only "Tap or click to use this control.").
  assert.equal(rowParts(row).button("cancel") === null, true, "Cancel gone once cancelled");
  const accept = everyUnder(rowOf(s.ben, t.id)).find((e) => e.dataset && e.dataset.ft === "accept");
  assert.equal(accept.attrs["data-tip-desc"], "Say yes. The file comes straight from Ann's device, and you can choose where to save it.", "Accept's hint");
});


test("16. a file that arrived whole but could not be saved says so, and can be saved again or given up", async () => {
  // Seen red 2026-10-12 with the try-save button left out of ftButtonsFor (the row had no buttons
  // at all, as before): "try again or give it up".
  const s = await pair();
  let choose = null;
  s.ben.ctx.showSaveFilePicker = () => new Promise((r) => { choose = r; });
  const bytes = zipBytes(7 * MB + 5, 64);
  const t = await offer(s, bytes);
  await waitFor(() => transfers(s.ben).length === 1 && only(s.ben).state === "asked", "asked");
  rowParts(rowOf(s.ben, t.id)).button("accept").onclick();
  await waitFor(() => only(s.ben).awaitingPlace === true, "every piece in, waiting for the place");
  choose({ name: "x.zip", async createWritable() { throw new Error("the disk is full"); } });
  await waitFor(() => wordsOf(s.ben, t.id) === "It arrived whole, but this browser could not save it", "the row says it could not be saved");
  assert.deepEqual(rowParts(rowOf(s.ben, t.id)).buttons, ["try-save", "cancel"], "try again or give it up");
  rowParts(rowOf(s.ben, t.id)).button("try-save").onclick();
  await waitFor(() => only(s.ben).state === "saved", "saved on the second try");
  assert.equal(s.ben.blobs.size, 1, "through the browser's own download");
  assert.ok(sameBytes((await savedFiles(s.ben))[0], bytes), "exactly its bytes");
});

// ── Red first ────────────────────────────────────────────────────────────
// 2026-10-11: each break made in a copy of web/ and this file run against it with HOS_WEB_DIR,
// seen failing with the assertion named; every other test passed unless named.
//  C1  chat-messages.js, a big file in a DM not routed to offerFileDirect: every test, "the attach
//      step is shown" (the file was refused with the limit's words instead).
//  C2  chat-file-transfer.js, ftTakeOffer not applying Files in messages: test 2, 'asked' == 'refused'.
//  C3  chat-file-transfer.js, Decline answering accept true: test 3 "the answer, no".
//  C4  chat-file-transfer.js, the dc_offer without the pass: test 4 "the dc_offer carries the pass".
//  C5  chat-file-transfer.js, iceTransportPolicy 'all' without direct allowed: test 4 (Ben's
//      connection settings) and test 12 "Ann's setting is off: her address stays hers".
//  C6  chat-file-transfer.js, Cancel without the cancel note: test 5 "the cancel note to Ben".
//  C7  chat-file-transfer.js, saving on "checking" (before the whole-file check): test 4 "one Blob
//      made" (two), and test 10, 'saved' == 'failed'.
//  C8  chat-social.js, ingestDmControl not handing the notes over: tests 2 to 12, "Ben's page has
//      the offer" / "asked".
//  C9  chat-voice-rooms.js, the file request without peer and file_pass: test 4 "{file, peer,
//      file_pass}".
//  C10 chat-file-transfer.js, my own answer from another device not closing the question: test 11
//      "the other device says so".
//  C11 chat-file-transfer.js, direct connections allowed with the protected setup on: test 12,
//      'all' == 'relay'.
//  C12 chat-file-transfer.js, Pause not telling the channel: test 6 "Ann's row says so". (Its first
//      form left a dangling `else`, a syntax error that failed every test and proved nothing; redone.)
//  C13 chat-file-transfer.js, the member list not dialing a waiting transfer: test 8 "Ben asks again".
//  C14 chat-file-transfer.js, an offer whose pass has ended asked about anyway: test 9, 'asked' ==
//      'expired'.
//  C15 chat-file-transfer.js, ftTakeOffer not checking the pass: passed every test at first (all the
//      passes were genuine), so test 1c was added; then test 1c "Ben's page keeps nothing of it".
//  C16 chat-file-transfer.js, no credentials not marked on the row: test 7 "Ben's row says so".
//  C17 chat-file-transfer.js, a piece written after a cancel left behind: passed at first (the test
//      never had a write outlast the cancel), so test 5 holds one with the stand-in IndexedDB's gate;
//      then test 5 "the pieces Ben had are dropped from his device, the one written after the cancel
//      too".
//  C18 chat-file-transfer.js, the receiver's store starting from 0 after a reload: test 8 "asking
//      from the first piece kept before the reload".
// C1 to C11, C13, C14, C16 and C18 were run before tests 1c and the gate were added; those changes
// only add assertions.
//
// 2026-10-12, the review's rules (rules 2 and 4, and the Accept click): each break made in a fresh
// copy of web/ (HOS_WEB_DIR) and the whole file run against it; the tests named failed with the
// assertion quoted, every other test passed.
//  C19 chat-file-transfer.js, ftDial returning when a connection with the person is there (one
//      transfer per connection, as before): test 13 "a. both files to Ben saved".
//  C20 chat-file-transfer.js, the answering side handing a new channel to the first transfer's
//      engine (as before, where link.id went to the newest channel): test 13 "a. both files to Ben
//      saved".
//  C21 chat-file-transfer.js, the dialling side taking no channel the other side opens: test 13 "b.
//      the file each way saved".
//  C22 chat-file-transfer.js, a transfer's end closing the whole connection (as before): test 13 "a.
//      both files to Ben saved".
//  C23 chat-file-transfer.js, the put to the sender without file_pass: test 14 "the answer's put
//      carries the offer's pass, the same object".
//  C24 chat-file-transfer.js, the Accept from before 2026-10-12 put back (it waited for the save
//      dialog and returned when it was closed): test 15 "the row moved on at once", and test 14 (that
//      Accept sent no pass).
//  C25 chat-file-transfer.js, ftFill rebuilding the row on every redraw (as before): test 15 "the
//      same Pause button after the redraws".
//  C26 chat-file-transfer.js, ftEnd drawing the row only after the pieces are dropped: test 15
//      "Cancel: at once".
