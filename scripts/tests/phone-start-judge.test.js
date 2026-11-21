// The verdict of `just verify-phone-start` (scripts/lib/phone-start-judge.js; Android rung A1,
// 2026-11-20): a phone starts pages-only and draws only on change. Pure node, no game: each test
// feeds the judge run.log lines shaped as engine/redraw.rs and engine/renderer_slot.rs write them,
// and breaks one rule at a time. A judge that cannot fail proves nothing, so every rule here is
// seen failing on its own.

"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const path = require("node:path");
const PJ = require(path.join(__dirname, "..", "lib", "phone-start-judge.js"));

const first = (ms, yes) => `[Boot] first frame drawn ${ms} ms after the window opened (pages-only start: ${yes ? "yes" : "no"})`;
const counts = (r, c, s, when, built) => `[PagesOnly] wgpu holds ${r} render pipelines, ${c} compute pipelines, ${s} shader modules ${when}; the world's renderer built: ${built ? "yes" : "no"}`;
const win = (mode, frames, wakes) => `[Redraw] ${mode}: ${frames} frames drawn in the last 10.0 s, ${wakes} wakes without one`;

function goodRun() {
  return {
    boots: {
      desktop: { log: [first(9200, false), counts(64, 9, 80, "at the first frame", true)].join("\n") },
      phone: {
        log: [first(1400, true), counts(3, 0, 3, "at the first frame", false), win("reactive", 40, 12), counts(3, 0, 3, "now", false), win("reactive", 0, 40), win("reactive", 0, 40), counts(3, 0, 3, "now", false)].join("\n"),
        idle_from: 1,
      },
      before: { log: [first(1500, true), win("continuous", 590, 0), win("continuous", 600, 0)].join("\n") },
      pages_first: {
        log: [first(1450, true), counts(3, 0, 3, "at the first frame", false), "[PagesOnly] the world's renderer built for entering the world: 5100 ms", win("continuous", 300, 0), counts(70, 9, 85, "now", true)].join("\n"),
        entered: { ok: true },
      },
    },
  };
}
const verdict = (run) => PJ.judgePhoneStart(run);
const failing = (run) => verdict(run).checks.filter((c) => !c.ok).map((c) => c.what);

test("a good run passes and states the numbers", () => {
  const r = verdict(goodRun());
  assert.ok(r.ok, JSON.stringify(failing(goodRun())));
  assert.deepStrictEqual(r.numbers.first_frame_ms, { desktop: 9200, phone: 1400, pages_first: 1450 });
  assert.strictEqual(r.numbers.idle_window.after.frames, 0);
  assert.strictEqual(r.numbers.idle_window.before.frames, 600);
  assert.strictEqual(r.numbers.world_build_on_entry_ms, 5100);
  assert.match(PJ.verdictLines(r).join("\n"), /PHONE START: PASS/);
});

test("a phone boot that built the world fails", () => {
  const run = goodRun();
  run.boots.phone.log += "\n" + counts(64, 9, 80, "now", true);
  assert.ok(failing(run).some((w) => /never built \(BUILT\)/.test(w)), failing(run).join("; "));
  assert.ok(failing(run).some((w) => /at most 8 render pipelines \(64\)/.test(w)));
  assert.ok(failing(run).some((w) => /no compute pipeline \(9\)/.test(w)));
});

test("a stray reach for the world fails, named by its place", () => {
  const run = goodRun();
  run.boots.phone.log += "\n[PagesOnly] world touched from src\\engine\\frame_sky.rs:42";
  const f = failing(run);
  assert.strictEqual(f.length, 1, f.join("; "));
  assert.match(f[0], /frame_sky\.rs:42/);
});

test("an idle window that keeps drawing fails", () => {
  const run = goodRun();
  run.boots.phone.log = run.boots.phone.log.replace(/0 frames drawn in the last 10\.0 s, 40 wakes/g, "580 frames drawn in the last 10.0 s, 0 wakes");
  assert.ok(failing(run).some((w) => /idle window draws at most 2 frames \(580/.test(w)), failing(run).join("; "));
});

test("the busy window before the idle stretch is not judged as idle", () => {
  const run = goodRun();
  run.boots.phone.idle_from = 0; // the 40-frame page-visiting window now counts as the first idle one
  // The judge takes the SECOND window of the stretch (the first may straddle its start): still 0.
  assert.ok(verdict(run).ok, failing(run).join("; "));
});

test("a phone boot that was not pages-only fails", () => {
  const run = goodRun();
  run.boots.phone.log = run.boots.phone.log.replace(first(1400, true), first(1400, false));
  assert.ok(failing(run).some((w) => /phone: a pages-only start \(no\)/.test(w)));
});

test("a count that cannot see frames fails the before boot", () => {
  const run = goodRun();
  run.boots.before.log = [first(1500, true), win("continuous", 3, 0), win("continuous", 2, 0)].join("\n");
  assert.ok(failing(run).some((w) => /before: drawing every frame/.test(w)));
});

test("pages-first must build the world on entering it, and enter", () => {
  let run = goodRun();
  run.boots.pages_first.log = run.boots.pages_first.log.replace("built for entering the world", "built for a reach through Deref");
  assert.ok(failing(run).some((w) => /built on entering the world/.test(w)));
  run = goodRun();
  run.boots.pages_first.entered = { ok: false, error: "no world" };
  assert.ok(failing(run).some((w) => /the world entered/.test(w)));
  run = goodRun();
  run.boots.pages_first.log = run.boots.pages_first.log.replace(counts(70, 9, 85, "now", true), counts(3, 0, 3, "now", false));
  assert.ok(failing(run).some((w) => /pipelines exist after entry/.test(w)));
});

test("a desktop boot must build the world at start", () => {
  const run = goodRun();
  run.boots.desktop.log = [first(1400, true), counts(3, 0, 3, "at the first frame", false)].join("\n");
  const f = failing(run);
  assert.ok(f.some((w) => /desktop: a full start \(pages-only start: yes\)/.test(w)));
  assert.ok(f.some((w) => /desktop: the world's renderer built at start/.test(w)));
});

test("--only judges the boots it asked for, and still fails a bad one", () => {
  const run = goodRun();
  run.only = ["phone"];
  delete run.boots.desktop;
  delete run.boots.before;
  delete run.boots.pages_first;
  assert.ok(verdict(run).ok, failing(run).join("; "));
  run.boots.phone.log += "\n[PagesOnly] world touched from src\\engine\\frame_sky.rs:42";
  assert.ok(!verdict(run).ok);
});

test("a panic in any boot fails, and a missing boot fails", () => {
  for (const b of ["desktop", "phone", "before", "pages_first"]) {
    const run = goodRun();
    run.boots[b].log += "\nPANIC at src/x.rs:1";
    assert.ok(failing(run).some((w) => /no panic \(1\)/.test(w)), b);
    const gone = goodRun();
    delete gone.boots[b];
    assert.ok(!verdict(gone).ok, `${b} missing`);
  }
});
