// Where you live, in the web chat (ship homes increment 6, lane 1F, 2026-10-12;
// docs/design/ship-homes-increment-6-plan.md sections 3.9 and 3.11): web/chat/chat-home.js, and the
// lines it needed in app.js (the relay's `game_home` handed to the window), chat-ui.js (the command
// palette's action), data/commands.json (the palette's item) and index.html (the script).
//
// Run: node --test scripts/tests/chat-home.test.js   (in `just rig-tests`)
//
// The window runs as in the browser, in its own global scope (node:vm), with the DOM replaced by a
// small tree that keeps what is written to it (and refuses innerHTML, which the window never uses:
// the server's words reach the screen through textContent only). The chat's socket is a stand-in
// that keeps what is sent. The answers are built to the Rust wire, src/ship/starts.rs (`HomeAnswer`,
// `StartOffer`, `HeldHome`, `HomeRefusal`): test 0 reads those types' fields out of starts.rs and
// holds every answer here to them, and the "as shipped" answers carry the words of
// data/homes/starts.ron, as the relay sends them. The relay's own half (lane 1B) is built to the
// same types. HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves:
//  0. The wire is starts.rs's: the window asks with HOME_REQUEST, chooses with CHOOSE_HOME, reads an
//     answer of type HOME_ANSWER and ignores any other.
//  1. A held home is shown in its start's own words from the answer ("Where you live here: a
//     Cabin, c2." and the Cabin's sentence), with no choice while a home is held.
//  2. With no home: "You have no home on this server yet.", one row per start in the answer's order,
//     each offered one with its free count ("15 free", "1 free", "none free") and a Choose (off when
//     none is free), one not offered greyed with the server's reason and no Choose.
//  3. Every name and sentence shown is the answer's: a server whose starts are renamed shows the new
//     names and sentences, and none of the shipped ones (the plan's own break: a hard-coded "Cabin").
//  4. Choose sends `game_choose_home` with the start's id and nothing else, says what it is asking
//     for in the answer's words, turns every Choose off while it waits, and a `chose` answer shows
//     the home as yours.
//  5. Each refusal (every HomeRefusal in starts.rs) shows the server's own sentence, word for word;
//     a refusal that carries no home and no starts keeps the choices shown before it.
//  6. Nothing of anyone else reaches the window (plan 3.11): only the answer's own fields are read.
//  7. app.js hands a `game_home` from the relay (a __game__ system message) to the window, through
//     its real dispatch; an answer that came before the window was opened is shown when it opens.
//  8. The command palette opens it ("Where you live", data/commands.json, the action in chat-ui.js),
//     and opening it asks the server; index.html loads the script, after app.js.
//  9. No connection: it says so and sends nothing; no answer within ten seconds: it says the server
//     has not answered.
//
// Red first, 2026-10-12: each break made alone in a copy of web/chat, this file run against it with
// HOS_WEB_DIR, and seen failing with the assertion named (every other test passing); each passes on
// the real web/. The breaks and their failures are written beside each test.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const STARTS_RS = fs.readFileSync(path.join(ROOT, "src", "ship", "starts.rs"), "utf8").replace(/\r\n/g, "\n");
const STARTS_RON = fs.readFileSync(path.join(ROOT, "data", "homes", "starts.ron"), "utf8").replace(/\r\n/g, "\n");

// ── The Rust wire, read out of src/ship/starts.rs ──────────────────────────

