// The Media page on the website (2026-10-11, docs/design/media-showcase.md "Increment 2, exact").
//
// Run: node --test scripts/tests/media-web.test.js   (in `just rig-tests`)
//
// The page's own files run as they do in the browser, against stand-ins: web/pages/media-app.js
// (the page), web/pages/media-admin.js (the admins' section) and web/shared/relay-session.js (the
// page's own signed-in connection). The document is a small stand-in that holds ONLY what the page
// built in it: getElementById answers null for anything that is not there, and reading or setting
// innerHTML throws, so a test that finds a button proves the page made it, and the page cannot
// slip server text in as HTML. The network is a stand-in that records every request and answers
// like src/relay/media.rs; signing is a stand-in that returns "sig:" plus the words it signed, so
// each request shows exactly which purpose its signature covers. The session's sign-in check is
// real ML-DSA-65 from the vendored noble bundle, verified the way the relay verifies it.
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves (the design's web Proof list):
//  1. Empty: the empty words, no tile, no featured strip, no More.
//  2. Items: tiles with titles, uploader names and pick counts; the featured strip; More pages
//     with `before`; the category, kind and person filters come from the server's answers.
//  3. A labelled tile shows its label instead of its picture, and no picture is even requested
//     until it is clicked.
//  4. The members-only line for someone not signed in, the unsigned public list then; a signed
//     list (media_list) for someone signed in, no line, and a members-only file fetched through
//     /api/media/file/{id} with the session's upload token.
//  5. Pictures load only from this server's own addresses.
//  6. Add to Media: the share shown ("You have 412 MB of your 512 MB left"), an upload over the
//     share stopped before it starts (no upload request at all), one within it uploaded with
//     ?media=1 and the session token, then published.
//  7. Put on Media (a Shared Files row): public only, the visibility choice not offered and the
//     line saying why; no upload. On a members-only server Add to Media offers no choice either.
//  8. Each signed purpose names its target: publish, pick, unpick, remove (with the words naming
//     what happens to the file), feature; and in the admins' section the category list's hash,
//     a suggestion's decision, a waiting upload's removal by name.
//  9. The admins' section: the visibility switch with the relay's members sentence under it, the
//     four settings saved in one server_settings_update, the meter ("12.4 GB of 50 GB used") with
//     its bar, the prune lists and the suggestions waiting with the suggester named.
// 10. The session signs in with a real Dilithium3 signature over the relay's exact words, sends
//     nothing else first, takes the upload token, role and limit from its own peer_list entry,
//     asks for the settings, and with no identity opens no socket.
//
// Seen red, 2026-10-11 (each with the code it guards broken in a copy of web/ under HOS_WEB_DIR,
// then green again on the real web/): see the note above each test.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const media = require(path.join(WEB, "pages", "media-app.js"));
const admin = require(path.join(WEB, "pages", "media-admin.js"));
const relaySession = require(path.join(WEB, "shared", "relay-session.js"));

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

// ── The network, signing and the session ─────────────────────────────────

function respond(status, body) {
  const json = typeof body === "string" ? null : body;
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => json,
    text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
  };
}

/** A fetch that answers from `routes` (in order: [test(url, init), answer(url, init)]) and records. */
function fakeFetch(routes) {
  const calls = [];
  const fetch = async (url, init) => {
    const call = { url: String(url), init: init || {}, body: null };
    if (init && typeof init.body === "string") call.body = JSON.parse(init.body);
    calls.push(call);
    for (const [when, answer] of routes) if (when(call)) return answer(call);
    return respond(404, "no route in the test for " + url);
  };
  return { fetch, calls };
}

const path0 = (u) => u.split("?")[0];
const at = (p, method) => (c) => path0(c.url) === p && (!method || (c.init.method || "GET") === method);

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

