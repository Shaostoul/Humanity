// Tasks of my own on the website (docs/design/tasks.md rung 1, 2026-11-02; fixes BUG-217).
//
// Run: node --test scripts/tests/tasks-mine-web.test.js   (in `just rig-tests`)
//
// Until this date the website's Tasks page was only the server's board: "Add to my tasks" opened
// the server's New Task form, and nothing of a person's own was kept anywhere. Now My tasks are
// kept in this browser, encrypted (/shared/mine-tasks.js), carried to the person's other devices
// as sealed notes to themselves in their own mailbox, and never readable by a server; the board
// is shown beside them and never merged into them.
//
// The page's scripts run as they do in the browser, in one shared global scope (node:vm), in
// tasks.html's order: the words (i18n-en.js, i18n.js), the real /chat/pq.js (its `import(` of the
// vendored noble bundle pointed at the same bundle), /shared/pq-relay-auth.js,
// /shared/mine-tasks.js and /pages/tasks-app.js, with the DOM, the WebSocket, fetch, IndexedDB
// and the timers replaced by stand-ins the test drives. The identity is REAL, from a seed Chat's
// own crypto.js saved in this browser, and the notes are really signed and sealed. HOS_WEB_DIR
// points the test at another copy of web/ (used to see each test red).
//
// What it proves:
//  1. The note is the desktop app's, byte for byte (scripts/tests/fixtures/task-note-v1.json,
//     which src/gui/mine_tasks_tests.rs holds the desktop to), and the markers, the padding
//     buckets and the signature domain are Chat's and the desktop's.
//  2. The merge: the later change wins in any order, a deletion stays, a repeat changes nothing.
//  3. BUG-217: Add to my tasks makes one of my own in one click, and it survives the server's
//     board arriving and a restart of the page; the board's tasks stay in their own list.
//  4. No word of a task of mine reaches a server: the page sends only sealed `dm_put`s to my own
//     mailbox, and no frame, fetch or stored byte holds a word of it.
//  5. My other device (the same recovery phrase) takes the note from my mailbox and shows the
//     task; someone else cannot open it, a task note they write to me is dropped, and they cannot
//     sign one as me. The chat never shows a task note as a message.
//  6. A record that is there but does not open is copied aside first and never written over; one
//     that is not there is an ordinary empty start; a read the browser refused keeps the page from
//     saving over it while my changes still go out as notes; the page says which, with a hint.
//
// Red first, 2026-11-02: see the end of this file for each deliberate break and what it tripped.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const FIXTURE = JSON.parse(fs.readFileSync(path.join(__dirname, "fixtures", "task-note-v1.json"), "utf8"));
const HOST = "localhost";
const hex = (u8) => Buffer.from(u8).toString("hex");

// ── The vendored noble bundle and the people ────────────────────────────

let NOBLE = null;
async function noble() {
  if (NOBLE) return NOBLE;
  const src = fs.readFileSync(path.join(WEB, "shared", "vendor", "noble-pq.bundle.js"));
  NOBLE = await import("data:text/javascript;base64," + Buffer.from(src).toString("base64"));
  return NOBLE;
}

// A person from a 32-byte seed: the Dilithium3 identity as the relay derives it, and the Kyber768
// pair every device derives from the same seed.
async function person(fill) {
  const n = await noble();
  const seed = new Uint8Array(32).fill(fill);
  const dilSeed = n.blake3.create({ context: new TextEncoder().encode("hum/dilithium3/v1"), dkLen: 32 }).update(seed).digest();
  const kp = n.ml_dsa65.keygen(dilSeed);
  const kseed = n.blake3.create({ context: new TextEncoder().encode("hum/kyber768/v1"), dkLen: 64 }).update(seed).digest();
  const kem = n.ml_kem768.keygen(kseed);
  return { seed, sk: kp.secretKey, key: hex(kp.publicKey), kyberPub: Buffer.from(kem.publicKey).toString("base64"), kyberSecret: kem.secretKey };
}

// ── Stand-ins ────────────────────────────────────────────────────────────

