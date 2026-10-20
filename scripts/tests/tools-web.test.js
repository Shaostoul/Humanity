// The web Tools page mirrors the desktop app's Tools page (2026-10-19, v0.1520.0): a library
// shelf in Grid or List, search, sort by Name or Category, the kind and category pills, a plain
// tile per entry, and an entry's details beside the shelf. The app's page is
// src/gui/pages/tools.rs (with tools_library.rs and tools_detail.rs); the web's is
// web/pages/tools.html and web/pages/tools-app.js. Both read data/external/catalog.json.
//
// Run: node --test scripts/tests/tools-web.test.js
//
// tools-app.js runs as it does in the browser, against a small stand-in document that holds ONLY
// what the page built in it: reading or setting innerHTML throws (the page writes the catalog's
// words as text, never as HTML), and append() writes anything that is not a node as text, as a
// browser does (so a stray null shows up as the word "null", the way it did on screen once). The
// network is a stand-in that answers the catalog; localStorage is a Map, or one that throws as a
// locked-down browser does. HOS_WEB_DIR points the test at another copy of web/ (used to see each
// test red).
//
// What it proves:
//  1. The catalog reads as the app's parse() reads it: one flat list, each entry carrying its
//     category's name, id, file types and kind; the real file has every entry, once.
//  2. Sort: by Name (letter case ignored, then the category as written, then catalog place) and
//     by Category (then by name), on a sample with the ties that matter and on the real catalog.
//  3. Search: every word, letter case ignored, in the name, the category or the description, and
//     nowhere else (as the app searches).
//  4. The kind and category pills: counts on the kind pills, the category pills only of the kind
//     chosen, a kind chosen lets go of the category, and the shelf shows what they narrow to.
//  5. The view survives a reload: Grid or List, the sort and both filters are kept in
//     localStorage and the next visit starts from them; storage refused, the page still works; a
//     kept value that is garbage, or a category no longer in the catalog, falls back.
//  6. The details: for a website-only entry (DaVinci Resolve), its Website link alone, the
//     catalog's reason why the desktop app has no Install for it, its description and facts; for
//     a program with a download page, a browser program and a help service, what each shows; and
//     never the word "null".
//  7. Tiles are buttons that open the details; the main button is Download (the download page)
//     or Open website, a link that opens in a new tab and sends no referrer. Escape closes the
//     details and gives the focus back to the tile.
//  8. The page's lines are the app's (read out of src/gui/pages/tools.rs), and it says plainly
//     what only the desktop app can do, with a link to /download. No logo: no image anywhere.
//  9. A category's hue is the FNV-1a hash the app's swatch_color uses, worked out here in BigInt
//     arithmetic as a second statement of it.
//
// Red first, 2026-10-19: see the end of this file for each deliberate break and what it tripped.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const tools = require(path.join(WEB, "pages", "tools-app.js"));

const CATALOG = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "external", "catalog.json"), "utf8"));
const HTML = fs.readFileSync(path.join(WEB, "pages", "tools.html"), "utf8");
const RUST = fs.readFileSync(path.join(ROOT, "src", "gui", "pages", "tools.rs"), "utf8");

