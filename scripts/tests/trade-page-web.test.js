// The web Trade page signs in as the person and reaches friends with their pass (2026-10-10).
//
// Run: node --test scripts/tests/trade-page-web.test.js   (in `just rig-tests`)
//
// Before this, web/pages/trade-app.js sent `identify` with an old key (or a made-up
// `viewer_...` one) and never answered the relay's `identify_challenge`, so its socket was never
// signed in: the relay dropped every trade message, closed the socket after 30 seconds, and the
// page reconnected every 3 seconds for ever. And since step B (docs/design/blocking-and-safe-mode.md
// 10c, 10c-ii) a trade request reaches someone only with the friendship pass they gave the sender,
// which the page never sent.
//
// The page's scripts run as they do in the browser, in one shared global scope (node:vm), in
// trade.html's order: the real /chat/pq.js (its `import(` of the vendored noble bundle pointed at
// the same bundle, which node:vm cannot import by itself), /shared/pq-relay-auth.js,
// /shared/reach.js, /chat/chat-dm-store.js (over a stand-in IndexedDB) and /pages/trade-app.js,
// with the DOM, the WebSocket, fetch and the timers replaced by stand-ins the test drives. The
// identity is REAL: derived from a seed Chat's own crypto.js saved in this browser, and the
// signatures are real ML-DSA-65, checked here the way the relay checks them. The friendship pass
// is put in the store by Chat's own code (crypto.js getDmStoreKey and chat-dm-store.js, in a
// second scope sharing the same localStorage and IndexedDB), so the page reading it proves the
// page derives the same store key from the same seed. HOS_WEB_DIR points the test at another copy
// of web/ (used to see each test red).
//
// What it proves:
//  1. trade.html loads what the page needs, in order, and starts with the New trade button hidden.
//  2. The page answers the challenge with a Dilithium3 signature over exactly
//     "hum/identify/v1\n{nonce}\n{public_key}" that verifies under the key it identified with,
//     which is the key derived from Chat's seed. Nothing else is sent before `peer_list`, the
//     New trade button stays hidden until then, and no made-up or old key is ever sent.
//  3. A refusal (a failed check, an erased account) closes the socket, stops the retry loop and
//     says why, with Try again; a plain drop retries with a delay that doubles to a minute.
//  4. With no identity in this browser the page opens no socket and says to open Chat first.
//  5. A trade request to a friend carries the pass Chat keeps (`friend_cert`), whose signature
//     names this server, the friend and the signed-in key; a request to anyone else carries
//     none; a name is looked up to its key; the page never writes Chat's store.
//  6. A refusal (`reach_refused`, kind "trade") shows the chat's sentence, REACH_REFUSED_TRADE.
//  7. An order book sell order is signed the way the relay checks it.
//
// Red first, 2026-10-10: see the end of this file for each deliberate break and what it tripped.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const reach = require(path.join(WEB, "shared", "reach.js"));
const fp = require(path.join(WEB, "shared", "friend-pass.js"));

const hex = (u8) => Buffer.from(u8).toString("hex");
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const HOST = "localhost";
const OLD_KEY = "ed25519-key-from-before-the-cutover";

// ── The vendored noble bundle and the people ────────────────────────────

let NOBLE = null;
async function noble() {
  if (NOBLE) return NOBLE;
  const src = fs.readFileSync(path.join(WEB, "shared", "vendor", "noble-pq.bundle.js"));
  NOBLE = await import("data:text/javascript;base64," + Buffer.from(src).toString("base64"));
  return NOBLE;
}

// A person from a 32-byte seed, derived the way the relay derives the identity
// (pq_crypto.rs: BLAKE3 derive_key "hum/dilithium3/v1", then ML-DSA-65 keygen).
async function person(fill) {
  const n = await noble();
  const seed = new Uint8Array(32).fill(fill);
  const dilSeed = n.blake3.create({ context: new TextEncoder().encode("hum/dilithium3/v1"), dkLen: 32 }).update(seed).digest();
  const kp = n.ml_dsa65.keygen(dilSeed);
  return { seed, pub: kp.publicKey, sk: kp.secretKey, key: hex(kp.publicKey) };
}