function fakeStorage() {
  const m = new Map();
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
    clear: () => m.clear(),
    _map: m,
  };
}

// Just enough IndexedDB for chat-dm-store.js and mine-tasks.js: one database, its two stores.
// `refuse(key)`: a read of that key fails (onerror), as a browser's storage can refuse one.
function fakeIndexedDB({ refuse } = {}) {
  const stores = { msgs: new Map(), meta: new Map() };
  const writes = [];
  const req = (result) => {
    const r = { result: undefined, onsuccess: null, onerror: null };
    setImmediate(() => { r.result = result; if (r.onsuccess) r.onsuccess(); });
    return r;
  };
  const refused = () => {
    const r = { result: undefined, error: null, onsuccess: null, onerror: null };
    setImmediate(() => { r.error = new Error("the storage refused the read"); if (r.onerror) r.onerror(); });
    return r;
  };
  const objectStore = (name) => ({
    get: (k) => (refuse && refuse(k) ? refused() : req(stores[name].get(k))),
    put: (v) => { writes.push(v.k ?? v.scope); stores[name].set(v.k ?? v.scope, JSON.parse(JSON.stringify(v))); return req(undefined); },
    delete: (k) => { stores[name].delete(k); return req(undefined); },
    index: () => ({ getAll: (scope) => req([...stores[name].values()].filter((v) => v.scope === scope)) }),
  });
  const db = { objectStoreNames: { contains: () => true }, transaction: (name) => ({ objectStore: () => objectStore(name) }) };
  return { open: () => req(db), stores, writes };
}

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

function classList() {
  const set = new Set();
  return {
    add: (c) => set.add(c),
    remove: (c) => set.delete(c),
    contains: (c) => set.has(c),
    toggle: (c, on) => { const want = on === undefined ? !set.has(c) : !!on; if (want) set.add(c); else set.delete(c); return want; },
  };
}

// The page's elements, made on first use.
function fakeDocument() {
  const make = (id) => {
    const el = {
      id,
      style: {},
      value: "",
      className: "",
      disabled: false,
      children: [],
      attrs: {},
      dataset: {},
      _html: "",
      _text: "",
      classList: classList(),
      get innerHTML() { return this._html + this.children.map((c) => c.innerHTML).join(""); },
      set innerHTML(v) { this._html = String(v); this.children = []; },
      get textContent() { return this._text; },
      set textContent(v) { this._text = String(v); },
      get firstChild() { return this.children[0] || null; },
      setAttribute(k, v) { this.attrs[k] = String(v); },
      getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; },
      addEventListener() {},
      appendChild(c) { this.children.push(c); c.parentNode = this; return c; },
      insertBefore(c) { this.children = [c, ...this.children.filter((x) => x !== c)]; c.parentNode = this; return c; },
      remove() { if (this.parentNode) this.parentNode.children = this.parentNode.children.filter((x) => x !== this); },
      querySelectorAll: () => [],
      querySelector: () => null,
      closest: () => null,
      focus() {},
    };
    return el;
  };
  const els = new Map();
  return {
    els,
    body: { classList: classList() },
    getElementById(id) {
      if (!els.has(id)) els.set(id, make(id));
      return els.get(id);
    },
    createElement: (tag) => make("<" + tag + ">"),
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
    close() { if (this.readyState !== 3) { this.readyState = 3; this.fire("close"); } }
    open() { this.readyState = 1; this.fire("open"); }
    receive(obj) { this.fire("message", { data: JSON.stringify(obj) }); }
  }
  Object.assign(FakeWS, { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 });
  return FakeWS;
}

async function until(cond, what, ms = 15000) {
  const t0 = Date.now();
  while (!cond()) {
    if (Date.now() - t0 > ms) throw new Error("timed out waiting for " + what);
    await new Promise((r) => setTimeout(r, 2));
  }
}
async function settle() {
  for (let i = 0; i < 40; i++) await new Promise((r) => setImmediate(r));
}

// ── Chat, in this browser: saves the seed ───────────────────────────────

function chatContext(storage, idb) {
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
  return ctx;
}