function fakeSession(over) {
  const sent = [];
  const listeners = [];
  const s = {
    sent,
    state: Object.assign({
      status: "bound", key: ME, role: "verified", uploadToken: "tok-1", uploadLimitBytes: 1024 * MIB,
      settings: { media_visibility: "public", media_budget_mb: 2048, media_per_person_mb: 512, media_public_previews_only: true, media_members_note: "Members are everyone signed in to this server. To keep it to people you let in, turn on lockdown." },
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

async function settle() {
  for (let i = 0; i < 40; i++) await new Promise((r) => setImmediate(r));
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
  }, over || {});
}

function listAnswer(items, over) {
  return Object.assign({
    items, next_before: null, members_view: true, media_visibility: "public", members_only_for_you: false,
    categories: CATEGORIES, labels: LABELS,
  }, over || {});
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
  const session = o.session === undefined ? fakeSession() : o.session;
  const page = media.createMediaPage({
    document: doc, root, fetch: net.fetch, origin: ORIGIN,
    myKey: o.myKey === undefined ? ME : o.myKey,
    sign: signer.sign, session,
    confirm: async (words) => { confirms.push(words); return o.confirm !== false; },
    FormData: FakeFormData, put: o.put || null, mountAdmin: o.mountAdmin || null,
  });
  await page.start();
  await settle();
  return { doc, root, page, net, signer, session, confirms };
}

/** Routes for a list that answers `answer`, and an empty picks list. */
function listRoutes(answer, more) {
  return [
    [at("/api/media/picks/" + ME), () => respond(200, listAnswer([], { members_view: false }))],
    [at("/api/media/list", "POST"), () => respond(200, answer)],
    [at("/api/media", "GET"), () => respond(200, answer)],
  ].concat(more || []);
}

// ── 1. Empty ─────────────────────────────────────────────────────────────

// Seen red with renderGrid's empty branch drawing nothing: "the empty words / actual: null".
test("1. an empty Media page says so and draws no tile, strip or More", async () => {
  const { doc } = await openPage({ routes: listRoutes(listAnswer([])) });
  const grid = byId(doc, "media-grid");
  assert.equal(text(grid.querySelector(".media-empty")), media.MEDIA_WORDS.empty, "the empty words");
  assert.equal(grid.querySelectorAll(".media-tile").length, 0, "no tile");
  assert.equal(byId(doc, "media-featured-row"), null, "no featured strip");
  assert.equal(byId(doc, "media-more"), null, "no More");
});

// ── 2. Items ─────────────────────────────────────────────────────────────

// Seen red with takeAnswer's `next_before` read as `nextBefore`: "More is there when the server
// says there is more / actual: null".
test("2. items show as tiles with names and picks, the featured strip leads, More pages on", async () => {
  const first = listAnswer([
    item({ id: 9, title: "Contact staff", featured: true, picks: 3 }),
    item({ id: 8, title: "", url: "/uploads/8_clip.mp4", kind: "video", uploader_key: ME, uploader_name: "Me", picks: 1 }),
  ], { next_before: 8 });
  const second = listAnswer([item({ id: 7, title: "Older", kind: "model", url: "/uploads/7_part.stl", uploader_key: "c3".repeat(32), uploader_name: "Cy" })]);
  let page = 0;
  const { doc, net } = await openPage({ routes: listRoutes(null, []).slice(0, 1).concat([
    [at("/api/media/list", "POST"), () => respond(200, page++ === 0 ? first : second)],
  ]) });
  const tiles = byId(doc, "media-grid").querySelectorAll(".media-tile");
  assert.equal(tiles.length, 2);
  assert.equal(text(tiles[0].querySelector(".media-tile-title")), "Contact staff");
  assert.equal(text(tiles[0].querySelector(".media-tile-meta")), "by Ann · 3 picks");
  assert.equal(tiles[0].querySelector("img").src, "/uploads/1_a.png", "a picture tile shows the picture from /uploads/");
  assert.equal(text(tiles[1].querySelector(".media-tile-title")), "8_clip.mp4", "an untitled item is called by its file name");
  assert.equal(text(tiles[1].querySelector(".media-tile-meta")), "by Me · 1 pick");
  const strip = byId(doc, "media-featured-row");
  assert.ok(strip, "the featured strip is there");
  assert.equal(strip.querySelectorAll(".media-tile").length, 1);
  assert.equal(text(strip.querySelector(".media-tile-title")), "Contact staff");
  // The filters come from the answers: the server's categories, the kinds its items carry, its people.
  const options = (id) => byId(doc, id).children.map((o) => o.textContent);
  assert.deepEqual(options("media-filter-category"), ["All categories", "General", "Fire flow arts"]);
  assert.deepEqual(options("media-filter-kind"), ["All kinds", "Pictures", "Videos"]);
  assert.deepEqual(options("media-filter-person"), ["Everyone", "Ann", "Me"]);
  const more = byId(doc, "media-more");
  assert.ok(more, "More is there when the server says there is more");
  more.click();
  await settle();
  const lists = net.calls.filter((c) => c.url === "/api/media/list");
  assert.equal(lists.at(-1).body.before, 8, "More asks for the items before the last one shown");
  assert.equal(byId(doc, "media-grid").querySelectorAll(".media-tile").length, 3, "the next page is added below");
  assert.equal(byId(doc, "media-more"), null, "no More after the last page");
  assert.deepEqual(options("media-filter-kind"), ["All kinds", "Pictures", "Videos", "Models"], "a kind seen later joins the filter");
  // A filter asks the server again with it.
  const kind = byId(doc, "media-filter-kind");
  kind.value = "video";
  kind.dispatch("change");
  await settle();
  assert.equal(net.calls.filter((c) => c.url === "/api/media/list").at(-1).body.kind, "video");
});

// ── 3. A labelled tile ───────────────────────────────────────────────────

// Seen red with thumb() skipping the veil (labels ignored): "a labelled tile requests no
// picture before it is clicked / 1 !== 0".
test("3. a labelled tile shows its label instead of its picture until clicked", async () => {
  const { doc } = await openPage({ routes: listRoutes(listAnswer([item({ id: 4, labels: ["flashing", "graphic"], title: "Fire show" })])) });
  const tile = byId(doc, "media-grid").querySelector(".media-tile");
  assert.equal(tile.querySelectorAll("img").length, 0, "a labelled tile requests no picture before it is clicked");
  const veil = tile.querySelector(".media-veil");
  assert.ok(veil, "the label stands in the picture's place");
  assert.equal(text(veil.querySelector(".media-veil-labels")), "Flashing lights, Graphic", "the label names come from the server's list");
  assert.deepEqual(tile.querySelectorAll(".media-badge-label").map((b) => b.textContent), ["Flashing lights", "Graphic"], "label badges");
  veil.click();
  const shown = byId(doc, "media-grid").querySelector(".media-tile");
  assert.equal(shown.querySelectorAll("img").length, 1, "after the click the picture shows");
  assert.equal(shown.querySelector("img").src, "/uploads/1_a.png");
  assert.equal(shown.querySelector(".media-veil"), null);
});

// ── 4. Members ───────────────────────────────────────────────────────────

// Seen red with renderMembers drawing nothing: "someone not signed in is told / actual: null";
// and with fetchList never signing: "a signed-in person's list is signed / actual: undefined".
test("4. the members-only line, the public list unsigned, the members' list signed, files by token", async () => {
  const publicAnswer = listAnswer([item({ id: 3 })], { members_view: false, categories: [], labels: [] });
  const out = await openPage({ myKey: "", session: null, routes: [[at("/api/media", "GET"), () => respond(200, publicAnswer)]] });
  assert.equal(text(byId(out.doc, "media-members-line")), "Sign in to this server to see its members' media.", "someone not signed in is told");
  assert.equal(out.net.calls.length, 1);
  assert.equal(out.net.calls[0].init.method, undefined, "the public list is a plain GET");
  assert.ok(out.net.calls[0].url.startsWith("/api/media?"), out.net.calls[0].url);
  assert.equal(out.signer.signed.length, 0, "nothing is signed without an identity");
  assert.equal(byId(out.doc, "media-add-btn"), null, "no Add to Media");
  assert.equal(byId(out.doc, "media-filter-picks"), null, "no My picks");
  assert.equal(out.doc.body.querySelectorAll("[data-action]").length, 0, "no Pick, Remove or Feature");
  assert.equal(text(byId(out.doc, "media-status").querySelector(".media-status-text")).startsWith("Not signed in on this page."), true);

  const membersAnswer = listAnswer([item({ id: 7, url: "/api/media/file/7", visibility: "members" }), item({ id: 6 })]);
  const inside = await openPage({ routes: listRoutes(membersAnswer) });
  const list = inside.net.calls.find((c) => c.url === "/api/media/list");
  assert.ok(list, "a signed-in person's list is signed (POST /api/media/list)");
  assert.equal(list.body.sig, "sig:media_list", "the list read is signed as media_list");
  assert.equal(list.body.key, ME);
  assert.equal(byId(inside.doc, "media-members-line"), null, "no line for a member");
  const img = byId(inside.doc, "media-grid").querySelector(".media-tile").querySelector("img");
  assert.equal(img.src, "/api/media/file/7?token=tok-1", "a members-only file comes through the token route");
  assert.ok(byId(inside.doc, "media-add-btn"), "Add to Media for a signed-in member");

  // Without the session's token yet, the members-only file is not requested at all.
  const early = await openPage({ session: fakeSession({ status: "connecting", uploadToken: "" }), routes: listRoutes(membersAnswer) });
  const first = byId(early.doc, "media-grid").querySelector(".media-tile");
  assert.equal(first.querySelectorAll("img").length, 0);
  assert.equal(text(first.querySelector(".media-thumb-missing")), media.MEDIA_WORDS.waitingToken);
});

// ── 5. Only this server's addresses ──────────────────────────────────────

// Seen red with ownFilePath accepting any address: "a picture from another website is not
// loaded / 1 !== 0".
test("5. a picture loads only from this server's own addresses", async () => {
  const answer = listAnswer([
    item({ id: 3, url: "https://elsewhere.example/x.png" }),
    item({ id: 2, url: "//elsewhere.example/uploads/x.png" }),
    item({ id: 1, url: "/uploads/../api/x.png" }),
  ]);
  const { doc } = await openPage({ routes: listRoutes(answer) });
  const tiles = byId(doc, "media-grid").querySelectorAll(".media-tile");
  assert.equal(tiles.length, 3);
  for (const t of tiles) {
    assert.equal(t.querySelectorAll("img").length, 0, "a picture from another website is not loaded");
    assert.equal(text(t.querySelector(".media-thumb-missing")), media.MEDIA_WORDS.notThisServer);
  }
  assert.equal(media.ownFilePath("/uploads/5_a.png", ORIGIN), "/uploads/5_a.png");
  assert.equal(media.ownFilePath(ORIGIN + "/uploads/5_a.png", ORIGIN), "/uploads/5_a.png");
  assert.equal(media.ownFilePath("/api/media/file/12", ORIGIN), "/api/media/file/12");
  assert.equal(media.ownFilePath("/api/media/file/12/../../x", ORIGIN), null);
  assert.equal(media.ownFilePath("javascript:alert(1)", ORIGIN), null);
});

// ── 6. Add to Media ──────────────────────────────────────────────────────

const mine = (n, size) => item({ id: 100 + n, uploader_key: ME, uploader_name: "Me", size_bytes: size });

function addRoutes(extra, gate) {
  const answer = listAnswer([mine(1, 60 * MIB), mine(2, 40 * MIB)]);
  // The person's own items (the share's use) can be held back until `gate` opens.
  const own = [(c) => c.url === "/api/media/list" && c.body && c.body.by === ME, async () => { if (gate) await gate; return respond(200, answer); }];
  return [own].concat(listRoutes(answer)).concat(extra || []);
}

async function pickFile(doc, file) {
  const input = byId(doc, "media-file-input");
  input.files = [file];
  input.dispatch("change");
  await settle();
}

// Seen red with openAddForm leaving out shareStop: "an upload over the share is stopped /
// actual: null"; with the form not waiting for the share (`checking` always false): "no upload
// while the share is being worked out / actual: false"; and with shareLeftWords showing the whole share as left: "'You have 512 MB of
// your 512 MB left' == 'You have 412 MB of your 512 MB left'".
test("6. Add to Media shows the share, stops an upload over it before it starts, uploads one within it", async () => {
  let open;
  const gate = new Promise((r) => { open = r; });
  const { doc, net, signer } = await openPage({ routes: addRoutes([
    [at("/api/upload", "POST"), () => respond(200, { url: "/uploads/1760_new.png", filename: "1760_new.png", size: 10 * MIB, type: "image/png" })],
    [at("/api/media", "POST"), (c) => respond(200, { ok: true, item: item({ id: 200, url: "/uploads/" + c.body.filename }) })],
  ], gate) });
  // Over the share: 100 MB used of 512 MB, a 413 MB file. While the share is being worked out,
  // nothing can be uploaded.
  await pickFile(doc, { name: "big.mp4", size: 413 * MIB });
  assert.equal(byId(doc, "media-form-go").disabled, true, "no upload while the share is being worked out");
  byId(doc, "media-form-go").disabled = false; // a determined click on a button that should not go
  byId(doc, "media-form-go").click();
  await settle();
  assert.equal(net.calls.filter((c) => path0(c.url) === "/api/upload").length, 0, "no upload while the share is being worked out");
  open();
  await settle();
  assert.equal(text(byId(doc, "media-share-line")), "You have 412 MB of your 512 MB left");
  const stop = byId(doc, "media-form-stop");
  assert.ok(stop, "an upload over the share is stopped");
  assert.match(text(stop), /^"big\.mp4" is 413 MB, more than the 412 MB you have left of your 512 MB share, so it was not uploaded\./);
  assert.equal(byId(doc, "media-form-go"), null, "nothing to press that would upload it");
  assert.equal(net.calls.filter((c) => path0(c.url) === "/api/upload").length, 0, "no upload request at all");
  byId(doc, "media-form-cancel").click();
  assert.equal(byId(doc, "media-form-stop"), null);

  // Within the share.
  await pickFile(doc, { name: "staff.png", size: 10 * MIB });
  assert.equal(byId(doc, "media-form-stop"), null);
  byId(doc, "media-form-title").value = "Spinning at dusk";
  byId(doc, "media-form-title").dispatch("input");
  const cat = byId(doc, "media-form-category");
  cat.value = "2";
  cat.dispatch("change");
  const flashing = byId(doc, "media-form-labels").querySelector('[data-label="flashing"]');
  flashing.checked = true;
  flashing.dispatch("change");
  byId(doc, "media-form-vis-members").checked = true;
  byId(doc, "media-form-vis-members").dispatch("change");
  byId(doc, "media-form-go").click();
  await settle();
  const up = net.calls.find((c) => path0(c.url) === "/api/upload");
  assert.equal(up.url, "/api/upload?token=tok-1&media=1", "uploaded with the session token as a Media upload");
  assert.deepEqual(up.init.body.parts.map((p) => p[0]), ["file"]);
  const pub = net.calls.find((c) => c.url === "/api/media" && c.init.method === "POST");
  assert.equal(pub.body.filename, "1760_new.png");
  assert.equal(pub.body.sig, "sig:media_publish:1760_new.png", "the publish is signed over its file name");
  assert.equal(pub.body.title, "Spinning at dusk");
  assert.equal(pub.body.category_id, 2);
  assert.deepEqual(pub.body.labels, ["flashing"]);
  assert.equal(pub.body.visibility, "members");
  assert.equal(byId(doc, "media-form-go"), null, "the form closes");
  assert.equal(text(byId(doc, "media-notice").querySelector(".media-notice-text")), media.MEDIA_WORDS.published);
  assert.ok(signer.signed.includes("media_publish:1760_new.png"));
});

// ── 7. Put on Media ──────────────────────────────────────────────────────

// Seen red with renderForm drawing Put on Media like Add to Media (its public-only branch
// skipped, so the Public / Members only choice was offered and the reason left out): "null ==
// 'This file is already public where it was shared.'".
test("7. Put on Media from a Shared Files row offers public only and says why; a members-only server offers no choice", async () => {
  const { doc, net } = await openPage({
    put: { filename: "1700_part.stl", name: "bushing.stl" },
    routes: listRoutes(listAnswer([])).concat([[at("/api/media", "POST"), () => respond(200, { ok: true, item: item() })]]),
  });
  assert.equal(text(byId(doc, "media-form").querySelector("h2")), "Put on Media");
  assert.equal(text(byId(doc, "media-form-public-only")), "This file is already public where it was shared.");
  assert.equal(byId(doc, "media-form-visibility"), null, "the visibility choice is not offered");
  assert.equal(byId(doc, "media-form-vis-members"), null);
  byId(doc, "media-form-go").click();
  await settle();
  assert.equal(net.calls.filter((c) => path0(c.url) === "/api/upload").length, 0, "no upload: the file is on the server already");
  const pub = net.calls.find((c) => c.url === "/api/media" && c.init.method === "POST");
  assert.equal(pub.body.visibility, "public");
  assert.equal(pub.body.filename, "1700_part.stl");
  assert.equal(pub.body.sig, "sig:media_publish:1700_part.stl");

  // A members-only server: Add to Media puts it on as members only, with no choice offered.
  const settings = Object.assign({}, fakeSession().state.settings, { media_visibility: "members" });
  const m = await openPage({ session: fakeSession({ settings }), routes: addRoutes([
    [at("/api/upload", "POST"), () => respond(200, { url: "/uploads/9_m.png", filename: "9_m.png" })],
    [at("/api/media", "POST"), () => respond(200, { ok: true, item: item() })],
  ]) });
  await pickFile(m.doc, { name: "m.png", size: MIB });
  assert.equal(byId(m.doc, "media-form-visibility"), null);
  assert.equal(text(byId(m.doc, "media-form-members-only")), media.MEDIA_WORDS.serverMembersOnly);
  byId(m.doc, "media-form-go").click();
  await settle();
  assert.equal(m.net.calls.find((c) => c.url === "/api/media" && c.init.method === "POST").body.visibility, "members");
});

// ── 8. Each signed purpose names its target ──────────────────────────────

// Seen red with purposes.pick dropping the id ("media_pick"): "pick is signed over its item /
// 'sig:media_pick' !== 'sig:media_pick:5'"; and with removeWords always saying the file stays:
// "a members-only item's file goes with it / ...Remove from Media. The file stays where it came
// from."
test("8. pick, unpick, remove and feature are each signed over their own item", async () => {
  const answer = listAnswer([item({ id: 5, picks: 0 }), item({ id: 6, uploader_key: ME, uploader_name: "Me", visibility: "members", url: "/api/media/file/6" })]);
  const routes = listRoutes(answer, [
    [at("/api/media/pick", "POST"), () => respond(200, { ok: true, id: 5, picked: true, picks: 1 })],
    [at("/api/media/unpick", "POST"), () => respond(200, { ok: true, id: 5, picked: false, picks: 0 })],
    [at("/api/media/remove", "POST"), () => respond(200, { ok: true, removed: 6, file_removed: true, file_kept: false })],
    [at("/api/media/feature", "POST"), () => respond(200, { ok: true, id: 5, featured: true })],
  ]);
  const { doc, net, confirms } = await openPage({ session: fakeSession({ role: "admin" }), routes });
  const tileOf = (id) => byId(doc, "media-grid").querySelectorAll(".media-tile").find((t) => t.dataset.id === String(id));
  assert.equal(tileOf(6).querySelector('[data-action="pick"]'), null, "no Pick on your own item");
  tileOf(5).querySelector('[data-action="pick"]').click();
  await settle();
  const last = () => net.calls.at(-1);
  assert.equal(last().url, "/api/media/pick");
  assert.equal(last().body.sig, "sig:media_pick:5", "pick is signed over its item");
  assert.equal(last().body.id, 5);
  assert.equal(text(tileOf(5).querySelector(".media-tile-meta")), "by Ann · 1 pick");
  tileOf(5).querySelector('[data-action="unpick"]').click();
  await settle();
  assert.equal(last().url, "/api/media/unpick");
  assert.equal(last().body.sig, "sig:media_pick:5", "unpick is signed over its item");
  tileOf(5).querySelector('[data-action="feature"]').click();
  await settle();
  assert.equal(last().url, "/api/media/feature");
  assert.equal(last().body.sig, "sig:media_feature:5");
  assert.equal(last().body.featured, true);
  assert.equal(text(byId(doc, "media-featured-row").querySelector(".media-tile-title")), "Contact staff", "a featured item joins the strip");
  tileOf(6).querySelector('[data-action="remove"]').click();
  await settle();
  assert.equal(confirms.at(-1), '"Contact staff"\n\n' + media.MEDIA_WORDS.removeDeletes, "a members-only item's file goes with it");
  assert.equal(last().url, "/api/media/remove");
  assert.equal(last().body.sig, "sig:media_remove:6");
  assert.equal(last().body.id, 6);
  assert.equal(tileOf(6), undefined, "the tile is gone");
  assert.equal(text(byId(doc, "media-notice").querySelector(".media-notice-text")), media.MEDIA_WORDS.removedDeleted);
  // A public item's answer does not say where its file came from, so the words cover both.
  assert.equal(media.removeWords(item()), media.MEDIA_WORDS.removeEither);
  assert.equal(media.removedWords({ file_kept: true }), "Removed from Media. The file stays where it came from.");
});

// Seen red with remove() not waiting for the confirmation: "a cancelled Remove sends nothing /
// 3 !== 2" (the removal went out).
test("8b. a cancelled Remove sends nothing", async () => {
  const { doc, net } = await openPage({ confirm: false, session: fakeSession({ role: "admin" }), routes: listRoutes(listAnswer([item({ id: 5 })])) });
  const before = net.calls.length;
  byId(doc, "media-grid").querySelector('[data-action="remove"]').click();
  await settle();
  assert.equal(net.calls.length, before, "a cancelled Remove sends nothing");
});

// ── 9. The admins' section ───────────────────────────────────────────────

const BLAKE = async (bytes) => new Uint8Array(require("node:crypto").createHash("sha256").update(Buffer.from(bytes)).digest());

async function openAdmin(opts) {
  const o = opts || {};
  const doc = fakeDocument();
  const box = doc.createElement("div");
  box.id = "media-admin-root";
  doc.body.appendChild(box);
  const signer = fakeSigner(ME);
  const usage = {
    used_bytes: Math.round(12.4 * 1024 * MIB), budget_bytes: 50 * 1024 * MIB, per_person_bytes: 512 * MIB,
    items: 3, people: [],
    largest: [item({ id: 11, title: "Big video", kind: "video", size_bytes: 3 * 1024 * MIB, uploader_name: "Ann" })],
    oldest: [item({ id: 12, title: "First" })],
    least_viewed: [],
    waiting: [{ filename: "1760_wait.mp4", uploader_key: ANN, uploader_name: "Ann", size_bytes: 40 * MIB, uploaded_at: 1, expires_at: 86400001 }],
  };
  const net = fakeFetch([
    [at("/api/media/list", "POST"), () => respond(200, listAnswer([], { categories: CATEGORIES }))],
    [at("/api/media/suggestions", "POST"), () => respond(200, { all: true, suggestions: [
      { id: 3, suggester_key: ANN, suggester_name: "Ann", name: "Builds", note: "Ship builds", created_at: 1, state: "open" },
      { id: 2, suggester_key: ANN, suggester_name: "Ann", name: "Old", note: "", created_at: 1, state: "declined" },
    ] })],
    [at("/api/media/suggestions/decide", "POST"), () => respond(200, { ok: true })],
    [at("/api/media/usage", "POST"), () => respond(200, usage)],
    [at("/api/media/remove", "POST"), () => respond(200, { ok: true, removed_upload: "1760_wait.mp4", file_removed: true, file_kept: false })],
    [at("/api/media/categories", "POST"), (c) => respond(200, { ok: true, categories: c.body.categories.map((x, i) => ({ id: x.id || 10 + i, name: x.name, description: x.description, position: i })) })],
  ]);
  const confirms = [];
  const session = o.session || fakeSession({ role: "admin" });
  const ctl = admin.mountMediaAdmin(box, {
    document: doc, fetch: net.fetch, sign: signer.sign, blake3: BLAKE, session,
    confirm: async (w) => { confirms.push(w); return true; },
  });
  await ctl.ready;
  await settle();
  return { doc, net, signer, session, confirms };
}

// Seen red with meterWords reading the budget as the used bytes: "the meter / '50 GB of 50 GB
// used'"; and with the members note left out of renderSettings: "the relay's sentence under the
// switch / actual: null". (That second break first failed as a TypeError: the section passed a
// null to append(), which a browser writes as the word "null". Fixed in media-admin.js, and the
// stand-in's append() now does what a browser does.)
test("9. the admins' section: settings with the members sentence, the meter, the prune lists, suggestions", async () => {
  const { doc, session } = await openAdmin();
  // Visibility, with the relay's own sentence under it.
  assert.equal(byId(doc, "media-admin-vis-public").checked, true);
  assert.equal(text(byId(doc, "media-admin-members-note")), "Members are everyone signed in to this server. To keep it to people you let in, turn on lockdown.", "the relay's sentence under the switch");
  assert.equal(byId(doc, "media-admin-budget").value, "2048");
  assert.equal(byId(doc, "media-admin-share").value, "512");
  assert.equal(byId(doc, "media-admin-previews").checked, true);
  byId(doc, "media-admin-vis-members").checked = true;
  byId(doc, "media-admin-vis-members").dispatch("change");
  byId(doc, "media-admin-budget").value = "51200";
  byId(doc, "media-admin-budget").dispatch("input");
  byId(doc, "media-admin-previews").checked = false;
  byId(doc, "media-admin-previews").dispatch("change");
  byId(doc, "media-admin-save").click();
  assert.deepEqual(session.sent.at(-1), {
    type: "server_settings_update", media_visibility: "members", media_budget_mb: 51200, media_per_person_mb: 512, media_public_previews_only: false,
  }, "the four settings in one server_settings_update");
  session.state.settings = Object.assign({}, session.state.settings, { media_visibility: "members", media_budget_mb: 51200, media_public_previews_only: false });
  session.emit({ type: "frame", msg: { type: "server_settings_state", settings: session.state.settings } });
  assert.equal(text(byId(doc, "media-admin-settings-msg")), "Saved.");
  // The meter.
  assert.equal(text(byId(doc, "media-admin-meter")), "12.4 GB of 50 GB used", "the meter");
  assert.equal(byId(doc, "media-admin-meter-bar").children[0].style.width, "24.8%");
  // The prune lists.
  assert.equal(text(byId(doc, "media-admin-largest").querySelector(".media-admin-row-text")), "Big video · 3 GB · by Ann");
  assert.equal(text(byId(doc, "media-admin-oldest").querySelector(".media-admin-row-text")), "First · 2 MB · by Ann");
  assert.equal(text(byId(doc, "media-admin-least-viewed").querySelector(".media-hint")), "Nothing here.");
  assert.match(text(byId(doc, "media-admin-waiting").querySelector(".media-admin-row-text")), /^1760_wait\.mp4 · 40 MB · by Ann · Deleted /);
  // The suggestions waiting, the suggester named; a decided one is not listed.
  const rows = byId(doc, "media-admin-suggestions").querySelectorAll("[data-suggestion]");
  assert.equal(rows.length, 1);
  assert.match(text(rows[0]), /Builds.*Suggested by Ann.*Ship builds/);
  // The categories, General without Remove.
  const catRows = byId(doc, "media-admin-cat-rows").querySelectorAll("[data-cat-row]");
  assert.equal(catRows.length, 2);
  assert.equal(catRows[0].querySelector("[data-cat-remove]"), null, "General stays");
  assert.ok(catRows[1].querySelector("[data-cat-remove]"));
});

// Seen red with categoriesBody re-serialising the list with its keys sorted, which the relay
// would hash as a different list: '[{"description":"","id":1,"name":"General"},...' ==
// '[{"id":1,"name":"General","description":""},...'.
test("9b. the admins' writes are each signed over their own target", async () => {
  const { doc, net, confirms } = await openAdmin();
  // Rename, add, reorder, then save: signed over the hash of the list text exactly as sent.
  const name1 = byId(doc, "media-admin-cat-rows").querySelector('[data-cat-name="1"]');
  name1.value = "Fire flow";
  name1.dispatch("input");
  byId(doc, "media-admin-cat-new-name").value = "Builds";
  byId(doc, "media-admin-cat-add").click();
  byId(doc, "media-admin-cat-rows").querySelector('[data-cat-up="2"]').click();
  byId(doc, "media-admin-cat-save").click();
  await settle();
  const save = net.calls.find((c) => c.url === "/api/media/categories");
  const raw = save.init.body;
  const listText = raw.slice(raw.indexOf(',"categories":') + ',"categories":'.length, -1);
  assert.equal(listText, '[{"id":1,"name":"General","description":""},{"name":"Builds","description":""},{"id":2,"name":"Fire flow","description":""}]');
  const hash = Buffer.from(await BLAKE(new TextEncoder().encode(listText))).toString("hex");
  assert.equal(save.body.sig, "sig:media_categories:" + hash, "the signature covers the list exactly as sent");
  assert.equal(text(byId(doc, "media-admin-cat-msg")), "Saved.");
  // Accept a suggestion.
  byId(doc, "media-admin-suggestions").querySelector('[data-decide="accept"]').click();
  await settle();
  const decide = net.calls.find((c) => c.url === "/api/media/suggestions/decide");
  assert.equal(decide.body.sig, "sig:media_suggestions:3:accept");
  assert.equal(decide.body.id, 3);
  assert.equal(decide.body.accept, true);
  // Remove an upload waiting to be published: by its file name.
  byId(doc, "media-admin-waiting").querySelector(".media-btn-danger").click();
  await settle();
  assert.equal(confirms.at(-1), '"1760_wait.mp4"\n\nRemove this upload and delete the file.');
  const rm = net.calls.find((c) => c.url === "/api/media/remove");
  assert.equal(rm.body.sig, "sig:media_remove:1760_wait.mp4");
  assert.equal(rm.body.filename, "1760_wait.mp4");
  assert.equal(rm.body.id, undefined);
  // The reads.
  assert.equal(net.calls.find((c) => c.url === "/api/media/usage").body.sig, "sig:media_usage");
  assert.equal(net.calls.find((c) => c.url === "/api/media/suggestions").body.sig, "sig:media_suggestions");
});

// Seen red with renderAdmin mounting for any signed-in person: "a member who is not an admin
// gets no admins' section / actual: [object]".
test("9c. the Media page shows the admins' section to admins only", async () => {
  const mounted = [];
  const member = await openPage({ mountAdmin: (c) => mounted.push(c), routes: listRoutes(listAnswer([])) });
  assert.equal(byId(member.doc, "media-admin"), null, "a member who is not an admin gets no admins' section");
  assert.equal(mounted.length, 0);
  const op = await openPage({ session: fakeSession({ role: "owner" }), mountAdmin: (c) => mounted.push(c), routes: listRoutes(listAnswer([])) });
  assert.ok(byId(op.doc, "media-admin"));
  assert.equal(mounted.length, 1);
  assert.equal(mounted[0].id, "media-admin-body");
});

// ── 10. The session ──────────────────────────────────────────────────────

let NOBLE = null;
async function noble() {
  if (NOBLE) return NOBLE;
  const src = fs.readFileSync(path.join(WEB, "shared", "vendor", "noble-pq.bundle.js"));
  NOBLE = await import("data:text/javascript;base64," + Buffer.from(src).toString("base64"));
  return NOBLE;
}

function fakeWebSocketClass(sockets) {
  class FakeWS {
    constructor(url) { this.url = url; this.readyState = 0; this.sent = []; this.listeners = {}; sockets.push(this); }
    addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); }
    fire(t, ev) { for (const f of this.listeners[t] || []) f(ev || {}); }
    send(s) { if (this.readyState !== 1) throw new Error("send on a socket that is not open"); this.sent.push(JSON.parse(s)); }
    close() { if (this.readyState === 3) return; this.readyState = 3; this.fire("close"); }
    open() { this.readyState = 1; this.fire("open"); }
    receive(o) { this.fire("message", { data: JSON.stringify(o) }); }
  }
  return FakeWS;
}

