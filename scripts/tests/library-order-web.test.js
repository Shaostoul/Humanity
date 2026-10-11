// The web Library lists its shelves A to Z, or in the catalog's suggested order (2026-10-10).
//
// Run: node --test scripts/tests/library-order-web.test.js
//
// The operator: "we want to sort the Library alphabetically cause it's hard to search through
// while not alphabetical." Both clients now list the shelves within each section, and the
// documents on each shelf, A to Z by default: by title, ignoring case and a leading "The ", "A "
// or "An " (titles are shown as written). Sections keep their order. A choice at the top of the
// tree switches to Suggested, the catalog's own order (the learning path), and is remembered.
// The desktop app's rule is rail_order() in src/gui/pages/library.rs, unit-tested there with the
// SAME sample and the same expected orders as section 1 here, so the two cannot drift apart
// silently.
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), in
// library.html's order (library-order.js, then library-app.js), with the DOM, fetch, history and
// localStorage replaced by small stand-ins: a rail element whose innerHTML is parsed just enough
// to find the buttons the page wires and to read back the order it drew. HOS_WEB_DIR points the
// test at another copy of web/ (used to see each test red).
//
// What it proves:
//  1. The rule (library-order.js): the article and case rule, A to Z within each section with
//     the catalog indices kept, sections in place, Suggested = the catalog's order.
//  2. The real manifest (data/library/index.json): Suggested is exactly the catalog; A to Z is a
//     reordering of every shelf, sorted by the key, with the sections untouched.
//  3. library.html loads library-order.js before library-app.js.
//  4. The page draws A to Z by default with "A to Z" pressed; pressing "Suggested" redraws the
//     catalog order and keeps the choice in localStorage; a later visit starts from the kept
//     choice; with storage refused the page still draws, A to Z.
//
// Red first, 2026-10-10: see the end of this file for each deliberate break and what it tripped.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const order = require(path.join(WEB, "pages", "library-order.js"));

// ── 1. The rule ─────────────────────────────────────────────────────────

// The same sample as the native tests in src/gui/pages/library.rs.
const SAMPLE = {
  sections: ["Learn", "Build"],
  categories: [
    { name: "Where You Are", section: "Learn",
      docs: ["Why Seasons Happen", "The Climate Where You Live", "an Ecosystem", "Reading the Tide"] },
    { name: "Grow Food", section: "Learn", docs: ["Your First Tomato", "Starting Seeds", "A Bed of Soil", "apples"] },
    { name: "The Accord", section: "Learn", docs: ["Preamble"] },
    { name: "Zebra Shelf", section: "Build", docs: ["only"] },
    { name: "Alpha Shelf", section: "Build", docs: ["Theory", "Then", "Anchors", "The"] },
  ].map((c) => ({ ...c, docs: c.docs.map((t) => ({ title: t, file: t.replace(/ /g, "_") + ".md", tags: [] })) })),
};

/** The titles in the order the rail would draw them, section by section. */
function drawn(manifest, suggested) {
  return order.railOrder(manifest, suggested).map((g) => [
    g.section,
    g.cats.map((x) => [manifest.categories[x.ci].name, x.dis.map((di) => manifest.categories[x.ci].docs[di].title)]),
  ]);
}

test("1a. the key sets aside a leading The, A or An and ignores case", () => {
  assert.strictEqual(order.titleSortKey("The Climate Where You Live"), "climate where you live");
  assert.strictEqual(order.titleSortKey("A Bed of Soil"), "bed of soil");
  assert.strictEqual(order.titleSortKey("an Ecosystem"), "ecosystem");
  assert.strictEqual(order.titleSortKey("  THE   Accord "), "accord");
  // Only a whole word: these keep their first letters.
  assert.strictEqual(order.titleSortKey("Theory"), "theory");
  assert.strictEqual(order.titleSortKey("Anchors"), "anchors");
  assert.strictEqual(order.titleSortKey("The"), "the");
});