/** A `pub const NAME: &str = "...";` string out of the app's tools.rs. */
function rustConst(name) {
  const m = RUST.match(new RegExp("pub const " + name + ': &str = "((?:[^"\\\\]|\\\\.)*)";'));
  assert.ok(m, "tools.rs has " + name);
  return m[1].replace(/\\"/g, '"');
}

// ── A document that holds only what was built in it ─────────────────────

class FakeText {
  constructor(t) { this.nodeType = 3; this._t = String(t); this.parentNode = null; }
  get textContent() { return this._t; }
  set textContent(v) { this._t = String(v); }
}

class FakeElement {
  constructor(doc, tag) {
    this.ownerDocument = doc;
    this.nodeType = 1;
    this.tagName = String(tag).toUpperCase();
    this.childNodes = [];
    this.parentNode = null;
    this.attributes = new Map();
    this.listeners = {};
    this.className = "";
    this.hidden = false;
    this._value = "";
    const props = {};
    this.style = { setProperty: (k, v) => { props[k] = String(v); }, getPropertyValue: (k) => props[k] || "" };
    const self = this;
    this.classList = {
      contains: (c) => self.className.split(/\s+/).includes(c),
      add: (c) => { if (!self.classList.contains(c)) self.className = (self.className + " " + c).trim(); },
      remove: (c) => { self.className = self.className.split(/\s+/).filter((x) => x && x !== c).join(" "); },
      toggle: (c, on) => { const want = on === undefined ? !self.classList.contains(c) : !!on; if (want) self.classList.add(c); else self.classList.remove(c); return want; },
    };
  }
  get id() { return this.getAttribute("id") || ""; }
  get children() { return this.childNodes.filter((c) => c.nodeType === 1); }
  get textContent() { return this.childNodes.map((c) => c.textContent).join(""); }
  set textContent(v) { this.replaceChildren(); if (String(v)) this.appendChild(new FakeText(v)); }
  get innerHTML() { throw new Error("the page read innerHTML"); }
  set innerHTML(v) { throw new Error("the page set innerHTML: " + String(v).slice(0, 80)); }
  get value() { return this._value; }
  set value(v) { this._value = String(v); }
  appendChild(c) {
    if (c.parentNode) c.parentNode.removeChild(c);
    c.parentNode = this;
    this.childNodes.push(c);
    return c;
  }
  // As a browser does: anything that is not a node is written as text (a null as "null").
  append(...cs) { for (const c of cs) this.appendChild(c && typeof c === "object" && c.nodeType ? c : new FakeText(String(c))); }
  removeChild(c) {
    const i = this.childNodes.indexOf(c);
    if (i >= 0) this.childNodes.splice(i, 1);
    c.parentNode = null;
    return c;
  }
  replaceChildren(...cs) {
    for (const c of this.childNodes.slice()) this.removeChild(c);
    this.append(...cs);
  }
  setAttribute(k, v) { this.attributes.set(k, String(v)); }
  getAttribute(k) { return this.attributes.has(k) ? this.attributes.get(k) : null; }
  removeAttribute(k) { this.attributes.delete(k); }
  addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); }
  dispatch(type, extra) {
    const ev = Object.assign({ type, target: this, preventDefault() {}, stopPropagation() {} }, extra || {});
    for (const f of (this.listeners[type] || []).slice()) f(ev);
  }
  click() { this.dispatch("click"); }
  focus() { this.ownerDocument.activeElement = this; }
  scrollIntoView() {}
  matches(sel) {
    let m;
    if ((m = /^#([\w-]+)$/.exec(sel))) return this.id === m[1];
    if ((m = /^\.([\w-]+)$/.exec(sel))) return this.classList.contains(m[1]);
    if ((m = /^\[([\w-]+)(?:="([^"]*)")?\]$/.exec(sel))) return m[2] === undefined ? this.attributes.has(m[1]) : this.getAttribute(m[1]) === m[2];
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
    listeners: {},
    activeElement: null,
    createElement: (tag) => new FakeElement(doc, tag),
    createTextNode: (t) => new FakeText(t),
    addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); },
    dispatch(type, extra) {
      const ev = Object.assign({ type, preventDefault() {} }, extra || {});
      for (const f of (this.listeners[type] || []).slice()) f(ev);
    },
  };
  doc.body = new FakeElement(doc, "body");
  doc.getElementById = (id) => doc.body.querySelector("#" + id);
  return doc;
}

/** Storage that keeps what it is given, or (refuse) throws as a locked-down browser does. */
function storage(initial, refuse) {
  const m = new Map(Object.entries(initial || {}));
  const no = () => { throw new Error("SecurityError: storage is refused"); };
  return {
    map: m,
    getItem: (k) => (refuse ? no() : (m.has(k) ? m.get(k) : null)),
    setItem: (k, v) => (refuse ? no() : m.set(k, String(v))),
    removeItem: (k) => (refuse ? no() : m.delete(k)),
  };
}

/** Boot the page against `catalog` with `ls` as its storage, as a browser would; wait for it. */
async function boot(ls, catalog) {
  const doc = fakeDocument();
  const host = doc.createElement("div");
  host.setAttribute("id", "tools-app");
  doc.body.appendChild(host);
  const page = tools.createToolsPage({
    document: doc,
    root: host,
    storage: ls,
    fetch: async (url) => (url === tools.DATA_URL
      ? { ok: true, json: async () => JSON.parse(JSON.stringify(catalog || CATALOG)) }
      : { ok: false, status: 404 }),
  });
  await page.start();
  return { doc, host, page };
}

// What the shelf shows, read back from the document.
const shelfOf = (host) => host.querySelector("#tools-shelf");
const isList = (host) => !!shelfOf(host).querySelector("table");
function shownNames(host) {
  const shelf = shelfOf(host);
  if (shelf.querySelector("table")) return shelf.querySelectorAll(".tool-row-name").map((b) => b.textContent);
  return shelf.querySelectorAll(".tool-tile").map((t) => t.querySelector(".tool-name").textContent);
}
/** The pills of a group and which one is pressed, as [words, pressed]. */
const pillsOf = (group) => group.children.map((b) => [b.textContent, b.getAttribute("aria-pressed")]);
const pressedIn = (group) => (group.children.find((b) => b.getAttribute("aria-pressed") === "true") || { textContent: null }).textContent;
function pill(group, startsWith) {
  const b = group.children.find((x) => x.textContent.startsWith(startsWith));
  assert.ok(b, "a pill starting " + startsWith + " in " + JSON.stringify(pillsOf(group)));
  return b;
}
const openButton = (host, name) => {
  const b = shelfOf(host).querySelectorAll("[data-open]").find((x) => x.textContent.includes(name) && (x.querySelector(".tool-name") || x).textContent === name);
  assert.ok(b, "a tile or row for " + name);
  return b;
};

// A small catalog with the ties that matter: the same name (letter case aside) in two
// categories, a category whose name starts lower case, a browser program and a help service.
const SAMPLE = {
  kinds: [{ id: "software", label: "Software" }, { id: "service", label: "Help and services" }],
  categories: [
    { id: "paint", name: "Painting", kind: "software", extensions: [".kra"], entries: [
      { name: "zebra Paint", description: "Paint stripes.", url: "https://zebra.example/", license: "GPL", platforms: ["windows", "linux"], size: "~10MB", winget: "Zebra.Paint" },
      { name: "Apple Draw", description: "Sketch fruit.", url: "https://apple.example/", license: "MIT", platforms: ["web"], size: "Web app" },
    ] },
    { id: "audio", name: "audio Tools", kind: "software", extensions: [".wav"], entries: [
      { name: "Banjo", description: "Record music.", url: "https://banjo.example/", download: "https://banjo.example/get", license: "GPL", platforms: ["windows"], size: "~5MB", winget: "Banjo.Banjo" },
      { name: "apple Draw", description: "Draw sound waves.", url: "https://apple-audio.example/", license: "BSD", platforms: ["mac"], size: "~1MB" },
    ] },
    { id: "food", name: "Food", kind: "service", entries: [
      { name: "Cupboard", description: "Find a food bank near you.", url: "https://www.cupboard.example/find" },
    ] },
  ],
};
const SAMPLE_ENTRIES = tools.entriesOf(SAMPLE);
const view = (over) => Object.assign({ layout: "grid", sort: "name", kind: null, category: null }, over || {});

// ── 1. The catalog ──────────────────────────────────────────────────────

test("1. the catalog reads as the app reads it: a flat list, each entry with its category", () => {
  assert.deepStrictEqual(SAMPLE_ENTRIES.map((e) => [e.i, e.name, e.category, e.categoryId, e.kind]), [
    [0, "zebra Paint", "Painting", "paint", "software"],
    [1, "Apple Draw", "Painting", "paint", "software"],
    [2, "Banjo", "audio Tools", "audio", "software"],
    [3, "apple Draw", "audio Tools", "audio", "software"],
    [4, "Cupboard", "Food", "food", "service"],
  ]);
  assert.deepStrictEqual(SAMPLE_ENTRIES[0].extensions, [".kra"], "the category's file types travel with its entries");
  assert.deepStrictEqual(SAMPLE_ENTRIES[4].extensions, []);

  const all = tools.entriesOf(CATALOG);
  const expected = CATALOG.categories.reduce((n, c) => n + c.entries.length, 0);
  assert.strictEqual(all.length, expected, "every entry of the real catalog");
  assert.ok(all.length >= 100, "the real catalog has its hundred or so entries");
  assert.ok(all.every((e, i) => e.i === i && e.category && e.categoryId && (e.kind === "software" || e.kind === "service")));
  assert.deepStrictEqual(tools.kindsOf(all), ["software", "service"]);
  const resolve = all.find((e) => e.name === "DaVinci Resolve");
  assert.ok(resolve.websiteOnly && resolve.websiteOnly.includes("Blackmagic"), "website_only is read");
});

// ── 2. Sort ─────────────────────────────────────────────────────────────

test("2a. by Name: letter case ignored, then the category as written, then catalog place", () => {
  const names = (order) => order.map((i) => SAMPLE_ENTRIES[i].name + " (" + SAMPLE_ENTRIES[i].category + ")");
  assert.deepStrictEqual(names(tools.orderOf(SAMPLE_ENTRIES, view(), "")), [
    "Apple Draw (Painting)", "apple Draw (audio Tools)", "Banjo (audio Tools)", "Cupboard (Food)", "zebra Paint (Painting)",
  ]);
});

test("2b. by Category: the category, letter case ignored, then by name", () => {
  assert.deepStrictEqual(tools.orderOf(SAMPLE_ENTRIES, view({ sort: "category" }), ""), [3, 2, 4, 1, 0]);
  assert.deepStrictEqual(tools.groupsOf(SAMPLE_ENTRIES, [3, 2, 4, 1, 0]).map((g) => [g.category, g.items]), [
    ["audio Tools", [3, 2]], ["Food", [4]], ["Painting", [1, 0]],
  ]);
});

test("2c. on the real catalog, both orders hold every entry once and are sorted by the rule", () => {
  const all = tools.entriesOf(CATALOG);
  // The test's own statement of the rule, so this cannot pass by agreeing with a broken order.
  const key = (e) => [e.name.toLowerCase(), e.category, e.i];
  const le = (a, b) => { for (let k = 0; k < a.length; k++) { if (a[k] < b[k]) return true; if (a[k] > b[k]) return false; } return true; };
  const byName = tools.orderOf(all, view(), "");
  assert.deepStrictEqual(byName.slice().sort((a, b) => a - b), all.map((e) => e.i));
  for (let k = 1; k < byName.length; k++) assert.ok(le(key(all[byName[k - 1]]), key(all[byName[k]])), "by name at " + k);
  const byCat = tools.orderOf(all, view({ sort: "category" }), "");
  const ckey = (e) => [e.category.toLowerCase()].concat(key(e));
  for (let k = 1; k < byCat.length; k++) assert.ok(le(ckey(all[byCat[k - 1]]), ckey(all[byCat[k]])), "by category at " + k);
});

test("2d. on the page: Sort pills Name and Category (no Installed first), and List headings sort too", async () => {
  const { host, page } = await boot(storage(), SAMPLE);
  assert.deepStrictEqual(pillsOf(page.parts.sortGroup), [["Name", "true"], ["Category", "false"]]);
  assert.deepStrictEqual(shownNames(host), ["Apple Draw", "apple Draw", "Banjo", "Cupboard", "zebra Paint"]);
  pill(page.parts.sortGroup, "Category").click();
  // The Grid sorted by category: each category under its name, with how many.
  const heads = shelfOf(host).querySelectorAll(".tools-group").map((h) => h.textContent);
  assert.deepStrictEqual(heads, ["audio Tools2", "Food1", "Painting2"]);
  assert.deepStrictEqual(shownNames(host), ["apple Draw", "Banjo", "Cupboard", "Apple Draw", "zebra Paint"]);
  pill(page.parts.layoutGroup, "List").click();
  assert.ok(isList(host));
  const nameHead = shelfOf(host).querySelector('[data-sort="name"]');
  nameHead.click();
  assert.strictEqual(pressedIn(page.parts.sortGroup), "Name", "a List heading sorts the shelf");
  assert.deepStrictEqual(shownNames(host), ["Apple Draw", "apple Draw", "Banjo", "Cupboard", "zebra Paint"]);
  const th = shelfOf(host).querySelectorAll("th").map((t) => [t.textContent, t.getAttribute("aria-sort")]);
  assert.deepStrictEqual(th, [["Link", null], ["Name", "ascending"], ["Category", null], ["Licence", null], ["Runs on", null], ["Size", null]]);
});

// ── 3. Search ───────────────────────────────────────────────────────────

test("3. search: every word, letter case ignored, in the name, category or description only", () => {
  const find = (q) => tools.orderOf(SAMPLE_ENTRIES, view(), q);
  assert.deepStrictEqual(find("draw"), [1, 3], "in the name");
  assert.deepStrictEqual(find("APPLE sketch"), [1], "every word, case ignored");
  assert.deepStrictEqual(find("audio"), [3, 2], "in the category");
  assert.deepStrictEqual(find("  food   bank "), [4], "in the description, spaces anywhere");
  assert.deepStrictEqual(find("gpl"), [], "not the licence");
  assert.deepStrictEqual(find("windows"), [], "not the platforms");
  assert.deepStrictEqual(find(".wav"), [], "not the file types");
});

test("3b. on the page: typing narrows the shelf, and nothing found says what was searched", async () => {
  const { host, page } = await boot(storage(), SAMPLE);
  page.parts.search.value = "Draw";
  page.parts.search.dispatch("input");
  assert.deepStrictEqual(shownNames(host), ["Apple Draw", "apple Draw"]);
  assert.strictEqual(page.parts.status.textContent, "Showing 2 of 5.");
  page.parts.search.value = "telescope";
  page.parts.search.dispatch("input");
  assert.deepStrictEqual(shownNames(host), []);
  assert.strictEqual(shelfOf(host).textContent, 'Nothing here matches "telescope".');
});

// ── 4. The kind and category pills ──────────────────────────────────────

test("4. the kind pills count, the category pills follow the kind, and a kind lets go of the category", async () => {
  const { host, page } = await boot(storage(), SAMPLE);
  assert.deepStrictEqual(pillsOf(page.parts.kindsRow), [["Everything 5", "true"], ["Software 4", "false"], ["Help and services 1", "false"]]);
  assert.deepStrictEqual(pillsOf(page.parts.catsRow).map((p) => p[0]), ["All", "Painting", "audio Tools", "Food"]);

  pill(page.parts.kindsRow, "Help and services").click();
  assert.deepStrictEqual(pillsOf(page.parts.catsRow).map((p) => p[0]), ["All", "Food"], "only the kind's categories");
  assert.deepStrictEqual(shownNames(host), ["Cupboard"]);

  pill(page.parts.kindsRow, "Software").click();
  pill(page.parts.catsRow, "audio Tools").click();
  assert.deepStrictEqual(shownNames(host), ["apple Draw", "Banjo"]);
  assert.strictEqual(pressedIn(page.parts.catsRow), "audio Tools");

  pill(page.parts.kindsRow, "Everything").click();
  assert.strictEqual(page.state.view.category, null, "a kind chosen lets go of the category");
  assert.strictEqual(pressedIn(page.parts.catsRow), "All");
  assert.strictEqual(shownNames(host).length, 5);

  // On the real catalog the counts are the file's.
  const real = await boot(storage());
  const all = tools.entriesOf(CATALOG);
  const n = (k) => all.filter((e) => e.kind === k).length;
  assert.deepStrictEqual(pillsOf(real.page.parts.kindsRow).map((p) => p[0]), [
    "Everything " + all.length, "Software " + n("software"), "Help and services " + n("service"),
  ]);
});

// ── 5. The view survives a reload ───────────────────────────────────────

test("5a. Grid or List, the sort and both filters are kept, and the next visit starts from them", async () => {
  const ls = storage();
  const first = await boot(ls, SAMPLE);
  assert.ok(!isList(first.host), "a first visit shows the Grid");
  pill(first.page.parts.layoutGroup, "List").click();
  pill(first.page.parts.sortGroup, "Category").click();
  pill(first.page.parts.kindsRow, "Software").click();
  pill(first.page.parts.catsRow, "Painting").click();
  assert.ok(ls.map.has(tools.VIEW_KEY), "the view is kept");
  assert.deepStrictEqual(JSON.parse(ls.map.get(tools.VIEW_KEY)), { layout: "list", sort: "category", kind: "software", category: "paint" });

  const again = await boot(ls, SAMPLE);
  assert.ok(isList(again.host), "the reload shows the List");
  assert.strictEqual(pressedIn(again.page.parts.layoutGroup), "List");
  assert.strictEqual(pressedIn(again.page.parts.sortGroup), "Category");
  assert.strictEqual(pressedIn(again.page.parts.kindsRow), "Software 4");
  assert.strictEqual(pressedIn(again.page.parts.catsRow), "Painting");
  assert.deepStrictEqual(shownNames(again.host), ["Apple Draw", "zebra Paint"]);

  pill(again.page.parts.layoutGroup, "Grid").click();
  const third = await boot(ls, SAMPLE);
  assert.ok(!isList(third.host), "and back to the Grid, kept too");
});

test("5b. storage refused: the page still draws, Grid by name, and the pills still work", async () => {
  const { host, page } = await boot(storage({}, true), SAMPLE);
  assert.deepStrictEqual(shownNames(host), ["Apple Draw", "apple Draw", "Banjo", "Cupboard", "zebra Paint"]);
  pill(page.parts.layoutGroup, "List").click();
  assert.ok(isList(host));
});

test("5c. a kept view that is garbage, or names a category no longer in the catalog, falls back", async () => {
  const garbage = await boot(storage({ [tools.VIEW_KEY]: "{not json" }), SAMPLE);
  assert.ok(!isList(garbage.host));
  assert.strictEqual(pressedIn(garbage.page.parts.sortGroup), "Name");

  const odd = await boot(storage({ [tools.VIEW_KEY]: JSON.stringify({ layout: "carousel", sort: "installed_first", kind: 7 }) }), SAMPLE);
  assert.deepStrictEqual(odd.page.state.view, { layout: "grid", sort: "name", kind: null, category: null });

  const gone = await boot(storage({ [tools.VIEW_KEY]: JSON.stringify({ layout: "list", sort: "name", kind: "software", category: "telescopes" }) }), SAMPLE);
  assert.strictEqual(gone.page.state.view.category, null, "a category no longer there is let go");
  assert.strictEqual(gone.page.state.view.kind, "software", "the kind is kept");
  assert.strictEqual(shownNames(gone.host).length, 4);

  // A category of another kind than the kept kind is let go too.
  assert.strictEqual(tools.fitView({ kind: "service", category: "paint" }, SAMPLE_ENTRIES).category, null);
});

// ── 6. The details ──────────────────────────────────────────────────────

/** The details panel as plain facts. */
function detailsShown(host) {
  const d = host.querySelector("#tools-details");
  if (d.hidden) return null;
  return {
    title: d.querySelector("h2").textContent,
    category: d.querySelector(".tool-detail-cat").textContent,
    links: d.querySelectorAll(".tool-btn").map((a) => [a.textContent, a.getAttribute("href"), a.getAttribute("target"), a.getAttribute("rel")]),
    note: (d.querySelector(".tool-note") || { textContent: null }).textContent,
    description: d.querySelector(".tool-desc").textContent,
    facts: d.querySelectorAll("dt").map((dt, k) => [dt.textContent, d.querySelectorAll("dd")[k].textContent]),
    comesFrom: !!d.querySelector(".tool-comes-from"),
    text: d.textContent,
  };
}

test("6a. a website-only entry: its Website alone, why the desktop app has no Install, its facts", async () => {
  const { host, page } = await boot(storage());
  assert.strictEqual(host.querySelector("#tools-details").hidden, true, "no details until a tile is chosen");
  openButton(host, "DaVinci Resolve").click();
  const all = tools.entriesOf(CATALOG);
  const e = all.find((x) => x.name === "DaVinci Resolve");
  const raw = CATALOG.categories.flatMap((c) => c.entries).find((x) => x.name === "DaVinci Resolve");
  const d = detailsShown(host);
  assert.strictEqual(d.title, "DaVinci Resolve");
  assert.strictEqual(d.category, e.category + "Close");
  assert.deepStrictEqual(d.links, [["Website", raw.url, "_blank", "noopener noreferrer"]], "no Download page: the catalog names none");
  assert.strictEqual(d.note, "The desktop app has no Install for it. " + raw.website_only, "the catalog's reason, word for word");
  assert.strictEqual(d.description, raw.description);
  assert.deepStrictEqual(d.facts, [
    ["Licence", raw.license],
    ["Runs on", "Windows, Mac, Linux"],
    ["Download size", raw.size],
    ["Opens", e.extensions.join(", ")],
    ["Website", "blackmagicdesign.com"],
  ]);
  assert.ok(d.comesFrom);
  assert.ok(!d.text.includes("null"), "never the word null");
  assert.ok(host.classList.contains("has-details"), "the shelf makes room for the details");
  assert.strictEqual(page.state.selected, e.i);
});

test("6b. a program with a download page, a browser program and a help service", async () => {
  const { host } = await boot(storage(), SAMPLE);
  openButton(host, "Banjo").click();
  let d = detailsShown(host);
  assert.deepStrictEqual(d.links.map((l) => [l[0], l[1]]), [["Website", "https://banjo.example/"], ["Download page", "https://banjo.example/get"]]);
  assert.strictEqual(d.note, null, "in winget: nothing to explain");
  assert.ok(d.comesFrom);
  assert.ok(!d.text.includes("null"), "never the word null");

  openButton(host, "zebra Paint").click();
  assert.deepStrictEqual(detailsShown(host).links.map((l) => l[0]), ["Website"], "no Download page when it is the website");

  openButton(host, "apple Draw").click();
  assert.strictEqual(detailsShown(host).note, "The desktop app has no Install for it: it is not in winget, Windows' own package manager, so Download opens its maker's own page.");

  openButton(host, "Apple Draw").click();
  d = detailsShown(host);
  assert.deepStrictEqual(d.links.map((l) => l[0]), ["Website"]);
  assert.strictEqual(d.note, null, "a browser program: nothing to install");
  assert.deepStrictEqual(d.facts, [["Licence", "MIT"], ["Runs on", "Web browser"], ["Opens", ".kra"], ["Website", "apple.example"]], "no download size");
  assert.ok(!d.comesFrom);

  openButton(host, "Cupboard").click();
  d = detailsShown(host);
  assert.deepStrictEqual(d.links.map((l) => [l[0], l[1]]), [["Open website", "https://www.cupboard.example/find"]]);
  assert.deepStrictEqual(d.facts, [["Website", "cupboard.example"]]);
  assert.ok(!d.comesFrom && d.note === null);
  assert.ok(!d.text.includes("null"), "never the word null");
});

// ── 7. Tiles, the main button, and Escape ──────────────────────────────

test("7a. a tile: one button that opens the details, its lines, and its main button", async () => {
  const { host } = await boot(storage(), SAMPLE);
  const tiles = shelfOf(host).querySelectorAll(".tool-tile");
  const read = (t) => ({
    open: t.querySelector(".tool-open").tagName + "/" + t.querySelector(".tool-open").getAttribute("type"),
    lines: [".tool-name", ".tool-meta", ".tool-line"].map((s) => t.querySelector(s).textContent),
    go: [t.querySelector(".tool-go").textContent, t.querySelector(".tool-go").getAttribute("href"), t.querySelector(".tool-go").getAttribute("target"), t.querySelector(".tool-go").getAttribute("rel")],
  });
  const byName = Object.fromEntries(tiles.map((t) => [t.querySelector(".tool-name").textContent, read(t)]));
  assert.deepStrictEqual(byName.Banjo, {
    open: "BUTTON/button",
    lines: ["Banjo", "audio Tools · GPL", "Windows · ~5MB"],
    go: ["Download", "https://banjo.example/get", "_blank", "noopener noreferrer"],
  });
  assert.deepStrictEqual(byName["Apple Draw"].lines, ["Apple Draw", "Painting · MIT", "Runs in your web browser"]);
  assert.deepStrictEqual(byName["Apple Draw"].go.slice(0, 2), ["Open website", "https://apple.example/"]);
  assert.deepStrictEqual(byName.Cupboard.lines, ["Cupboard", "Food", "cupboard.example"], "a help service: its website");
  assert.deepStrictEqual(byName.Cupboard.go.slice(0, 2), ["Open website", "https://www.cupboard.example/find"]);
  // Each tile's cover in its category's hue; no picture anywhere (no logo is shipped or fetched).
  const cover = tiles[0].querySelector(".tool-cover");
  assert.strictEqual(cover.style.getPropertyValue("--tool-hue"), String(tools.hueOf("paint")));
  assert.strictEqual(host.querySelectorAll("img").length, 0);

  const open = openButton(host, "Banjo");
  assert.strictEqual(open.getAttribute("aria-label"), "Banjo, audio Tools · GPL, Windows · ~5MB", "named by its name first, then its lines");
  assert.strictEqual(open.getAttribute("aria-expanded"), "false");
  open.click();
  assert.strictEqual(openButton(host, "Banjo").getAttribute("aria-expanded"), "true");
  assert.ok(shelfOf(host).querySelectorAll(".tool-tile").find((t) => t.classList.contains("selected")).textContent.includes("Banjo"));
});

test("7b. Escape closes the details and gives the focus back to the tile; Close does the same", async () => {
  const { doc, host } = await boot(storage(), SAMPLE);
  openButton(host, "Banjo").click();
  assert.strictEqual(doc.activeElement, host.querySelector("#tools-details"), "the focus moves to the details");
  doc.dispatch("keydown", { key: "Escape" });
  assert.strictEqual(host.querySelector("#tools-details").hidden, true, "Escape closes them");
  assert.ok(!host.classList.contains("has-details"));
  assert.strictEqual(doc.activeElement, openButton(host, "Banjo"), "the focus goes back to the tile");

  openButton(host, "Cupboard").click();
  host.querySelector(".tool-close").click();
  assert.strictEqual(host.querySelector("#tools-details").hidden, true, "Close closes them");
  assert.strictEqual(doc.activeElement, openButton(host, "Cupboard"));
  doc.dispatch("keydown", { key: "Escape" });
  assert.strictEqual(host.querySelector("#tools-details").hidden, true, "Escape with nothing open does nothing");
});

test("7c. a catalog that will not load says so", async () => {
  const doc = fakeDocument();
  const host = doc.createElement("div");
  doc.body.appendChild(host);
  const page = tools.createToolsPage({ document: doc, root: host, storage: storage(), fetch: async () => ({ ok: false, status: 500 }) });
  const errors = [];
  const was = console.error;
  console.error = (...a) => errors.push(a);
  try { await page.start(); } finally { console.error = was; }
  assert.strictEqual(shelfOf(host).textContent, tools.WORDS.loadFailed);
  assert.strictEqual(errors.length, 1);
});

// ── 8. The page's lines ─────────────────────────────────────────────────

test("8. the page's lines are the app's, and it says what only the desktop app can do", () => {
  const text = (s) => s.replace(/<[^>]+>/g, "").replace(/&#39;/g, "'").replace(/\s+/g, " ");
  const page = text(HTML);
  for (const name of ["INTRO", "SAFETY_LINE", "NOT_AFFILIATED"]) {
    assert.ok(page.includes(rustConst(name)), "tools.html says the app's " + name);
  }
  assert.ok(page.includes("The desktop app can also find the programs on your computer, start them, and install them after you agree to their licence."), "the line about the desktop app");
  assert.match(HTML, /<a href="\/download">[^<]+<\/a>/, "with a link to /download");
  assert.match(HTML, /href="\/library#2026-10-19-tools-page-install-launch-and-icons"/, "Why opens the finding in the Library");
  assert.ok(HTML.includes('<script src="/pages/tools-app.js'), "tools.html loads tools-app.js");
  assert.ok(!/<img\b/i.test(HTML), "no logo or picture");
  // The colours are the theme's: no colour literal in the page's styles.
  const style = HTML.slice(HTML.indexOf("<style>"), HTML.indexOf("</style>"));
  assert.ok(!/#[0-9a-f]{3,8}\b|rgba?\(/i.test(style), "no hand-written colour in tools.html");
  // The words on the tiles and in the details are the app's.
  for (const [k, name] of [["download", "DOWNLOAD"], ["openWebsite", "OPEN_WEBSITE"], ["website", "WEBSITE"], ["downloadPage", "DOWNLOAD_PAGE"], ["close", "CLOSE"], ["searchHint", "SEARCH_HINT"], ["everything", "EVERYTHING"], ["all", "ALL"]]) {
    assert.strictEqual(tools.WORDS[k], rustConst(name), name);
  }
  assert.ok(!/Install/.test(tools.SORTS.map((s) => s.words).join(" ")), "no Installed first on the web");
});

// ── 9. A category's hue ─────────────────────────────────────────────────

test("9. a category's hue is the app's FNV-1a swatch hue", () => {
  // FNV-1a, 32 bits, over the id's UTF-8 bytes (src/gui/widgets/mod.rs swatch_color), here in
  // BigInt arithmetic.
  const fnv = (s) => {
    let h = 2166136261n;
    for (const b of Buffer.from(s, "utf8")) { h ^= BigInt(b); h = (h * 16777619n) % 4294967296n; }
    return Number(h % 360n);
  };
  for (const id of ["3d-modeling", "image-editing", "water", "food", "a", "", "été"]) {
    assert.strictEqual(tools.hueOf(id), fnv(id), id);
  }
  const hues = new Set(CATALOG.categories.map((c) => tools.hueOf(c.id)));
  assert.ok(hues.size > CATALOG.categories.length / 2, "the categories spread across the hues");
});

// Red first, 2026-10-19, each break made in a copy of web/pages (HOS_WEB_DIR) and this file run
// against it, every other test passing; each passes again on the real web/:
//  - orderOf sorting by name whatever the sort: 2b, 2c and 2d failed (by Category gave the name
//    order).
//  - the search looking at the name alone: 3 failed ("every word, case ignored": "APPLE sketch"
//    found nothing, its second word being in the description).
//  - a kind pill keeping the category: 4 failed ("a kind chosen lets go of the category").
//  - writeView writing nothing: 5a failed ("the view is kept").
//  - readView without its try/catch: 5b failed (the boot threw "SecurityError: storage is
//    refused").
//  - renderDetails appending its parts without dropping the nulls: 6b failed ("never the word
//    null"), the bug the first browser check showed on screen under GIMP's links. 6a stayed
//    green: DaVinci Resolve has a note and a closing line, so nothing it appends is null.
//  - detailsOf without the website-only reason: 6a failed ("the catalog's reason, word for word").
//  - the Escape listener not added: 7b failed ("Escape closes them").
//  - tools.html without the desktop app's line: 8 failed ("the line about the desktop app").
//  - hueOf hashing UTF-16 code units instead of UTF-8 bytes: 9 failed on "été". (A first
//    try, dropping the `>>> 0` after Math.imul, stayed green, rightly: the hash is the same mod
//    2^32 and the last step makes it unsigned, so that was no break at all.)