// Chat's own crypto.js writes the seed backup the Tasks page reads (pq-relay-auth.js).
function chatSaves(storage, idb, me) {
  const ctx = chatContext(storage, idb);
  for (const rel of ["shared/friend-pass.js", "chat/crypto.js"]) {
    vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  }
  vm.runInContext("(s) => { saveSeedBackup(s); mySeed32 = s; }", ctx)(me.seed);
  return ctx;
}

// ── The Tasks page ──────────────────────────────────────────────────────

const SERVER_TASKS = [{ id: 7, title: "Translate the Library", status: "backlog", priority: "medium", labels: "[\"scope:city\"]", created_at: Date.now() }];

async function loadTasksPage({ storage, idb }) {
  const n = await noble();
  const document = fakeDocument();
  const sockets = [];
  const timers = [];
  const fetches = [];
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document,
    navigator: anything(),
    location: { protocol: "https:", host: HOST, pathname: "/tasks", search: "" },
    localStorage: storage,
    indexedDB: idb,
    WebSocket: fakeWebSocketClass(sockets),
    setTimeout: (fn, ms) => { timers.push({ fn, ms }); return timers.length; },
    clearTimeout: () => {},
    fetch: async (url, opts) => {
      const u = String(url);
      fetches.push({ url: u, opts: opts || {} });
      const file = (rel) => JSON.parse(fs.readFileSync(path.join(ROOT, rel), "utf8"));
      const answer = (v) => ({ ok: true, status: 200, json: async () => v });
      if (u === "/api/tasks") return answer({ tasks: SERVER_TASKS });
      if (u === "/data/onboarding/quests.json") return answer(file("data/onboarding/quests.json"));
      if (u === "/data/tasks/scopes.json") return answer(file("data/tasks/scopes.json"));
      if (u === "/data/skills/xp_curve.json") return answer(file("data/skills/xp_curve.json"));
      if (u.startsWith("/api/projects")) return answer([]);
      return { ok: false, status: 404, json: async () => ({}) };
    },
    alert: () => {},
    confirm: () => true,
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
  run("shared/i18n-en.js");
  run("shared/i18n.js");
  run("chat/pq.js", (s) => s.replace(/\bimport\(/g, "__testImport("));
  run("shared/pq-relay-auth.js");
  if (fs.existsSync(path.join(WEB, "shared", "mine-tasks.js"))) run("shared/mine-tasks.js");
  run("pages/tasks-app.js");
  const page = {
    ctx,
    document,
    sockets,
    timers,
    fetches,
    el: (id) => document.getElementById(id),
    get ws() { return sockets[sockets.length - 1]; },
    js: (src) => vm.runInContext(src, ctx),
    // Every timer due now, as if the time passed (the page's socket opens on one).
    async runTimers() {
      const due = timers.splice(0);
      for (const t of due) await t.fn();
      await settle();
    },
    // My own tasks as the page holds them.
    mine() {
      return vm.runInContext("mine ? mine.store.list.tasks() : []", ctx);
    },
  };
  await until(() => vm.runInContext("typeof mineState === 'undefined' || mineState !== 'opening'", ctx), "my tasks to open");
  await settle();
  return page;
}

// A page whose socket is signed in (challenge answered, peer_list in) and whose mailbox fetch was
// answered with `mail` (envelopes, oldest first).
async function signedIn(page, mail = []) {
  await page.runTimers();
  await until(() => page.sockets.length === 1, "the socket");
  page.ws.open();
  page.ws.receive({ type: "identify_challenge", nonce: "ab".repeat(32) });
  await until(() => page.ws.sent.some((m) => m.type === "identify_response"), "the identify_response");
  page.ws.receive({ type: "peer_list", peers: [] });
  await until(() => page.ws.sent.some((m) => m.type === "dm_fetch"), "the mailbox fetch");
  const fetch = page.ws.sent.find((m) => m.type === "dm_fetch");
  page.ws.receive({ type: "dm_batch", ref: fetch.ref, done: true, messages: mail.map((content, i) => ({ id: i + 1, content })) });
  await settle();
  return fetch;
}

const addFromCard = (page, chainId) =>
  page.js("onSuggestionClick")({ target: { closest: (sel) => (sel === "[data-add]" ? { dataset: { add: chainId } } : null) } });

// Every string anywhere in a value.
function stringsIn(v, out = []) {
  if (typeof v === "string") out.push(v);
  else if (v && typeof v === "object") for (const x of Object.values(v)) stringsIn(x, out);
  return out;
}

// ── 1 and 2: the note and the merge ─────────────────────────────────────

test("the note is the desktop app's, byte for byte, with Chat's and the desktop's markers and buckets", () => {
  const M = require(path.join(WEB, "shared", "mine-tasks.js"));
  assert.equal(M.CTL_TASK, FIXTURE.marker);
  const records = FIXTURE.records.map(M.clampRecord);
  assert.deepEqual(records, FIXTURE.records, "the fixture's records are already as kept");
  const texts = M.noteTexts(records);
  assert.equal(texts.length, 1);
  assert.equal(texts[0], FIXTURE.text, "the note text, byte for byte");
  assert.deepEqual(M.parseNote(FIXTURE.text), FIXTURE.records, "and it reads back");
  for (const c of FIXTURE.read) assert.deepEqual(M.parseNote(c.text), c.records, c.name);

  const native = fs.readFileSync(path.join(ROOT, "src", "net", "dm_pq.rs"), "utf8");
  assert.ok(native.includes(`pub const CTL_TASK: &str = "${M.CTL_TASK}";`), "the desktop app's marker is the same");
  const buckets = native.match(/DM_PAD_BUCKETS:\s*\[usize;\s*\d+\]\s*=\s*\[([^\]]+)\]/);
  assert.deepEqual(buckets[1].split(",").map((s) => Number(s.trim().replace(/_/g, ""))), M.DM_PAD_BUCKETS, "the desktop app's padding buckets");
  const crypto = fs.readFileSync(path.join(WEB, "chat", "crypto.js"), "utf8");
  assert.ok(crypto.includes(`const CTL_TASK = '${M.CTL_TASK}';`), "Chat's marker, which it swallows by");
  assert.ok(crypto.includes(`const DM_SIG_DOMAIN_V2 = '${M.DM_SIG_DOMAIN_V2}';`), "Chat's signature domain");
  assert.ok(crypto.includes(`const DM_PAD_BUCKETS = [${M.DM_PAD_BUCKETS.join(", ")}];`), "Chat's padding buckets");
  assert.ok(fs.readFileSync(path.join(WEB, "chat", "chat-social.js"), "utf8").includes("inner.text.startsWith(CTL_TASK)) return true;"), "the chat swallows a task note");
});

