// The verdict of `just verify-phone-start` (scripts/lib/phone-start-judge.js; Android rung A1,
// 2026-11-20): a phone starts pages-only and draws only on change, and a desktop starts pages-first
// and prepares the 3D world when Play is pressed (the default) or beside the pages
// (engine/world_prep.rs). Pure node, no game: each test feeds the judge run.log lines shaped as
// engine/redraw.rs, engine/renderer_slot.rs and engine/world_prep.rs write them, and breaks one rule
// at a time. A judge that cannot fail proves nothing, so every rule here is seen failing on its own.
//
// RED FIRST (2026-11-20, the desktop's pages-first start): this file's goodRun (a desktop that starts
// pages-only and prepares the world on Play, a background boot, a play_during boot, an old_order boot)
// was run against the judge before the change: eleven of the fifteen tests failed, the good run with
// "desktop: a full start (pages-only start: yes)" and "desktop: the world's renderer built at start
// (6 render pipelines)". The old judge could not tell a pages-first desktop from a broken one. Each
// rule below fails on its own against the judge as it is now.

"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const path = require("node:path");
const PJ = require(path.join(__dirname, "..", "lib", "phone-start-judge.js"));

const first = (ms, yes) => `[Boot] first frame drawn ${ms} ms after the window opened (pages-only start: ${yes ? "yes" : "no"})`;
const counts = (r, c, s, when, built) => `[PagesOnly] wgpu holds ${r} render pipelines, ${c} compute pipelines, ${s} shader modules ${when}; the world's renderer built: ${built ? "yes" : "no"}`;
const win = (mode, frames, wakes) => `[Redraw] ${mode}: ${frames} frames drawn in the last 10.0 s, ${wakes} wakes without one`;
const ready = (at, since) => `[WorldPrep] the 3D world is ready: ${at} ms after the window opened, ${since} ms after its preparation began`;
const onFrame = (worst, n) => `[WorldPrep] on-frame work while it was prepared: worst ${worst} ms over ${n} frames (budget 2 ms)`;
const during = (n, mean, worst) => `[WorldPrep] page frames while it was prepared: ${n}, ${mean} ms apart on average, worst ${worst} ms`;
const pf = (n, mean, worst, world) => `[WorldPrep] page frames in the last 5.0 s: ${n}, ${mean} ms apart on average, worst ${worst} ms; the world: ${world}`;
const pressed = "[WorldPrep] Play pressed before the world was ready: finishing it now, the page shows how far along it is";
const opening = (ms) => `[WorldPrep] opening the world ${ms} ms after Play was pressed`;
const loaded = (ms) => `3D world loaded in ${ms}ms`;

function goodRun() {
  return {
    boots: {
      desktop: {
        log: [
          first(1100, true),
          counts(6, 0, 6, "at the first frame", false),
          pressed,
          "[PagesOnly] the world's renderer built for entering the world: 7900 ms (on a worker thread)",
          ready(10200, 9100),
          onFrame("0.44", 640),
          opening(9050),
          loaded(2100),
          counts(70, 12, 85, "now", true),
        ].join("\n"),
        entered: { ok: true, waited_for_world_ms: 11300 },
      },
      background: {
        log: [
          first(1080, true),
          counts(6, 0, 6, "at the first frame", false),
          "[PagesOnly] the world's renderer built for the background, after the first page frame: 9400 ms (on a worker thread)",
          pf(600, 8.3, 14.2, "being prepared"),
          ready(11800, 10600),
          onFrame("0.41", 1270),
          during(1270, 8.4, 16.9),
          pf(600, 8.3, 9.1, "ready"),
          loaded(2100),
        ].join("\n"),
        entered: { ok: true, waited_for_world_ms: 2150 },
      },
      play_during: {
        log: [first(1050, true), counts(6, 0, 6, "at the first frame", false), pressed, ready(9900, 8800), onFrame("0.38", 900), opening(8600), loaded(2200)].join("\n"),
        entered: { ok: true, waited_for_world_ms: 10900 },
      },
      old_order: { log: [first(18500, false), counts(64, 9, 80, "at the first frame", true)].join("\n") },
      phone: {
        log: [first(1400, true), counts(3, 0, 3, "at the first frame", false), win("reactive", 40, 12), counts(3, 0, 3, "now", false), win("reactive", 0, 40), win("reactive", 0, 40), counts(3, 0, 3, "now", false)].join("\n"),
        idle_from: 1,
      },
      before: { log: [first(1500, true), win("continuous", 590, 0), win("continuous", 600, 0)].join("\n") },
    },
  };
}
const verdict = (run) => PJ.judgePhoneStart(run);
const failing = (run) => verdict(run).checks.filter((c) => !c.ok).map((c) => c.what);

