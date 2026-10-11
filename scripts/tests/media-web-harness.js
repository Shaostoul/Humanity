// The stand-ins the Media page's web tests run against (2026-10-11): shared by
// scripts/tests/media-web.test.js, media-web-round2.test.js, media-web-round3.test.js and
// media-web-round4.test.js. Not a test itself
// (its name does not end in .test.js, so `node --test "scripts/tests/*.test.js"` does not run it).
//
// The page's own files run as they do in the browser: web/pages/media-app.js (the page),
// web/pages/media-upload.js (its resumable upload), web/pages/media-admin.js (the admins'
// section), web/shared/relay-session.js (the page's own signed-in connection) and
// web/shared/report.js (a report's words). The document is a small stand-in that holds ONLY what
// the page built in it: getElementById answers null for anything that is not there, and reading
// or setting innerHTML throws, so a test that finds a button proves the page made it, and the
// page cannot slip server text in as HTML. The network is a stand-in that records every request
// (with its headers) and answers like src/relay/media.rs; signing is a stand-in that returns
// "sig:" plus the words it signed, so each request shows exactly which purpose its signature
// covers. Object URLs are a stand-in that records what was made and revoked; localStorage is a
// Map. HOS_WEB_DIR points the tests at another copy of web/ (used to see each test red).

const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const media = require(path.join(WEB, "pages", "media-app.js"));
const upload = require(path.join(WEB, "pages", "media-upload.js"));
const admin = require(path.join(WEB, "pages", "media-admin.js"));
const relaySession = require(path.join(WEB, "shared", "relay-session.js"));
const report = require(path.join(WEB, "shared", "report.js"));

// The shipped data files the page reads.
const KINDS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "media", "kinds.json"), "utf8"));

const ORIGIN = "https://united-humanity.us";
const ME = "a1".repeat(32);
const ANN = "b2".repeat(32);
const MIB = 1024 * 1024;

// ── A document that holds only what was built in it ─────────────────────

const camel = (s) => s.replace(/-([a-z])/g, (_m, c) => c.toUpperCase());

class FakeText {
  constructor(t) { this.nodeType = 3; this._t = String(t); this.parentNode = null; }
  get textContent() { return this._t; }
  set textContent(v) { this._t = String(v); }
  remove() { if (this.parentNode) this.parentNode.removeChild(this); }
}

