// The web Apps page mirrors the desktop app's Apps page (src/gui/pages/apps.rs and apps/, 2026-10-19:
// the Games library and the Tools page made one, in six coloured parts). The website's Tools page
// became it on 2026-10-20 (web/pages/apps.html and apps-app.js; the old names tools.html,
// tools-app.js, /tools and hos_tools_view are gone, and so is this test's old name,
// tools-web.test.js). It reads the files the app reads: data/external/catalog.json and
// data/gui/humanityos_tools.json.
//
// Run: node --test scripts/tests/apps-web.test.js
//
// apps-app.js runs as it does in the browser, against a small stand-in document that holds ONLY
// what the page built in it: reading or setting innerHTML throws (the page writes the data's words
// as text, never as HTML), and append() writes anything that is not a node as text, as a browser
// does (so a stray null shows up as the word "null", the way it did on screen once). The network
// is a stand-in that answers the two data files; localStorage is a Map, or one that throws as a
// locked-down browser does. HOS_WEB_DIR points the test at another copy of web/ (used to see each
// test red).
//
// What it proves:
//  1. The data reads as the app reads it: the catalog as one flat list, each entry carrying its
//     category's name, id, file types, kind and section; HumanityOS's own tools in the file's order.
//  2. THE SIX PARTS, in the app's order, each under a band with its name and its colour: the ids,
//     names, colours, chip hover words and catalog filters are the app's own (read out of
//     apps.rs), and the page draws them in that order.
//  3. HELP AND SERVICES ARE SPLIT BY THE DATA: each takes the service categories whose "section"
//     says so, and moving a category in the data moves its entries.
//  4. THE GREEN TILES' LINKS: every tool in the shipped file has a tile; each opens the website's
//     page for it, and that page exists; the two the website has no page for say "In the desktop
//     app" and link to /download; a page id the table does not know is noticed.
//  5. The toolbar: Sort offers Category only with a catalog part shown; a chip per part with its
//     count (the parts the website cannot list carry their name alone); a chip narrows to its
//     part and pressed again lets go.
//  6. Search and sort across the parts, as the app's order functions.
//  7. THE VIEW SURVIVES A RELOAD under the renamed key, hos_apps_view: Grid or List, the sort, the
//     part and each catalog part's category; its fields are the app's `apps_view` fields; storage
//     refused, the page still works; a kept value that is garbage, or a category no longer in the
//     catalog, falls back.
//  8. Tiles are short: the name once (on the cover), one line, a button with a fact; a tool's cover
//     carries its page's icon. The details, Escape and Close.
//  9. The page's lines are the app's, word for word: the safety line, both not-affiliated lines,
//     Help's and Services' lines, the intro, the search hint, where programs come from (up to the
//     Install a web page lacks) and its Why (the finding and the hover words); under the parts
//     stays only what the app keeps outside its fold.
// 10. THE NAV ENTRY: Apps beside Play in the website's menu and in the phone menu, with a four
//     squares icon, lit on /apps; nothing on the website links to /tools any more; the Justfile's
//     rig-tests runs this file.
// 11. A hue is the FNV-1a hash the app's swatch_color uses.
// 12. HOW THIS PAGE WORKS (2026-10-20, the app's widgets/how_it_works.rs, apps.rs draw_head and
//     apps/library.rs how_it_works; web/shared/how-it-works.js): one small toggle beside the
//     heading, a real button whose words and hover words are the app's LABEL and TIP, closed by
//     default; closed, where the programs come from and Services' line are nowhere on the page
//     while the safety line, both not-affiliated lines and Help's line stay in their parts; open,
//     the fold holds those two under their parts' names and colours, the Why with them;
//     aria-expanded follows; it is kept under hos_apps_how_open and survives a reload, storage
//     refused it still opens and closes, a kept value that is not "1" is closed; the fold follows
//     the parts shown; its styles are theme variables only.
//
// Red first, 2026-10-19 (the Tools tests kept from tools-web.test.js) and 2026-10-20 (the Apps
// tests): see the end of this file for each deliberate break and what it tripped.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const apps = require(path.join(WEB, "pages", "apps-app.js"));

const CATALOG = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "external", "catalog.json"), "utf8"));
const TOOLS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "gui", "humanityos_tools.json"), "utf8"));
// Every text file read with its line ends as LF: a Windows checkout has CRLF in the working copy.
const readText = (p) => fs.readFileSync(p, "utf8").replace(/\r\n/g, "\n");
const HTML = readText(path.join(WEB, "pages", "apps.html"));
const SHELL = readText(path.join(WEB, "shared", "shell.js"));
const ICONS = readText(path.join(WEB, "shared", "icons.js"));

// The desktop app's words: its Apps page and that page's parts.
const rust = (...p) => readText(path.join(ROOT, "src", "gui", "pages", ...p));
const APPS_RS = rust("apps.rs");
const LIBRARY_RS = rust("apps", "library.rs");
const RUST = [APPS_RS, rust("apps", "programs.rs"), rust("apps", "games.rs")].join("\n");

const unescapeRust = (s) => s.replace(/\\"/g, '"').replace(/\\\\/g, "\\");

/** A `pub const NAME: &str = "...";` string out of the app's Apps page (`from`: one file). */
function rustConst(name, from) {
  const m = (from || RUST).match(new RegExp("pub const " + name + ': &str = "((?:[^"\\\\]|\\\\.)*)";'));
  assert.ok(m, "the Apps page has " + name);
  return unescapeRust(m[1]);
}

/** The arms of `fn <name>(self)` in apps.rs's `impl <type>`: { Variant: value as written }. */
function rustArms(type, fnName) {
  const impl = APPS_RS.indexOf("impl " + type + " {");
  const start = APPS_RS.indexOf("pub fn " + fnName + "(self)", impl);
  assert.ok(impl >= 0 && start > impl, "apps.rs has " + type + "::" + fnName);
  const body = APPS_RS.slice(start, APPS_RS.indexOf("\n    }\n", start));
  const out = {};
  // An arm: one to three variants, then a quoted string (commas and all), a `Some((...))`, or a
  // value up to its comma.
  const arm = type + "::(\\w+)(?: \\| " + type + "::(\\w+))?(?: \\| " + type + "::(\\w+))? => (\"(?:[^\"\\\\]|\\\\.)*\"|Some\\(\\(.*?\\)\\)|[^,\\n]+),";
  for (const m of body.matchAll(new RegExp(arm, "g"))) {
    const value = m[4].trim();
    for (const v of [m[1], m[2], m[3]].filter(Boolean)) out[v] = value.startsWith('"') ? unescapeRust(value.slice(1, -1)) : value;
  }
  return out;
}

/** The app's parts in their order (`Section::ALL`), as variant names. */
const RUST_SECTIONS = (() => {
  const m = APPS_RS.match(/pub const ALL: \[Section; 6\] = \[([^\]]+)\];/);
  assert.ok(m, "apps.rs lists Section::ALL");
  return m[1].split(",").map((s) => s.trim().replace("Section::", ""));
})();
/** serde's snake_case of a one-word variant name: the id config.json keeps. */
const snake = (v) => v.replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase();

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
  get firstChild() { return this.childNodes[0] || null; }
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
  contains(n) { for (let x = n; x; x = x.parentNode) if (x === this) return true; return false; }
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

/** A stand-in hosIcon: the SVG /shared/icons.js would give for a name it knows. */
const fakeIcon = (name, size, colour) => '<svg viewBox="0 0 48 48" width="' + size + '" height="' + size + '"><g stroke="' + colour + '"><path d="M0,0" data-icon="' + name + '"/></g></svg>';

/** Boot the page against `catalog` and `tools` with `ls` as its storage, as a browser would; wait
    for it. `answer(url)`: a response other than the data file's (a failure). */
async function boot(ls, catalog, tools, more) {
  const doc = fakeDocument();
  const host = doc.createElement("div");
  host.setAttribute("id", "apps-app");
  doc.body.appendChild(host);
  const o = more || {};
  const page = apps.createAppsPage({
    document: doc,
    root: host,
    storage: ls,
    icon: o.icon,
    fetch: async (url) => {
      if (o.answer && o.answer(url)) return o.answer(url);
      if (url === apps.CATALOG_URL) return { ok: true, json: async () => JSON.parse(JSON.stringify(catalog || CATALOG)) };
      if (url === apps.TOOLS_URL) return { ok: true, json: async () => JSON.parse(JSON.stringify(tools || TOOLS)) };
      return { ok: false, status: 404 };
    },
  });
  await page.start();
  return { doc, host, page };
}