test("the merge: the later change wins in any order, a deletion stays, a repeat changes nothing", () => {
  const M = require(path.join(WEB, "shared", "mine-tasks.js"));
  const id = "a".repeat(32);
  const v1 = { id, title: "Buy seeds", status: "todo", priority: "medium", updated_at: 10 };
  const v2 = { ...v1, status: "in_progress", updated_at: 20 };
  const gone = M.gone(id, 30);
  for (const order of [[v1, v2, gone], [gone, v2, v1], [v2, gone, v1]]) {
    const list = new M.MineTasks();
    for (const r of order) list.apply([r]);
    assert.deepEqual(list.tasks(), [], "the deletion is the latest, whatever the order");
  }
  const list = new M.MineTasks();
  assert.ok(list.apply([v2]));
  assert.ok(!list.apply([v2]), "a repeat changes nothing");
  assert.ok(!list.apply([v1]), "an older version changes nothing");
  assert.ok(list.apply([M.gone(id, 20)]), "at the same time a deletion wins");
  const later = new M.MineTasks();
  later.apply([{ ...v1, updated_at: 100 }]);
  assert.ok(later.change(id, { status: "done" }, 5), "a slow clock");
  assert.equal(later.record(id).updated_at, 101, "a change here is stamped just after what it changed");
});

// ── 3: BUG-217 ──────────────────────────────────────────────────────────

