// Reports the admins can check, in the web chat client (step D, 2026-10-09,
// docs/design/blocking-and-safe-mode.md section 8 and 10e), mirroring the desktop app.
//
// Run: node --test scripts/tests/report-web.test.js   (in `just rig-tests`)
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with the
// DOM replaced by a stub that keeps what is written to it, as in block-web.test.js (whose harness,
// idle-waiting settle() included, this copies): the real report.js, friend-pass.js, reach.js,
// block.js, crypto.js, chat-dm-store.js (over a stand-in IndexedDB), app.js, chat-messages.js,
// chat-dms.js, chat-social.js, chat-groups-p2p.js, chat-ui.js, chat-voice-rooms.js,
// chat-voice-calls.js, chat-voice-modal.js, chat-privacy.js, chat-reports.js and chat-p2p.js, in
// index.html's order. The stub DOM keeps the click listeners a script adds, so a test can press a
// button the voice modal built.
// BLAKE3 is the REAL one, the vendored bundle the page loads (web/shared/vendor/noble-pq.bundle.js),
// so the evidence hash here is the one the relay recomputes. Only the Dilithium and Kyber
// primitives are stand-ins: "signing" returns the signed words, so a test reads exactly what was
// signed; "sealing" base64s the plaintext. The real primitives are covered by scripts/pq-kat.mjs
// and the relay's tests. The reasons file (data/safety/report_reasons.json, the relay team's) is a
// stub here; when the real file is in the checkout its ids are held to the spec too.
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves (10e's web items):
//  0. The words the reporter signs are the relay's: read out of the relay's Rust test when it pins a
//     "hum/report/v1\n..." literal, else out of 10e's own text, saying so. The frame's field names,
//     the contexts, the decisions and the reason ids are 10e's, and the hash is plain BLAKE3.
//  1. The builder: the frame is exactly 10e's, the hash is over the evidence array's JSON exactly as
//     it travels inside the sent frame, the signature is over the 10e words, and what the relay
//     would refuse (myself, too many items, too much text, no reason) is refused before sending.
//  2. A message's menu opens the dialog (not the old prompt) on that post: the reasons from the data
//     file, a reason's help shown when chosen (the danger sentence for child_danger), and Send sends
//     exactly {type:"report_v2", target, context:"post", reason, note, evidence:[the post], ts, sig},
//     nothing else, and blocks no one; report_received shows the short confirmation.
//  3. A DM report from the conversation header lists that person's messages only, newest first, the
//     most recent ticked; each item sent is the verified inner payload {kind:"dm",from,to,ts,text,sig}
//     whose sig is the DM signature the relay checks; "Also block them" is ticked and blocks
//     (passes withdrawn, the note to myself), the report going first; unticked, it does not;
//     a 21st message cannot be ticked.
//  4. The member list's menu and /report <name> report a profile, with no evidence; a group
//     message's menu sends the words seen, unproven.
//  5. The Reports view (admins and mods, in the chat app): reports_list for open and decided, the
//     checked and unproven badges, what a checked signature does not prove, "a member" when the
//     relay leaves the reporter out, Delete the post only for a post, and report_decide sent exactly;
//     a plain member cannot open it.
//  6. The privacy explanation carries 10e's sentence.
//  7. A message with a file (its text carries the key that opens the file, and the signature covers
//     the whole text, so it cannot be cut out) is never offered in the DM picker, never made into an
//     item, refused by the frame builder, and never travels; the dialog says "Messages with files
//     cannot be included in a report." A group message with a file is left out the same way.
//  8. The voice modal's Block, Unblock, Follow, Direct message, a moderation action and Report reach
//     the person (chat-ui.js setCtxMenuTarget; setting window.ctxMenuTarget never reached chat-ui's
//     `let ctxMenuTarget`, so these did nothing before 2026-10-09).
//
// Red first, 2026-10-09: each mutation made in a fresh copy of web/, this test run against it with
// HOS_WEB_DIR, and seen failing with the assertion named (each passing again on the real web/):
//  0: report.js reportPreimage writing the target before the reporter: "the preimage is the relay's"
//     (and "signed over 10e's words with my key as reporter" in test 1, the sent frames in 2 and 3).
//  1: report.js reportEvidenceJson as JSON.stringify(evidence, null, 1): "the hash is over the
//     evidence exactly as it travels in the frame".
//  2: chat-ui.js reportUser taking context 'profile' always: "context post, the post as evidence"
//     (and the group case in test 4). chat-reports.js reportDialogHtml not drawing the chosen
//     reason's help: "the chosen reason's help".
//  3: chat-dm-store.js insert without `sig` in the record: "the store keeps each message's
//     signature". openReportDialog ticking none: "the most recent one ticked". `alsoBlock: false`
//     for DM reports: "Also block them is ticked".
//  4: openReportDialog keeping a group message as a post item: "the words seen in the group, unproven".
//  5: report.js reportEvidenceBadge ignoring `checked`: "the forged one is Not proven".
//     decideReport sending `note: ''`: "report_decide exactly, then the open list again".
//     reportsViewModel naming the target when the relay leaves the reporter out: "a member, when
//     the relay leaves the reporter out" (and "a mod sees a member").
//  6: chat-privacy.js privacyExplanationText without the sentence: "the privacy explanation".
//  7: each file guard taken out alone, each caught by its own assertion: reportDmCandidates without
//     its reportTextHasFile filter: "the picker never offers a message with a file";
//     dmEvidenceItem's check: "dmEvidenceItem gives no item for a file"; groupEvidenceItem's:
//     "groupEvidenceItem gives no item for a file"; reportEvidenceProblem's: "the frame builder
//     refuses a file item put there by hand"; the sentence left out of the DM dialog: "the dialog
//     says so".
//  8: chat-voice-modal.js withTarget setting window.ctxMenuTarget again, as before the fix:
//     "Block blocks them".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const report = require(path.join(WEB, "shared", "report.js"));
const fp = require(path.join(WEB, "shared", "friend-pass.js"));

const SPEC = fs.readFileSync(path.join(ROOT, "docs", "design", "blocking-and-safe-mode.md"), "utf8");
// 10e with its line wrapping undone (a list or a sentence may wrap in the source).
const TEN_E = SPEC.slice(SPEC.indexOf("## 10e."), SPEC.indexOf("## 11.")).replace(/\s+/g, " ");

// The real BLAKE3, from the bundle the page loads. Imported from its text as a module (it is ESM
// in a .js file under a package.json with no "type", which Node would otherwise warn about).
let nobleBlake3 = null;
async function blake3() {
  if (!nobleBlake3) {
    const src = fs.readFileSync(path.join(WEB, "shared", "vendor", "noble-pq.bundle.js"));
    const m = await import("data:text/javascript;base64," + src.toString("base64"));
    nobleBlake3 = (bytes) => m.blake3.create({ dkLen: 32 }).update(bytes).digest();
  }
  return nobleBlake3;
}
const hex = (u8) => Buffer.from(u8).toString("hex");

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
// rest. Until its HTML is written, its innerHTML is its text escaped, as a browser's is.
const escapeText = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
// Its listeners are kept too, so a test can click a button a script built (the voice modal).
function fakeElement(tag) {
  const own = { tag, children: [], style: {}, dataset: {}, className: "", textContent: "", value: "", disabled: false, listeners: {} };
  own.appendChild = (c) => { own.children.push(c); return c; };
  own.addEventListener = (type, fn) => { (own.listeners[type] = own.listeners[type] || []).push(fn); };
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

function fakeDocument(state) {
  const byId = new Map();
  const bySelector = new Map();
  const kept = (map, k) => { if (!map.has(k)) map.set(k, fakeElement(k)); return map.get(k); };
  return new Proxy({}, {
    get(_t, prop) {
      if (prop === "createElement") return (tag) => { const e = fakeElement(tag); state.created.push(e); return e; };
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
  return { readyState: 1, sent: [], raw: [], send(s) { this.raw.push(s); this.sent.push(JSON.parse(s)); }, close() {} };
}

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
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const MY_KYBER = "my-kyber";
const kyberOf = (k) => k === ME ? MY_KYBER : "kyber-" + k.slice(0, 4);
const MAY = "invite,message,trade,voice_message";
const DANGER = "If anyone is in danger right now, contact your local emergency number. This server's admins are volunteers, not police.";

// The reasons file as 10e describes it (the relay team writes the real one).
const SPEC_REASON_IDS = ["spam", "scam", "harassment", "threats", "hate", "unwanted_sexual", "impersonation", "child_danger", "someone_in_danger", "other"];
const STUB_REASONS = {
  _purpose: "Stub for scripts/tests/report-web.test.js",
  reasons: SPEC_REASON_IDS.map((id) => ({
    id,
    label: "Label " + id,
    help: id === "child_danger" || id === "someone_in_danger" ? DANGER : "Help for " + id,
  })),
};

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

async function loadChat(opts = {}) {
  const state = { appended: [], hidden: false, created: [] };
  const notified = [];
  const fetched = [];
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDocument(state),
    navigator: new Proxy({}, { get: (_t, p) => (p === "serviceWorker" ? serviceWorker : anything()), has: () => true }),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    indexedDB: fakeIndexedDB(),
    // No network but the two files the page asks this server for here.
    fetch: (url) => {
      fetched.push(String(url));
      if (String(url).startsWith("/api/federation/servers")) return Promise.resolve({ ok: true, json: async () => [] });
      if (String(url).startsWith(report.REPORT_REASONS_URL)) {
        return opts.noReasons ? Promise.reject(new Error("offline")) : Promise.resolve({ ok: true, json: async () => JSON.parse(JSON.stringify(STUB_REASONS)) });
      }
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
    prompt: () => { throw new Error("the old Report prompt was used"); },
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
  run("shared/report.js");
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
  run("chat/chat-voice-modal.js");
  run("chat/chat-privacy.js");
  run("chat/chat-reports.js");
  run("chat/chat-p2p.js");
  for (const name of ["hosIcon", "holdToConfirm", "holdConfirm", "updateStats"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  ctx.appendMessage = (el) => { state.appended.push(el); };
  ctx.notifyNewMessage = (...args) => { notified.push(args); };
  ctx.playNotificationChime = () => {};
  ctx.sendSWNotification = () => {};
  // The stand-in primitives (see the top of this file), and the real BLAKE3.
  const key = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => key;
  ctx.pqSignMessage = async (_secret, bytes) => new Uint8Array(bytes);
  ctx.pqVerifyMessage = async (_pk, bytes, sig) => Buffer.from(bytes).equals(Buffer.from(sig));
  ctx.pqDmSeal = async (pub, plaintext) => ({ ek_ct_b64: b64(pub), nonce_b64: "AAAA", ct_b64: b64(plaintext) });
  ctx.pqDmOpen = async (_secret, ek, _nonce, ct) => (unb64(ek) === MY_KYBER ? unb64(ct) : null);
  const realBlake3 = await blake3();
  ctx.pqBlake3 = async (bytes) => realBlake3(bytes);
  const sock = fakeSocket();
  vm.runInContext(`(s, me) => {
    ws = s; myKey = me; myName = 'Me_1'; activeChannel = 'general';
    myDilithiumPublicHex = me; myDilithiumSecret = new Uint8Array(4);
    myKyberPublicBase64 = '${MY_KYBER}'; myKyberSecret = new Uint8Array(4);
  }`, ctx)(sock, ME);
  const handle = async (msg) => { await vm.runInContext("handleMessage", ctx)(msg); await settle(); };
  await handle({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  const store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await store.init(ME, "localhost"), "the store loads");
  store.setPassServer(SERVER);
  const users = [[ME, "Me_1", opts.myRole || ""], [ANN, "Ann", ""], [BEN, "Ben", ""], [CY, "Cy", ""]]
    .map(([k, name, role]) => ({ public_key: k, name, role, kyber_public: kyberOf(k), online: true }));
  await handle({ type: "full_user_list", users });
  // "Anyone" may message me, so DMs from strangers are stored as messages.
  await handle({ type: "reach_settings", settings: { message: "anyone", call: "chosen", trade: "friends" } });
  sock.sent.length = 0;
  sock.raw.length = 0;
  state.appended.length = 0;
  const fn = (name) => vm.runInContext(name, ctx);
  const el = (id) => ctx.document.getElementById(id);
  const dialog = () => vm.runInContext("reportDialog", ctx);
  return { ctx, sock, store, state, appended: state.appended, notified, fetched, handle, fn, el, dialog };
}

function textOf(e) {
  if (!e) return "";
  const own = [e.textContent, typeof e.innerHTML === "string" ? e.innerHTML : ""].filter((s) => typeof s === "string").join(" ");
  return [own, ...(e.children || []).map(textOf)].join(" ");
}
const shown = (appended) => appended.map(textOf).join("\n");

// A DM from `from` to `to`, signed with the stand-in signer and sealed to my DM key.
function envelope(from, to, text, ts) {
  const sig = b64(`hum/dm/v2\n${from}\n${to}\n${ts}\n${text}`);
  const inner = JSON.stringify({ v: 2, from, to, ts, text, sig });
  return JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(inner) });
}
const reportFrames = (sock) => sock.sent.filter((m) => m.type === "report_v2");
const ev = { preventDefault() {}, stopPropagation() {}, clientX: 10, clientY: 10 };

// What the relay does with a report frame (10e): hash the evidence value's JSON as received, and
// check the reporter's signature over the words. The stand-in signature IS the signed words.
function relaySide(raw) {
  const at = raw.indexOf('"evidence":');
  assert.ok(at > 0, "the frame carries evidence");
  // The evidence value's text exactly as received: the array from its '[' to its matching ']'.
  let depth = 0, inStr = false, esc = false, end = -1;
  const start = at + '"evidence":'.length;
  for (let i = start; i < raw.length; i++) {
    const c = raw[i];
    if (inStr) { if (esc) esc = false; else if (c === "\\") esc = true; else if (c === '"') inStr = false; continue; }
    if (c === '"') inStr = true;
    else if (c === "[") depth++;
    else if (c === "]") { depth--; if (depth === 0) { end = i + 1; break; } }
  }
  const evidenceText = raw.slice(start, end);
  const frame = JSON.parse(raw);
  return { frame, evidenceText, signed: unb64(frame.sig) };
}

test("the words the reporter signs are the relay's, and the frame is 10e's", async (t) => {
  // The relay's Rust test pins the preimage with a literal when it exists in this checkout.
  const rsFiles = [];
  const walk = (dir) => {
    for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, ent.name);
      if (ent.isDirectory()) walk(p);
      else if (ent.name.endsWith(".rs")) rsFiles.push(p);
    }
  };
  walk(path.join(ROOT, "src"));
  const unescape = (s) => s.replace(/\\(.)/g, (_, c) => ({ n: "\n", t: "\t", r: "\r", "\\": "\\", '"': '"' })[c] ?? c);
  const pins = [];
  for (const f of rsFiles) {
    const src = fs.readFileSync(f, "utf8");
    if (!src.includes("hum/report/v1")) continue;
    for (const m of src.matchAll(/"(hum\/report\/v1\\n(?:[^"\\]|\\.)*)"/g)) {
      const words = unescape(m[1]);
      if (words.split("\n").length === 6) pins.push({ file: path.relative(ROOT, f), words });
    }
  }
  if (pins.length) {
    for (const { file, words } of pins) {
      const [, reporter, target, reason, hash, ts] = words.split("\n");
      assert.equal(report.reportPreimage(reporter, target, reason, hash, ts), words, `the preimage is the relay's (${file})`);
    }
    t.diagnostic(`held to ${pins.length} pinned literal(s) in the relay's Rust`);
  } else {
    // Not in this checkout yet (the relay is built in parallel): 10e's own text is the reference.
    t.diagnostic("no Rust test pins hum/report/v1 in this checkout yet: compared with 10e's text instead");
    const m = TEN_E.match(/`"(hum\/report\/v1\\n\{reporter\}\\n\{target\}\\n\{reason\}\\n\{evidence_hash\}\\n\{ts\})"`/);
    assert.ok(m, "10e gives the words");
    const fill = { reporter: ME, target: BEN, reason: "harassment", evidence_hash: "ab".repeat(32), ts: "1760000000000" };
    const want = unescape(m[1]).replace(/\{(\w+)\}/g, (_, k) => fill[k]);
    assert.equal(report.reportPreimage(ME, BEN, "harassment", "ab".repeat(32), 1760000000000), want, "the preimage is the relay's");
  }
  assert.equal(report.REPORT_DOMAIN, "hum/report/v1");

  // The frame's field names, in 10e's order.
  const shape = TEN_E.match(/`\{"type":"report_v2",([^`]*)\}`/);
  assert.ok(shape, "10e gives the frame");
  const specKeys = ["type", ...[...shape[1].matchAll(/"(\w+)"/g)].map((x) => x[1])];
  assert.deepEqual(specKeys, ["type", "target", "context", "reason", "note", "evidence", "ts", "sig"]);
  // The lists 10e names.
  const words = (re) => { const mm = TEN_E.match(re); assert.ok(mm, String(re)); return [...mm[1].matchAll(/`(\w+)`/g)].map((x) => x[1]); };
  assert.deepEqual(report.REPORT_CONTEXTS, words(/`context` one of ((?:`\w+`(?:, |,? or )?)+)/), "the contexts are 10e's");
  assert.deepEqual(report.REPORT_DECISIONS, words(/`decision` one of ((?:`\w+`(?:, |,? or )?)+)/), "the decisions are 10e's");
  assert.deepEqual(SPEC_REASON_IDS, words(/with ids ((?:`\w+`(?:,\s*|,?\s*and\s*)?)+)/), "the stub's reason ids are 10e's");
  assert.equal(report.REPORT_MAX_ITEMS, 20);
  assert.equal(report.REPORT_MAX_EVIDENCE_BYTES, 64 * 1024);
  assert.equal(report.REPORT_NOTE_MAX, 500);
  assert.ok(/At most 20 items, and at most 64 KB of evidence/.test(TEN_E) && /`note` at most 500/.test(TEN_E));

  // The real reasons file, once the relay team's is in this checkout.
  const real = path.join(ROOT, "data", "safety", "report_reasons.json");
  if (fs.existsSync(real)) {
    const reasons = report.reportReasonsFrom(JSON.parse(fs.readFileSync(real, "utf8")));
    assert.deepEqual(reasons.map((r) => r.id), SPEC_REASON_IDS, "data/safety/report_reasons.json reads as 10e's reasons, in order");
    for (const id of ["child_danger", "someone_in_danger"]) {
      assert.ok(reasons.find((r) => r.id === id).help.includes("contact your local emergency number"), `${id}'s help names emergency services`);
    }
  } else {
    t.diagnostic("data/safety/report_reasons.json is not in this checkout yet: only the stub was read");
  }
  // The file read either as a bare list or as an object holding one.
  assert.deepEqual(report.reportReasonsFrom(STUB_REASONS.reasons).map((r) => r.id), SPEC_REASON_IDS);
  assert.deepEqual(report.reportReasonsFrom(STUB_REASONS).map((r) => r.id), SPEC_REASON_IDS);

  // Plain BLAKE3 (the official test vectors), not a keyed or derive-key mode.
  const b3 = await blake3();
  assert.equal(hex(b3(new Uint8Array(0))), "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262");
  assert.equal(hex(b3(new TextEncoder().encode("abc"))), "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85");
});