// What the parts show, read back from the document.
const partsOf = (host) => host.querySelector("#apps-parts");
const partEl = (host, id) => partsOf(host).querySelector('[data-part="' + id + '"]');
const shownParts = (host) => partsOf(host).querySelectorAll(".apps-part").map((p) => p.getAttribute("data-part"));
const isList = (host, id) => !!partEl(host, id).querySelector("table");
function namesIn(host, id) {
  const part = partEl(host, id);
  if (!part) return null;
  if (part.querySelector("table")) return part.querySelectorAll(".app-row-name").map((b) => b.textContent);
  return part.querySelectorAll(".app-tile").map((t) => t.querySelector(".app-cover-band").textContent);
}
/** The pills of a group and which one is pressed, as [words, pressed]. */
const pillsOf = (group) => group.children.map((b) => [b.textContent, b.getAttribute("aria-pressed")]);
const pressedIn = (group) => (group.children.find((b) => b.getAttribute("aria-pressed") === "true") || { textContent: null }).textContent;
function pill(group, startsWith) {
  const b = group.children.find((x) => x.textContent.startsWith(startsWith));
  assert.ok(b, "a pill starting " + startsWith + " in " + JSON.stringify(pillsOf(group)));
  return b;
}
const catsOf = (host, id) => partEl(host, id).querySelector('[data-cats="' + id + '"]');
const openButton = (host, name) => {
  const b = partsOf(host).querySelectorAll("[data-open]").find((x) => {
    const band = x.querySelector(".app-cover-band");
    return (band ? band.textContent : x.textContent) === name;
  });
  assert.ok(b, "a tile or row for " + name);
  return b;
};

// A small catalog with the ties that matter: the same name (letter case aside) in two
// categories, a category whose name starts lower case, a browser program, and help services in
// two sections.
const SAMPLE = {
  categories: [
    { id: "paint", name: "Painting", kind: "software", extensions: [".kra"], entries: [
      { name: "zebra Paint", description: "Paint stripes.", url: "https://zebra.example/", license: "GPL", platforms: ["windows", "linux"], size: "~10MB", winget: "Zebra.Paint" },
      { name: "Apple Draw", description: "Sketch fruit.", url: "https://apple.example/", license: "MIT", platforms: ["web"], size: "Web app" },
    ] },
    { id: "audio", name: "audio Tools", kind: "software", extensions: [".wav"], entries: [
      { name: "Banjo", description: "Record music.", url: "https://banjo.example/", download: "https://banjo.example/get", license: "GPL", platforms: ["windows"], size: "~5MB", winget: "Banjo.Banjo" },
      { name: "apple Draw", description: "Draw sound waves.", url: "https://apple-audio.example/", license: "BSD", platforms: ["mac"], size: "~1MB" },
    ] },
    { id: "food", name: "Food", kind: "service", section: "services", entries: [
      { name: "Cupboard", description: "Find a food bank near you.", url: "https://www.cupboard.example/find" },
    ] },
    { id: "crisis", name: "Crisis", kind: "service", section: "help", entries: [
      { name: "Lifeline", description: "Call any time.", url: "https://lifeline.example/" },
    ] },
    { id: "law", name: "Legal", kind: "service", section: "help", entries: [
      { name: "Counsel", description: "Free legal aid.", url: "https://counsel.example/" },
    ] },
  ],
};
const SAMPLE_TOOLS = { tools: [
  { name: "Maps", page: "maps", line: "Every map in one place." },
  { name: "Studio", page: "studio", line: "Stream live from this computer." },
  { name: "Notes", page: "notes", line: "Notes, written and kept here." },
] };
const SAMPLE_ENTRIES = apps.entriesOf(SAMPLE);
const sec = (id) => apps.SECTIONS.find((s) => s.id === id);

// ── 1. The data ─────────────────────────────────────────────────────────

test("1. the data reads as the app reads it: the catalog flat, each entry with its category and section; the tools in order", () => {
  assert.deepStrictEqual(SAMPLE_ENTRIES.map((e) => [e.i, e.name, e.category, e.categoryId, e.kind, e.section]), [
    [0, "zebra Paint", "Painting", "paint", "software", ""],
    [1, "Apple Draw", "Painting", "paint", "software", ""],
    [2, "Banjo", "audio Tools", "audio", "software", ""],
    [3, "apple Draw", "audio Tools", "audio", "software", ""],
    [4, "Cupboard", "Food", "food", "service", "services"],
    [5, "Lifeline", "Crisis", "crisis", "service", "help"],
    [6, "Counsel", "Legal", "law", "service", "help"],
  ]);
  assert.deepStrictEqual(SAMPLE_ENTRIES[0].extensions, [".kra"], "the category's file types travel with its entries");

  const all = apps.entriesOf(CATALOG);
  const expected = CATALOG.categories.reduce((n, c) => n + c.entries.length, 0);
  assert.strictEqual(all.length, expected, "every entry of the real catalog");
  assert.ok(all.every((e, i) => e.i === i && e.category && e.categoryId && (e.kind === "software" || e.kind === "service")));
  assert.ok(all.filter((e) => e.kind === "service").every((e) => e.section === "help" || e.section === "services"), "every help service says which part it is in");
  const resolve = all.find((e) => e.name === "DaVinci Resolve");
  assert.ok(resolve.websiteOnly && resolve.websiteOnly.includes("Blackmagic"), "website_only is read");

  const tools = apps.toolsOf(TOOLS);
  assert.deepStrictEqual(tools.map((t) => [t.i, t.name, t.page, t.line]), TOOLS.tools.map((t, i) => [i, t.name, t.page, t.line]), "the tools in the file's order");
  assert.deepStrictEqual(apps.toolsOf(null), [], "no file: no tools");
});

// ── 2. The six parts ────────────────────────────────────────────────────

// Seen red 2026-10-20 with Help and Services swapped in SECTIONS: "the app's order, by the ids
// config.json keeps" failed (["games","tools","programs","services","help","added"] against the
// app's; 2b and 5 too), and with Services given cyan: "Services is Tint::Magenta" failed.
test("2a. the six parts are the app's: ids, names, colours, chip words and catalog filters, in its order", () => {
  const words = rustArms("Section", "words");
  const tips = rustArms("Section", "tip");
  const tints = rustArms("Section", "tint");
  const catalog = rustArms("Section", "catalog");
  assert.deepStrictEqual(apps.SECTIONS.map((s) => s.id), RUST_SECTIONS.map(snake), "the app's order, by the ids config.json keeps");
  apps.SECTIONS.forEach((s, k) => {
    const v = RUST_SECTIONS[k];
    assert.strictEqual(s.words, words[v], v + "'s name");
    assert.strictEqual(s.tip, tips[v], v + "'s chip words");
    assert.strictEqual("Tint::" + s.tint[0].toUpperCase() + s.tint.slice(1), tints[v], v + " is " + tints[v]);
    const want = catalog[v] === "None" ? null : catalog[v];
    const have = s.kind ? 'Some(("' + s.kind + '", ' + (s.part ? 'Some("' + s.part + '")' : "None") + "))" : null;
    assert.strictEqual(have, want, v + "'s catalog entries");
  });
  assert.deepStrictEqual(apps.SECTIONS.map((s) => s.tint), ["red", "green", "blue", "cyan", "magenta", "yellow"]);
  assert.strictEqual(sec("services").tint, "magenta", "Services is magenta");
  // Each colour has its band rule in the page's styles, from the theme's tokens.
  for (const [tint, fill, edge] of [["red", "--dm-bg", "--nav-red"], ["green", "--group-bg", "--nav-green"], ["blue", "--server-bg", "--nav-blue"],
    ["cyan", "--section-cyan-bg", "--section-cyan"], ["magenta", "--section-magenta-bg", "--section-magenta"], ["yellow", "--section-yellow-bg", "--section-yellow"]]) {
    assert.match(HTML, new RegExp("\\.apps-tint-" + tint + "\\s*\\{\\s*--band-fill:var\\(" + fill + "\\);\\s*--band-edge:var\\(" + edge + "\\);"), tint + "'s band colours");
  }
});