test("BUG-217: Add to my tasks makes one of my own, which survives the server's board and a restart", async () => {
  const me = await person(41);
  const storage = fakeStorage();
  const idb = fakeIndexedDB();
  chatSaves(storage, idb, me);
  const page = await loadTasksPage({ storage, idb });
  await until(() => page.js("suggestionChains.length > 0"), "the suggestions");
  addFromCard(page, "water");
  await settle();
  assert.equal(page.el("modal-overlay").classList.contains("open"), false, "no server form opens");
  const made = page.mine();
  assert.equal(made.length, 1, "Add to my tasks made one of my own");
  assert.equal(made[0].chain, "water");
  addFromCard(page, "water");
  assert.equal(page.mine().length, 1, "never twice");

  // The server's board arrives (what every load and reconnect brings).
  await page.js("loadTasks")();
  await settle();
  assert.equal(page.mine().length, 1, "the server's board left my own tasks alone");
  assert.deepEqual(page.js("allTasks").map((t) => t.id), [7], "and is its own list");
  assert.ok(page.el("col-backlog").innerHTML.includes("Water Independence"), "My tasks shows it");
  assert.ok(!page.el("col-backlog").innerHTML.includes("Translate the Library"), "and not the board's");
  page.js("setTaskKind")("board");
  assert.ok(page.el("col-backlog").innerHTML.includes("Translate the Library"), "the board shows the board's");
  assert.ok(!page.el("col-backlog").innerHTML.includes("Water Independence"), "and not mine");
  assert.match(page.el("kind-who").textContent, /Everyone on this server/, "and says who can see it");
  page.js("setTaskKind")("mine");
  await page.js("mineQueue");

  // A restart: a new page in the same browser.
  const again = await loadTasksPage({ storage, idb });
  assert.deepEqual(again.mine().map((t) => t.title), ["Water Independence"], "kept across a restart");
  await until(() => again.js("suggestionChains.length > 0"), "the suggestions");
  assert.ok(!again.js("visibleSuggestions()").some((c) => c.id === "water"), "so its suggestion stays away");
});

// ── 4: no word reaches a server ─────────────────────────────────────────

test("no word of a task of mine reaches a server: only sealed notes to my own mailbox", async () => {
  const me = await person(42);
  const storage = fakeStorage();
  const idb = fakeIndexedDB();
  chatSaves(storage, idb, me);
  const page = await loadTasksPage({ storage, idb });
  await signedIn(page);
  const before = page.ws.sent.length;
  page.js("mineAdd")({ title: "Pick up insulin for Mum", description: "From the chemist on Elm Street", priority: "critical", labels: ["health"] });
  await page.js("mineQueue");
  const id = page.mine()[0].id;
  page.js("mineChange")(id, { status: "in_progress" });
  await page.js("mineQueue");
  const out = page.ws.sent.slice(before);
  assert.equal(out.length, 2, "one note for each change");
  // Words of six letters or more: a shorter one turns up in base64 by chance.
  const words = ["insulin", "chemist", "Street", "health", "in_progress", "critical"];
  for (const f of out) {
    const text = JSON.stringify(f);
    for (const w of words) assert.ok(!text.includes(w), "a frame to the server carries " + w);
    assert.equal(f.type, "dm_put", "only sealed notes go out");
    assert.equal(f.to, me.key, "to my own mailbox only");
    assert.equal(JSON.parse(f.content).v, 2, "a sealed v2 envelope");
  }
  for (const f of page.fetches) for (const w of words) assert.ok(!stringsIn(f).join(" ").includes(w), "a fetch carries " + w);
  const stored = JSON.stringify([...idb.stores.meta.values()]);
  for (const w of words) assert.ok(!stored.includes(w), "the browser's store holds " + w + " in the clear");
  for (const v of storage._map.values()) for (const w of words) assert.ok(!v.includes(w), "localStorage holds " + w);
  // Opened with my own key, it is my note.
  const M = page.ctx.HosMineTasks;
  const recs = await M.takeEnvelope(out[1].content, { me: me.key, kyberSecret: me.kyberSecret });
  assert.deepEqual(recs.map((r) => [r.title, r.status]), [["Pick up insulin for Mum", "in_progress"]]);
});