// What the relay does with an identify_response (broadcast.rs verify_dilithium_b64): the key from
// hex, the signature from standard base64, ML-DSA-65 over the preimage bytes.
async function relayAccepts(publicKeyHex, preimage, sigB64) {
  const n = await noble();
  try {
    return n.ml_dsa65.verify(Buffer.from(sigB64, "base64"), new TextEncoder().encode(preimage), Buffer.from(publicKeyHex, "hex"));
  } catch {
    return false;
  }
}

// ── Stand-ins ────────────────────────────────────────────────────────────

function fakeStorage() {
  const m = new Map();
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
    clear: () => m.clear(),
  };
}

// Just enough IndexedDB for chat-dm-store.js, shared by Chat's scope and the page's; it counts writes.
function fakeIndexedDB() {
  const stores = { msgs: new Map(), meta: new Map() };
  const writes = [];
  const req = (result) => {
    const r = { result: undefined, onsuccess: null, onerror: null };
    setImmediate(() => { r.result = result; if (r.onsuccess) r.onsuccess(); });
    return r;
  };
  const objectStore = (name) => ({
    get: (k) => req(stores[name].get(k)),
    put: (v) => { writes.push(name); stores[name].set(v.k ?? v.scope, v); return req(undefined); },
    delete: (k) => { writes.push(name); stores[name].delete(k); return req(undefined); },
    index: () => ({ getAll: (scope) => req([...stores[name].values()].filter((v) => v.scope === scope)) }),
  });
  const db = { objectStoreNames: { contains: () => true }, transaction: (name) => ({ objectStore: () => objectStore(name) }) };
  return { open: () => req(db), writes };
}

// Whatever a script reaches for that this test does not care about.
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

// The page's elements, created on first use. Elements trade.html hides with an inline
// `display:none` start hidden, read from the page itself.
function fakeDocument(htmlText) {
  const hidden = new Set();
  for (const m of htmlText.matchAll(/<[a-z]+\b[^>]*\bid="?([\w-]+)"?[^>]*>/gi)) {
    if (/display:\s*none/.test(m[0])) hidden.add(m[1]);
  }
  const make = (id) => {
    const el = {
      id,
      style: { display: hidden.has(id) ? "none" : "" },
      innerHTML: "",
      value: "",
      className: "",
      disabled: false,
      children: [],
      listeners: {},
      _text: "",
      get textContent() { return this._text + this.children.map((c) => c.textContent).join(""); },
      set textContent(v) { this._text = String(v); this.children = []; },
      appendChild(c) { this.children.push(c); return c; },
      addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); },
      focus() {},
      classList: { add() {}, remove() {} },
    };
    return el;
  };
  const els = new Map();
  return {
    els,
    getElementById(id) {
      if (!els.has(id)) els.set(id, make(id));
      return els.get(id);
    },
    createElement: (tag) => make("<" + tag + ">"),
    createTextNode: (t) => ({ textContent: String(t) }),
    querySelectorAll: () => [],
    addEventListener() {},
  };
}

function fakeWebSocketClass(sockets) {
  class FakeWS {
    constructor(url) {
      this.url = url;
      this.readyState = 0;
      this.sent = [];
      this.listeners = {};
      sockets.push(this);
    }
    addEventListener(type, fn) { (this.listeners[type] = this.listeners[type] || []).push(fn); }
    fire(type, ev) { for (const fn of this.listeners[type] || []) fn(ev || {}); }
    send(s) {
      if (this.readyState !== 1) throw new Error("send on a socket that is not open");
      this.sent.push(JSON.parse(s));
    }
    close() {
      if (this.readyState === 3) return;
      this.readyState = 3;
      this.fire("close");
    }
    // Test controls.
    open() { this.readyState = 1; this.fire("open"); }
    receive(obj) { this.fire("message", { data: JSON.stringify(obj) }); }
  }
  FakeWS.CONNECTING = 0;
  FakeWS.OPEN = 1;
  FakeWS.CLOSING = 2;
  FakeWS.CLOSED = 3;
  return FakeWS;
}