// Seen red 2026-10-20 with renderParts walking SECTIONS backwards: "in the app's order" failed
// (["added","services",...]); and with the band's count left out: "each band carries its name and
// how many" failed ("HumanityOS tools" for "HumanityOS tools3").
test("2b. on the page: six parts in order, each under a band in its colour with its name (and how many, where the website lists it)", async () => {
  const { host } = await boot(storage(), SAMPLE, SAMPLE_TOOLS);
  assert.deepStrictEqual(shownParts(host), ["games", "tools", "programs", "help", "services", "added"], "in the app's order");
  const bands = partsOf(host).querySelectorAll(".apps-band").map((b) => [b.textContent, b.className]);
  assert.deepStrictEqual(bands, [
    ["Games", "apps-band apps-tint-red"],
    ["HumanityOS tools3", "apps-band apps-tint-green"],
    ["Other programs4", "apps-band apps-tint-blue"],
    ["Help2", "apps-band apps-tint-cyan"],
    ["Services1", "apps-band apps-tint-magenta"],
    ["Added by you", "apps-band apps-tint-yellow"],
  ], "each band carries its name and how many");
  // Each part is named by its band for assistive technology.
  for (const id of shownParts(host)) {
    const p = partEl(host, id);
    assert.strictEqual(p.getAttribute("aria-labelledby"), "apps-band-" + id);
    assert.ok(p.querySelector("#apps-band-" + id));
  }
  // The two parts the website cannot list say so, and link to the desktop app.
  const games = partEl(host, "games");
  assert.strictEqual(games.querySelector(".apps-desktop").textContent, apps.WORDS.gamesLine + " Get the desktop app");
  assert.strictEqual(games.querySelector(".apps-desktop").querySelector("a").getAttribute("href"), "/download");
  assert.match(apps.WORDS.gamesLine, /Steam, the Epic Games Store and GOG/);
  const added = partEl(host, "added");
  assert.strictEqual(added.querySelector(".apps-desktop").querySelector("a").getAttribute("href"), "/download");
  assert.strictEqual(added.querySelectorAll(".app-tile").length, 0);
});

// ── 3. Help and Services split by the data ──────────────────────────────

// Seen red 2026-10-20 with inPart ignoring the section (its section test replaced by `true`, so
// every service went to both parts): "Help holds the help section" got
// ["Counsel","Cupboard","Lifeline"].
test("3. Help and Services take the service categories by their section in the data; moving one moves it", async () => {
  const { host } = await boot(storage(), SAMPLE, SAMPLE_TOOLS);
  assert.deepStrictEqual(namesIn(host, "help"), ["Counsel", "Lifeline"], "Help holds the help section");
  assert.deepStrictEqual(namesIn(host, "services"), ["Cupboard"], "Services holds the services section");
  assert.deepStrictEqual(namesIn(host, "programs"), ["Apple Draw", "apple Draw", "Banjo", "zebra Paint"], "Other programs holds the software");
  // Help's category pills are its own categories only; Services has one, so no pills.
  assert.deepStrictEqual(pillsOf(catsOf(host, "help")).map((p) => p[0]), ["All", "Crisis", "Legal"]);
  assert.strictEqual(catsOf(host, "services"), null, "one category: no pills");

  // The same page with Legal moved to Services in the data.
  const moved = JSON.parse(JSON.stringify(SAMPLE));
  moved.categories.find((c) => c.id === "law").section = "services";
  const again = await boot(storage(), moved, SAMPLE_TOOLS);
  assert.deepStrictEqual(namesIn(again.host, "help"), ["Lifeline"]);
  assert.deepStrictEqual(namesIn(again.host, "services"), ["Counsel", "Cupboard"], "it went where the data says");

  // On the real catalog: every service entry is in exactly one of the two, by its section.
  const real = await boot(storage());
  const all = apps.entriesOf(CATALOG);
  const want = (s) => all.filter((e) => e.kind === "service" && e.section === s).map((e) => e.name).sort((a, b) => a.toLowerCase() < b.toLowerCase() ? -1 : 1);
  const sorted = (xs) => xs.slice().sort((a, b) => a.toLowerCase() < b.toLowerCase() ? -1 : 1);
  assert.deepStrictEqual(sorted(namesIn(real.host, "help")), want("help"));
  assert.deepStrictEqual(sorted(namesIn(real.host, "services")), want("services"));
  assert.strictEqual(namesIn(real.host, "help").length + namesIn(real.host, "services").length, all.filter((e) => e.kind === "service").length, "none lost, none twice");
  assert.ok(namesIn(real.host, "help").includes("988 Lifeline") && namesIn(real.host, "services").includes("Appropedia"));
});

// ── 4. The green tiles' links ───────────────────────────────────────────

/** Whether the website serves a page at `href` (nginx's try_files $uri.html over the flattened
    web/pages, and /chat, its own folder). */
function webPageExists(href) {
  if (href === "/chat") return fs.existsSync(path.join(WEB, "chat", "index.html"));
  return fs.existsSync(path.join(WEB, "pages", href.slice(1) + ".html"));
}

// Seen red 2026-10-20 with Browser's page written "/browser" in TOOL_PAGES: "Browser opens a page
// the website has: /browser" failed (no web/pages/browser.html: the website's Browser is
// web.html); with toolButton linking every tool to /download: "Browser opens /web" got
// "/download"; and with a tool's page id taken out of TOOL_PAGES: "every page id in the data is in
// the table: wallet" failed.
test("4. every tool of the shipped file has a tile; each opens the website's page for it, or says it is in the desktop app", async () => {
  const { host } = await boot(storage());
  const part = partEl(host, "tools");
  const tiles = part.querySelectorAll(".app-tile");
  assert.strictEqual(tiles.length, TOOLS.tools.length, "a tile per tool");
  for (const t of TOOLS.tools) {
    assert.ok(Object.prototype.hasOwnProperty.call(apps.TOOL_PAGES, t.page), "every page id in the data is in the table: " + t.page);
  }
  const desktopOnly = [];
  for (const tile of tiles) {
    const name = tile.querySelector(".app-cover-band").textContent;
    const t = TOOLS.tools.find((x) => x.name === name);
    const known = apps.TOOL_PAGES[t.page];
    const open = tile.querySelector(".app-btn");
    if (known.href) {
      assert.ok(open, name + " has Open");
      assert.strictEqual(open.textContent, "Open");
      assert.strictEqual(open.getAttribute("href"), known.href, name + " opens " + known.href);
      assert.strictEqual(open.getAttribute("target"), null, name + " opens on this website, in this tab");
      assert.ok(webPageExists(known.href), name + " opens a page the website has: " + known.href);
    } else {
      assert.strictEqual(open, null, name + " has no Open");
      const say = tile.querySelector(".app-desktop-only");
      assert.strictEqual(say.textContent, "In the desktop app");
      assert.strictEqual(say.getAttribute("href"), "/download");
      desktopOnly.push(name);
    }
  }
  assert.deepStrictEqual(desktopOnly.sort(), ["Quests", "Studio"], "the two the website has no page for");
  assert.strictEqual(openButton(host, "Maps").parentNode.querySelector(".app-btn").getAttribute("href"), "/maps", "Maps opens /maps");
  assert.strictEqual(openButton(host, "Browser").parentNode.querySelector(".app-btn").getAttribute("href"), "/web", "Browser opens the website's Web page");

  // A tool the table does not know yet (a new entry in the data) is shown, said to be in the
  // desktop app, rather than left out or linked nowhere.
  const more = await boot(storage(), CATALOG, { tools: [{ name: "Garden", page: "garden", line: "A garden." }] });
  const g = partEl(more.host, "tools").querySelector(".app-tile");
  assert.strictEqual(g.querySelector(".app-desktop-only").textContent, "In the desktop app");
});

// ── 5. The toolbar ──────────────────────────────────────────────────────

// Seen red 2026-10-20 with sortsFor offering Category whatever the part: "Name alone with only
// HumanityOS tools shown" got ["Name","Category"]; and with the All chip counting the catalog
// alone (not HumanityOS's own tools): the chips' words got "All 7" for "All 10".
test("5. the toolbar: a chip per part with its count, Sort with Category only for a catalog part, and a chip pressed twice lets go", async () => {
  const { host, page } = await boot(storage(), SAMPLE, SAMPLE_TOOLS);
  const chips = page.parts.chipsRow;
  assert.deepStrictEqual(pillsOf(chips), [
    ["All 10", "true"], ["Games", "false"], ["HumanityOS tools 3", "false"], ["Other programs 4", "false"],
    ["Help 2", "false"], ["Services 1", "false"], ["Added by you", "false"],
  ]);
  assert.deepStrictEqual(chips.children.map((c) => c.className), [
    "apps-chip", "apps-chip apps-tint-red", "apps-chip apps-tint-green", "apps-chip apps-tint-blue",
    "apps-chip apps-tint-cyan", "apps-chip apps-tint-magenta", "apps-chip apps-tint-yellow",
  ], "each part's chip in its colour, with its name");
  assert.deepStrictEqual(chips.children.slice(1).map((c) => c.getAttribute("title")), apps.SECTIONS.map((s) => s.tip));
  assert.deepStrictEqual(pillsOf(page.parts.sortGroup), [["Name", "true"], ["Category", "false"]]);

  pill(chips, "HumanityOS tools").click();
  assert.deepStrictEqual(shownParts(host), ["tools"], "a chip narrows to its part");
  assert.deepStrictEqual(pillsOf(page.parts.sortGroup), [["Name", "true"]], "Name alone with only HumanityOS tools shown");
  pill(chips, "HumanityOS tools").click();
  assert.strictEqual(shownParts(host).length, 6, "pressed again, it lets go");
  assert.strictEqual(pressedIn(chips), "All 10");

  pill(chips, "Help").click();
  assert.deepStrictEqual(shownParts(host), ["help"]);
  pill(chips, "All").click();
  assert.strictEqual(shownParts(host).length, 6);

  // On the real data: the counts are the files'.
  const real = await boot(storage());
  const all = apps.entriesOf(CATALOG);
  const n = (id) => all.filter((e) => apps.inPart(e, sec(id))).length;
  const total = TOOLS.tools.length + n("programs") + n("help") + n("services");
  assert.deepStrictEqual(pillsOf(real.page.parts.chipsRow).map((p) => p[0]), [
    "All " + total, "Games", "HumanityOS tools " + TOOLS.tools.length, "Other programs " + n("programs"),
    "Help " + n("help"), "Services " + n("services"), "Added by you",
  ]);
});