// ── 5: my other device, and nobody else's ───────────────────────────────

test("a task made on one device arrives on my other device and never on anyone else's", async () => {
  const me = await person(43);
  const eve = await person(44);
  // My laptop: makes a task.
  const laptopStore = fakeStorage();
  const laptopDb = fakeIndexedDB();
  chatSaves(laptopStore, laptopDb, me);
  const laptop = await loadTasksPage({ storage: laptopStore, idb: laptopDb });
  await signedIn(laptop);
  const before = laptop.ws.sent.length;
  laptop.js("mineAdd")({ title: "Renew the passport", priority: "high" });
  await laptop.js("mineQueue");
  const note = laptop.ws.sent.slice(before).find((m) => m.type === "dm_put").content;

  // My phone (the same recovery phrase, its own browser): the note is in my mailbox.
  const phoneStore = fakeStorage();
  const phoneDb = fakeIndexedDB();
  chatSaves(phoneStore, phoneDb, me);
  const phone = await loadTasksPage({ storage: phoneStore, idb: phoneDb });
  await signedIn(phone, [note]);
  assert.deepEqual(phone.mine().map((t) => t.title), ["Renew the passport"], "my phone has it");
  // And a deletion made on the laptop arrives live.
  const id = laptop.mine()[0].id;
  const mark = laptop.ws.sent.length;
  laptop.js("mineDelete")(id);
  await laptop.js("mineQueue");
  const del = laptop.ws.sent.slice(mark).find((m) => m.type === "dm_put").content;
  phone.ws.receive({ type: "dm_new", id: 2, content: del });
  await phone.js("mineQueue");
  assert.deepEqual(phone.mine(), [], "the deletion reached my phone too");

  // Eve's page, given my note: it cannot open it.
  const eveStore = fakeStorage();
  const eveDb = fakeIndexedDB();
  chatSaves(eveStore, eveDb, eve);
  const evePage = await loadTasksPage({ storage: eveStore, idb: eveDb });
  await signedIn(evePage, [note]);
  assert.deepEqual(evePage.mine(), [], "someone else's device shows nothing of it");

  // A task note Eve seals to me is dropped; one claiming to be from me fails its signature.
  const M = laptop.ctx.HosMineTasks;
  const text = M.noteTexts([{ id: "b".repeat(32), title: "Planted by Eve", status: "todo", priority: "medium", updated_at: 9 }])[0];
  const n = await noble();
  const forge = async (from) => {
    const preimage = `hum/dm/v2\n${from}\n${me.key}\n9\n${text}`;
    const sig = Buffer.from(n.ml_dsa65.sign(new TextEncoder().encode(preimage), eve.sk)).toString("base64");
    const sealed = await laptop.ctx.pqDmSeal(me.kyberPub, JSON.stringify({ v: 2, from, to: me.key, ts: 9, text, sig }));
    return JSON.stringify({ v: 2, ...sealed });
  };
  const mine = { me: me.key, kyberSecret: me.kyberSecret };
  assert.deepEqual(await M.takeEnvelope(await forge(eve.key), mine), [], "a task note from someone else is dropped");
  assert.deepEqual(await M.takeEnvelope(await forge(me.key), mine), [], "and one signed by someone else as me fails its check");
});

// ── 6: a record that does not open is never written over ────────────────

// The scope my tasks' record is kept under (mine-tasks.js openStore).
const mineScope = (key) => require("node:crypto").createHash("sha256").update(`${key}\ntasks-mine`).digest("hex");
const keptCopies = (idb, scope) => [...idb.stores.meta.keys()].filter((k) => k.startsWith(scope + ":unreadable-"));
const WORDS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "i18n", "en.json"), "utf8")).tasks;