async function until(cond, what, ms = 8000) {
  const t0 = Date.now();
  while (!cond()) {
    if (Date.now() - t0 > ms) throw new Error("timed out waiting for " + what);
    await new Promise((r) => setTimeout(r, 2));
  }
}
async function settle() {
  for (let i = 0; i < 30; i++) await new Promise((r) => setImmediate(r));
}

// ── Chat, in this browser: saves the seed and keeps a pass ──────────────

// Chat's own code (crypto.js saveSeedBackup + getDmStoreKey, chat-dm-store.js) writes the seed
// backup and, when given, the passes into the shared localStorage and IndexedDB.
async function chatKeeps({ storage, idb, me, passes }) {
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: anything(),
    navigator: anything(),
    location: { hash: "", host: HOST, protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: storage,
    sessionStorage: fakeStorage(),
    indexedDB: idb,
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: () => 0,
    clearTimeout: () => {},
    setInterval: () => 0,
    clearInterval: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    matchMedia: () => anything(),
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
  run("shared/friend-pass.js");
  run("chat/crypto.js");
  run("chat/chat-dm-store.js");
  vm.runInContext("(s) => { saveSeedBackup(s); mySeed32 = s; }", ctx)(me.seed);
  if (passes) {
    const store = vm.runInContext("hosDmStore", ctx);
    assert.ok(await store.init(me.key, HOST), "Chat's store loads");
    const before = idb.writes.length;
    for (const [peer, pass] of Object.entries(passes)) store.storeCertFrom(peer, pass);
    // Each pass is one encrypted write of the meta box; wait for all of them to land, so none
    // arrives after the page has started (it would read as the page writing).
    await until(() => idb.writes.length >= before + Object.keys(passes).length, "Chat's writes");
  }
}

// The pass `issuer` gives `grantee` on SERVER, really signed: what Chat receives and keeps.
async function realPass(issuer, grantee) {
  const n = await noble();
  const serial = "00112233445566778899aabbccddeeff";
  const may = "invite,message,trade,voice_message";
  const sig = n.ml_dsa65.sign(new TextEncoder().encode(fp.friendPassPreimage(SERVER, issuer.key, grantee.key, serial, may)), issuer.sk);
  return fp.friendPassJson(serial, may, Buffer.from(sig).toString("base64"));
}

// ── The Trade page ──────────────────────────────────────────────────────

async function loadTradePage({ storage, idb, members }) {
  const n = await noble();
  const html = fs.readFileSync(path.join(WEB, "pages", "trade.html"), "utf8");
  const document = fakeDocument(html);
  const sockets = [];
  const timers = new Map();
  let timerId = 0;
  const fetches = [];
  const alerts = [];
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document,
    location: { protocol: "https:", host: HOST, pathname: "/trade" },
    localStorage: storage,
    indexedDB: idb,
    WebSocket: fakeWebSocketClass(sockets),
    setTimeout: (fn, ms) => { timers.set(++timerId, { fn, ms }); return timerId; },
    clearTimeout: (id) => { timers.delete(id); },
    fetch: async (url, opts) => {
      fetches.push({ url: String(url), opts: opts || {} });
      if (String(url).startsWith("/api/members?")) {
        return { ok: true, status: 200, json: async () => ({ members: members || [], total: (members || []).length }) };
      }
      return { ok: true, status: 200, json: async () => ({ id: 1, status: "open" }) };
    },
    alert: (m) => { alerts.push(String(m)); },
    prompt: () => null,
    holdConfirm: async () => true,
    crypto: globalThis.crypto,
    btoa: globalThis.btoa,
    atob: globalThis.atob,
    TextEncoder,
    TextDecoder,
    URLSearchParams,
    URL,
    __testImport: async (spec) => {
      if (spec !== "/shared/vendor/noble-pq.bundle.js") throw new Error("no module " + spec);
      return n;
    },
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel, rewrite) => {
    let src = fs.readFileSync(path.join(WEB, rel), "utf8");
    if (rewrite) src = rewrite(src);
    vm.runInContext(src, ctx, { filename: rel });
  };
  // trade.html's own scripts, in its order (test 1 holds the page to this list).
  run("chat/pq.js", (s) => s.replace(/\bimport\(/g, "__testImport("));
  run("shared/pq-relay-auth.js");
  run("shared/reach.js");
  run("chat/chat-dm-store.js");
  run("pages/trade-app.js");
  const el = (id) => document.getElementById(id);
  const page = {
    ctx,
    sockets,
    timers,
    fetches,
    alerts,
    el,
    get ws() { return sockets[sockets.length - 1]; },
    status: () => el("trade-status").textContent,
    newTradeShown: () => el("trade-new-btn").style.display !== "none",
    call: (name, ...args) => vm.runInContext(name, ctx)(...args),
    // Every timer due now, as if the time passed.
    async runTimers() {
      const due = [...timers.entries()];
      timers.clear();
      for (const [, t] of due) await t.fn();
      await settle();
    },
  };
  return page;
}

// A page signed in: identity derived, socket opened, challenge answered, peer_list received.
async function signedInPage(opts) {
  const page = await loadTradePage(opts);
  await until(() => page.sockets.length === 1, "the socket");
  page.ws.open();
  page.ws.receive({ type: "identify_challenge", nonce: "ab".repeat(32), server_did: SERVER });
  await until(() => page.ws.sent.some((m) => m.type === "identify_response"), "the identify_response");
  page.ws.receive({ type: "peer_list", peers: [] });
  await settle();
  return page;
}

// Every string anywhere in the frames a page sent.
function stringsIn(v, out = []) {
  if (typeof v === "string") out.push(v);
  else if (v && typeof v === "object") for (const x of Object.values(v)) stringsIn(x, out);
  return out;
}

// ── Tests ────────────────────────────────────────────────────────────────

test("trade.html loads what the page needs, in order, with New trade hidden", () => {
  const html = fs.readFileSync(path.join(WEB, "pages", "trade.html"), "utf8");
  const srcs = [...html.matchAll(/<script[^>]*\bsrc="?([^"\s>]+)/g)].map((m) => m[1].replace(/\?.*$/, ""));
  const need = ["/chat/pq.js", "/shared/pq-relay-auth.js", "/shared/reach.js", "/chat/chat-dm-store.js", "/pages/trade-app.js"];
  const at = need.map((s) => srcs.indexOf(s));
  assert.ok(at.every((i) => i >= 0), "trade.html loads " + need.join(", ") + "; it loads " + srcs.join(", "));
  assert.deepEqual([...at].sort((a, b) => a - b), at, "in that order, the page last");
  assert.ok(!srcs.includes("/chat/crypto.js"), "not Chat's crypto.js: the page reads the store with its own copy of the key");
  const btn = html.match(/<button[^>]*id="trade-new-btn"[^>]*>/);
  assert.ok(btn && /display:\s*none/.test(btn[0]), "the New trade button starts hidden");
});