function rustConst(name) {
  const m = STARTS_RS.match(new RegExp(`pub const ${name}: &str = "([^"]+)";`));
  assert.ok(m, `src/ship/starts.rs defines ${name}`);
  return m[1];
}
/** A struct's fields, each with whether serde leaves it out when empty. */
function rustFields(name) {
  const m = STARTS_RS.match(new RegExp(`pub struct ${name} \\{\\n([\\s\\S]*?)\\n\\}`));
  assert.ok(m, `src/ship/starts.rs defines ${name}`);
  const fields = {};
  let skip = false;
  for (const line of m[1].split("\n")) {
    if (/skip_serializing_if/.test(line)) skip = true;
    const f = line.match(/^\s*pub (\w+):/);
    if (f) { fields[f[1]] = { optional: skip }; skip = false; }
  }
  return fields;
}
/** HomeRefusal's variants as they go on the wire (serde rename_all = "snake_case"). */
function rustRefusals() {
  const m = STARTS_RS.match(/#\[serde\(rename_all = "snake_case"\)\]\npub enum HomeRefusal \{\n([\s\S]*?)\n\}/);
  assert.ok(m, "src/ship/starts.rs defines HomeRefusal, snake_case on the wire");
  return [...m[1].matchAll(/^\s*([A-Z]\w*),/gm)].map((v) => v[1].replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase());
}
const HOME_REQUEST = rustConst("HOME_REQUEST");
const CHOOSE_HOME = rustConst("CHOOSE_HOME");
const HOME_ANSWER = rustConst("HOME_ANSWER");
const FIELDS = { HomeAnswer: rustFields("HomeAnswer"), StartOffer: rustFields("StartOffer"), HeldHome: rustFields("HeldHome") };
const REFUSALS = rustRefusals();

/** The starts as data/homes/starts.ron writes them: id, name, sentence, offered, why not. */
function shippedStarts() {
  const rows = [];
  for (const m of STARTS_RON.matchAll(/\(\n\s*id: "([^"]+)",\n\s*name: "([^"]+)",\n\s*sentence: "([^"]+)",\n\s*offered: (true|false),\n(?:\s*not_offered_why: "([^"]*)",\n)?/g)) {
    rows.push({ id: m[1], name: m[2], sentence: m[3], offered: m[4] === "true", why: m[5] || "" });
  }
  return rows;
}
const SHIPPED = shippedStarts();
const shipped = (id) => SHIPPED.find((s) => s.id === id);

// Answers, shaped as serde writes HomeAnswer::to_message: `why_not`, `chose`, `refused` and
// `message` only when they say something.
function offer(s, free) {
  const o = { id: s.id, name: s.name, sentence: s.sentence, offered: s.offered };
  if (!s.offered && s.why) o.why_not = s.why;
  o.free = free;
  return o;
}
function answer({ plot = null, starts = [], chose = false, refused = null, message = "" } = {}) {
  const a = { type: HOME_ANSWER, plot, starts };
  if (chose) a.chose = true;
  if (refused) a.refused = refused;
  if (message) a.message = message;
  return a;
}
const held = (id, kind, start) => ({ id, kind, start: start || kind });

/** The shipped starts with free counts by id. */
function shippedOffers(free) {
  return SHIPPED.map((s) => offer(s, free[s.id] || 0));
}

// A server that renamed its starts and added one: the window must show these words and no others.
const RENAMED = [
  { id: "cabin", name: "Bunk Room", sentence: "A bunk and a locker in a shared dormitory; meals in the galley.", offered: true, why: "" },
  { id: "pod", name: "Sleep Pod", sentence: "A sealed pod with a bed and a reading light.", offered: false, why: "Pods arrive with the second ring, in a later update." },
  { id: "homestead", name: "Smallholding", sentence: "A plot with a cottage and a vegetable garden.", offered: true, why: "" },
  { id: "loft", name: "Orbital Loft", sentence: "A loft with a window on the stars.", offered: true, why: "" },
];

// ── The page ───────────────────────────────────────────────────────────────

function makeDom() {
  function el(tag) {
    const listeners = {};
    const e = {
      tagName: String(tag).toUpperCase(),
      id: "",
      className: "",
      children: [],
      parentNode: null,
      style: {},
      attrs: {},
      disabled: false,
      _text: "",
      get textContent() { return this._text + this.children.map((c) => c.textContent).join(""); },
      set textContent(v) { this._text = String(v); for (const c of this.children) c.parentNode = null; this.children = []; },
      get innerHTML() { throw new Error("chat-home.js reads innerHTML"); },
      set innerHTML(_v) { throw new Error("chat-home.js writes innerHTML: the server's words must go through textContent"); },
      appendChild(c) { c.parentNode = this; this.children.push(c); return c; },
      setAttribute(k, v) { this.attrs[k] = String(v); },
      getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; },
      addEventListener(type, fn) { (listeners[type] = listeners[type] || []).push(fn); },
      click() {
        if (this.disabled) return;
        const ev = { type: "click", target: this, stopPropagation() {} };
        for (let n = this; n; n = n.parentNode) for (const fn of (n._listeners().click || [])) fn(ev);
      },
      _listeners() { return listeners; },
    };
    e.classList = {
      add: (c) => { const s = new Set(e.className.split(/\s+/).filter(Boolean)); s.add(c); e.className = [...s].join(" "); },
      remove: (c) => { e.className = e.className.split(/\s+/).filter((x) => x && x !== c).join(" "); },
      contains: (c) => e.className.split(/\s+/).includes(c),
    };
    return e;
  }
  const body = el("body");
  const all = (n, out = []) => { out.push(n); for (const c of n.children) all(c, out); return out; };
  return {
    body,
    all,
    createElement: el,
    getElementById: (id) => all(body).find((n) => n.id === id) || null,
  };
}