// Seen red with identifyPreimage leaving out the nonce: "the relay accepts the sign-in / actual:
// false"; and with takePeerList reading `token`: "the upload token from the person's own entry
// / '' !== 'tok-9'".
test("10. the session signs in with a real Dilithium3 signature and takes its own token, role and settings", async () => {
  const n = await noble();
  const seed = new Uint8Array(32).fill(7);
  const kp = n.ml_dsa65.keygen(n.blake3.create({ context: new TextEncoder().encode("hum/dilithium3/v1"), dkLen: 32 }).update(seed).digest());
  const identity = { dilithiumPublicHex: Buffer.from(kp.publicKey).toString("hex"), dilithiumSecret: kp.secretKey };
  const sockets = [];
  const storage = new Map([["humanity_name", "Shaostoul"]]);
  const s = relaySession.createRelaySession({
    getIdentity: async () => identity,
    signBytes: async (secret, bytes) => n.ml_dsa65.sign(bytes, secret),
    WebSocket: fakeWebSocketClass(sockets),
    location: { protocol: "https:", host: "united-humanity.us" },
    storage: { getItem: (k) => storage.get(k) || null },
    setTimeout: () => 0, clearTimeout: () => {},
  });
  const events = [];
  s.subscribe((ev) => events.push(ev));
  assert.equal(await s.start(), true);
  const ws = sockets[0];
  assert.equal(ws.url, "wss://united-humanity.us/ws");
  ws.open();
  assert.deepEqual(ws.sent, [{ type: "identify", public_key: identity.dilithiumPublicHex, display_name: "Shaostoul" }]);
  assert.equal(s.send({ type: "anything" }), false, "nothing else is sent before the relay accepts the sign-in");
  ws.receive({ type: "identify_challenge", nonce: "n0nce" });
  await settle();
  const resp = ws.sent[1];
  assert.equal(resp.type, "identify_response");
  const words = new TextEncoder().encode("hum/identify/v1\nn0nce\n" + identity.dilithiumPublicHex);
  assert.equal(n.ml_dsa65.verify(Buffer.from(resp.sig_b64, "base64"), words, kp.publicKey), true, "the relay accepts the sign-in");
  assert.equal(s.state.uploadToken, "");
  ws.receive({ type: "peer_list", peers: [
    { public_key: "f0".repeat(32), role: "admin", upload_token: null },
    { public_key: identity.dilithiumPublicHex, role: "admin", upload_token: "tok-9", upload_limit_mb: 100 },
  ] });
  assert.equal(s.state.status, "bound");
  assert.equal(s.state.uploadToken, "tok-9", "the upload token from the person's own entry");
  assert.equal(s.state.role, "admin");
  assert.equal(s.state.uploadLimitBytes, 100 * MIB);
  assert.deepEqual(ws.sent[2], { type: "server_settings_request" }, "it asks for the server's settings");
  ws.receive({ type: "server_settings_state", settings: { media_visibility: "members", media_per_person_mb: 64 } });
  assert.equal(s.state.settings.media_per_person_mb, 64);
  // A later peer list (someone joining) without a token keeps the token.
  ws.receive({ type: "peer_list", peers: [{ public_key: identity.dilithiumPublicHex, role: "admin" }] });
  assert.equal(s.state.uploadToken, "tok-9");
  // A dropped socket's token is gone.
  ws.close();
  assert.equal(s.state.status, "closed");
  assert.equal(s.state.uploadToken, "");
  assert.ok(events.some((e) => e.type === "frame" && e.msg.type === "server_settings_state"));

  // No identity here: no socket at all.
  const none = [];
  const empty = relaySession.createRelaySession({ getIdentity: async () => null, WebSocket: fakeWebSocketClass(none), location: { protocol: "https:", host: "x" } });
  assert.equal(await empty.start(), false);
  assert.equal(none.length, 0, "no identity, no socket");
  assert.equal(empty.state.status, "no_identity");
});