// ── 6. Search and sort ──────────────────────────────────────────────────

test("6a. a catalog part by Name: letter case ignored, then the category as written, then catalog place; by Category, then by name", () => {
  const names = (order) => order.map((i) => SAMPLE_ENTRIES[i].name + " (" + SAMPLE_ENTRIES[i].category + ")");
  assert.deepStrictEqual(names(apps.catalogOrder(SAMPLE_ENTRIES, sec("programs"), null, "name", "")), [
    "Apple Draw (Painting)", "apple Draw (audio Tools)", "Banjo (audio Tools)", "zebra Paint (Painting)",
  ]);
  assert.deepStrictEqual(apps.catalogOrder(SAMPLE_ENTRIES, sec("programs"), null, "category", ""), [3, 2, 1, 0]);
  assert.deepStrictEqual(apps.groupsOf(SAMPLE_ENTRIES, [3, 2, 1, 0]).map((g) => [g.category, g.items]), [["audio Tools", [3, 2]], ["Painting", [1, 0]]]);
  assert.deepStrictEqual(apps.catalogOrder(SAMPLE_ENTRIES, sec("programs"), "paint", "name", ""), [1, 0], "narrowed to a category");

  // On the real catalog, each part's orders hold its entries once and follow the rule.
  const all = apps.entriesOf(CATALOG);
  const key = (e) => [e.name.toLowerCase(), e.category, e.i];
  const le = (a, b) => { for (let k = 0; k < a.length; k++) { if (a[k] < b[k]) return true; if (a[k] > b[k]) return false; } return true; };
  for (const id of ["programs", "help", "services"]) {
    const byName = apps.catalogOrder(all, sec(id), null, "name", "");
    assert.deepStrictEqual(byName.slice().sort((a, b) => a - b), all.filter((e) => apps.inPart(e, sec(id))).map((e) => e.i));
    for (let k = 1; k < byName.length; k++) assert.ok(le(key(all[byName[k - 1]]), key(all[byName[k]])), id + " by name at " + k);
    const byCat = apps.catalogOrder(all, sec(id), null, "category", "");
    const ckey = (e) => [e.category.toLowerCase()].concat(key(e));
    for (let k = 1; k < byCat.length; k++) assert.ok(le(ckey(all[byCat[k - 1]]), ckey(all[byCat[k]])), id + " by category at " + k);
  }
});

test("6b. search: every word, letter case ignored; a catalog entry's name, category or description, a tool's name or line", () => {
  const find = (q) => apps.catalogOrder(SAMPLE_ENTRIES, sec("programs"), null, "name", q);
  assert.deepStrictEqual(find("draw"), [1, 3], "in the name");
  assert.deepStrictEqual(find("APPLE sketch"), [1], "every word, case ignored");
  assert.deepStrictEqual(find("audio"), [3, 2], "in the category");
  assert.deepStrictEqual(find("gpl"), [], "not the licence");
  assert.deepStrictEqual(find("windows"), [], "not the platforms");
  const tools = apps.toolsOf(TOOLS);
  const tnames = (q) => apps.toolsOrder(tools, q).map((i) => tools[i].name);
  const all = tnames("");
  assert.ok(all.every((n, k) => k === 0 || all[k - 1].toLowerCase() <= n.toLowerCase()), "the tools by name: " + all);
  assert.deepStrictEqual(tnames("THIS COMPUTER"), ["Studio"], "a tool by its line");
  assert.deepStrictEqual(tnames("maps"), ["Maps"]);
});

test("6c. on the page: one search narrows every part, and each says what found nothing", async () => {
  const { host, page } = await boot(storage(), SAMPLE, SAMPLE_TOOLS);
  page.parts.search.value = "draw";
  page.parts.search.dispatch("input");
  assert.deepStrictEqual(namesIn(host, "programs"), ["Apple Draw", "apple Draw"]);
  assert.deepStrictEqual(namesIn(host, "tools"), []);
  assert.strictEqual(partEl(host, "tools").querySelector(".apps-muted").textContent, 'No HumanityOS tool holds "draw".');
  assert.strictEqual(partEl(host, "help").querySelector(".apps-muted").textContent, 'Nothing here matches "draw".');
  assert.strictEqual(page.parts.status.textContent, "Showing 2 of 10.");
  page.parts.search.value = "notes";
  page.parts.search.dispatch("input");
  assert.deepStrictEqual(namesIn(host, "tools"), ["Notes"]);
});

test("6d. Sort and the List: the Grid by category shows each category under its name; a List heading sorts", async () => {
  const { host, page } = await boot(storage(), SAMPLE, SAMPLE_TOOLS);
  pill(page.parts.sortGroup, "Category").click();
  const heads = partEl(host, "programs").querySelectorAll(".apps-group").map((h) => h.textContent);
  assert.deepStrictEqual(heads, ["audio Tools2", "Painting2"]);
  assert.deepStrictEqual(namesIn(host, "programs"), ["apple Draw", "Banjo", "Apple Draw", "zebra Paint"]);
  pill(page.parts.layoutGroup, "List").click();
  assert.ok(isList(host, "programs") && isList(host, "tools") && isList(host, "help"), "every part a List");
  partEl(host, "programs").querySelector('[data-sort="name"]').click();
  assert.strictEqual(pressedIn(page.parts.sortGroup), "Name", "a List heading sorts the parts");
  const th = (id) => partEl(host, id).querySelectorAll("th").map((t) => t.textContent);
  assert.deepStrictEqual(th("programs"), ["Link", "Name", "Category", "Licence", "Runs on", "Size"]);
  assert.deepStrictEqual(th("help"), ["Link", "Name", "Category", "Website"]);
  assert.deepStrictEqual(th("tools"), ["Link", "Name", "What it is"]);
  // A category pill keeps its focus when the parts are drawn again.
  const legal = pill(catsOf(host, "help"), "Legal");
  legal.focus();
  legal.click();
  assert.deepStrictEqual(namesIn(host, "help"), ["Counsel"]);
  assert.strictEqual(page.state.view.help_category, "law");
  assert.strictEqual(host.ownerDocument.activeElement.textContent, "Legal", "the pill pressed keeps the focus");
  assert.strictEqual(host.ownerDocument.activeElement.getAttribute("aria-pressed"), "true");
});

// ── 7. The view survives a reload ───────────────────────────────────────

// Seen red 2026-10-20 with VIEW_KEY left at "hos_tools_view": "the renamed key" failed (and 10's
// "nothing links to /tools" named apps-app.js); with writeView writing nothing: "the view is
// kept" failed; and with normalView dropping the section: the kept view came back with
// `section: null` where "programs" was chosen.
test("7a. Grid or List, the sort, the part and a category are kept under hos_apps_view, and the next visit starts from them", async () => {
  assert.strictEqual(apps.VIEW_KEY, "hos_apps_view", "the renamed key");
  const ls = storage();
  const first = await boot(ls, SAMPLE, SAMPLE_TOOLS);
  assert.ok(!isList(first.host, "programs"), "a first visit shows the Grid");
  pill(first.page.parts.layoutGroup, "List").click();
  pill(first.page.parts.sortGroup, "Category").click();
  pill(catsOf(first.host, "programs"), "Painting").click();
  pill(first.page.parts.chipsRow, "Other programs").click();
  assert.ok(ls.map.has("hos_apps_view"), "the view is kept");
  assert.ok(!ls.map.has("hos_tools_view"), "never under the old name");
  const kept = JSON.parse(ls.map.get("hos_apps_view"));
  assert.deepStrictEqual(kept, { layout: "list", sort: "category", section: "programs", programs_category: "paint", help_category: null, services_category: null });
  // Its fields are the app's apps_view fields (src/gui/pages/apps.rs `View`).
  const viewStruct = APPS_RS.slice(APPS_RS.indexOf("pub struct View {"), APPS_RS.indexOf("\n}\n", APPS_RS.indexOf("pub struct View {")));
  for (const k of Object.keys(kept)) assert.match(viewStruct, new RegExp("pub " + k + ":"), k + " is an apps_view field");

  const again = await boot(ls, SAMPLE, SAMPLE_TOOLS);
  assert.deepStrictEqual(shownParts(again.host), ["programs"], "the reload shows the part chosen");
  assert.ok(isList(again.host, "programs"), "the reload shows the List");
  assert.strictEqual(pressedIn(again.page.parts.layoutGroup), "List");
  assert.strictEqual(pressedIn(again.page.parts.sortGroup), "Category");
  assert.strictEqual(pressedIn(again.page.parts.chipsRow), "Other programs 4");
  assert.strictEqual(pressedIn(catsOf(again.host, "programs")), "Painting");
  assert.deepStrictEqual(namesIn(again.host, "programs"), ["Apple Draw", "zebra Paint"]);

  pill(again.page.parts.layoutGroup, "Grid").click();
  const third = await boot(ls, SAMPLE, SAMPLE_TOOLS);
  assert.ok(!isList(third.host, "programs"), "and back to the Grid, kept too");
});