test("1b. A to Z sorts the shelves and documents and keeps the sections in place", () => {
  assert.deepStrictEqual(drawn(SAMPLE, false), [
    ["Learn", [
      ["The Accord", ["Preamble"]],
      ["Grow Food", ["apples", "A Bed of Soil", "Starting Seeds", "Your First Tomato"]],
      ["Where You Are", ["The Climate Where You Live", "an Ecosystem", "Reading the Tide", "Why Seasons Happen"]],
    ]],
    ["Build", [
      ["Alpha Shelf", ["Anchors", "The", "Then", "Theory"]],
      ["Zebra Shelf", ["only"]],
    ]],
  ]);
  // The indices are the catalog's, so a click opens the right document.
  const learn = order.railOrder(SAMPLE, false)[0];
  assert.deepStrictEqual(learn.cats[1], { ci: 1, dis: [3, 2, 1, 0] });
});

test("1c. Suggested is the catalog's order", () => {
  assert.deepStrictEqual(order.railOrder(SAMPLE, true), [
    { section: "Learn", cats: [{ ci: 0, dis: [0, 1, 2, 3] }, { ci: 1, dis: [0, 1, 2, 3] }, { ci: 2, dis: [0] }] },
    { section: "Build", cats: [{ ci: 3, dis: [0] }, { ci: 4, dis: [0, 1, 2, 3] }] },
  ]);
});

// ── 2. The real manifest ────────────────────────────────────────────────

test("2. on the real Library, Suggested is the catalog and A to Z sorts every shelf", () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "library", "index.json"), "utf8"));
  const cats = manifest.categories;
  const declared = manifest.sections.filter((s) => cats.some((c) => c.section === s));

  const sug = order.railOrder(manifest, true);
  assert.deepStrictEqual(sug.map((g) => g.section).filter((s) => cats.some((c) => c.section === s)), declared);
  assert.deepStrictEqual(
    sug.flatMap((g) => g.cats.map((x) => x.ci)),
    cats.map((_, ci) => ci).sort((a, b) => declared.indexOf(cats[a].section) - declared.indexOf(cats[b].section) || a - b),
  );
  for (const g of sug) for (const x of g.cats) assert.deepStrictEqual(x.dis, cats[x.ci].docs.map((_, i) => i));

  const az = order.railOrder(manifest, false);
  assert.deepStrictEqual(az.map((g) => g.section), sug.map((g) => g.section), "sections keep their order");
  // The test's own statement of the rule, so this cannot pass by agreeing with a broken key.
  const key = (t) => t.trim().toLowerCase().replace(/^(the|a|an) +/, "");
  const sorted = (keys) => keys.every((k, i) => i === 0 || keys[i - 1] <= k);
  for (const g of az) {
    assert.ok(sorted(g.cats.map((x) => key(cats[x.ci].name))), `shelves of ${g.section} are A to Z`);
    assert.deepStrictEqual(g.cats.map((x) => x.ci).sort((a, b) => a - b), cats.flatMap((c, ci) => (c.section === g.section ? [ci] : [])));
    for (const x of g.cats) {
      const docs = cats[x.ci].docs;
      assert.ok(sorted(x.dis.map((di) => key(docs[di].title))), `${cats[x.ci].name} is A to Z`);
      assert.deepStrictEqual(x.dis.slice().sort((a, b) => a - b), docs.map((_, i) => i), "every document, once");
    }
  }
  // The point of it: Grow Food no longer reads in planting order.
  const grow = az.flatMap((g) => g.cats).find((x) => cats[x.ci].name === "Grow Food");
  if (grow) assert.notDeepStrictEqual(grow.dis, cats[grow.ci].docs.map((_, i) => i));
});

// ── 3. library.html ─────────────────────────────────────────────────────

test("3. library.html loads library-order.js before library-app.js", () => {
  const html = fs.readFileSync(path.join(WEB, "pages", "library.html"), "utf8");
  const a = html.indexOf('<script src="/pages/library-order.js"></script>');
  const b = html.indexOf('<script src="/pages/library-app.js"></script>');
  assert.ok(a >= 0, "library.html loads /pages/library-order.js");
  assert.ok(b > a, "library-order.js comes before library-app.js");
});

// ── 4. The page ─────────────────────────────────────────────────────────