function loadPage({ open = true, timersOut } = {}) {
  const dom = makeDom();
  const timers = timersOut || [];
  let nextTimer = 1;
  const sock = {
    readyState: 1,
    sent: [],
    send(s) { this.sent.push(JSON.parse(s)); },
  };
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: dom,
    ws: sock,
    WebSocket: { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 },
    setTimeout: (fn, ms) => { const id = nextTimer++; timers.push({ id, fn, ms }); return id; },
    clearTimeout: (id) => { const i = timers.findIndex((t) => t.id === id); if (i >= 0) timers.splice(i, 1); },
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "chat", "chat-home.js"), "utf8"), ctx, { filename: "chat-home.js" });
  const page = {
    ctx, dom, sock, timers,
    overlay: () => dom.getElementById("where-overlay"),
    answer: (a) => ctx.renderWhereYouLive(JSON.parse(JSON.stringify(a))),
    status: () => (dom.getElementById("where-status") || { textContent: "" }).textContent,
    // Every piece of text in the window, one element's own words per line, in order.
    lines: () => { const o = dom.getElementById("where-overlay"); return o ? dom.all(o).map((n) => n._text).filter(Boolean) : []; },
    text: () => page.lines().join("\n"),
    rows: () => { const o = dom.getElementById("where-overlay"); return o ? dom.all(o).filter((n) => n.className.split(/\s+/).includes("gameadmin-row")) : []; },
    row: (id) => page.rows().find((r) => r.getAttribute("data-start") === id),
    chooseButtons: () => { const o = dom.getElementById("where-overlay"); return o ? dom.all(o).filter((n) => n.tagName === "BUTTON" && n.getAttribute("data-start")) : []; },
    chooseOf: (id) => page.chooseButtons().find((b) => b.getAttribute("data-start") === id),
    fireTimers: () => { for (const t of timers.splice(0)) t.fn(); },
  };
  if (open) ctx.openWhereYouLive();
  return page;
}

/** The words a row shows, one per line. */
function rowLines(page, row) { return page.dom.all(row).map((n) => n._text).filter(Boolean); }

// ── 0. The wire ────────────────────────────────────────────────────────────