test("the builder signs 10e's words over the evidence exactly as it travels", async () => {
  const { ctx, fn } = await loadChat();
  const build = fn("pqBuildReport");
  // Text a JSON writer could be tempted to rewrite: quotes, a backslash, a newline, an emoji.
  const evidence = [
    { kind: "dm", from: BEN, to: ME, ts: 1760000000123, text: 'He said "no"\\ then\nleft \u{1F600} café', sig: "c2ln" },
    report.postEvidenceItem(BEN, 1760000000999),
  ];
  const before = Date.now();
  const built = await build({ target: BEN, context: "dm", reason: "harassment", note: "  it keeps happening  ", evidence });
  assert.ok(built && built.frame, built && built.error);
  const f = built.frame;
  assert.deepEqual(Object.keys(f), ["type", "target", "context", "reason", "note", "evidence", "ts", "sig"], "10e's fields, in 10e's order");
  assert.equal(f.type, "report_v2");
  assert.equal(f.target, BEN);
  assert.equal(f.note, "it keeps happening", "the note trimmed");
  assert.ok(Number.isSafeInteger(f.ts) && f.ts >= before && f.ts <= Date.now(), "ts is now, in milliseconds");

  // What the relay does with the frame as it is sent.
  const raw = JSON.stringify(f);
  const { evidenceText, signed } = relaySide(raw);
  assert.equal(evidenceText, JSON.stringify(evidence), "the evidence travels as compact JSON");
  const b3 = await blake3();
  const hash = hex(b3(new TextEncoder().encode(evidenceText)));
  assert.equal(built.evidenceHash, hash, "the hash is over the evidence exactly as it travels in the frame");
  assert.match(hash, /^[0-9a-f]{64}$/, "lowercase hex");
  assert.equal(signed, `hum/report/v1\n${ME}\n${BEN}\nharassment\n${hash}\n${f.ts}`, "signed over 10e's words with my key as reporter");
  assert.equal(built.preimage, signed);

  // What the relay would refuse is refused here, and nothing is built.
  const no = async (fields, why) => {
    const r = await build({ target: BEN, context: "post", reason: "spam", evidence: [], ...fields });
    assert.ok(r && r.error && !r.frame, why);
    return r.error;
  };
  assert.equal(await no({ target: ME }, "myself"), "You can't report yourself.");
  await no({ target: "Ben" }, "a name is not a key");
  await no({ context: "chat" }, "an unknown context");
  await no({ reason: "" }, "no reason");
  const dm = (i) => ({ kind: "dm", from: BEN, to: ME, ts: i, text: "x", sig: "c2ln" });
  await no({ evidence: Array.from({ length: 21 }, (_, i) => dm(i)) }, "21 items");
  await no({ evidence: [{ ...dm(1), text: "y".repeat(64 * 1024) }] }, "more than 64 KB");
  await no({ evidence: [{ kind: "dm", from: BEN, to: ME, ts: 1, text: "x" }] }, "a DM item without its signature");
  assert.ok((await build({ target: BEN, context: "post", reason: "spam", evidence: Array.from({ length: 20 }, (_, i) => dm(i)) })).frame, "20 items go");
  // A note longer than 500 characters goes as its first 500 (characters, not bytes).
  const long = await build({ target: BEN, context: "profile", reason: "other", note: "\u{1F600}".repeat(600), evidence: [] });
  assert.equal(Array.from(long.frame.note).length, 500);
  assert.equal(relaySide(JSON.stringify(long.frame)).evidenceText, "[]", "no evidence is an empty list");
  // Not signed in: nothing.
  vm.runInContext("myDilithiumSecret = null", ctx);
  assert.ok((await build({ target: BEN, context: "post", reason: "spam", evidence: [] })).error);
});