/** Read the start tags out of an HTML string: just enough DOM for the rail. */
function parseTags(html, doc) {
  const out = [];
  const re = /<([a-z]+)\b([^>]*)>/gi;
  let m;
  while ((m = re.exec(html))) {
    const attrs = {};
    const ar = /([a-zA-Z-]+)(?:="([^"]*)")?/g;
    let a;
    while ((a = ar.exec(m[2]))) attrs[a[1]] = a[2] === undefined ? "" : a[2];
    out.push(makeEl(doc, attrs));
  }
  return out;
}

function matches(el, sel) {
  const attr = sel.match(/^\[([a-z-]+)(?:="([^"]*)")?\]$/i);
  if (attr) return attr[2] === undefined ? attr[1] in el.attrs : el.attrs[attr[1]] === attr[2];
  if (sel.startsWith(".")) {
    const have = (el.attrs.class || "").split(/\s+/);
    return sel.slice(1).split(".").every((c) => have.includes(c));
  }
  return false;
}

function makeEl(doc, attrs) {
  const el = {
    attrs: attrs || {},
    listeners: {},
    kids: [],
    html: "",
    hidden: false,
    style: { display: "", top: "", setProperty() {} },
    classList: { add() {}, remove() {}, toggle() {}, contains: () => false },
    offsetParent: null,
    offsetHeight: 0,
    get innerHTML() { return this.html; },
    set innerHTML(v) { this.html = String(v); this.kids = parseTags(this.html, doc); },
    querySelectorAll(sel) { return this.kids.filter((k) => matches(k, sel)); },
    querySelector(sel) { return this.querySelectorAll(sel)[0] || null; },
    contains(x) { return x === this || this.kids.includes(x); },
    addEventListener(type, fn) { (this.listeners[type] = this.listeners[type] || []).push(fn); },
    getAttribute(n) { return n in this.attrs ? this.attrs[n] : null; },
    setAttribute(n, v) { this.attrs[n] = String(v); },
    getBoundingClientRect: () => ({ top: 0, bottom: 0, left: 0, right: 0, width: 0, height: 0 }),
    focus() { doc.activeElement = this; },
    blur() {},
    select() {},
    scrollIntoView() {},
    click() { (this.listeners.click || []).forEach((f) => f({ preventDefault() {} })); },
  };
  return el;
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

/** Boot library.html's scripts against `manifest`, as a browser would, and wait for it to settle. */
async function bootPage(manifest, ls) {
  const doc = { activeElement: null, listeners: {} };
  const ids = {};
  for (const id of ["lib-rail", "lib-rail-wrap", "lib-content", "lib-tags", "lib-search", "lib-results"]) {
    ids[id] = makeEl(doc, { id });
  }
  doc.activeElement = makeEl(doc, {});
  Object.assign(doc, {
    documentElement: { style: { setProperty() {} } },
    getElementById: (id) => ids[id] || null,
    querySelector: () => null,
    querySelectorAll: () => [],
    addEventListener(type, fn) { (this.listeners[type] = this.listeners[type] || []).push(fn); },
  });
  const files = {
    "/data/library/index.json": manifest,
    "/data/curriculum/syllabus.json": { subjects: [], topics: [] },
  };
  const ctx = {
    document: doc,
    localStorage: ls,
    location: { hash: "" },
    history: { pushState() {}, replaceState() {} },
    console: { log() {}, warn() {}, error() {} },
    setTimeout: (f) => setTimeout(f, 0),
    scrollY: 0,
    innerHeight: 800,
    addEventListener() {},
    scrollTo() {},
    getComputedStyle: () => ({ position: "static", getPropertyValue: () => "" }),
    fetch: async (url) => {
      const u = String(url);
      if (u in files) return { ok: true, json: async () => JSON.parse(JSON.stringify(files[u])) };
      if (u.startsWith("/data/library/")) return { ok: true, text: async () => "# A document\n\nSome words." };
      return { ok: false, status: 404 };
    },
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  for (const f of ["library-order.js", "library-app.js"]) {
    vm.runInContext(fs.readFileSync(path.join(WEB, "pages", f), "utf8"), ctx, { filename: f });
  }
  (doc.listeners.DOMContentLoaded || []).forEach((f) => f());
  for (let i = 0; i < 10; i++) await new Promise((r) => setImmediate(r));
  return { rail: ids["lib-rail"], ctx };
}

/** The documents the rail drew, in order, as "Shelf: Title". */
function railTitles(rail, manifest) {
  const out = [];
  const re = /data-ci="(\d+)" data-di="(\d+)">([^<]*)</g;
  let m;
  while ((m = re.exec(rail.innerHTML))) out.push(manifest.categories[+m[1]].name + ": " + m[3]);
  return out;
}

const AZ_TITLES = [
  "The Accord: Preamble",
  "Grow Food: apples", "Grow Food: A Bed of Soil", "Grow Food: Starting Seeds", "Grow Food: Your First Tomato",
  "Where You Are: The Climate Where You Live", "Where You Are: an Ecosystem",
  "Where You Are: Reading the Tide", "Where You Are: Why Seasons Happen",
  "Alpha Shelf: Anchors", "Alpha Shelf: The", "Alpha Shelf: Then", "Alpha Shelf: Theory",
  "Zebra Shelf: only",
];
const CATALOG_TITLES = SAMPLE.categories.flatMap((c) => c.docs.map((d) => c.name + ": " + d.title));

function pressed(rail) {
  return rail.querySelectorAll("[data-order]").map((b) => [b.getAttribute("data-order"), b.getAttribute("aria-pressed")]);
}

test("4a. the page draws A to Z by default, and Suggested is kept once chosen", async () => {
  const ls = storage();
  const { rail } = await bootPage(SAMPLE, ls);
  assert.deepStrictEqual(railTitles(rail, SAMPLE), AZ_TITLES);
  assert.deepStrictEqual(pressed(rail), [["az", "true"], ["suggested", "false"]]);
  const sug = rail.querySelector('[data-order="suggested"]');
  assert.strictEqual(sug.getAttribute("title"), order.ORDER_TIP_SUGGESTED, "the chip says what Suggested is");

  sug.click();
  assert.deepStrictEqual(railTitles(rail, SAMPLE), CATALOG_TITLES);
  assert.deepStrictEqual(pressed(rail), [["az", "false"], ["suggested", "true"]]);
  assert.strictEqual(ls.map.get("hos_library_order"), "suggested");

  rail.querySelector('[data-order="az"]').click();
  assert.deepStrictEqual(railTitles(rail, SAMPLE), AZ_TITLES);
  assert.strictEqual(ls.map.get("hos_library_order"), "az");
});

test("4b. a later visit starts from the kept choice", async () => {
  const { rail } = await bootPage(SAMPLE, storage({ hos_library_order: "suggested" }));
  assert.deepStrictEqual(railTitles(rail, SAMPLE), CATALOG_TITLES);
  assert.deepStrictEqual(pressed(rail), [["az", "false"], ["suggested", "true"]]);
});

test("4c. with storage refused the page still draws, A to Z, and the chips still work", async () => {
  const { rail } = await bootPage(SAMPLE, storage({}, true));
  assert.deepStrictEqual(railTitles(rail, SAMPLE), AZ_TITLES);
  rail.querySelector('[data-order="suggested"]').click();
  assert.deepStrictEqual(railTitles(rail, SAMPLE), CATALOG_TITLES);
});

// Red first, 2026-10-10, each break made in a copy of web/pages (HOS_WEB_DIR), the rest passing:
//  - library-order.js, titleSortKey without toLowerCase(): 1a, 1b, 2, 4a and 4c failed (lowercase
//    "apples" filed after every capital letter, and "  THE   Accord " kept its "the"). Test 2
//    passed this break until it stated the rule itself instead of calling titleSortKey.
//  - library-order.js, ARTICLES without 'the ': 1a, 1b, 2, 4a and 4c failed ("The Accord" filed
//    under T).
//  - library-app.js, renderRail calling railOrder(manifest, false) whatever the choice: 4a, 4b and
//    4c failed (Suggested still drew A to Z).
//  - library.html without the library-order.js script tag: 3 failed.