test("the page answers the challenge with a real signature over the exact words, and sends nothing else before peer_list", async () => {
  const me = await person(9);
  const storage = fakeStorage();
  const idb = fakeIndexedDB();
  await chatKeeps({ storage, idb, me });
  storage.setItem("humanity_key", OLD_KEY); // what the page used to send
  const page = await loadTradePage({ storage, idb });
  await until(() => page.sockets.length === 1, "the socket");
  const ws = page.ws;
  assert.equal(ws.url, "wss://" + HOST + "/ws");
  assert.equal(ws.sent.length, 0, "nothing before the socket opens");
  assert.ok(!page.newTradeShown(), "New trade hidden while signing in");

  ws.open();
  assert.equal(ws.sent.length, 1);
  assert.equal(ws.sent[0].type, "identify");
  assert.equal(ws.sent[0].public_key, me.key, "identify carries the Dilithium3 key derived from Chat's seed");
  assert.equal(ws.sent[0].display_name, null, "no made-up name");

  const nonce = "5f".repeat(32);
  ws.receive({ type: "identify_challenge", nonce, server_did: SERVER });
  await until(() => ws.sent.length === 2, "the identify_response");
  const resp = ws.sent[1];
  assert.equal(resp.type, "identify_response");
  assert.ok(await relayAccepts(me.key, "hum/identify/v1\n" + nonce + "\n" + me.key, resp.sig_b64),
    "the relay's check passes: ML-DSA-65 over \"hum/identify/v1\\n{nonce}\\n{public_key}\" under the identified key");
  assert.ok(!(await relayAccepts(me.key, "hum/identify/v1\n" + "00".repeat(32) + "\n" + me.key, resp.sig_b64)),
    "and fails for another nonce, so the check means something");

  // Before peer_list: no button, and every action sends nothing.
  assert.ok(!page.newTradeShown(), "New trade still hidden before peer_list");
  page.call("openTradeRequestModal");
  assert.notEqual(page.el("trade-request-modal").style.display, "flex", "the form does not open");
  page.el("trade-target-input").value = "c3".repeat(40);
  await page.call("sendTradeRequest");
  page.call("respondToTrade", "trade_1", true);
  page.call("confirmTrade", "trade_1");
  await page.call("cancelTrade", "trade_1");
  await page.call("createSellOrder");
  await settle();
  assert.deepEqual(ws.sent.map((m) => m.type), ["identify", "identify_response"], "nothing else is sent before peer_list");
  assert.equal(page.fetches.length, 0, "nor fetched");

  ws.receive({ type: "peer_list", peers: [] });
  await settle();
  assert.ok(page.newTradeShown(), "New trade shows once signed in");
  assert.deepEqual(ws.sent.map((m) => m.type), ["identify", "identify_response", "trade_list_request"], "then the trade list is asked for");

  const all = stringsIn(ws.sent);
  assert.ok(!all.some((s) => s.startsWith("viewer_")), "no made-up viewer_ key");
  assert.ok(!all.includes(OLD_KEY), "never the old key");
});