test("7b. storage refused: the page still draws, Grid by name, and the pills still work", async () => {
  const { host, page } = await boot(storage({}, true), SAMPLE, SAMPLE_TOOLS);
  assert.strictEqual(shownParts(host).length, 6);
  assert.deepStrictEqual(namesIn(host, "programs"), ["Apple Draw", "apple Draw", "Banjo", "zebra Paint"]);
  pill(page.parts.layoutGroup, "List").click();
  assert.ok(isList(host, "programs"));
});

test("7c. a kept view that is garbage, or names a category no longer in the catalog, falls back", async () => {
  const garbage = await boot(storage({ hos_apps_view: "{not json" }), SAMPLE, SAMPLE_TOOLS);
  assert.ok(!isList(garbage.host, "programs"));
  assert.strictEqual(pressedIn(garbage.page.parts.sortGroup), "Name");

  const odd = await boot(storage({ hos_apps_view: JSON.stringify({ layout: "carousel", sort: "installed_first", section: "telescopes", help_category: 7 }) }), SAMPLE, SAMPLE_TOOLS);
  assert.deepStrictEqual(odd.page.state.view, { layout: "grid", sort: "name", section: null, programs_category: null, help_category: null, services_category: null });

  const gone = await boot(storage({ hos_apps_view: JSON.stringify({ layout: "list", section: "help", help_category: "telescopes", programs_category: "paint" }) }), SAMPLE, SAMPLE_TOOLS);
  assert.strictEqual(gone.page.state.view.help_category, null, "a category no longer there is let go");
  assert.strictEqual(gone.page.state.view.programs_category, "paint", "one still there is kept");
  assert.deepStrictEqual(namesIn(gone.host, "help"), ["Counsel", "Lifeline"]);
  // A category of another part is let go too (food is a Services category, not Help's).
  assert.strictEqual(apps.fitView({ help_category: "food" }, SAMPLE_ENTRIES).help_category, null);
});

// ── 8. Tiles, the details, Escape ───────────────────────────────────────

// Seen red 2026-10-20 with the tile's old second name line put back (a `.app-name` span under
// the cover): "Banjo: its name once" counted it twice.
test("8a. a tile is short: its name once, on the cover; one line; its button with a fact at its right", async () => {
  const { host } = await boot(storage(), SAMPLE, SAMPLE_TOOLS, { icon: fakeIcon });
  const tileOf = (name) => openButton(host, name).parentNode;
  const read = (t) => ({
    open: t.querySelector(".app-open").tagName + "/" + t.querySelector(".app-open").getAttribute("type"),
    cover: t.querySelector(".app-cover-band").textContent,
    meta: t.querySelector(".app-meta").textContent,
    button: t.querySelector(".app-btn") ? [t.querySelector(".app-btn").textContent, t.querySelector(".app-btn").getAttribute("href"), t.querySelector(".app-btn").getAttribute("target"), t.querySelector(".app-btn").getAttribute("rel")] : null,
    fact: t.querySelector(".app-fact") ? t.querySelector(".app-fact").textContent : null,
  });
  assert.deepStrictEqual(read(tileOf("Banjo")), {
    open: "BUTTON/button", cover: "Banjo", meta: "audio Tools · GPL",
    button: ["Download", "https://banjo.example/get", "_blank", "noopener noreferrer"], fact: "~5MB",
  });
  assert.deepStrictEqual(read(tileOf("Apple Draw")).fact, "In browser");
  assert.deepStrictEqual(read(tileOf("Apple Draw")).button.slice(0, 2), ["Open website", "https://apple.example/"]);
  assert.deepStrictEqual(read(tileOf("Cupboard")), {
    open: "BUTTON/button", cover: "Cupboard", meta: "Food", button: ["Open website", "https://www.cupboard.example/find", "_blank", "noopener noreferrer"], fact: "cupboard.example",
  }, "a help service: its website at the right");
  assert.deepStrictEqual(read(tileOf("Maps")), { open: "BUTTON/button", cover: "Maps", meta: "Every map in one place.", button: ["Open", "/maps", null, null], fact: null });
  for (const name of ["Banjo", "Cupboard", "Maps"]) {
    const t = tileOf(name);
    const said = t.querySelectorAll("span").filter((s) => s.children.length === 0 && s.textContent === name).length;
    assert.strictEqual(said, 1, name + ": its name once");
  }
  // The covers: an entry in its category's hue with its first letter; a tool in its page's hue
  // with its page's icon (the letter when its page has none, as the app draws it).
  const cover = (name) => tileOf(name).querySelector(".app-cover");
  assert.strictEqual(cover("Banjo").style.getPropertyValue("--app-hue"), String(apps.hueOf("audio")));
  assert.strictEqual(cover("Banjo").querySelector(".app-cover-letter").textContent, "B");
  assert.strictEqual(cover("Maps").style.getPropertyValue("--app-hue"), String(apps.hueOf("maps")));
  const mark = cover("Maps").querySelector(".app-cover-icon");
  assert.ok(mark && decodeURIComponent(mark.style.getPropertyValue("--app-icon")).includes('data-icon="map"'), "Maps carries the map icon");
  assert.ok(decodeURIComponent(mark.style.getPropertyValue("--app-icon")).includes('xmlns="http://www.w3.org/2000/svg"'), "a picture of its own needs its namespace");
  assert.strictEqual(cover("Notes").querySelector(".app-cover-letter").textContent, "N", "Notes has no icon: its letter");
  assert.strictEqual(host.querySelectorAll("img").length, 0, "no picture anywhere: no logo is shipped or fetched");

  const open = openButton(host, "Banjo");
  assert.strictEqual(open.getAttribute("aria-label"), "Banjo, audio Tools · GPL, ~5MB", "named by its name first, then its lines");
  assert.strictEqual(open.getAttribute("aria-expanded"), "false");
  open.click();
  assert.strictEqual(openButton(host, "Banjo").getAttribute("aria-expanded"), "true");
  assert.ok(openButton(host, "Banjo").parentNode.classList.contains("selected"));
  // In the page's styles the tiles are the app's 168 px at the least.
  assert.match(HTML, /\.apps-grid \{[^}]*minmax\(168px,1fr\)/);
});

/** The details panel as plain facts. */
function detailsShown(host) {
  const d = host.querySelector("#apps-details");
  if (d.hidden) return null;
  return {
    title: d.querySelector("h2").textContent,
    category: d.querySelector(".app-detail-cat").textContent,
    links: d.querySelectorAll(".app-btn").map((a) => [a.textContent, a.getAttribute("href"), a.getAttribute("target"), a.getAttribute("rel")]),
    note: (d.querySelector(".app-note") || { textContent: null }).textContent,
    description: d.querySelector(".app-desc").textContent,
    facts: d.querySelectorAll("dt").map((dt, k) => [dt.textContent, d.querySelectorAll("dd")[k].textContent]),
    comesFrom: (d.querySelector(".app-comes-from") || { textContent: null }).textContent,
    text: d.textContent,
  };
}

