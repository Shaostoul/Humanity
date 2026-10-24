// The website's How-to page (web/pages/how-to-app.js) and its rules (web/shared/how-to-logic.js),
// run under Node against the shipped data/how_to/ (docs/design/how-to.md increment 1, 2026-10-23).
//
// The page runs as it does in the browser, on the stand-in document of files-web-harness.js (which
// holds only what the page built in it: a test that finds a button proves the page made it). The
// network is a stand-in that answers /data/... from this repository's data/ folder; the runs are an
// in-memory list behind the same load/save the browser's IndexedDB store has.
//
// The rules are held to the desktop's: the same tests as src/how_to/tests.rs where the two share a
// rule (Short never hides a serious or lethal note; a No goes to its problem or back to its step; a
// run survives its how-to being reordered and edited; search finds a symptom).
//
//   node --test scripts/tests/how-to-web.test.js

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const H = require("./files-web-harness.js");
const ROOT = H.ROOT;
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const L = require(path.join(WEB, "shared", "how-to-logic.js"));
const PANES = require(path.join(WEB, "pages", "files-panes.js"));
const HOW = require(path.join(WEB, "shared", "how-it-works.js"));
const PAGE = require(path.join(WEB, "pages", "how-to-app.js"));

const data = (rel) => JSON.parse(fs.readFileSync(path.join(ROOT, "data", rel), "utf8"));
const INDEX = data("how_to/index.json");
const body = (id) => data("how_to/" + INDEX.how_tos.find((e) => e.id === id).file);

/** A network that answers /data/<path> from data/. */
function dataFetch() {
  const asked = [];
  return {
    asked,
    fetch(url) {
      asked.push(url);
      const rel = url.replace(/^\/data\//, "");
      const p = path.join(ROOT, "data", rel);
      if (!url.startsWith("/data/") || !fs.existsSync(p)) return Promise.resolve({ ok: false, status: 404, json: () => Promise.reject(new Error("404")) });
      const text = fs.readFileSync(p, "utf8");
      return Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve(JSON.parse(text)) });
    },
  };
}

function memoryRuns(start) {
  const kept = (start || []).map((r) => JSON.parse(JSON.stringify(r)));
  return { kept, load: () => Promise.resolve(kept.map((r) => JSON.parse(JSON.stringify(r)))), save: (r) => { const i = kept.findIndex((x) => x.run === r.run); if (i >= 0) kept[i] = r; else kept.push(r); return Promise.resolve(); } };
}

async function boot(o) {
  o = o || {};
  const doc = H.fakeDocument();
  const root = doc.createElement("main");
  root.setAttribute("id", "howto-root");
  const announce = doc.createElement("div");
  doc.body.appendChild(root);
  doc.body.appendChild(announce);
  const net = dataFetch();
  const runs = o.runs || memoryRuns();
  let clock = o.now || 1_792_789_331_000;
  const location = { hash: o.hash || "" };
  const page = PAGE.createHowToPage({
    document: doc, root, announce, fetch: net.fetch, storage: H.fakeStorage(), runs,
    now: () => clock, location, history: null, setInterval: null, logic: L, panes: PANES, how: HOW, win: null,
    copy: () => {},
  });
  await page.start();
  await settle();
  return { doc, root, announce, net, runs, page, location, advance: (ms) => { clock += ms; } };
}

const settle = () => new Promise((r) => setTimeout(r, 0));
const all = (root) => { const out = []; const walk = (n) => { for (const c of n.children) { out.push(c); walk(c); } }; walk(root); return out; };
const textOf = (root) => root.textContent;
const buttons = (root, words) => all(root).filter((e) => e.tagName === "BUTTON" && e.textContent === words);

test("1. the page reads the same four files as the desktop, and lists every how-to and every planned one", async () => {
  const { root, net } = await boot();
  for (const f of ["/data/how_to/words.json", "/data/how_to/categories.json", "/data/how_to/planned.json", "/data/how_to/index.json"]) {
    assert.ok(net.asked.includes(f), "reads " + f);
  }
  const t = textOf(root);
  assert.ok(t.includes("How-to"), "the heading");
  for (const e of INDEX.how_tos) assert.ok(t.includes(e.title), "lists " + e.title);
  assert.ok(t.includes("Not written yet"), "planned ones are listed dimmed");
  assert.ok(t.includes("6 written, 48 planned"), "All how-tos counts both");
  for (const c of data("how_to/categories.json").categories) assert.ok(t.includes(c.title), "the tree has " + c.title);
});

test("2. a lethal how-to: Stop and call first, the lethal line in Short and in Full, the reasons in Full only", async () => {
  for (const view of ["short", "full"]) {
    const { root, page } = await boot();
    page.state.view = view;
    await page.openAndLoad("mix_ors_packet");
    const t = textOf(root);
    assert.ok(t.includes("Stop and call someone if"), view + ": the stop box");
    assert.ok(t.indexOf("Stop and call someone if") < t.indexOf("Steps"), view + ": before the steps");
    assert.ok(t.includes("Use exactly the amount of water the packet says."), view + ": the lethal line at its step");
    assert.ok(t.includes("Can kill you if done wrong"), view + ": the hazard in words");
    assert.strictEqual(t.includes("Packets are made for different amounts of water"), view === "full", view + ": a step's reason is Full's alone");
  }
});