// Red: chat-home.js with HOME_ANSWER spelt "game_home_answer": "an answer of starts.rs's type is
// read and shown" (no "Where you live here" line; every test that reads an answer failed with it).
test("the wire is starts.rs's: ask, choose and read the answer by its own names", () => {
  // The answers here carry exactly the Rust fields, the optional ones only when they say something.
  const a = answer({ plot: held("c2", "cabin"), starts: shippedOffers({ cabin: 15 }), chose: true, refused: "full", message: "x" });
  assert.deepEqual(Object.keys(a).filter((k) => k !== "type").sort(), Object.keys(FIELDS.HomeAnswer).sort(), "the answer's fields are HomeAnswer's");
  assert.deepEqual(Object.keys(a.plot).sort(), Object.keys(FIELDS.HeldHome).sort(), "the held home's are HeldHome's");
  const notOffered = a.starts.find((s) => !s.offered);
  assert.deepEqual(Object.keys(notOffered).sort(), Object.keys(FIELDS.StartOffer).sort(), "a start's are StartOffer's");
  for (const [k, f] of Object.entries(FIELDS.HomeAnswer)) assert.equal(f.optional, ["chose", "refused", "message"].includes(k), `${k} is left out when empty: ${f.optional}`);
  assert.deepEqual(SHIPPED.map((s) => s.id), ["cabin", "apartment", "homestead"], "the shipped starts read from data/homes/starts.ron");
  assert.deepEqual(REFUSALS, ["holds_one", "full", "not_offered", "no_ship", "again"]);

  const page = loadPage();
  assert.deepEqual(page.sock.sent, [{ type: HOME_REQUEST }], "opening the window asks with HOME_REQUEST, nothing else");
  assert.equal(page.status(), "Asking the server where you live...");
  page.answer({ ...answer({ plot: held("c2", "cabin"), starts: shippedOffers({}) }), type: "game_fleet_ledger" });
  assert.equal(page.status(), "Asking the server where you live...", "another type of message is not an answer");
  assert.ok(!page.text().includes("Where you live here"), "and shows nothing");
  page.answer(answer({ plot: held("c2", "cabin"), starts: shippedOffers({ cabin: 14 }) }));
  assert.ok(page.lines().includes("Where you live here: a Cabin, c2."), "an answer of starts.rs's type is read and shown");
  assert.equal(page.status(), "", "and the asking line goes");
});

// ── 1. A held home ─────────────────────────────────────────────────────────

// Red: chat-home.js drawing the start rows whether or not a home is held: "no choice while a home is
// held" (two Choose buttons beside "Where you live here: a Cabin, c2."; tests 4 and 5 failed with it,
// "and no choice now that one is held" and "2 == 0").
test("a held home: its start's own name and sentence, the plot, and no choice", () => {
  const page = loadPage();
  page.answer(answer({ plot: held("c2", "cabin"), starts: shippedOffers({ cabin: 14, homestead: 12 }) }));
  const lines = page.lines();
  assert.ok(lines.includes("Where you live here: a Cabin, c2."), "the held home, named as the answer names its start: " + lines.join(" | "));
  assert.ok(lines.includes(shipped("cabin").sentence), "with the Cabin's sentence from the answer");
  assert.equal(page.chooseButtons().length, 0, "no choice while a home is held");
  assert.equal(page.rows().length, 0, "no choice while a home is held");
  assert.ok(!lines.includes("You have no home on this server yet."));
  assert.ok(!page.text().includes(shipped("homestead").sentence), "only the held home's words");

  page.answer(answer({ plot: held("p3", "homestead"), starts: shippedOffers({ cabin: 14 }) }));
  assert.ok(page.lines().includes("Where you live here: a Homestead, p3."), "a homestead held");
  // A plot whose start the answer does not list is named by its kind, as the server wrote it.
  page.answer(answer({ plot: held("a1", "apartment_block", "flat"), starts: [] }));
  assert.ok(page.lines().includes("Where you live here: an apartment block, a1."), "named by the answer's kind: " + page.lines().join(" | "));
  // The note: a server home starts fresh.
  assert.ok(page.lines().includes("On a server you start fresh: only your name and look come with you. The game keeps your things there on the computer you play on."));
});

// ── 2. The choice ──────────────────────────────────────────────────────────