// ── The words ────────────────────────────────────────────────────────────

// Seen red with sizeWords keeping the ".0": "the design's own example / '12.4 GB of 50.0 GB used'".
test("11. sizes, the share and the meter read as the design writes them", () => {
  assert.equal(media.meterWords(Math.round(12.4 * 1024 * MIB), 50 * 1024 * MIB), "12.4 GB of 50 GB used", "the design's own example");
  assert.equal(media.shareLeftWords(100 * MIB, 512 * MIB), "You have 412 MB of your 512 MB left");
  assert.equal(media.shareLeftWords(600 * MIB, 512 * MIB), "You have 0 bytes of your 512 MB left");
  assert.equal(media.sizeWords(1.5 * MIB), "1.5 MB");
  assert.equal(media.sizeWords(900 * 1024), "900 KB");
  assert.equal(media.shareStop({ name: "a", size: 10 }, 0, 512 * MIB), null);
  assert.equal(media.limitStop({ name: "a", size: 2 * MIB }, MIB).startsWith('"a" is 2 MB, over your 1 MB limit'), true);
  // No em dash in anything the page or the section says.
  for (const w of Object.values(media.MEDIA_WORDS).concat(Object.values(admin.ADMIN_WORDS))) assert.ok(!w.includes("—"), w);
});