test("a refusal closes the socket, stops the retry loop and says why; a plain drop backs off to a minute", async () => {
  const me = await person(9);
  const storage = fakeStorage();
  const idb = fakeIndexedDB();
  await chatKeeps({ storage, idb, me });

  // The relay's check fails: the page stops.
  const page = await loadTradePage({ storage, idb });
  await until(() => page.sockets.length === 1, "the socket");
  page.ws.open();
  page.ws.receive({ type: "identify_challenge", nonce: "ab".repeat(32), server_did: SERVER });
  await until(() => page.ws.sent.length === 2, "the identify_response");
  page.ws.receive({ type: "system", message: "Identify challenge verification failed. Reconnect and re-identify." });
  await settle();
  assert.equal(page.ws.readyState, 3, "the page closes the refused socket");
  assert.match(page.status(), /did not accept this page's sign-in: Identify challenge verification failed/, "and says why");
  assert.match(page.status(), /Try again/, "with Try again");
  assert.ok(!page.newTradeShown());
  for (let i = 0; i < 4; i++) await page.runTimers();
  assert.equal(page.sockets.length, 1, "no retry after a refusal");

  // Try again starts over.
  const retry = page.el("trade-status").children.find((c) => typeof c.onclick === "function");
  assert.ok(retry, "the Try again button");
  retry.onclick();
  await until(() => page.sockets.length === 2, "the socket Try again opens");

  // An erased account is a refusal too.
  page.ws.open();
  page.ws.receive({ type: "account_erased" });
  await settle();
  assert.match(page.status(), /your account on this server was erased/);
  for (let i = 0; i < 4; i++) await page.runTimers();
  assert.equal(page.sockets.length, 2, "no retry after an erased account either");

  // A plain drop retries, each wait twice the last, up to a minute.
  const page2 = await loadTradePage({ storage, idb });
  await until(() => page2.sockets.length === 1, "the socket");
  const waits = [];
  for (let i = 0; i < 6; i++) {
    page2.ws.close();
    await settle();
    assert.match(page2.status(), /Lost the connection to the server/);
    const pending = [...page2.timers.values()];
    assert.equal(pending.length, 1, "one retry waiting");
    waits.push(pending[0].ms);
    await page2.runTimers();
    await until(() => page2.sockets.length === i + 2, "the retry's socket");
  }
  assert.deepEqual(waits, [5000, 10000, 20000, 40000, 60000, 60000]);
});

test("with no identity in this browser, no socket opens, no key is made up, and the page says to open Chat", async () => {
  const storage = fakeStorage();
  storage.setItem("humanity_key", OLD_KEY);
  const page = await loadTradePage({ storage, idb: fakeIndexedDB() });
  await until(() => /Open Chat in this browser first/.test(page.status()), "the help line");
  for (let i = 0; i < 4; i++) await page.runTimers();
  assert.equal(page.sockets.length, 0, "no socket, so nothing sent under any key");
  assert.ok(!page.newTradeShown());
  assert.match(page.status(), /Try again/);
});

test("a trade request to a friend carries the pass Chat keeps, to anyone else none, and a refusal says the chat's sentence", async () => {
  const me = await person(9);
  const ann = await person(1);
  const ben = await person(2);
  const storage = fakeStorage();
  const idb = fakeIndexedDB();
  const annPass = await realPass(ann, me);
  await chatKeeps({ storage, idb, me, passes: { [ann.key]: annPass } });
  const writesBefore = idb.writes.length;
  assert.ok(writesBefore > 0, "Chat wrote its store");

  const members = [
    { name: "Ann", public_key: ann.key, role: "" },
    { name: "Annie", public_key: "e5".repeat(40), role: "" },
    { name: "Ben", public_key: ben.key, role: "" },
  ];
  const page = await signedInPage({ storage, idb, members });
  const ws = page.ws;
  const send = async (target, note) => {
    page.call("openTradeRequestModal");
    assert.equal(page.el("trade-request-modal").style.display, "flex", "the form opens once signed in");
    page.el("trade-target-input").value = target;
    page.el("trade-message-input").value = note || "";
    const before = ws.sent.length;
    await page.call("sendTradeRequest");
    await settle();
    return ws.sent.slice(before);
  };

  // To Ann, by key (as pasted, in capitals): her pass rides along.
  let out = await send(ann.key.toUpperCase(), "20 wheat seeds for a hand axe?");
  assert.equal(out.length, 1);
  assert.deepEqual(out[0], { type: "trade_request", target_key: ann.key, message: "20 wheat seeds for a hand axe?", friend_cert: annPass },
    "the request carries the pass Ann gave me, as Chat keeps it");
  const pass = fp.friendPassParse(out[0].friend_cert);
  const n = await noble();
  assert.ok(n.ml_dsa65.verify(Buffer.from(pass.sig, "base64"),
    new TextEncoder().encode(fp.friendPassPreimage(SERVER, ann.key, me.key, pass.serial, pass.may)), ann.pub),
  "and the pass checks out the way the relay checks it: this server, Ann as issuer, the signed-in key as grantee");
  assert.equal(page.el("trade-request-modal").style.display, "none", "the form closes");

  // To Ann, by name: looked up to the same key, pass and all.
  out = await send("ann");
  assert.equal(out.length, 1);
  assert.equal(out[0].target_key, ann.key, "the name is looked up to her key (one exact match, not Annie)");
  assert.equal(out[0].friend_cert, annPass);

  // To Ben: no pass held, none sent.
  out = await send(ben.key);
  assert.equal(out.length, 1);
  assert.equal(out[0].target_key, ben.key);
  assert.ok(!("friend_cert" in out[0]), "no pass for someone who gave none");

  // A name nobody has: nothing sent, the form says so.
  out = await send("Nobody_here");
  assert.equal(out.length, 0, "a name with no match sends nothing");
  assert.match(page.el("trade-request-msg").textContent, /No one named "Nobody_here" is listed/);

  // Ben's Trades audience refuses: the chat's own sentence.
  ws.receive({ type: "reach_refused", kind: "trade", to: ben.key });
  await settle();
  assert.equal(page.status(), reach.REACH_REFUSED_TRADE, "the refusal is said in the chat's words");

  // The page never wrote Chat's store, and could not: the store is open read-only, so even a
  // write asked for through it goes nowhere.
  assert.equal(idb.writes.length, writesBefore, "the page only reads Chat's store");
  const store = vm.runInContext("hosDmStore", page.ctx);
  assert.equal(store.readOnly, true, "the page opened the store read-only");
  store.storeCertFrom(ben.key, annPass);
  await store.insert({ from: ben.key, to: me.key, ts: 1, text: "x", sig: "c2ln" });
  await settle();
  await new Promise((r) => setTimeout(r, 100)); // time for an encrypted write, were one under way
  assert.equal(idb.writes.length, writesBefore, "a write through the page's read-only store is refused");
});

test("an order book sell order is signed the way the relay checks it", async () => {
  const me = await person(9);
  const storage = fakeStorage();
  const idb = fakeIndexedDB();
  await chatKeeps({ storage, idb, me });
  const page = await signedInPage({ storage, idb });
  page.el("ob-create-item").value = "wood";
  page.el("ob-create-qty").value = "5";
  page.el("ob-create-price").value = "2.5";
  page.el("ob-create-currency").value = "credits";
  await page.call("createSellOrder");
  await settle();
  const post = page.fetches.find((f) => f.url === "/api/trade/orders");
  assert.ok(post, "the order is posted");
  const body = JSON.parse(post.opts.body);
  assert.equal(body.public_key, me.key);
  const n = await noble();
  // api_market.rs: format!("trade_order\n{}\n{}\n{}", item_type, quantity, price_per_unit), then
  // verify_dilithium_signature adds "\n{timestamp}"; the signature is hex.
  const words = "trade_order\nwood\n5\n2.5\n" + body.timestamp;
  assert.ok(n.ml_dsa65.verify(Buffer.from(body.signature, "hex"), new TextEncoder().encode(words), me.pub),
    "a Dilithium3 signature over the relay's words");
});

// Red first, 2026-10-10. Each break made in a fresh copy of web/ (HOS_WEB_DIR), each seen to fail
// in the test named, at the assertion quoted, then the real tree passing all six:
//  - tradeIdentifyPreimage signing "hum/identify/v1\n" + key + "\n" + nonce (nonce and key
//    swapped): "the page answers the challenge..." failed at "the relay's check passes".
//  - tradeSignedIn() without `tradeWsBound` (signed in as soon as the socket opens): the same test
//    failed at "the form does not open" (before peer_list).
//  - the close handler without `if (tradeWsRefusal) return;`: "a refusal closes the socket..."
//    failed at "and says why" (the retry's "Lost the connection" line replaced the reason).
//  - ensureTradeWs falling back to a made-up 'viewer_' key when there is no identity: "with no
//    identity in this browser..." failed at "the help line" (a socket opened instead).
//  - sendTradeRequest without `if (pass) frame.friend_cert = pass;`: "a trade request to a
//    friend..." failed at "the request carries the pass Ann gave me, as Chat keeps it".
//  - pq-relay-auth.js getPqDmStoreKey with the info "hum/dm-store-web/v2" (a derivation drifted
//    from Chat's crypto.js): the same test, at the same assertion (the store would not open).
//  - handleTradeMessage without its reach_refused branch: the same test, at "the refusal is said
//    in the chat's words".
//  - trade-app.js opening the store without `{ readOnly: true }`: the same test, at "the page
//    opened the store read-only".
//  - chat-dm-store.js _persistMeta without its readOnly guard: the same test failed, on the
//    page's key being decrypt-only ("The requested operation is not valid for the provided key"),
//    the second guard. With both taken away (the key also allowed to encrypt), at "a write
//    through the page's read-only store is refused".