// Red: chat-home.js giving a start not offered its count and Choose too (the not-offered return
// taken out of startRow): "greyed with the server's reason" (the Apartment's row read "none free",
// "Choose" after its reason).
test("no home: a row per start with its free count; Choose when one is free; one not offered greyed with its reason", () => {
  const page = loadPage();
  page.answer(answer({ starts: shippedOffers({ cabin: 15, homestead: 0 }) }));
  assert.ok(page.lines().includes("You have no home on this server yet."));
  assert.deepEqual(page.rows().map((r) => r.getAttribute("data-start")), ["cabin", "apartment", "homestead"], "one row per start, in the answer's order");

  const cabin = rowLines(page, page.row("cabin"));
  assert.deepEqual(cabin, ["Cabin", shipped("cabin").sentence, "15 free", "Choose"], "the Cabin: name, sentence, count, Choose");
  assert.equal(page.chooseOf("cabin").disabled, false, "a Cabin can be chosen");
  assert.equal(page.chooseOf("cabin").getAttribute("aria-label"), "Choose a Cabin");

  const home = rowLines(page, page.row("homestead"));
  assert.deepEqual(home, ["Homestead", shipped("homestead").sentence, "none free", "Choose"]);
  assert.equal(page.chooseOf("homestead").disabled, true, "none free: Choose is off");

  const apt = page.row("apartment");
  assert.deepEqual(rowLines(page, apt), ["Apartment", shipped("apartment").sentence, shipped("apartment").why], "greyed with the server's reason");
  assert.equal(page.chooseOf("apartment"), undefined, "no Choose for a start not offered");
  assert.equal(apt.style.opacity, "0.6", "greyed");

  page.answer(answer({ starts: shippedOffers({ cabin: 1, homestead: 12 }) }));
  assert.equal(rowLines(page, page.row("cabin"))[2], "1 free");
  assert.equal(rowLines(page, page.row("homestead"))[2], "12 free");
  assert.equal(page.chooseOf("homestead").disabled, false);

  page.answer(answer({}));
  assert.ok(page.lines().includes("This server has no homes to choose from right now."), "no starts at all says so");
});

// ── 3. The server's words, never a copy ────────────────────────────────────

// Red (the plan's own break, 3.9 and lane 1F): chat-home.js naming the starts from a table typed into
// it (`var NAMES = { cabin: 'Cabin', apartment: 'Apartment', homestead: 'Homestead' }`, shown as
// `NAMES[s.id] || s.name`): "the cabin row says the server's name" (it said "Cabin", not "Bunk Room");
// test 4 failed with it too, "the home chosen, as yours".
test("every name and sentence is the answer's: renamed starts show the new names, never the shipped ones", () => {
  const page = loadPage();
  const offers = RENAMED.map((s, i) => offer(s, [3, 0, 9, 1][i]));
  page.answer(answer({ starts: offers }));
  for (const s of RENAMED) {
    const lines = rowLines(page, page.row(s.id));
    assert.equal(lines[0], s.name, `the ${s.id} row says the server's name`);
    assert.equal(lines[1], s.sentence, `and the server's sentence`);
  }
  assert.ok(rowLines(page, page.row("pod")).includes(RENAMED[1].why), "the server's reason for a start not offered");
  assert.equal(page.chooseOf("cabin").getAttribute("aria-label"), "Choose a Bunk Room");
  assert.equal(page.chooseOf("loft").getAttribute("aria-label"), "Choose an Orbital Loft");
  for (const s of SHIPPED) {
    assert.ok(!page.text().includes(s.name), `no shipped name: ${s.name}`);
    assert.ok(!page.text().includes(s.sentence), `no shipped sentence: ${s.id}`);
  }

  page.answer(answer({ plot: held("c4", "cabin"), starts: offers }));
  assert.ok(page.lines().includes("Where you live here: a Bunk Room, c4."), "the held home in the server's name: " + page.lines().join(" | "));
  assert.ok(page.lines().includes(RENAMED[0].sentence));
  page.answer(answer({ plot: held("L1", "loft"), starts: offers }));
  assert.ok(page.lines().includes("Where you live here: an Orbital Loft, L1."));
  for (const s of SHIPPED) assert.ok(!page.text().includes(s.name), `no shipped name: ${s.name}`);
});

// ── 4. Choose ──────────────────────────────────────────────────────────────