test("a message's menu opens the dialog on that post, and Send sends exactly the frame", async () => {
  const { sock, store, appended, fetched, handle, fn, el, dialog } = await loadChat();
  // The menu opened on Ben's post (app.js passes the message it was opened on).
  fn("showUserContextMenu")(ev, "Ben", BEN, { timestamp: 1760000000555, text: "buy my scam coin", group: false });
  const menu = el("user-context-menu").innerHTML;
  assert.ok(menu.indexOf("blockFromCtx()") < menu.indexOf("reportUser()"), "Report beside Block");
  fn("reportUser")();
  await settle();
  const st = dialog();
  assert.ok(st, "the dialog is open (no prompt)");
  assert.equal(st.context, "post", "context post, the post as evidence");
  assert.deepEqual(JSON.parse(JSON.stringify(fn("reportDialogEvidence")(st))), [{ kind: "post", from: BEN, timestamp: 1760000000555 }], "context post, the post as evidence");
  assert.equal(st.alsoBlock, false, "Also block them is offered, not ticked, for a post");
  assert.ok(fetched.includes(report.REPORT_REASONS_URL), "the reasons come from the data file");
  let html = el("report-card").innerHTML;
  for (const r of STUB_REASONS.reasons) assert.ok(html.includes(`data-report-reason="${r.id}"`) && html.includes(r.label), `reason ${r.id} listed`);
  assert.ok(html.includes("buy my scam coin"), "the post is shown");
  assert.ok(html.includes("data-report-send disabled"), "Send waits for a reason");

  // Send before a reason: nothing goes.
  assert.equal(await fn("submitReportDialog")(), false);
  assert.deepEqual(reportFrames(sock), []);
  assert.ok(el("report-card").innerHTML.includes("Choose a reason first."));

  // A reason's help is shown when chosen; the danger reasons say where to go first.
  assert.ok(fn("reportDialogChoose")("child_danger"));
  html = el("report-card").innerHTML;
  assert.ok(html.includes(`data-report-help="child_danger"`) && html.includes(DANGER.replace(/'/g, "&#39;")), "the chosen reason's help");
  assert.ok(fn("reportDialogChoose")("scam"));
  html = el("report-card").innerHTML;
  assert.ok(html.includes("Help for scam") && !html.includes("contact your local emergency number"), "the chosen reason's help, and only it");
  assert.equal(fn("reportDialogChoose")("not_a_reason"), false, "only the file's reasons");
  fn("reportDialogSetNote")("He posts this in every channel.");

  assert.equal(await fn("submitReportDialog")(), true);
  await settle();
  const sent = reportFrames(sock);
  assert.equal(sent.length, 1);
  const f = sent[0];
  assert.deepEqual(Object.keys(f), ["type", "target", "context", "reason", "note", "evidence", "ts", "sig"]);
  assert.deepEqual({ ...f, ts: 0, sig: "" }, {
    type: "report_v2", target: BEN, context: "post", reason: "scam", note: "He posts this in every channel.",
    evidence: [{ kind: "post", from: BEN, timestamp: 1760000000555 }], ts: 0, sig: "",
  }, "the exact frame");
  const { evidenceText, signed } = relaySide(sock.raw[sock.sent.indexOf(f)]);
  const hash = hex((await blake3())(new TextEncoder().encode(evidenceText)));
  assert.equal(signed, `hum/report/v1\n${ME}\n${BEN}\nscam\n${hash}\n${f.ts}`);
  assert.deepEqual(sock.sent.map((m) => m.type), ["report_v2"], "nothing else is sent");
  assert.equal(store.isBlocked(BEN), false, "and no one is blocked");
  assert.equal(dialog(), null, "the dialog closes");

  // The relay's answer, as one short line.
  await handle({ type: "report_received", id: 41 });
  assert.ok(shown(appended).includes(report.REPORT_RECEIVED_LINE));
  assert.ok(!/who reported/.test(report.REPORT_RECEIVED_LINE) || /not told who reported/.test(report.REPORT_RECEIVED_LINE));
});

test("a DM report carries their messages' inner payloads, the most recent ticked, and blocks them", async () => {
  const { ctx, sock, store, handle, fn, el, dialog } = await loadChat();
  const S1 = "11".repeat(16);
  store.recordPassSent(BEN, S1, MAY);
  const ts = [1760000001000, 1760000002000, 1760000003000];
  const words = ["first, with a \"quote\"", "second \u{1F620}", "third\nand last"];
  let id = 100;
  for (let i = 0; i < 3; i++) await handle({ type: "dm_new", id: ++id, content: envelope(BEN, ME, words[i], ts[i]) });
  // My own message to Ben (its self-copy), and Ann's to me: neither is Ben's evidence.
  await handle({ type: "dm_new", id: ++id, content: envelope(ME, BEN, "my reply", 1760000002500) });
  await handle({ type: "dm_new", id: ++id, content: envelope(ANN, ME, "Ann's words", 1760000004000) });
  assert.ok(store.conversation(BEN).every((m) => typeof m.sig === "string" && m.sig), "the store keeps each message's signature");

  // The conversation header offers Report.
  vm.runInContext("(k) => { activeDmPartner = k; activeDmPartnerName = 'Ben'; }", ctx)(BEN);
  fn("renderDmHeader")();
  const header = el("channel-header").innerHTML;
  assert.ok(header.includes("reportActiveDm()") && header.includes(">Report</button>") && header.includes(">Block</button>"), "the DM header offers Report beside Block");
  fn("reportActiveDm")();
  await settle();
  const st = dialog();
  assert.equal(st.context, "dm");
  assert.equal(st.picks.length, 3, "only messages kept with their signature can be evidence, and only theirs to me");
  assert.deepEqual(st.picks.map((p) => p.item.text), [words[2], words[1], words[0]], "newest first");
  assert.deepEqual(st.picks.map((p) => p.picked), [true, false, false], "the most recent one ticked");
  assert.equal(st.alsoBlock, true, "Also block them is ticked");
  let html = el("report-card").innerHTML;
  assert.ok(html.includes("data-report-block checked"), "Also block them is ticked");
  assert.ok(html.includes('data-report-evidence="0" checked') && html.includes('data-report-evidence="2"') && !html.includes('data-report-evidence="2" checked'));
  assert.ok(!html.includes("my reply") && !html.includes("Ann&#39;s words") && !html.includes("Ann's words"), "my messages and others' are not offered");
  assert.ok(html.includes("1 of at most 20 chosen."));

  assert.ok(fn("reportDialogToggle")(2, true), "tick the oldest too");
  fn("reportDialogChoose")("harassment");
  sock.sent.length = 0;
  sock.raw.length = 0;
  assert.equal(await fn("submitReportDialog")(), true);
  await settle();
  const [f] = reportFrames(sock);
  assert.ok(f, "sent");
  assert.equal(f.context, "dm");
  assert.equal(f.evidence.length, 2);
  f.evidence.forEach((item, i) => {
    const k = i === 0 ? 2 : 0; // newest, then the oldest
    assert.deepEqual(Object.keys(item), ["kind", "from", "to", "ts", "text", "sig"], "the DM item's fields, in 10e's order");
    assert.deepEqual({ ...item, sig: "" }, { kind: "dm", from: BEN, to: ME, ts: ts[k], text: words[k], sig: "" });
    assert.equal(unb64(item.sig), `hum/dm/v2\n${BEN}\n${ME}\n${ts[k]}\n${words[k]}`, "the sig is their DM signature, the one the relay checks");
  });
  const { evidenceText, signed } = relaySide(sock.raw[sock.sent.indexOf(f)]);
  assert.equal(signed, `hum/report/v1\n${ME}\n${BEN}\nharassment\n${hex((await blake3())(new TextEncoder().encode(evidenceText)))}\n${f.ts}`);

  // Then Block ran: the report first, then the pass withdrawn and the note to myself.
  assert.ok(store.isBlocked(BEN), "Also block them blocks");
  assert.deepEqual(sock.sent.map((m) => m.type), ["report_v2", "cert_revoke", "dm_put"], "the report first, then Block");
  assert.equal(sock.sent[1].serial, S1);
  assert.equal(sock.sent[2].to, ME, "the block note goes to my own mailbox");
  assert.ok(!sock.sent.some((m) => m.to === BEN), "nothing goes to them");

  // Unticked, it does not block.
  for (let i = 0; i < 2; i++) await handle({ type: "dm_new", id: ++id, content: envelope(CY, ME, "cy " + i, 1760000005000 + i) });
  vm.runInContext("(k) => { activeDmPartner = k; activeDmPartnerName = 'Cy'; }", ctx)(CY);
  await fn("reportActiveDm")();
  fn("reportDialogSetBlock")(false);
  fn("reportDialogChoose")("spam");
  sock.sent.length = 0;
  assert.equal(await fn("submitReportDialog")(), true);
  await settle();
  assert.deepEqual(sock.sent.map((m) => m.type), ["report_v2"], "unticked: the report only");
  assert.equal(store.isBlocked(CY), false);

  // At most 20 can be ticked.
  for (let i = 0; i < 21; i++) await handle({ type: "dm_new", id: ++id, content: envelope(ANN, ME, "ann " + i, 1760000010000 + i) });
  vm.runInContext("(k) => { activeDmPartner = k; activeDmPartnerName = 'Ann'; }", ctx)(ANN);
  await fn("reportActiveDm")();
  const st2 = dialog();
  assert.equal(st2.picks.length, 22, "all 22 of Ann's messages are listed");
  for (let i = 1; i < 20; i++) assert.ok(fn("reportDialogToggle")(i, true));
  assert.equal(fn("reportDialogToggle")(20, true), false, "a 21st cannot be ticked");
  assert.ok(el("report-card").innerHTML.includes("At most 20 messages can be included."));
  assert.equal(fn("reportDialogEvidence")(st2).length, 20);
  fn("closeReportDialog")();
});

test("the member list and /report report a profile; a group message is sent as seen, unproven", async () => {
  const { ctx, sock, fn, el, dialog } = await loadChat();
  // The member list's menu: no message, so a profile report with no evidence.
  fn("showUserContextMenu")(ev, "Cy", CY);
  fn("reportUser")();
  await settle();
  assert.equal(dialog().context, "profile");
  assert.deepEqual(Array.from(fn("reportDialogEvidence")(dialog())), []);
  fn("reportDialogChoose")("impersonation");
  await fn("submitReportDialog")();
  await settle();
  const [f] = reportFrames(sock);
  assert.deepEqual([f.target, f.context, f.reason, f.evidence.length], [CY, "profile", "impersonation", 0]);
  assert.equal(relaySide(sock.raw[sock.sent.indexOf(f)]).evidenceText, "[]");

  // /report <name>, through the composer.
  const input = el("msg-input");
  input.value = "/report ben";
  await fn("sendMessage")();
  await settle();
  assert.equal(dialog().target, BEN, "/report takes the member list's name, letter case aside");
  assert.equal(dialog().context, "profile");
  assert.ok(!sock.sent.some((m) => m.type === "chat" && /\/report/.test(m.content || "")), "and nothing goes to the relay as a command");
  fn("closeReportDialog")();

  // A message's menu inside a P2P group: the words seen, never proven.
  fn("showUserContextMenu")(ev, "Ann", ANN, { timestamp: 1760000000777, text: "group words", group: true });
  fn("reportUser")();
  await settle();
  const st = dialog();
  assert.equal(st.context, "group");
  assert.deepEqual(JSON.parse(JSON.stringify(fn("reportDialogEvidence")(st))), [{ kind: "group_text", from: ANN, ts: 1760000000777, text: "group words" }], "the words seen in the group, unproven");
  assert.ok(el("report-card").innerHTML.includes(report.REPORT_GROUP_UNPROVEN.replace(/'/g, "&#39;")), "and the dialog says so");

  // Myself, or someone whose key is not known: no dialog.
  fn("closeReportDialog")();
  assert.equal(await fn("openReportDialog")({ target: ME, context: "profile" }), null);
  assert.equal(await fn("openReportDialog")({ target: "fed:someone", context: "post" }), null);
  assert.equal(dialog(), null);
  void ctx;
});

test("the reasons file not loading means no report can be sent, and says so", async () => {
  const { sock, fn, el, dialog } = await loadChat({ noReasons: true });
  await fn("openReportDialog")({ target: BEN, context: "profile" });
  assert.deepEqual(Array.from(dialog().reasons), []);
  assert.ok(el("report-card").innerHTML.includes("could not be loaded"));
  assert.equal(fn("reportDialogChoose")("spam"), false);
  assert.equal(await fn("submitReportDialog")(), false);
  assert.deepEqual(reportFrames(sock), []);
});

test("the Reports view: open and decided, the badges, and report_decide", async () => {
  const { sock, handle, fn, el } = await loadChat({ myRole: "admin" });
  assert.equal(fn("openReportsView")(), true);
  assert.deepEqual(sock.sent, [{ type: "reports_list", state: "open" }], "asks for the open reports");
  assert.ok(el("reports-card").innerHTML.includes("Asking the server"));

  const created = 1760000100000;
  await handle({
    type: "reports",
    items: [
      {
        id: 5, target: BEN, target_name: "Ben", context: "dm", reason: "harassment", note: "He keeps at it.",
        evidence: [
          { kind: "dm", from: BEN, to: ANN, ts: 1760000000001, text: "genuine words", sig: "c2ln", checked: true },
          { kind: "dm", from: BEN, to: ANN, ts: 1760000000002, text: "forged words", sig: "eA==", checked: false },
        ],
        created_at: created, state: "open", reporter: ANN, reporter_name: "Ann",
      },
      {
        id: 6, target: CY, target_name: "Cy", context: "post", reason: "spam", note: "",
        evidence: [{ kind: "post", from: CY, timestamp: 1760000000003, text: "buy now", checked: true }],
        created_at: created + 1, state: "open",
      },
      {
        id: 7, target: ANN, target_name: "Ann", context: "group", reason: "other", note: "",
        evidence: [{ kind: "group_text", from: ANN, ts: 1760000000004, text: "group text", checked: false }],
        created_at: created + 2, state: "open",
      },
    ],
  });
  const html = el("reports-card").innerHTML;
  const card = (rid) => { const at = html.indexOf(`data-report-id="${rid}"`); const next = html.indexOf('class="report-item"', at + 1); return html.slice(at, next < 0 ? undefined : next); };
  assert.ok(card(5).includes("Signature checked: sent by Ben to the reporter") && card(5).includes("genuine words"), "the genuine one is checked");
  const forged = card(5).slice(card(5).indexOf("forged words") - 600, card(5).indexOf("forged words"));
  assert.ok(forged.includes(">Not proven<") && !forged.includes("Signature checked"), "the forged one is Not proven");
  assert.ok(card(6).includes("Checked: posted by Cy on this server"), "a post the server found");
  assert.ok(card(7).includes(">Not proven<") && card(7).includes("group text"), "a group message is never proven");
  assert.ok(html.includes(report.REPORT_SIGNATURE_LIMITS.replace(/'/g, "&#39;")), "what a checked signature does not prove");
  assert.ok(/the time is the sender&#39;s own clock/.test(html) && /chose which messages to include/.test(html));
  assert.ok(card(5).includes("reported by Ann"), "an admin is told who reported");
  assert.ok(card(6).includes("reported by a member"), "a member, when the relay leaves the reporter out");
  assert.ok(card(5).includes("He keeps at it."));
  for (const d of ["dismiss", "warn", "mute", "kick", "ban"]) assert.ok(card(5).includes(`data-report-decide="5" data-decision="${d}"`), d);
  assert.ok(!card(5).includes('data-decision="delete_post"') && card(6).includes('data-report-decide="6" data-decision="delete_post"'), "Delete the post, only for a post");

  // A decision, with its note.
  sock.sent.length = 0;
  fn("reportsView").notes["5"] = "  First warning.  "; // as typed into report 5's note box
  assert.equal(fn("decideReport")(5, "mute"), true);
  assert.deepEqual(sock.sent, [
    { type: "report_decide", id: 5, decision: "mute", note: "First warning." },
    { type: "reports_list", state: "open" },
  ], "report_decide exactly, then the open list again");
  assert.equal(fn("decideReport")(5, "shout"), false, "only 10e's decisions");

  // The decided list.
  await handle({ type: "reports", items: [] });
  sock.sent.length = 0;
  fn("reportsShowTab")("decided");
  assert.deepEqual(sock.sent, [{ type: "reports_list", state: "decided" }]);
  await handle({
    type: "reports",
    items: [{ id: 5, target: BEN, target_name: "Ben", context: "dm", reason: "harassment", evidence: [], created_at: created, state: "decided", decision: "mute", reviewer: ME, decided_at: created + 50, decision_note: "First warning." }],
  });
  const dec = el("reports-card").innerHTML;
  assert.ok(dec.includes("Decided: Mute by Me_1") && dec.includes("First warning."), "who decided what");
  assert.ok(!dec.includes("data-report-decide="), "no decision buttons on a decided report");
});

test("a plain member cannot open the Reports view; a mod can, and /reports opens it", async () => {
  const member = await loadChat();
  assert.equal(member.fn("openReportsView")(), false);
  assert.deepEqual(member.sock.sent, [], "nothing asked");
  const mod = await loadChat({ myRole: "mod" });
  const input = mod.el("msg-input");
  input.value = "/reports";
  await mod.fn("sendMessage")();
  assert.deepEqual(mod.sock.sent, [{ type: "reports_list", state: "open" }], "/reports opens the view for a mod");
  await mod.handle({ type: "reports", items: [{ id: 9, target: BEN, context: "profile", reason: "spam", evidence: [], created_at: 1760000000000, state: "open" }] });
  assert.ok(mod.el("reports-card").innerHTML.includes("reported by a member"), "a mod sees a member");
  // The command palette's action is registered.
  assert.equal(typeof mod.fn("CMD_PALETTE_ACTIONS").openReports, "function");
});

test("the privacy explanation carries 10e's sentence", async () => {
  const m = TEN_E.match(/The privacy explanation gains one sentence: ([^.]+\.)/);
  assert.ok(m, "10e gives the sentence");
  const want = m[1].charAt(0).toUpperCase() + m[1].slice(1);
  assert.equal(report.REPORT_PRIVACY_SENTENCE, want);
  const { fn } = await loadChat();
  assert.ok(fn("privacyExplanationText")().includes(want), "the privacy explanation");
  // The page loads the shared words before crypto.js, and the dialog after Block.
  const index = fs.readFileSync(path.join(WEB, "chat", "index.html"), "utf8");
  const at = (s) => index.indexOf(s);
  assert.ok(at("/shared/report.js") > 0 && at("/shared/report.js") < at("/chat/crypto.js"), "report.js before crypto.js");
  assert.ok(at("/chat/chat-privacy.js") < at("/chat/chat-reports.js"), "chat-reports.js after chat-privacy.js");
  // The old prompt-and-slash-command Report is gone.
  const ui = fs.readFileSync(path.join(WEB, "chat", "chat-ui.js"), "utf8");
  const body = ui.slice(ui.indexOf("function reportUser()"), ui.indexOf("function blockFromCtx()"));
  assert.ok(!/prompt\(|\/report \$\{/.test(body), "reportUser opens the dialog");
  assert.ok(fs.readFileSync(path.join(WEB, "chat", "app.js"), "utf8").includes("showUserContextMenu(e, author, fromKey, menuMessage)"), "a message's menu knows its message");
});

test("a message with a file is never offered, built or sent: its text carries the key to the file", async () => {
  const { ctx, sock, handle, fn, el, dialog } = await loadChat();
  // The client's own marker is the one the report words refuse (any version of it).
  const FILE_MARKER = vm.runInContext("FILE_MARKER", ctx);
  assert.ok(FILE_MARKER.startsWith(report.REPORT_FILE_MARKER_PREFIX), "crypto.js's file marker is the one report.js refuses");
  assert.equal(report.REPORT_NO_FILES, "Messages with files cannot be included in a report.");
  const KEY = "S0VZLVRIQVQtT1BFTlMtVEhFLUZJTEU=";
  const fileText = fn("pqBuildFileMarker")({ name: "photo.jpg", mime: "image/jpeg", size: 12, url: "/uploads/x.enc", k: KEY, n: "Tk9OQ0U=" });
  assert.ok(fileText.startsWith(FILE_MARKER));

  // Ben: a plain message, then a file, then words with a marker inside them.
  await handle({ type: "dm_new", id: 301, content: envelope(BEN, ME, "plain words", 1760000001000) });
  await handle({ type: "dm_new", id: 302, content: envelope(BEN, ME, fileText, 1760000002000) });
  await handle({ type: "dm_new", id: 303, content: envelope(BEN, ME, "look " + fileText, 1760000003000) });
  assert.equal(fn("hosDmStore").conversation(BEN).length, 3, "all three are kept in the conversation");
  assert.deepEqual(fn("reportDmCandidates")(BEN).map((m) => m.text), ["plain words"], "the picker never offers a message with a file");

  vm.runInContext("(k) => { activeDmPartner = k; activeDmPartnerName = 'Ben'; }", ctx)(BEN);
  await fn("reportActiveDm")();
  const st = dialog();
  assert.deepEqual(st.picks.map((p) => [p.item.text, p.picked]), [["plain words", true]], "the most recent message without a file is ticked");
  const html = el("report-card").innerHTML;
  assert.ok(html.includes("Messages with files cannot be included in a report."), "the dialog says so");
  assert.ok(!html.includes("photo.jpg") && !html.includes(KEY) && !html.includes("hum:file"), "nothing of the file is shown");
  fn("reportDialogChoose")("unwanted_sexual");
  sock.sent.length = 0;
  sock.raw.length = 0;
  assert.equal(await fn("submitReportDialog")(), true);
  await settle();
  const [f] = reportFrames(sock);
  assert.deepEqual(f.evidence.map((it) => it.text), ["plain words"]);
  assert.ok(!sock.raw.some((s) => s.includes("hum:file") || s.includes(KEY)), "no frame carries the marker or the key");

  // Each layer refuses on its own: the item builders, and the frame builder.
  const inner = { from: BEN, to: ME, ts: 1, text: fileText, sig: "c2ln" };
  assert.equal(report.dmEvidenceItem(inner), null, "dmEvidenceItem gives no item for a file");
  assert.equal(report.dmEvidenceItem({ ...inner, text: "see " + FILE_MARKER.replace("v1", "v2") + "x" }), null, "nor for another version of the marker");
  assert.equal(report.groupEvidenceItem(BEN, 1, fileText), null, "groupEvidenceItem gives no item for a file");
  const forced = await fn("pqBuildReport")({ target: BEN, context: "dm", reason: "spam", evidence: [{ kind: "dm", ...inner }] });
  assert.ok(!forced.frame && forced.error === report.REPORT_NO_FILES, "the frame builder refuses a file item put there by hand");
  const forcedGroup = await fn("pqBuildReport")({ target: BEN, context: "group", reason: "spam", evidence: [{ kind: "group_text", from: BEN, ts: 1, text: fileText }] });
  assert.ok(!forcedGroup.frame && forcedGroup.error === report.REPORT_NO_FILES, "and a group file item");

  // A group message with a file: nothing of it is included, and the dialog says so.
  fn("showUserContextMenu")(ev, "Ann", ANN, { timestamp: 1760000000888, text: fileText, group: true });
  fn("reportUser")();
  await settle();
  assert.equal(dialog().context, "group");
  assert.deepEqual(Array.from(fn("reportDialogEvidence")(dialog())), [], "a group file message is not evidence");
  const g = el("report-card").innerHTML;
  assert.ok(g.includes("Messages with files cannot be included in a report.") && !g.includes(KEY), "and the group dialog says so");
  fn("closeReportDialog")();
});

test("the voice modal's Block, Unblock, Follow, Direct message and moderation actions reach the person", async () => {
  const { ctx, sock, store, state, fn, dialog } = await loadChat({ myRole: "mod" });
  // Click the newest button the voice modal built with this label.
  const click = async (label) => {
    const b = state.created.filter((e) => e.tag === "button" && e.textContent === label).pop();
    assert.ok(b, `the voice modal has ${label}`);
    for (const h of b.listeners.click || []) h({ target: b, preventDefault() {}, stopPropagation() {} });
    await settle();
  };
  const open = () => fn("openVoiceUserModal")("Ben", BEN);

  open();
  await click("Block");
  assert.ok(store.isBlocked(BEN), "Block blocks them");
  open();
  await click("Unblock");
  assert.equal(store.isBlocked(BEN), false, "Unblock unblocks them");
  open();
  await click("Follow");
  assert.ok(vm.runInContext("(k) => myFollowing.has(k)", ctx)(BEN), "Follow follows them");
  open();
  await click("Direct message");
  assert.equal(vm.runInContext("activeDmPartner", ctx), BEN, "Direct message opens the conversation");
  sock.sent.length = 0;
  open();
  await click("Text mute");
  assert.ok(sock.sent.some((m) => m.type === "chat" && m.content === "/mute Ben"), "a moderation action goes out for them");
  open();
  await click("Report");
  assert.deepEqual([dialog().target, dialog().context], [BEN, "profile"], "Report opens the dialog for them");
  fn("closeReportDialog")();
  assert.equal(typeof fn("setCtxMenuTarget"), "function", "chat-ui exposes the setter the modal uses");
});