test("3. a run: ticking starts it and keeps it; a No points at its problem; it is still there after a reload", async () => {
  const one = await boot();
  await one.page.openAndLoad("change_furnace_filter");
  const tick = all(one.root).find((e) => e.tagName === "INPUT" && e.id === "tick-power_off");
  assert.ok(tick, "the first step has a box to tick");
  tick.checked = true;
  tick.dispatch("change");
  await settle();
  assert.strictEqual(one.runs.kept.length, 1, "a run was kept");
  assert.ok(one.runs.kept[0].steps.power_off.done, "the tick was kept, by the step's id");
  const no = all(one.root).filter((e) => e.tagName === "BUTTON" && e.textContent === "No");
  assert.ok(no.length, "the arrow check has No");
  no[0].click();
  await settle();
  assert.strictEqual(one.runs.kept[0].steps.arrow.went_to, "arrow_backwards");
  const lit = all(one.root).find((e) => e.id === "problem-arrow_backwards");
  assert.ok(lit && lit.className.includes("howto-problem-lit"), "its problem is marked");

  const two = await boot({ runs: memoryRuns(one.runs.kept) });
  assert.ok(textOf(two.root).includes("Doing now"), "after a reload the open run is in the rail");
  await two.page.openAndLoad("change_furnace_filter");
  assert.ok(textOf(two.root).includes("Step 2 of"), "where it is");
});

test("4. a timer runs from its stored end, and says so when it is up", async () => {
  const b = await boot();
  await b.page.openAndLoad("boil_water_safe");
  for (const [step, option] of [["cloudy", "Clear"], ["height", "Lower than 1,000 metres (about 3,300 feet)"]]) {
    const btn = buttons(b.root, option)[0];
    assert.ok(btn, "the option " + option + " of " + step);
    btn.click();
    await settle();
  }
  const startTimer = buttons(b.root, "Start the timer")[0];
  assert.ok(startTimer, "the one-minute boil has a timer");
  startTimer.click();
  await settle();
  assert.ok(textOf(b.root).includes("1:00 left"), "a minute left");
  b.advance(61_000);
  b.page.tick();
  await settle();
  assert.ok(b.announce.textContent.includes("Time is up: Boil water to make it safe to drink"), "the live region says so");
  assert.ok(textOf(b.root).includes("Time is up"), "and the step");
});

test("5. search finds a how-to by what you see when something goes wrong", async () => {
  const b = await boot();
  const box = all(b.root).find((e) => e.tagName === "INPUT" && e.className === "howto-search");
  box.value = "springs up";
  box.dispatch("input");
  await settle();
  const cards = all(b.root).filter((e) => e.className && e.className.split(/\s+/).includes("howto-card"));
  assert.deepStrictEqual(cards.map((c) => c.querySelector("strong").textContent), ["Test the seals on home-canned jars"]);
});

test("6. a deep link opens the how-to and its step", async () => {
  const b = await boot({ hash: "#test_jar_seals/look" });
  assert.strictEqual(b.page.state.open, "test_jar_seals");
  assert.ok(all(b.root).some((e) => e.id === "step-look"), "the step is on the page");
});

// ── The rules, as the desktop's ──

test("7. Short never hides a serious or lethal note, on every shipped how-to, whatever is chosen", () => {
  let seen = 0;
  for (const e of INDEX.how_tos) {
    const h = body(e.id);
    const run = L.startRun(h, e.hash, 0);
    for (const s of h.steps.filter((x) => x.kind === "choose")) {
      for (const o of s.options) {
        run.choices[s.id] = o.id;
        const short = L.shown(h, run, "short").map((x) => x.id);
        for (const v of L.visibleSteps(h, run)) {
          if (v.safety && L.isGrave(v.safety.level)) { seen++; assert.ok(short.includes(v.id), e.id + ": " + v.id + " is in Short"); }
        }
      }
    }
  }
  assert.ok(seen > 0);
  assert.ok(L.inShort({ short: false, safety: { level: "lethal" } }), "the second lock: short: false cannot hide a lethal step");
});

test("8. a No goes to its problem, or back to its step; a run survives its how-to being reordered and edited", () => {
  const phrase = body("back_up_recovery_phrase");
  const run = L.startRun(phrase, "", 0);
  for (const s of ["private", "open_account", "unlock", "show", "write"]) L.setDone(run, s, true, 1000);
  L.answer(phrase, run, "check_back", false, 2000);
  assert.ok(!L.isDone(run, "write") && !L.isDone(run, "check_back") && L.isDone(run, "show"));
  assert.strictEqual(L.currentStep(phrase, run).id, "write");

  const furnace = body("change_furnace_filter");
  const r = L.startRun(furnace, "", 0);
  L.setDone(r, "power_off", true, 1);
  L.record(r, "size", "16 x 25 x 1", 2);
  L.choose(r, "filter_kind", "disposable", 3);
  const edited = JSON.parse(JSON.stringify(furnace));
  edited.steps.reverse();
  edited.steps = edited.steps.filter((s) => s.id !== "power_off");
  assert.deepStrictEqual(L.removedSteps(edited, r), ["power_off"]);
  assert.strictEqual(r.steps.size.value, "16 x 25 x 1");
  assert.strictEqual(L.currentStep(edited, r).id, "power_on");
});

test("9. the website's hash of every shipped how-to is the index's (the Rust test compares the index with its own)", () => {
  const blake3 = require(path.join(WEB, "shared", "blake3.js"));
  for (const e of INDEX.how_tos) assert.strictEqual(L.hash(body(e.id), blake3), e.hash, e.id);
});