class FakeElement {
  constructor(doc, tag) {
    this.ownerDocument = doc;
    this.nodeType = 1;
    this.tagName = String(tag).toUpperCase();
    this.childNodes = [];
    this.parentNode = null;
    this.attributes = new Map();
    this.dataset = {};
    this.style = {};
    this.listeners = {};
    this.className = "";
    this.id = "";
    this._value = "";
    this.checked = false;
    this.disabled = false;
    this.hidden = false;
    this.selected = false;
    this.files = null;
  }
  get children() { return this.childNodes.filter((c) => c.nodeType === 1); }
  get textContent() { return this.childNodes.map((c) => c.textContent).join(""); }
  set textContent(v) { this.replaceChildren(); if (String(v)) this.appendChild(new FakeText(v)); }
  get innerHTML() { throw new Error("the page read innerHTML"); }
  set innerHTML(v) { throw new Error("the page set innerHTML: " + String(v).slice(0, 80)); }
  get value() {
    if (this.tagName === "SELECT") {
      const opts = this.children.filter((c) => c.tagName === "OPTION");
      const chosen = opts.find((o) => o.selected) || opts[0];
      return chosen ? chosen.value : "";
    }
    return this._value;
  }
  set value(v) {
    if (this.tagName === "SELECT") {
      for (const o of this.children) o.selected = o.value === String(v);
      return;
    }
    this._value = String(v);
  }
  appendChild(c) {
    if (c.parentNode) c.parentNode.removeChild(c);
    c.parentNode = this;
    this.childNodes.push(c);
    return c;
  }
  // As a browser does: anything that is not a node is written as text (a null as "null").
  append(...cs) { for (const c of cs) this.appendChild(c && typeof c === "object" && c.nodeType ? c : new FakeText(String(c))); }
  insertBefore(c, ref) {
    if (!ref) return this.appendChild(c);
    if (c.parentNode) c.parentNode.removeChild(c);
    c.parentNode = this;
    this.childNodes.splice(this.childNodes.indexOf(ref), 0, c);
    return c;
  }
  removeChild(c) {
    const i = this.childNodes.indexOf(c);
    if (i >= 0) this.childNodes.splice(i, 1);
    c.parentNode = null;
    return c;
  }
  remove() { if (this.parentNode) this.parentNode.removeChild(this); }
  replaceChildren(...cs) {
    for (const c of this.childNodes.slice()) this.removeChild(c);
    this.append(...cs);
  }
  setAttribute(k, v) { this.attributes.set(k, String(v)); if (k === "id") this.id = String(v); }
  getAttribute(k) { return this.attributes.has(k) ? this.attributes.get(k) : null; }
  removeAttribute(k) { this.attributes.delete(k); }
  addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); }
  removeEventListener(t, f) { this.listeners[t] = (this.listeners[t] || []).filter((x) => x !== f); }
  dispatch(type, extra) {
    const ev = Object.assign({ type, target: this, preventDefault() {}, stopPropagation() {} }, extra || {});
    for (const f of (this.listeners[type] || []).slice()) f(ev);
  }
  click() { if (!this.disabled) this.dispatch("click"); }
  focus() {}
  matches(sel) {
    let m;
    if ((m = /^#([\w-]+)$/.exec(sel))) return this.id === m[1];
    if ((m = /^\.([\w-]+)$/.exec(sel))) return this.className.split(/\s+/).includes(m[1]);
    if ((m = /^\[data-([\w-]+)(?:="([^"]*)")?\]$/.exec(sel))) {
      const v = this.dataset[camel(m[1])];
      return m[2] === undefined ? v !== undefined : v === m[2];
    }
    if (/^[a-z][a-z0-9]*$/i.test(sel)) return this.tagName === sel.toUpperCase();
    throw new Error("the stand-in document does not know the selector " + sel);
  }
  querySelectorAll(sel) {
    const out = [];
    const walk = (n) => { for (const c of n.children) { if (c.matches(sel)) out.push(c); walk(c); } };
    walk(this);
    return out;
  }
  querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
}

function fakeDocument() {
  const doc = {
    readyState: "complete",
    createElement: (tag) => new FakeElement(doc, tag),
    createTextNode: (t) => new FakeText(t),
    addEventListener() {},
    removeEventListener() {},
  };
  doc.body = new FakeElement(doc, "body");
  doc.getElementById = (id) => (doc.body.id === id ? doc.body : doc.body.querySelector("#" + id));
  return doc;
}

const byId = (doc, id) => doc.getElementById(id);
const text = (el) => (el ? el.textContent : null);

/** Every element in the document under the body. */
function everyElement(doc) {
  const out = [];
  const walk = (n) => { for (const c of n.children) { out.push(c); walk(c); } };
  walk(doc.body);
  return out;
}

// ── The network, signing and the session ─────────────────────────────────

/** A response. `body`: JSON (an object) or words (a string); `bytes`: what blob() holds. */
function respond(status, body, bytes) {
  const json = typeof body === "string" ? null : body;
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => json,
    text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
    blob: async () => new Blob([bytes || new Uint8Array([1, 2, 3])]),
  };
}

/**
 * A fetch that answers from `routes` (in order: [test(call), answer(call)]) and records. A request
 * made with a `signal` is rejected with an AbortError the moment that signal is aborted, as a
 * browser's is, and its call is marked `aborted`.
 */
function fakeFetch(routes) {
  const calls = [];
  const fetch = (url, init) => {
    const call = { url: String(url), init: init || {}, body: null, headers: (init && init.headers) || {}, aborted: false };
    if (init && typeof init.body === "string") { try { call.body = JSON.parse(init.body); } catch (e) { call.body = init.body; } }
    calls.push(call);
    const answered = (async () => {
      for (const [when, answer] of routes) if (when(call)) return answer(call);
      return respond(404, "no route in the test for " + url);
    })();
    const signal = init && init.signal;
    if (!signal) return answered;
    return new Promise((resolve, reject) => {
      const abort = () => {
        call.aborted = true;
        const err = new Error("The operation was aborted.");
        err.name = "AbortError";
        reject(err);
      };
      if (signal.aborted) { abort(); return; }
      signal.addEventListener("abort", abort, { once: true });
      answered.then(resolve, reject);
    });
  };
  return { fetch, calls };
}

const path0 = (u) => u.split("?")[0];
const at = (p, method) => (c) => path0(c.url) === p && (!method || (c.init.method || "GET") === method);
const queryOf = (u) => new URLSearchParams(u.includes("?") ? u.slice(u.indexOf("?") + 1) : "");

/** Signing as a stand-in: the "signature" is the words signed, so a request shows its purpose. */
function fakeSigner(key) {
  const signed = [];
  let ts = 1760000000000;
  return {
    signed,
    sign: async (purpose) => {
      signed.push(purpose);
      return { key, timestamp: ++ts, sig: "sig:" + purpose };
    },
  };
}

// The relay's role list (storage/roles.rs RoleDef), the fields the page reads.
const ROLES = [
  { id: "admin", label: "Admin", can_upload: true },
  { id: "owner", label: "Owner", can_upload: true },
  { id: "mod", label: "Moderator", can_upload: true },
  { id: "verified", label: "Verified", can_upload: true },
  { id: "unverified", label: "Unverified", can_upload: false },
];

function fakeSession(over) {
  const sent = [];
  const listeners = [];
  const s = {
    sent,
    state: Object.assign({
      status: "bound", key: ME, role: "verified", uploadToken: "tok-1", uploadLimitBytes: 1024 * MIB,
      settings: { media_visibility: "public", media_budget_mb: 2048, media_per_person_mb: 512, media_public_previews_only: true, media_members_note: "Members are everyone signed in to this server. To keep it to people you let in, turn on lockdown." },
      roles: ROLES,
      refusal: "", retryInSeconds: 0,
    }, over || {}),
    subscribe(fn) { listeners.push(fn); return () => {}; },
    start() {},
    retry() {},
    send(frame) { sent.push(frame); return s.state.status === "bound"; },
    emit(ev) { for (const fn of listeners) fn(ev, s.state); },
  };
  return s;
}

class FakeFormData {
  constructor() { this.parts = []; }
  append(k, v) { this.parts.push([k, v]); }
}

/** Object URLs: what was made (for which blob) and what was revoked. */
function fakeURLs() {
  const made = [];
  const revoked = [];
  return {
    made, revoked,
    createObjectURL(blob) { const u = "blob:test/" + (made.length + 1); made.push({ url: u, blob }); return u; },
    revokeObjectURL(u) { revoked.push(u); },
    live() { return made.map((m) => m.url).filter((u) => !revoked.includes(u)); },
  };
}

/** localStorage as a Map; `refuse` makes every call throw, as a browser that blocks storage does. */
function fakeStorage(refuse) {
  const m = new Map();
  return {
    m,
    getItem(k) { if (refuse) throw new Error("storage refused"); return m.has(k) ? m.get(k) : null; },
    setItem(k, v) { if (refuse) throw new Error("storage refused"); m.set(k, String(v)); },
    removeItem(k) { if (refuse) throw new Error("storage refused"); m.delete(k); },
  };
}

async function settle() {
  for (let i = 0; i < 60; i++) await new Promise((r) => setImmediate(r));
}

const LABELS = [
  { id: "spoiler", label: "Spoiler", desc: "Shows something people may want to find out for themselves" },
  { id: "flashing", label: "Flashing lights", desc: "Rapid flashes or strobing" },
  { id: "graphic", label: "Graphic", desc: "Injury or blood" },
];
const CATEGORIES = [{ id: 1, name: "General", description: "", position: 0 }, { id: 2, name: "Fire flow arts", description: "", position: 1 }];

function item(over) {
  return Object.assign({
    id: 1, url: "/uploads/1_a.png", title: "Contact staff", description: "", category: { id: 2, name: "Fire flow arts" },
    kind: "picture", labels: [], visibility: "public", featured: false, size_bytes: 2 * MIB, views: 0,
    uploaded_at: 1, published_at: 1, uploader_key: ANN, uploader_name: "Ann", picks: 0,
    original_name: "", keeps_file: false, kept: false,
  }, over || {});
}

function listAnswer(items, over) {
  return Object.assign({
    items, next_before: null, members_view: true, media_visibility: "public", members_only_for_you: false,
    categories: CATEGORIES, labels: LABELS,
  }, over || {});
}

/** The relay's answer to /api/media/mine (A4): 100 MB used of a 512 MB share, nothing waiting. */
function mineAnswer(over) {
  return Object.assign({
    used_bytes: 100 * MIB, share_bytes: 512 * MIB, budget_left_bytes: 10 * 1024 * MIB, in_progress_bytes: 0,
    file_limit_bytes: 0, waiting: [],
  }, over || {});
}

/** A list request for the featured strip (`featured=1`, signed or not). */
const isFeatured = (c) => (path0(c.url) === "/api/media/list" && c.body && c.body.featured === 1) ||
  (path0(c.url) === "/api/media" && (c.init.method || "GET") === "GET" && queryOf(c.url).get("featured") === "1");

/**
 * Routes for a list that answers `answer` (its featured items for the strip), the kinds file, an
 * empty picks list and the person's share. `more` comes FIRST, so a test can answer any of these
 * its own way.
 */
function listRoutes(answer, more) {
  const featured = () => listAnswer(((answer && answer.items) || []).filter((i) => i.featured), answer ? { members_view: answer.members_view, categories: answer.categories, labels: answer.labels } : {});
  return (more || []).concat([
    [at("/data/media/kinds.json"), () => respond(200, KINDS)],
    [at("/api/media/mine", "POST"), () => respond(200, mineAnswer())],
    [at("/api/media/picks", "POST"), () => respond(200, listAnswer([]))],
    [isFeatured, () => respond(200, featured())],
    [at("/api/media/list", "POST"), () => respond(200, answer)],
    [at("/api/media", "GET"), () => respond(200, answer)],
  ]);
}

/** The page, started, with a document, a network and (signed in) a session. */
async function openPage(opts) {
  const o = opts || {};
  const doc = fakeDocument();
  const root = doc.createElement("div");
  root.id = "media-root";
  doc.body.appendChild(root);
  const signer = fakeSigner(o.myKey === undefined ? ME : o.myKey);
  const net = fakeFetch(o.routes || []);
  const confirms = [];
  const saved = [];
  const timers = [];
  const urls = fakeURLs();
  const storage = o.storage || fakeStorage();
  const session = o.session === undefined ? fakeSession() : o.session;
  // What a test's mountAdmin is handed with the container: the page (set before it starts), the network.
  const ctx = { page: null, net, doc };
  const page = media.createMediaPage({
    document: doc, root, fetch: net.fetch, origin: ORIGIN,
    myKey: o.myKey === undefined ? ME : o.myKey,
    sign: o.sign || signer.sign, session,
    // A report's signature: the words signed, as bytes, so a test reads exactly what was signed.
    signBytes: async (bytes) => new TextEncoder().encode("sig:" + new TextDecoder().decode(bytes)),
    blake3: BLAKE,
    confirm: async (words) => { confirms.push(words); return o.confirm !== false; },
    FormData: FakeFormData, put: o.put || null, openId: o.openId || null,
    mountAdmin: o.mountAdmin ? (c) => o.mountAdmin(c, ctx) : null,
    URL: urls, storage, save: (url, name) => saved.push({ url, name }),
    sleep: o.sleep || (() => Promise.resolve()), now: o.now || (() => 1760000000000),
    // The fingerprint's SHA-256 (WebCrypto's takes a few hundred event-loop turns in Node, longer
    // than settle() waits; this one is the same hash, at once).
    digest: o.digest || SHA256,
    setTimeout: (fn, ms) => { timers.push({ fn, ms }); return timers.length; }, clearTimeout: () => {},
    report, language: "en-GB",
  });
  ctx.page = page;
  await page.start();
  await settle();
  return { doc, root, page, net, signer, session, confirms, saved, timers, urls, storage };
}

// ── The admins' section ──────────────────────────────────────────────────

const BLAKE = async (bytes) => new Uint8Array(require("node:crypto").createHash("sha256").update(Buffer.from(bytes)).digest());
/** SHA-256, as WebCrypto's digest answers it (an ArrayBuffer), but at once. */
const SHA256 = async (bytes) => {
  const d = require("node:crypto").createHash("sha256").update(Buffer.from(bytes)).digest();
  return d.buffer.slice(d.byteOffset, d.byteOffset + d.byteLength);
};
/** The fingerprint media-upload.js works out for a file whose bytes are `bytes` (its first and last MiB). */
function fingerprintOf(bytes) {
  const b = Buffer.from(bytes);
  const head = b.subarray(0, Math.min(b.length, MIB));
  const tail = b.subarray(Math.max(0, b.length - MIB));
  return require("node:crypto").createHash("sha256").update(Buffer.concat([head, tail])).digest("hex");
}
/** The bytes a piece's request carries, however it was sent (a Uint8Array, an ArrayBuffer or a Blob). */
function bodyLen(body) {
  if (body == null) return 0;
  if (typeof body.byteLength === "number") return body.byteLength;
  if (typeof body.size === "number") return body.size;
  return String(body).length;
}

// ── The resumable upload's stand-ins (rounds 3 and 4) ───────────────────
//
// Moved here from media-web-round3.test.js in round 4, so media-web-round4.test.js runs against
// the same ones. The stand-in relay answers a cancel the way round 4's does.

const T0 = 1760000000000;

/**
 * A file the upload reads: `fill` is its every byte (so two files of one name, size and date can
 * differ), and `gone` (set by a test) makes every read fail as a browser's does once the file was
 * moved or its drive unplugged.
 */
function aFile(name, size, opts) {
  const o = opts || {};
  const f = {
    name, size, type: o.type || "video/mp4", lastModified: o.lastModified || 1700000000000, gone: false,
    slice(a, b) {
      const len = Math.max(0, Math.min(size, b) - a);
      return {
        size: len,
        arrayBuffer: async () => {
          if (f.gone) { const e = new Error("A requested file or directory could not be found."); e.name = "NotFoundError"; throw e; }
          return new Uint8Array(len).fill(o.fill || 0).buffer;
        },
      };
    },
  };
  return f;
}

/** Waits that end only when the test wakes them; each asked-for length is kept. */
function sleeper() {
  const s = { waits: [], wakes: [] };
  s.sleep = (ms) => { s.waits.push(ms); return new Promise((r) => s.wakes.push(r)); };
  s.wakeAll = () => { while (s.wakes.length) s.wakes.shift()(); };
  return s;
}

/** Wake every wait and let the page run, until `until()` holds or `turns` rounds have passed. */
async function drive(sl, until, turns) {
  for (let i = 0; i < (turns || 40); i++) {
    await settle();
    if (until && until()) return;
    sl.wakeAll();
  }
  await settle();
}

/**
 * A stand-in for src/relay/resumable.rs keeping several uploads at once, by id. `hooks.start(c)`,
 * `hooks.piece(c, sess, offset, len)`, `hooks.status(c, sess)`, `hooks.del(c, sess)` and
 * `hooks.finish(sess)` may answer a request themselves (or throw: a dropped connection). `log`
 * holds every request it got, in order.
 *
 * A cancel (DELETE) answers as round 4's relay does (docs/design/media-showcase.md "Round 4", a
 * cancel means nothing stays, and its "As built (relay, round 4)"): 409 `{error, busy: true}`
 * while a piece or the finish is being written, nothing done; `{ok: true, removed_stored: true}`
 * on a finished upload not yet published, whose stored file it deletes too
 * (`sess.storedRemoved`); `{ok: true, published: true}` on one already published
 * (`sess.published`, set by a test), whose item it leaves; else `{ok: true}`. After it, the
 * session is gone (404). A finished one whose stored file cannot be deleted (`sess.cannotDelete`,
 * set by a test) is answered 500 in plain words, and everything stays, to be asked again.
 */
function resumableRelay(hooks) {
  const r = { next: 0, sessions: new Map(), log: [], pieces: [], hooks: hooks || {}, chunk: 8 * MIB };
  const doneOf = (sess) => ({ url: "/uploads/" + sess.stored, filename: sess.stored, size: sess.size, type: sess.type });
  r.doneOf = doneOf;
  r.routes = [
    [at("/api/upload/resumable", "POST"), async (c) => {
      r.log.push({ kind: "start", c });
      if (r.hooks.start) { const a = await r.hooks.start(c); if (a) return a; }
      const id = String.fromCharCode(97 + r.next++).repeat(32);
      r.sessions.set(id, { id, offset: 0, size: c.body.size, stored: "1760_" + c.body.filename, type: c.body.content_type, done: null, deleted: false });
      return respond(201, { id, offset: 0, size: c.body.size, chunk_max: r.chunk });
    }],
    [(c) => /^\/api\/upload\/resumable\/[a-z]{32}$/.test(path0(c.url)), async (c) => {
      const id = path0(c.url).split("/").pop();
      const method = c.init.method || "GET";
      r.log.push({ kind: method, id, c });
      const sess = r.sessions.get(id);
      if (!sess || sess.deleted) return respond(404, "There is no upload with that id.");
      if (method === "GET") {
        if (r.hooks.status) { const a = await r.hooks.status(c, sess); if (a) return a; }
        return respond(200, Object.assign({ offset: sess.offset, size: sess.size, chunk_max: r.chunk }, sess.done ? { done: sess.done } : {}));
      }
      // While the last piece is being stored, a cancel or another piece is answered busy (R2).
      const busy = () => respond(409, { error: "Another piece of this upload is being written. Wait for its answer, then ask where the upload is (GET) and carry on from there.", busy: true });
      if (method === "DELETE") {
        if (r.hooks.del) { const a = await r.hooks.del(c, sess); if (a) return a; }
        if (sess.busy) return busy();
        if (sess.done && !sess.published && sess.cannotDelete) return respond(500, "The uploaded file could not be removed from the server (Access is denied. (os error 5)). Try again in a moment.");
        sess.deleted = true;
        if (sess.done && sess.published) return respond(200, { ok: true, published: true });
        if (sess.done) { sess.storedRemoved = true; return respond(200, { ok: true, removed_stored: true }); }
        return respond(200, { ok: true });
      }
      const off = Number(queryOf(c.url).get("offset"));
      const len = bodyLen(c.init.body);
      r.pieces.push({ id, offset: off, len, c });
      if (r.hooks.piece) { const a = await r.hooks.piece(c, sess, off, len); if (a) return a; }
      if (sess.busy) return busy();
      if (sess.done) return respond(200, sess.done);
      if (off !== sess.offset) return respond(409, { error: "This piece starts at byte " + off + ", but the server has " + sess.offset + ".", offset: sess.offset, size: sess.size });
      sess.offset += len;
      if (sess.offset >= sess.size) {
        sess.busy = true;
        try {
          if (r.hooks.finish) { const a = await r.hooks.finish(sess); if (a) return a; }
        } finally {
          sess.busy = false;
        }
        sess.done = doneOf(sess);
        return respond(200, sess.done);
      }
      return respond(200, { offset: sess.offset, size: sess.size });
    }],
  ];
  return r;
}

/** One uploader against a stand-in relay, with a stand-in sleep. */
function uploaderOn(relay, file, o) {
  const opts = o || {};
  const net = fakeFetch(relay.routes);
  const sl = opts.sleeper || sleeper();
  const phases = [];
  const result = { done: null };
  const storage = opts.storage || fakeStorage();
  let clock = opts.now || T0;
  const u = upload.createUploader({
    fetch: net.fetch, token: opts.token || (() => "tok-1"), file, owner: ME, storage, digest: SHA256,
    sleep: sl.sleep, now: () => (typeof clock === "function" ? clock() : clock),
    onChange: (s) => phases.push(s.phase), onDone: (a) => { result.done = a; },
    AbortController: opts.AbortController,
  });
  return { u, net, sl, phases, result, storage, setNow: (t) => { clock = t; } };
}

// nginx's own refusal pages: HTML, no JSON.
const NGINX_503 = "<html>\r\n<head><title>503 Service Temporarily Unavailable</title></head>\r\n<body><center><h1>503 Service Temporarily Unavailable</h1></center><hr><center>nginx</center></body></html>";
const NGINX_429 = "<html><head><title>429 Too Many Requests</title></head><body><center><h1>429 Too Many Requests</h1></center></body></html>";

function usageAnswer(over) {
  return Object.assign({
    used_bytes: Math.round(12.4 * 1024 * MIB), budget_bytes: 50 * 1024 * MIB, per_person_bytes: 512 * MIB,
    items: 3, people: [],
    largest: [item({ id: 11, title: "Big video", kind: "video", url: "/uploads/11_big.mp4", size_bytes: 3 * 1024 * MIB, uploader_name: "Ann" })],
    oldest: [item({ id: 12, title: "First", url: "/uploads/12_first.png" })],
    least_viewed: [],
    waiting: [{ filename: "1760_wait.mp4", uploader_key: ANN, uploader_name: "Ann", size_bytes: 40 * MIB, uploaded_at: 1, expires_at: 86400001 }],
    kept_bytes: 0,
  }, over || {});
}

async function openAdmin(opts) {
  const o = opts || {};
  const doc = fakeDocument();
  const box = doc.createElement("div");
  box.id = "media-admin-root";
  doc.body.appendChild(box);
  const signer = o.signer || fakeSigner(ME);
  const usage = o.usage || usageAnswer();
  const net = fakeFetch((o.routes || []).concat([
    [at("/api/media/list", "POST"), () => respond(200, listAnswer([], { categories: CATEGORIES }))],
    [at("/api/media/suggestions", "POST"), () => respond(200, { all: true, suggestions: [
      { id: 3, suggester_key: ANN, suggester_name: "Ann", name: "Builds", note: "Ship builds", created_at: 1, state: "open" },
      { id: 2, suggester_key: ANN, suggester_name: "Ann", name: "Old", note: "", created_at: 1, state: "declined" },
    ] })],
    [at("/api/media/suggestions/decide", "POST"), () => respond(200, { ok: true })],
    [at("/api/media/usage", "POST"), () => respond(200, usage)],
    [at("/api/media/remove", "POST"), () => respond(200, { ok: true, removed_upload: "1760_wait.mp4", file_removed: true, file_kept: false })],
    [at("/api/media/keep", "POST"), (c) => respond(200, { ok: true, id: c.body.id, kept: c.body.kept })],
    [at("/api/media/categories", "POST"), (c) => respond(200, { ok: true, categories: c.body.categories.map((x, i) => ({ id: x.id || 10 + i, name: x.name, description: x.description, position: i })) })],
  ]));
  const confirms = [];
  const timers = [];
  const session = o.session || fakeSession({ role: "admin" });
  const ctl = admin.mountMediaAdmin(box, {
    document: doc, fetch: net.fetch, sign: o.sign || signer.sign, blake3: BLAKE, session,
    confirm: async (w) => { confirms.push(w); return true; },
    onChange: o.onChange,
    setTimeout: (fn, ms) => { timers.push({ fn, ms }); return timers.length; }, clearTimeout: () => {},
  });
  await ctl.ready;
  await settle();
  return { doc, net, signer, session, confirms, timers, ctl };
}

module.exports = {
  ROOT, WEB, media, upload, admin, relaySession, report, KINDS, ORIGIN, ME, ANN, MIB,
  FakeText, FakeElement, fakeDocument, byId, text, everyElement,
  respond, fakeFetch, path0, at, queryOf, fakeSigner, ROLES, fakeSession, FakeFormData, fakeURLs, fakeStorage, settle,
  LABELS, CATEGORIES, item, listAnswer, mineAnswer, isFeatured, listRoutes, openPage, BLAKE, SHA256, fingerprintOf,
  bodyLen, usageAnswer, openAdmin,
  T0, aFile, sleeper, drive, resumableRelay, uploaderOn, NGINX_503, NGINX_429,
};