// Red: chat-home.js sending the start's name (`chooseHome(s.name)`): "Choose sends the start's id"
// ({type: 'game_choose_home', start: 'Bunk Room'} against start 'cabin').
test("Choose sends game_choose_home with the start's id and nothing else, then shows the home as yours", () => {
  const page = loadPage();
  const offers = RENAMED.map((s, i) => offer(s, [3, 0, 9, 1][i]));
  page.answer(answer({ starts: offers }));
  page.sock.sent.length = 0;
  page.chooseOf("cabin").click();
  assert.deepEqual(page.sock.sent, [{ type: CHOOSE_HOME, start: "cabin" }], "Choose sends the start's id");
  assert.equal(page.status(), "Asking the server for a Bunk Room...", "in the server's words");
  assert.ok(page.chooseButtons().every((b) => b.disabled), "every Choose is off while it waits");
  page.chooseOf("homestead").click();
  assert.equal(page.sock.sent.length, 1, "and a second click sends nothing");

  page.answer(answer({ plot: held("c4", "cabin"), starts: offers.map((o) => (o.id === "cabin" ? { ...o, free: 2 } : o)), chose: true }));
  assert.ok(page.lines().includes("Where you live here: a Bunk Room, c4."), "the home chosen, as yours");
  assert.equal(page.status(), "Chosen: this is your home on this server now.");
  assert.equal(page.chooseButtons().length, 0, "and no choice now that one is held");
  assert.equal(page.timers.length, 0, "nothing left waiting");
});

// ── 5. Refusals ────────────────────────────────────────────────────────────

// Red: chat-home.js showing its own sentence for each refusal (a table keyed by `refused`, with
// "No cabin is free on this ship right now." for full, and so on, ahead of `message`): "the
// holds_one refusal shows the server's own sentence" (the first one tried).
test("each refusal shows the server's own sentence, word for word", () => {
  const offers = shippedOffers({ cabin: 0, homestead: 4 });
  // What the relay's answer carries with each (plan 3.5): your home with holds_one, the starts with
  // full and not_offered, nothing with no_ship and again.
  const shapes = {
    holds_one: { plot: held("p3", "homestead"), starts: offers },
    full: { starts: offers },
    not_offered: { starts: offers },
    no_ship: {},
    again: {},
  };
  assert.deepEqual(Object.keys(shapes), REFUSALS, "every refusal in starts.rs is tried");
  for (const refused of REFUSALS) {
    const page = loadPage();
    page.answer(answer({ starts: offers }));
    const message = `The server's own words for ${refused}, 7 of them.`;
    page.answer(answer({ ...shapes[refused], refused, message }));
    assert.equal(page.status(), message, `the ${refused} refusal shows the server's own sentence`);
    if (refused === "holds_one") {
      assert.ok(page.lines().includes("Where you live here: a Homestead, p3."), "holds_one: your home");
      assert.equal(page.chooseButtons().length, 0);
    } else {
      assert.ok(page.lines().includes("You have no home on this server yet."));
      assert.equal(page.rows().length, 3, `${refused}: the choices stay`);
    }
  }
  // As the first answer, no_ship has nothing to keep, and says only its sentence.
  const page = loadPage();
  page.answer(answer({ refused: "no_ship", message: "This server's ship did not load, so there is no home to choose." }));
  assert.equal(page.rows().length, 0);
  assert.ok(!page.lines().includes("This server has no homes to choose from right now."), "the refusal says why, nothing else");
  assert.equal(page.status(), "This server's ship did not load, so there is no home to choose.");
  // A refusal that came without words still says something.
  page.answer(answer({ refused: "full" }));
  assert.equal(page.status(), "The server did not give you that home.");
  // The next plain answer clears it.
  page.answer(answer({ starts: offers }));
  assert.equal(page.status(), "");
});

// ── 6. Nothing of anyone else ──────────────────────────────────────────────