test("a damaged record is kept aside, never written over by the next save, and the page says so", async () => {
  const me = await person(45);
  const storage = fakeStorage();
  const idb = fakeIndexedDB();
  chatSaves(storage, idb, me);
  const scope = mineScope(me.key);
  const damaged = { scope, box: { iv: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12], ct: [9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9] } };
  idb.stores.meta.set(scope, JSON.parse(JSON.stringify(damaged)));
  const page = await loadTasksPage({ storage, idb });
  assert.deepEqual(page.mine(), [], "the list starts empty from there");
  const copies = keptCopies(idb, scope);
  assert.equal(copies.length, 1, "a copy was kept aside");
  assert.deepEqual(idb.stores.meta.get(copies[0]).box, damaged.box, "the copy is the damaged record, as it was");
  assert.equal(page.el("mine-notice").textContent, WORDS.mine_kept_aside_web, "the page says it was kept aside");
  assert.notEqual(page.el("mine-notice").style.display, "none");
  assert.equal(page.el("mine-notice").getAttribute("data-i18n-tip"), "tasks.mine_kept_aside_web_hint", "with its hint");
  page.js("mineAdd")({ title: "Mend the fence" });
  await page.js("mineQueue");
  assert.deepEqual(idb.stores.meta.get(copies[0]).box, damaged.box, "the next save did not write over the copy");
  const again = await loadTasksPage({ storage, idb });
  assert.deepEqual(again.mine().map((t) => t.title), ["Mend the fence"], "and the new record opens");
  assert.equal(keptCopies(idb, scope).length, 1, "with no second copy");
  assert.equal(again.el("mine-notice").style.display, "none", "and nothing to say");
});

test("a record that is not there is an ordinary empty start", async () => {
  const me = await person(46);
  const storage = fakeStorage();
  const idb = fakeIndexedDB();
  chatSaves(storage, idb, me);
  const page = await loadTasksPage({ storage, idb });
  assert.deepEqual(page.mine(), [], "an empty start");
  assert.equal(page.el("mine-notice").style.display, "none", "with nothing to say");
  assert.deepEqual(keptCopies(idb, mineScope(me.key)), [], "and nothing kept aside");
  page.js("mineAdd")({ title: "Sweep the porch" });
  await page.js("mineQueue");
  assert.ok(idb.stores.meta.has(mineScope(me.key)), "the first save makes the record");
});

test("a read the browser refused keeps the page from saving over the record, and my changes still go out", async () => {
  const me = await person(47);
  const storage = fakeStorage();
  const scope = mineScope(me.key);
  const idb = fakeIndexedDB({ refuse: (k) => k === scope });
  chatSaves(storage, idb, me);
  const page = await loadTasksPage({ storage, idb });
  assert.equal(page.el("mine-notice").textContent, WORDS.mine_unreadable_web, "the page says it could not be read");
  assert.equal(page.el("mine-notice").getAttribute("data-i18n-tip"), "tasks.mine_unreadable_web_hint", "with its hint");
  await signedIn(page);
  const before = page.ws.sent.length;
  page.js("mineAdd")({ title: "Oil the hinges" });
  await page.js("mineQueue");
  assert.equal(page.ws.sent.slice(before).filter((m) => m.type === "dm_put").length, 1, "my change still went to my other devices");
  assert.ok(!idb.writes.includes(scope), "nothing was saved over the record");
  assert.deepEqual(keptCopies(idb, scope), [], "nor was anything copied");
});

// Red first, 2026-11-02, each with HOS_WEB_DIR at a copy of web/ broken on purpose (or at HEAD's
// web/ from before the fix), then put back:
//  - HEAD's web/ (before the fix): every test failed; the BUG-217 one at "no server form opens"
//    (Add to my tasks opened the server's New Task form and made nothing of mine).
//  - noteTexts writing `created_at` after `updated_at`: "the note text, byte for byte".
//  - wins() without the deletion's rank: "at the same time a deletion wins".
//  - renderBoard drawing allTasks on My tasks too: "and not the board's".
//  - sealSelfNote leaving the inner unsealed (content = the inner JSON): "a frame to the server
//    carries insulin".
//  - takeEnvelope not checking `from`: "a task note from someone else is dropped".
//  - (the review of rung 1) mine-tasks.js as committed before the fix, which started empty on a
//    record that did not open and on a refused read: "a copy was kept aside" (0) and "the page
//    says it could not be read"; openStore taking a record that is not there as a refused read:
//    "with nothing to say".