// The page is wired in: the nav beside Watch, its scripts in order, the Shared Files row link.
// Seen red with the /media link taken out of shell.js: "the nav links the page / false".
test("12. the page is in the nav beside Watch and loads its scripts in order", () => {
  const shell = fs.readFileSync(path.join(WEB, "shared", "shell.js"), "utf8");
  assert.ok(/mobileLink\('\/watch',\s*'Watch'\)\s*\+[\s\S]{0,300}?mobileLink\('\/media',\s*'Media'\)/.test(shell), "the nav links the page");
  assert.ok(/p\.startsWith\('\/media'\)/.test(shell), "the page lights its tab");
  const html = fs.readFileSync(path.join(WEB, "pages", "media.html"), "utf8");
  const order = ["/shared/hold-confirm.js", "/chat/pq.js", "/shared/pq-relay-auth.js", "/shared/relay-session.js", "/pages/media-app.js", "/pages/media-admin.js"];
  const at2 = order.map((s) => html.indexOf('src="' + s));
  assert.ok(at2.every((i) => i > 0), "every script is loaded: " + at2);
  assert.deepEqual(at2.slice().sort((a, b) => a - b), at2, "in order");
  assert.ok(html.includes('id="media-root"'));
  const shared = fs.readFileSync(path.join(WEB, "pages", "shared-files.html"), "utf8");
  assert.ok(shared.includes("'/media?put=' + encodeURIComponent("), "a Shared Files row links to Put on Media");
  assert.ok(/f\.uploader_key !== myKey/.test(shared), "on the person's own rows only");
});