// Red: chat-home.js adding a details line with the whole answer as received
// (`body.appendChild(el('pre', '', JSON.stringify(game)))` in renderWhereYouLive): "nobody else's
// name reaches the window: Bo Lindqvist" (and test 1, "only the held home's words").
test("nothing of anyone else reaches the window: only the answer's own fields are read", () => {
  const page = loadPage();
  const someone = "a1b2c3d4".repeat(8);
  const a = answer({ starts: shippedOffers({ cabin: 3 }) });
  a.holders = [{ plot: "c3", name: "Bo Lindqvist", key: someone }];
  a.starts[0].held_by = ["Cy Okafor"];
  page.answer(a);
  const b = answer({ plot: { ...held("c2", "cabin"), neighbours: ["Dee Mwangi in c3"] }, starts: shippedOffers({ cabin: 3 }) });
  b.who_lives_where = { c3: "Bo Lindqvist" };
  for (const ans of [a, b]) {
    page.answer(ans);
    const t = page.text();
    for (const other of ["Bo Lindqvist", "Cy Okafor", "Dee Mwangi", someone, "c3"]) {
      assert.ok(!t.includes(other), `nobody else's name reaches the window: ${other}`);
    }
  }
  assert.ok(page.lines().includes("Where you live here: a Cabin, c2."), "only your own home");
  assert.ok(page.lines().includes("Your home on this server's ship. The server tells you about your own home only: nobody is told who lives where."), "and the window says so");
});

// ── 7. app.js hands the answer over ────────────────────────────────────────

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
function storage() { const m = new Map(); return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k) }; }

/** The real app.js, loaded as the chat page loads it (its DOM a stand-in that accepts anything). */
function loadApp(page) {
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: anything(),
    navigator: anything(),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", origin: "https://localhost", reload() {} },
    localStorage: storage(),
    sessionStorage: storage(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: () => 0, clearTimeout() {}, setInterval: () => 0, clearInterval() {},
    addEventListener() {}, removeEventListener() {},
    matchMedia: () => anything(),
    requestAnimationFrame: () => 0,
    crypto: globalThis.crypto, TextEncoder, TextDecoder, URL, URLSearchParams,
    WebSocket: { OPEN: 1 },
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "chat", "app.js"), "utf8"), ctx, { filename: "app.js" });
  // The page's scripts share one global scope: the window's function is the one app.js finds.
  ctx.renderWhereYouLive = page.ctx.renderWhereYouLive;
  const handle = vm.runInContext("handleMessage", ctx);
  return { ctx, frame: (game) => handle({ type: "system", message: "__game__:" + JSON.stringify(game) }) };
}

// Red: app.js without its call to renderWhereYouLive under `case 'game_home':`: "app.js hands the
// relay's game_home to the window" (the window showed its title, intro and note, and no home).
test("app.js hands the relay's game_home to the window, through its own dispatch", async () => {
  const page = loadPage();
  const app = loadApp(page);
  await app.frame(answer({ plot: held("c2", "cabin"), starts: shippedOffers({ cabin: 14 }) }));
  assert.ok(page.lines().includes("Where you live here: a Cabin, c2."), "app.js hands the relay's game_home to the window: " + page.lines().join(" | "));
  assert.equal(page.status(), "");
  // An answer that arrives before the window was ever opened (the game on this account asked, say)
  // is kept and shown when it opens, while a fresh one is asked for.
  const later = loadPage({ open: false });
  const app2 = loadApp(later);
  await app2.frame(answer({ plot: held("c5", "cabin"), starts: shippedOffers({ cabin: 11 }) }));
  assert.equal(later.overlay(), null, "nothing is built until it is opened");
  later.ctx.openWhereYouLive();
  assert.ok(later.lines().includes("Where you live here: a Cabin, c5."), "kept until the window opens");
  assert.deepEqual(later.sock.sent, [{ type: HOME_REQUEST }], "and opening asks again");
});

// ── 8. The command palette, and the page ───────────────────────────────────

/** chat-ui.js's CMD_PALETTE_ACTIONS, as written there, evaluated with the window's functions. */
function paletteActions(page) {
  const ui = fs.readFileSync(path.join(WEB, "chat", "chat-ui.js"), "utf8");
  const at = ui.indexOf("const CMD_PALETTE_ACTIONS = {");
  assert.ok(at >= 0, "chat-ui.js has the palette's actions");
  const end = ui.indexOf("\n};", at);
  const literal = ui.slice(at + "const CMD_PALETTE_ACTIONS = ".length, end + 2);
  const ctx = { openWhereYouLive: page.ctx.openWhereYouLive, window: {} };
  vm.createContext(ctx);
  return vm.runInContext("(" + literal + ")", ctx);
}