test("a good run passes and states the numbers", () => {
  const r = verdict(goodRun());
  assert.ok(r.ok, JSON.stringify(failing(goodRun())));
  assert.deepStrictEqual(r.numbers.first_frame_ms, { old_order: 18500, desktop: 1100, background: 1080, phone: 1400 });
  assert.strictEqual(r.numbers.idle_window.after.frames, 0);
  assert.strictEqual(r.numbers.idle_window.before.frames, 600);
  assert.strictEqual(r.numbers.world_build_on_entry_ms, 7900);
  assert.strictEqual(r.numbers.world_ready_after_play_ms, 9100);
  assert.strictEqual(r.numbers.world_ready_ms, 11800);
  assert.deepStrictEqual(r.numbers.on_frame_worst_ms, { desktop: 0.44, background: 0.41 });
  assert.deepStrictEqual(r.numbers.page_frames_during, { count: 1270, mean_ms: 8.4, worst_ms: 16.9 });
  assert.deepStrictEqual(r.numbers.page_frames_after, { count: 600, mean_ms: 8.3, worst_ms: 9.1 });
  assert.deepStrictEqual(r.numbers.play, { on_play_ms: 11300, during_ms: 10900, after_ms: 2150 });
  const lines = PJ.verdictLines(r).join("\n");
  assert.match(lines, /PHONE START: PASS/);
  assert.match(lines, /the world ready after Play: 9100 ms/);
  assert.match(lines, /background: the world ready 11800 ms after the window opened/);
});

test("a desktop start must draw its pages before any world pipeline exists", () => {
  for (const b of ["desktop", "background"]) {
    let run = goodRun();
    run.boots[b].log = run.boots[b].log.replace(counts(6, 0, 6, "at the first frame", false), counts(64, 9, 80, "at the first frame", true));
    assert.ok(failing(run).some((w) => new RegExp(`${b}: the pages drawn before any world pipeline existed \\(64 render, 9 compute, the world built: yes\\)`).test(w)), failing(run).join("; "));
    run = goodRun();
    run.boots[b].log = run.boots[b].log.replace(/first frame drawn (\d+) ms after the window opened \(pages-only start: yes\)/, "first frame drawn 18500 ms after the window opened (pages-only start: no)");
    assert.ok(failing(run).some((w) => new RegExp(`${b}: a pages-first start \\(pages-only start: no\\)`).test(w)), failing(run).join("; "));
  }
});