test("8b. the details of a website-only program, one with a download page, a browser program, a help service and a tool", async () => {
  const real = await boot(storage());
  assert.strictEqual(real.host.querySelector("#apps-details").hidden, true, "no details until a tile is chosen");
  openButton(real.host, "DaVinci Resolve").click();
  const raw = CATALOG.categories.flatMap((c) => c.entries).find((x) => x.name === "DaVinci Resolve");
  let d = detailsShown(real.host);
  assert.strictEqual(d.title, "DaVinci Resolve");
  assert.deepStrictEqual(d.links, [["Website", raw.url, "_blank", "noopener noreferrer"]], "no Download page: the catalog names none");
  assert.strictEqual(d.note, "The desktop app has no Install for it. " + raw.website_only, "the catalog's reason, word for word");
  assert.strictEqual(d.description, raw.description);
  assert.ok(d.facts.some((f) => f[0] === "Website" && f[1] === "blackmagicdesign.com"));
  assert.ok(!d.text.includes("null"), "never the word null");
  assert.ok(real.host.classList.contains("has-details"), "the parts make room for the details");

  const { host } = await boot(storage(), SAMPLE, SAMPLE_TOOLS);
  openButton(host, "Banjo").click();
  d = detailsShown(host);
  assert.deepStrictEqual(d.links.map((l) => [l[0], l[1]]), [["Website", "https://banjo.example/"], ["Download page", "https://banjo.example/get"]]);
  assert.strictEqual(d.note, null, "in winget: nothing to explain");
  assert.strictEqual(d.comesFrom, apps.WORDS.comesFrom);
  openButton(host, "apple Draw").click();
  assert.strictEqual(detailsShown(host).note, apps.WORDS.notInWinget);
  openButton(host, "Apple Draw").click();
  d = detailsShown(host);
  assert.deepStrictEqual(d.facts, [["Licence", "MIT"], ["Runs on", "Web browser"], ["Opens", ".kra"], ["Website", "apple.example"]], "no download size");
  assert.strictEqual(d.comesFrom, null);
  openButton(host, "Cupboard").click();
  d = detailsShown(host);
  assert.deepStrictEqual(d.links.map((l) => [l[0], l[1]]), [["Open website", "https://www.cupboard.example/find"]]);
  assert.deepStrictEqual(d.facts, [["Website", "cupboard.example"]]);
  assert.ok(!d.text.includes("null"), "never the word null");

  // A tool: its part's name, its line, Open (its page), and that it is part of HumanityOS.
  openButton(host, "Maps").click();
  d = detailsShown(host);
  assert.strictEqual(d.title, "Maps");
  assert.strictEqual(d.category, "HumanityOS toolsClose");
  assert.strictEqual(d.description, "Every map in one place.");
  assert.deepStrictEqual(d.links, [["Open", "/maps", null, null]]);
  assert.strictEqual(d.comesFrom, apps.WORDS.toolOpensHere);
  openButton(host, "Studio").click();
  d = detailsShown(host);
  assert.deepStrictEqual(d.links, [["Get the desktop app", "/download", null, null]]);
  assert.strictEqual(d.comesFrom, apps.WORDS.toolDesktopOnly);
  assert.ok(!d.text.includes("null"), "never the word null");
});

test("8c. Escape closes the details and gives the focus back to the tile; Close does the same", async () => {
  const { doc, host } = await boot(storage(), SAMPLE, SAMPLE_TOOLS);
  openButton(host, "Banjo").click();
  assert.strictEqual(doc.activeElement, host.querySelector("#apps-details"), "the focus moves to the details");
  doc.dispatch("keydown", { key: "Escape" });
  assert.strictEqual(host.querySelector("#apps-details").hidden, true, "Escape closes them");
  assert.ok(!host.classList.contains("has-details"));
  assert.strictEqual(doc.activeElement, openButton(host, "Banjo"), "the focus goes back to the tile");

  openButton(host, "Maps").click();
  host.querySelector(".app-close").click();
  assert.strictEqual(host.querySelector("#apps-details").hidden, true, "Close closes them");
  assert.strictEqual(doc.activeElement, openButton(host, "Maps"));
  doc.dispatch("keydown", { key: "Escape" });
  assert.strictEqual(host.querySelector("#apps-details").hidden, true, "Escape with nothing open does nothing");
});

test("8d. a data file that will not load says so in its own parts, and the others still draw", async () => {
  const errors = [];
  const was = console.error;
  console.error = (...a) => errors.push(a);
  let r;
  try {
    r = await boot(storage(), SAMPLE, SAMPLE_TOOLS, { answer: (url) => (url === apps.CATALOG_URL ? { ok: false, status: 500 } : null) });
  } finally { console.error = was; }
  for (const id of ["programs", "help", "services"]) {
    assert.strictEqual(partEl(r.host, id).querySelector(".apps-muted").textContent, apps.WORDS.loadFailed, id + " says it could not load");
  }
  assert.deepStrictEqual(namesIn(r.host, "tools"), ["Maps", "Notes", "Studio"], "HumanityOS tools still draw");
  assert.strictEqual(errors.length, 1);
});

// ── 9. The page's lines are the app's ───────────────────────────────────

// Seen red 2026-10-20 with one word of the safety line changed in LINES ("even the top search
// result"): "the app's SAFETY_LINE" failed; and with the Why's slug changed to the games finding:
// "the Why opens the app's finding" failed.
test("9. the page's lines are the app's, word for word, and it says what only the desktop app can do", async () => {
  const programs = rust("apps", "programs.rs");
  const games = rust("apps", "games.rs");
  assert.strictEqual(apps.LINES.safety, rustConst("SAFETY_LINE", programs), "the app's SAFETY_LINE");
  assert.strictEqual(apps.LINES.notAffiliated, rustConst("NOT_AFFILIATED", programs));
  assert.strictEqual(apps.LINES.gamesNotAffiliated, rustConst("NOT_AFFILIATED", games));
  assert.strictEqual(apps.LINES.help, rustConst("HELP_LINE", programs));
  assert.strictEqual(apps.LINES.services, rustConst("SERVICES_LINE", programs));
  // Where programs come from: the app's line up to the Install a web page does not have.
  const where = rustConst("WHERE_FROM", programs);
  assert.strictEqual(apps.WORDS.whereFrom, where.split(", and Install")[0] + ".");
  assert.strictEqual(apps.WHY.slug, rustConst("FINDING_SLUG", programs), "the Why opens the app's finding");
  const tip = LIBRARY_RS.match(/line_with_why\(ui, theme, programs::WHERE_FROM, "((?:[^"\\]|\\.)*)"/);
  assert.ok(tip);
  assert.strictEqual(apps.WHY.tip, unescapeRust(tip[1]), "the Why's hover words");
  assert.strictEqual(apps.WHY.words, rustConst("WHY", APPS_RS));
  assert.strictEqual(apps.WORDS.searchHint, rustConst("SEARCH_HINT", APPS_RS));
  for (const [k, name] of [["download", "DOWNLOAD"], ["openWebsite", "OPEN_WEBSITE"], ["website", "WEBSITE"], ["downloadPage", "DOWNLOAD_PAGE"], ["close", "CLOSE"], ["all", "ALL"]]) {
    assert.strictEqual(apps.WORDS[k], rustConst(name), name);
  }
  assert.strictEqual(apps.WORDS.open, rustConst("OPEN", rust("apps", "own_tools.rs")));
  // The sorts and Grid or List: the app's words and hover words.
  const words = rustArms("SortBy", "words");
  const tips = rustArms("SortBy", "tip");
  for (const s of apps.SORTS) {
    const v = s.id[0].toUpperCase() + s.id.slice(1);
    assert.strictEqual(s.words, words[v]);
    assert.strictEqual(s.tip, tips[v]);
  }
  assert.ok(LIBRARY_RS.includes('["' + apps.LAYOUTS[0].tip + '", "' + apps.LAYOUTS[1].tip + '"]'), "Grid's and List's hover words");

  // The page draws them where the app does: under Other programs and Help, what the app keeps
  // outside "How this page works" (the safety line, not affiliated, Help's line); where the
  // programs come from, with its Why, and Services' line are in the fold (12).
  const { host } = await boot(storage(), SAMPLE, SAMPLE_TOOLS);
  const lines = (id) => partEl(host, id).querySelectorAll(".apps-line").map((p) => p.textContent);
  assert.deepStrictEqual(lines("programs"), [apps.LINES.safety, apps.WORDS.programsDesktop + " Get the desktop app", apps.LINES.notAffiliated]);
  assert.strictEqual(partEl(host, "programs").querySelector(".apps-why"), null, "the Why is in the fold");
  assert.deepStrictEqual(lines("help"), [apps.LINES.help]);
  assert.deepStrictEqual(lines("services"), [], "Services' line is in the fold");
  assert.deepStrictEqual(lines("games"), [apps.WORDS.gamesLine + " Get the desktop app", apps.LINES.gamesNotAffiliated]);

  // The page's head: the app's title and intro.
  const text = (s) => s.replace(/<[^>]+>/g, "").replace(/&#39;/g, "'").replace(/\s+/g, " ");
  assert.ok(text(HTML).includes(rustConst("INTRO", APPS_RS)), "apps.html says the app's INTRO");
  assert.match(HTML, /<h1>Apps<\/h1>/);
  assert.match(HTML, /<title>Apps, HumanityOS<\/title>/);
  assert.ok(HTML.includes('<script src="/pages/apps-app.js'), "apps.html loads apps-app.js");
  assert.ok(!/<img\b/i.test(HTML), "no logo or picture");
  // The colours are the theme's: no colour literal in the page's styles.
  const style = HTML.slice(HTML.indexOf("<style>"), HTML.indexOf("</style>"));
  assert.ok(!/#[0-9a-f]{3,8}\b|rgba?\(/i.test(style), "no hand-written colour in apps.html");
  assert.ok(!/Install/.test(apps.SORTS.map((s) => s.words).join(" ")), "no Installed first on the web");
});

// ── 10. The nav entry ───────────────────────────────────────────────────

/** Every page and script of the website, as [path, text]. */
function webFiles() {
  const out = [];
  const walk = (dir) => {
    for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, ent.name);
      if (ent.isDirectory()) walk(p);
      else if (/\.(html|js|css|json)$/.test(ent.name)) out.push([path.relative(WEB, p).replace(/\\/g, "/"), fs.readFileSync(p, "utf8")]);
    }
  };
  walk(WEB);
  return out;
}