// Red: chat-ui.js without its `openWhereYouLive` action: "the palette knows the action" (the item in
// data/commands.json would be drawn and do nothing). And, alone, index.html without the
// chat-home.js script tag: "the chat page loads chat-home.js once, with a ?v= stamp".
test("the command palette opens it, and opening it asks the server; the chat page loads it", () => {
  const commands = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "commands.json"), "utf8"));
  const found = [];
  for (const cat of commands.categories) for (const item of cat.items) if (item.action_id === "openWhereYouLive") found.push({ cat, item });
  assert.equal(found.length, 1, "data/commands.json has one Where you live item");
  assert.equal(found[0].item.label, "Where you live");
  assert.equal(found[0].cat.requires_role, null, "for everyone, not only admins");

  const page = loadPage({ open: false });
  const actions = paletteActions(page);
  assert.equal(typeof actions.openWhereYouLive, "function", "the palette knows the action");
  actions.openWhereYouLive();
  assert.ok(page.overlay() && page.overlay().classList.contains("open"), "the window opens");
  assert.deepEqual(page.sock.sent, [{ type: HOME_REQUEST }], "and asks the server where you live");
  assert.ok(page.lines().includes("Where you live"), "titled Where you live");
  // Closing, then the palette again: the same window, asked again.
  page.dom.getElementById("where-close").click();
  assert.ok(!page.overlay().classList.contains("open"), "Close closes it");
  actions.openWhereYouLive();
  assert.equal(page.dom.all(page.dom.body).filter((n) => n.id === "where-overlay").length, 1, "one window, built once");
  assert.equal(page.sock.sent.length, 2);
  page.dom.getElementById("where-refresh").click();
  assert.deepEqual(page.sock.sent[2], { type: HOME_REQUEST }, "Refresh asks again");

  const index = fs.readFileSync(path.join(WEB, "chat", "index.html"), "utf8");
  const tags = [...index.matchAll(/<script src="\/chat\/chat-home\.js\?v=\d+"><\/script>/g)];
  assert.equal(tags.length, 1, "the chat page loads chat-home.js once, with a ?v= stamp");
  assert.ok(index.indexOf("/chat/app.js?v=") < tags[0].index, "after app.js, whose socket it uses");
});

// ── 9. No connection, no answer ────────────────────────────────────────────

// Red: chat-home.js's connected() without its readyState check (`typeof ws !== 'undefined' && !!ws`):
// "'Asking the server where you live...' == 'Not connected to the server.'" (the request went to
// the closed socket).
test("no connection says so and sends nothing; no answer says the server has not answered", () => {
  const closed = loadPage({ open: false });
  closed.sock.readyState = 3;
  closed.ctx.openWhereYouLive();
  assert.equal(closed.status(), "Not connected to the server.");
  closed.answer(answer({ starts: shippedOffers({ cabin: 2 }) }));
  closed.chooseOf("cabin").click();
  assert.deepEqual(closed.sock.sent, [], "nothing is sent on a closed socket");
  assert.equal(closed.status(), "Not connected to the server.");

  const page = loadPage();
  assert.equal(page.timers.length, 1, "it waits for the answer");
  assert.equal(page.timers[0].ms, 10000, "ten seconds");
  page.fireTimers();
  assert.equal(page.status(), "This server has not answered. It may not offer homes yet.");
  // An answer in time clears the wait.
  const ok = loadPage();
  ok.answer(answer({ starts: shippedOffers({ cabin: 2 }) }));
  assert.equal(ok.timers.length, 0, "an answer clears the wait");
  ok.chooseOf("cabin").click();
  ok.fireTimers();
  assert.equal(ok.status(), "This server has not answered. It may not offer homes yet.", "a choice unanswered says so too");
  assert.equal(ok.chooseOf("cabin").disabled, false, "and Choose is on again");
});