test("the default start prepares the world on Play, waits for it, and enters", () => {
  let run = goodRun();
  run.boots.desktop.log = run.boots.desktop.log.replace(pressed + "\n", "");
  assert.ok(failing(run).some((w) => /desktop: Play waited for the world it prepares \(no such line\)/.test(w)), failing(run).join("; "));
  run = goodRun();
  run.boots.desktop.log = run.boots.desktop.log.replace("built for entering the world", "built for a reach through Deref");
  assert.ok(failing(run).some((w) => /desktop: the world's renderer built for entering the world/.test(w)), failing(run).join("; "));
  run = goodRun();
  run.boots.desktop.log = run.boots.desktop.log.replace(ready(10200, 9100) + "\n", "");
  assert.ok(failing(run).some((w) => /desktop: the world ready after Play \(never\)/.test(w)), failing(run).join("; "));
  run = goodRun();
  run.boots.desktop.entered = { ok: false, error: "no world" };
  assert.ok(failing(run).some((w) => /desktop: the world entered/.test(w)), failing(run).join("; "));
  run = goodRun();
  run.boots.desktop.log = run.boots.desktop.log.replace(counts(70, 12, 85, "now", true), counts(6, 0, 6, "now", false));
  assert.ok(failing(run).some((w) => /desktop: the world's pipelines exist after entry/.test(w)), failing(run).join("; "));
});

test("the frame's share of the preparation must stay in its 2 ms budget", () => {
  for (const [b, good] of [["desktop", "0.44"], ["background", "0.41"]]) {
    let run = goodRun();
    run.boots[b].log = run.boots[b].log.replace(new RegExp(`worst ${good} ms`), "worst 3.10 ms");
    assert.ok(failing(run).some((w) => new RegExp(`${b}: the frame's share of the preparation never over 2 ms \\(worst 3\\.1 ms`).test(w)), failing(run).join("; "));
    run = goodRun();
    run.boots[b].log = run.boots[b].log.split("\n").filter((l) => !/on-frame work/.test(l)).join("\n");
    assert.ok(failing(run).some((w) => new RegExp(`${b}: .*never over 2 ms \\(no measurement\\)`).test(w)), failing(run).join("; "));
  }
});

test("the background choice must prepare the world before Play, and enter", () => {
  let run = goodRun();
  run.boots.background.log = run.boots.background.log.replace(ready(11800, 10600) + "\n", "");
  assert.ok(failing(run).some((w) => /background: the world prepared beside the pages, before Play \(never\)/.test(w)), failing(run).join("; "));
  run = goodRun();
  run.boots.background.log += "\n" + pressed;
  assert.ok(failing(run).some((w) => /background: .*Play had to wait/.test(w)), failing(run).join("; "));
  run = goodRun();
  run.boots.background.entered = { ok: false };
  assert.ok(failing(run).some((w) => /background: the world entered once it was ready/.test(w)), failing(run).join("; "));
});

test("Play during the background preparation must finish it and enter", () => {
  let run = goodRun();
  run.boots.play_during.log = run.boots.play_during.log.replace(pressed + "\n", "");
  assert.ok(failing(run).some((w) => /play_during: Play pressed before the world was ready/.test(w)), failing(run).join("; "));
  run = goodRun();
  run.boots.play_during.log = run.boots.play_during.log.replace(opening(8600) + "\n", "");
  assert.ok(failing(run).some((w) => /play_during: the world opened by itself once ready/.test(w)), failing(run).join("; "));
  run = goodRun();
  run.boots.play_during.entered = { ok: false };
  assert.ok(failing(run).some((w) => /play_during: the world entered/.test(w)), failing(run).join("; "));
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
  for (const b of ["desktop", "background", "play_during"]) {
    const r = goodRun();
    r.boots[b].log += "\n[PagesOnly] world touched from src\\engine\\frame_sky.rs:42";
    assert.ok(failing(r).some((w) => new RegExp(`${b}: no reach for the world`).test(w)), failing(r).join("; "));
  }
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

test("the old order must build the world at start", () => {
  const run = goodRun();
  run.boots.old_order.log = [first(1400, true), counts(3, 0, 3, "at the first frame", false)].join("\n");
  const f = failing(run);
  assert.ok(f.some((w) => /old_order: a full start \(pages-only start: yes\)/.test(w)), f.join("; "));
  assert.ok(f.some((w) => /old_order: the world's renderer built at start/.test(w)));
});

test("--only judges the boots it asked for, and still fails a bad one", () => {
  const run = goodRun();
  run.only = ["phone"];
  for (const b of PJ.BOOTS.filter((b) => b !== "phone")) delete run.boots[b];
  assert.ok(verdict(run).ok, failing(run).join("; "));
  run.boots.phone.log += "\n[PagesOnly] world touched from src\\engine\\frame_sky.rs:42";
  assert.ok(!verdict(run).ok);
});

test("a panic in any boot fails, and a missing boot fails", () => {
  assert.deepStrictEqual(PJ.BOOTS, ["desktop", "background", "play_during", "old_order", "phone", "before"]);
  for (const b of PJ.BOOTS) {
    const run = goodRun();
    run.boots[b].log += "\nPANIC at src/x.rs:1";
    assert.ok(failing(run).some((w) => /no panic \(1\)/.test(w)), b);
    const gone = goodRun();
    delete gone.boots[b];
    assert.ok(!verdict(gone).ok, `${b} missing`);
  }
});