// Seen red 2026-10-20 with the Apps tab left out of the Play group: "Apps beside Play, as in the
// app" failed (with the tab's icon named 'tool' too: the check reads the icon); with the phone
// menu's link put back as mobileLink('/tools', 'Tools'): "in the phone menu" failed; with the
// icons.js entry renamed: "icons.js has an apps icon" failed; and with the Justfile's rig-tests
// naming tools-web.test.js: "the Justfile runs this file" failed.
test("10. the nav: Apps beside Play with the four squares, in the phone menu, lit on /apps; nothing links to /tools; rig-tests runs this file", () => {
  const sim = SHELL.match(/'<span class="nav-group-sim">' \+([\s\S]*?)'<\/span>'/);
  assert.ok(sim, "shell.js has the Play group");
  const tabs = [...sim[1].matchAll(/navTab\('([^']*)',\s*'([^']*)',\s*'([^']*)',\s*'([^']*)'\)/g)].map((m) => m.slice(1));
  assert.deepStrictEqual(tabs, [["/download", "games", "Play", "play"], ["/apps", "apps", "Apps", "apps"]], "Apps beside Play, as in the app");
  assert.match(SHELL, /mobileLink\('\/apps',\s*'Apps'\)/, "in the phone menu");
  assert.match(SHELL, /else if \(p\.startsWith\('\/apps'\)\)\s*active = 'apps';/, "lit on /apps");
  // The icon: four rounded squares, two by two (the app's paint_apps).
  const icon = ICONS.match(/\n\s*apps: '([^']*)',/);
  assert.ok(icon, "icons.js has an apps icon");
  const rects = [...icon[1].matchAll(/<rect x="(\d+)" y="(\d+)" width="(\d+)" height="(\d+)" rx="[\d.]+"\/>/g)].map((m) => m.slice(1).map(Number));
  assert.strictEqual(rects.length, 4, "the four squares");
  assert.ok(rects.every((r) => r[2] === r[3]), "squares");
  assert.strictEqual(new Set(rects.map((r) => r[0])).size, 2, "two columns");
  assert.strictEqual(new Set(rects.map((r) => r[1])).size, 2, "two rows");

  // Nothing on the website links to the old page or names its files or its key.
  const stale = [];
  for (const [rel, text] of webFiles()) {
    if (/href="\/tools"|['"]\/tools['"]|tools\.html|tools-app\.js|hos_tools_view|startsWith\('\/tools'\)/.test(text)) stale.push(rel);
  }
  assert.deepStrictEqual(stale, [], "nothing links to /tools");
  assert.ok(!fs.existsSync(path.join(WEB, "pages", "tools.html")) && !fs.existsSync(path.join(WEB, "pages", "tools-app.js")), "the old files are gone");
  // The Library and Platform pages point people here.
  for (const f of ["library.html", "platform.html"]) {
    assert.match(readText(path.join(WEB, "pages", f)), /<a href="\/apps">Apps<\/a> page/, f + " links to the Apps page");
  }
  const just = readText(path.join(ROOT, "Justfile"));
  const rig = just.match(/\nrig-tests:\n\s+node --test ([^\n]+)/);
  assert.ok(rig && rig[1].includes("scripts/tests/apps-web.test.js"), "the Justfile runs this file");
  assert.ok(!rig[1].includes("tools-web.test.js"), "and not the old one");
});

// ── 11. A hue ───────────────────────────────────────────────────────────

test("11. a hue is the app's FNV-1a swatch hue", () => {
  // FNV-1a, 32 bits, over the seed's UTF-8 bytes (src/gui/widgets/mod.rs swatch_color), here in
  // BigInt arithmetic.
  const fnv = (s) => {
    let h = 2166136261n;
    for (const b of Buffer.from(s, "utf8")) { h ^= BigInt(b); h = (h * 16777619n) % 4294967296n; }
    return Number(h % 360n);
  };
  for (const id of ["3d-modeling", "image-editing", "water", "food", "maps", "a", "", "été"]) {
    assert.strictEqual(apps.hueOf(id), fnv(id), id);
  }
  const hues = new Set(CATALOG.categories.map((c) => apps.hueOf(c.id)));
  assert.ok(hues.size > CATALOG.categories.length / 2, "the categories spread across the hues");
});

// ── 12. How this page works ─────────────────────────────────────────────

const HOW = require(path.join(WEB, "shared", "how-it-works.js"));
const WIDGET_RS = readText(path.join(ROOT, "src", "gui", "widgets", "how_it_works.rs"));
const HOW_CSS = readText(path.join(WEB, "shared", "how-it-works.css"));

/** Boot the page with its header, as apps.html has it: the title in .apps-title, then the line. */
async function bootWithHead(ls, catalog, tools) {
  const doc = fakeDocument();
  const head = doc.createElement("header");
  head.setAttribute("id", "apps-head");
  const title = doc.createElement("div");
  title.className = "apps-title";
  const h1 = doc.createElement("h1");
  h1.textContent = "Apps";
  title.appendChild(h1);
  const intro = doc.createElement("p");
  intro.className = "apps-intro";
  intro.textContent = rustConst("INTRO", APPS_RS);
  head.append(title, intro);
  const host = doc.createElement("div");
  host.setAttribute("id", "apps-app");
  doc.body.append(head, host);
  const page = apps.createAppsPage({
    document: doc, root: host, head, storage: ls,
    fetch: async (url) => {
      if (url === apps.CATALOG_URL) return { ok: true, json: async () => JSON.parse(JSON.stringify(catalog || CATALOG)) };
      if (url === apps.TOOLS_URL) return { ok: true, json: async () => JSON.parse(JSON.stringify(tools || TOOLS)) };
      return { ok: false, status: 404 };
    },
  });
  await page.start();
  return { doc, head, host, page, toggle: doc.getElementById("apps-how-toggle"), fold: doc.getElementById("apps-how") };
}

/** The words a person sees: every text not inside a hidden element. */
function visibleText(n) {
  if (n.nodeType === 3) return n.textContent;
  if (n.hidden) return "";
  return n.childNodes.map(visibleText).join("\n");
}
/** The fold's parts as [name, colour, [its lines]]. */
function foldParts(fold) {
  const out = [];
  for (const c of fold.children) {
    if (c.tagName === "H2") out.push([c.textContent, c.getAttribute("data-tint"), []]);
    else if (c.tagName === "P" && out.length) out[out.length - 1][2].push(c.textContent);
  }
  return out;
}

// Seen red 2026-10-20, each break made in a copy of web/ (HOS_WEB_DIR): with readOpen answering
// open unless "0" is kept, "closed by default" failed (actual 'true', expected 'false'); with
// where the programs come from put back under Other programs, "closed, where the programs come
// from is nowhere on the page" failed (and 9's lines under Other programs); with the fold filled
// whatever its state (render's `open &&` taken out), "closed, the fold holds nothing" failed
// (actual 4, expected 0); with render's aria-expanded line taken out, "closed by default" failed
// (actual null).
test("12a. How this page works: a toggle beside the heading, closed by default, the must-see lines in their parts; opened, the folded lines under their parts' names", async () => {
  assert.strictEqual(HOW.LABEL, rustConst("LABEL", WIDGET_RS), "the app's LABEL");
  assert.strictEqual(HOW.TIP, rustConst("TIP", WIDGET_RS), "the app's TIP");
  const ls = storage();
  const { doc, head, toggle, fold } = await bootWithHead(ls, SAMPLE, SAMPLE_TOOLS);
  assert.ok(toggle && fold, "the toggle and its fold are on the page");
  assert.strictEqual(toggle.parentNode, head.querySelector(".apps-title"), "beside the title");
  assert.strictEqual(toggle.tagName, "BUTTON", "a real button (Tab reaches it, Enter and Space press it)");
  assert.strictEqual(toggle.getAttribute("type"), "button");
  assert.strictEqual(toggle.textContent, HOW.LABEL, "its words");
  assert.strictEqual(toggle.getAttribute("title"), HOW.TIP, "its hover words");
  assert.strictEqual(toggle.getAttribute("aria-controls"), "apps-how", "it names the fold it opens");
  assert.strictEqual(fold.parentNode, head, "the fold in the header");
  assert.ok(head.childNodes.indexOf(fold) > head.childNodes.indexOf(head.querySelector(".apps-intro")), "after the page's line, as the app's");

  // Closed.
  assert.strictEqual(toggle.getAttribute("aria-expanded"), "false", "closed by default");
  assert.strictEqual(fold.hidden, true);
  assert.strictEqual(fold.children.length, 0, "closed, the fold holds nothing");
  const seen = visibleText(doc.body);
  assert.ok(!seen.includes(apps.WORDS.whereFrom), "closed, where the programs come from is nowhere on the page");
  assert.ok(!seen.includes(apps.LINES.services), "nor Services' line");
  assert.strictEqual(doc.body.querySelectorAll(".apps-why").length, 0, "nor its Why");
  for (const [words, what] of [[apps.LINES.safety, "the safety line"], [apps.LINES.notAffiliated, "not made by the programs' makers"],
    [apps.LINES.gamesNotAffiliated, "not affiliated with the stores"], [apps.LINES.help, "Help's line (call the emergency number first)"]]) {
    assert.ok(seen.includes(words), what + " stays on the page");
  }
  assert.strictEqual(ls.map.has(apps.HOW_KEY), false, "nothing kept until it is pressed");

  // Opened.
  toggle.click();
  assert.strictEqual(toggle.getAttribute("aria-expanded"), "true", "aria-expanded follows");
  assert.strictEqual(fold.hidden, false);
  assert.deepStrictEqual(foldParts(fold), [
    ["Other programs", "blue", [apps.WORDS.whereFrom + apps.WHY.words]],
    ["Services", "magenta", [apps.LINES.services]],
  ], "each part's explanations under its name and colour, word for word");
  const why = fold.querySelector(".apps-why");
  assert.strictEqual(why.getAttribute("href"), "/library#" + apps.WHY.slug, "the Why opens the finding");
  assert.strictEqual(why.getAttribute("title"), apps.WHY.tip);
  assert.ok(visibleText(doc.body).includes(apps.LINES.safety), "the safety line is still under Other programs");
  assert.strictEqual(ls.map.get(apps.HOW_KEY), "1", "kept, under hos_apps_how_open");
  assert.ok(toggle.className.includes("how-open"), "drawn open (the triangle turned down)");

  toggle.click();
  assert.strictEqual(toggle.getAttribute("aria-expanded"), "false", "pressed again, it closes");
  assert.strictEqual(fold.hidden, true);
  assert.strictEqual(ls.map.get(apps.HOW_KEY), "0");
});

// Seen red 2026-10-20 in a copy of web/: with writeOpen writing nothing, "it survives a reload"
// failed (actual 'false', expected 'true'; 12a's "kept, under hos_apps_how_open" too); with
// readOpen without its try/catch, the boot threw "SecurityError: storage is refused" (and 7b's);
// with render's aria-expanded line taken out, "it survives a reload" failed (actual null).
test("12b. How this page works survives a reload; storage refused, it still opens and closes; a kept value that is not 1 is closed", async () => {
  const ls = storage();
  const first = await bootWithHead(ls, SAMPLE, SAMPLE_TOOLS);
  first.toggle.click();
  const again = await bootWithHead(ls, SAMPLE, SAMPLE_TOOLS);
  assert.strictEqual(again.toggle.getAttribute("aria-expanded"), "true", "it survives a reload");
  assert.strictEqual(again.fold.hidden, false);
  assert.deepStrictEqual(foldParts(again.fold).map((p) => p[0]), ["Other programs", "Services"], "open from the start, its parts drawn");
  again.toggle.click();
  const closed = await bootWithHead(ls, SAMPLE, SAMPLE_TOOLS);
  assert.strictEqual(closed.toggle.getAttribute("aria-expanded"), "false", "closed, and that is kept too");
  assert.strictEqual(closed.fold.hidden, true);

  const refused = await bootWithHead(storage(null, true), SAMPLE, SAMPLE_TOOLS);
  assert.strictEqual(refused.toggle.getAttribute("aria-expanded"), "false", "storage refused: closed");
  refused.toggle.click();
  assert.strictEqual(refused.toggle.getAttribute("aria-expanded"), "true", "and it still opens");
  assert.strictEqual(refused.fold.children.length > 0, true);

  for (const kept of ["true", "open", "", "2"]) {
    const odd = await bootWithHead(storage({ [apps.HOW_KEY]: kept }), SAMPLE, SAMPLE_TOOLS);
    assert.strictEqual(odd.toggle.getAttribute("aria-expanded"), "false", JSON.stringify(kept) + " kept: closed");
  }
  assert.strictEqual(HOW.readOpen(null, apps.HOW_KEY), false, "no storage at all: closed");
});

// Seen red 2026-10-20 in a copy of web/ with howPartsShown returning every part whatever the
// view: "narrowed to Other programs, only its part" failed (["Other programs","Services"]).
test("12c. the fold follows the parts shown; its styles are linked, loaded before the page and theme variables only", async () => {
  const ls = storage({ [apps.HOW_KEY]: "1" });
  const { page, fold } = await bootWithHead(ls, SAMPLE, SAMPLE_TOOLS);
  page.setView({ section: "programs" });
  assert.deepStrictEqual(foldParts(fold).map((p) => p[0]), ["Other programs"], "narrowed to Other programs, only its part");
  page.setView({ section: "services" });
  assert.deepStrictEqual(foldParts(fold).map((p) => p[0]), ["Services"]);
  page.setView({ section: "help" });
  assert.deepStrictEqual(foldParts(fold).map((p) => p[0]), ["Other programs", "Services"], "a part with nothing folded: every part, never an empty box");
  assert.deepStrictEqual(apps.howPartsShown(apps.normalView({}), { programs: 0, services: 3 }), ["services"], "a part known to be empty is left out");

  // The page links the styles and loads the script before apps-app.js.
  assert.match(HTML, /<link rel="stylesheet" href="\/shared\/how-it-works\.css\?v=\d+">/, "apps.html links how-it-works.css");
  const s = HTML.indexOf('<script src="/shared/how-it-works.js'), a = HTML.indexOf('<script src="/pages/apps-app.js');
  assert.ok(s > 0 && s < a, "how-it-works.js loads before apps-app.js");
  assert.match(HTML, /<header class="apps-head" id="apps-head">\s*<div class="apps-title"><h1>Apps<\/h1><\/div>/, "the header the toggle goes beside");
  // Theme variables only: no hand-written colour; a hidden fold takes no room; a visible focus ring.
  assert.ok(!/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i.test(HOW_CSS), "no hand-written colour in how-it-works.css");
  assert.match(HOW_CSS, /\.how-panel\[hidden\]\s*\{\s*display:\s*none;/);
  assert.match(HOW_CSS, /\.how-toggle:focus-visible\s*\{[^}]*outline:/);
  for (const t of HOW.TINTS) assert.match(HOW_CSS, new RegExp('\\.how-part\\[data-tint="' + t + '"\\]\\s*\\{\\s*--how-dot: var\\(--'), t + "'s dot is a theme colour");
});

// Red first. The Tools tests kept from tools-web.test.js were each seen red on 2026-10-19, in a
// copy of web/pages (HOS_WEB_DIR) with every other test passing, and the same breaks made again
// on 2026-10-20 in apps-app.js trip them still: the catalog order sorting by name whatever the
// sort (6a and 6d); the search looking at the name alone (6b, "every word, case ignored");
// writeView writing nothing (7a); readView without its try/catch (7b, the boot threw
// "SecurityError: storage is refused"); the details appending their parts without dropping the
// nulls (8b, "never the word null"); detailsOf without the website-only reason (8b, "the
// catalog's reason, word for word"); the Escape listener not added (8c, "Escape closes them");
// hueOf hashing UTF-16 code units instead of UTF-8 bytes (11, on "été": 51 for 55).
//
// The Apps tests, 2026-10-20, each break made in a copy of web/ (HOS_WEB_DIR) or, for the
// Justfile, in the repository and put back; each passes again on the real files. What each
// showed is in the comment above its test: 2a (parts swapped, a colour changed), 2b (parts drawn
// backwards, the count left out), 3 (the section ignored), 4 (a wrong page, every tool to
// /download, a page id missing from the table), 5 (Category always offered, the All count), 7a
// (the old key, nothing written, the section dropped), 8a (the name twice), 9 (a word of the
// safety line, the Why's slug), 10 (the tab in the wrong group, the old mobile link, the wrong
// icon, the Justfile), 12a-c (How this page works: open by default, where the programs come from
// left in its part, the fold filled while closed, aria-expanded never set, nothing kept, the read
// without its try/catch, the fold blind to the view).
