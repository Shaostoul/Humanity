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
          during(640, 14.1, 60.2),
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
        log: [first(1050, true), counts(6, 0, 6, "at the first frame", false), pressed, ready(9900, 8800), onFrame("0.38", 900), during(900, 12.2, 70.4), opening(8600), loaded(2200)].join("\n"),
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

// A long gap between two page frames as engine/world_prep.rs `note_spacing` writes it (2026-11-21).
const gapLine = (gap, at, [before, present, between, thisMs], [beforeLongest, thisLongest], world, drawing) =>
  `[WorldPrep] a page frame ${gap} ms after the last (${at} ms after the window opened; the frame before took ${before} ms, ${present} of them presenting, then ${between} ms between frames, and this one ${thisMs} ms to here; the frame before's longest: ${beforeLongest}; this one's: ${thisLongest}); the world: ${world}; drawing: ${drawing}`;

// THE SLOW PAGE FRAME BEFORE PLAY (2026-11-21). Red evidence: the run of 2026-11-21 08:25
// (v0.1567.0, .probe-rig/phone-start/runs/20261121-082542) re-judged with --dry-verdict failed
// "desktop: before Play, no stretch of a page frame over 100 ms (between frames 3037 ms; a gap of
// 3051 ms, 4078 ms after the window opened)" and the same for the background boot (3906 ms), where
// the judge before passed both and printed "worst 3050.6 ms" as a number nobody judged.
test("a page frame before Play with a stretch over the limit fails, named by the stretch", () => {
  const limit = PJ.IDLE_STRETCH_MAX_MS;
  for (const b of ["desktop", "background"]) {
    const run = goodRun();
    run.boots[b].log += "\n" + gapLine(3051, 4078, [3040, 1, 2, 2], [`instrument_packs ${limit * 30} ms, ui_run 9 ms, present 1 ms`, "station 1 ms"], "not started", "continuous");
    assert.ok(
      failing(run).some((w) => new RegExp(`${b}: before Play, no stretch of a page frame over ${limit} ms \\(instrument_packs ${limit * 30} ms in the frame before; a gap of 3051 ms, 4078 ms after the window opened\\)`).test(w)),
      failing(run).join("; "),
    );
    // A gap whose stretches are each in the limit passes, and the frame cap's sleep is pacing,
    // never a cause, however long a cap makes it.
    const ok = goodRun();
    ok.boots[b].log += "\n" + gapLine(limit + 120, 2000, [limit + 80, 30, 0, 50], [`fps_cap_sleep ${limit + 50} ms, present 30 ms`, "network 45 ms"], "not started", "continuous");
    assert.ok(verdict(ok).ok, failing(ok).join("; "));
  }
});

// THE PAGE FRAMES WHILE THE WORLD IS BUILT (2026-11-21). Red evidence: the run of 2026-11-21 09:27
// (the build's threads on every core) re-judged with --dry-verdict failed "desktop: while the world
// was built, at most 5% of the page frames 100 ms or more after the one before (19 of 188; the
// slowest: ui_paint 390 ms in the frame before; ...)" and play_during (17 of 209); v0.1567.0's run
// failed the same two (16 of 221, 23 of 232). With the frame's core left to it (09:55): 7 of 301,
// 0 of 355 and 2 of 278, a pass.
test("long gaps while the world is built fail past their share, the slowest named; a few pass", () => {
  const limit = PJ.BUSY_LONG_GAP_SHARE_MAX;
  for (const b of ["desktop", "background", "play_during"]) {
    const n = 200;
    const many = Math.floor(n * limit) + 1;
    const line = (i) => gapLine(150 + i, 5000 + i * 300, [150 + i, 140, 0, 1], [`present ${140 + i} ms, ui_paint 1 ms`, "ecs 0 ms"], "being prepared, step pipelines", "continuous");
    const run = goodRun();
    run.boots[b].log = run.boots[b].log.replace(/page frames while it was prepared: \d+, /, `page frames while it was prepared: ${n}, `);
    const base = run.boots[b].log;
    run.boots[b].log = base + "\n" + Array.from({ length: many }, (_, i) => line(i)).join("\n");
    assert.ok(
      failing(run).some((w) => new RegExp(`${b}: while the world was built, at most ${Math.round(limit * 100)}% of the page frames 100 ms or more after the one before \\(${many} of ${n}; the slowest: present ${140 + many - 1} ms in the frame before`).test(w)),
      failing(run).join("; "),
    );
    run.boots[b].log = base + "\n" + Array.from({ length: many - 1 }, (_, i) => line(i)).join("\n");
    assert.ok(verdict(run).ok, failing(run).join("; "));
  }
  // No count of the page frames while it was built fails rather than passing on nothing.
  const none = goodRun();
  none.boots.desktop.log = none.boots.desktop.log.replace(/\n\[WorldPrep\] page frames while it was prepared: [^\n]*/, "");
  assert.ok(failing(none).some((w) => /desktop: while the world was built, .*no count of the page frames/.test(w)), failing(none).join("; "));
});

// ONE FROZEN PAGE FRAME WHILE THE WORLD IS BUILT (2026-11-21, BUG-338's third cause). Red evidence:
// re-judged with --dry-verdict, the run of 2026-11-21 10:36 (.probe-rig/phone-start/runs/
// 20261121-103629) failed "desktop: while the world was built, no page frame more than 300 ms after
// the one before (worst 606.8 ms; the slowest named: ui_paint_submit_uploads 363 ms in the frame
// before; ...)", 11:58 (20261121-115841, the screen passes side by side) failed desktop (849.4 ms,
// ui_paint_buffers 502 ms) and play_during (440 ms), and 12:14 (20261121-121459, the gate closed
// at the pipeline step only) desktop (390.9 ms, ui_paint_submit 287 ms), where the share rule
// passed all four; with the gate closed at the worker's start (12:29, 20261121-122917) the worst
// were 127, 198.4 and 120.1 ms, a pass. This test itself failed with the rule broken to pass any
// worst (`worst !== null || ...`), at its message assertion.
test("one page frame frozen past the cap while the world is built fails, named; at the cap it passes", () => {
  const cap = PJ.BUSY_WORST_GAP_MAX_MS;
  const worstIs = (log, ms) => log.replace(/(page frames while it was prepared: \d+, [\d.]+ ms apart on average, worst )[\d.]+/, `$1${ms}`);
  for (const b of ["desktop", "background", "play_during"]) {
    const run = goodRun();
    run.boots[b].log = worstIs(run.boots[b].log, cap + 207);
    run.boots[b].log += "\n" + gapLine(cap + 207, 8435, [cap + 207, 0, 0, 1], [`ui_paint_submit_uploads ${cap - 37} ms, world_passes 241 ms`, "ecs 0 ms"], "being prepared, step effects", "continuous");
    assert.ok(
      failing(run).some((w) => new RegExp(`${b}: while the world was built, no page frame more than ${cap} ms after the one before \\(worst ${cap + 207} ms; the slowest named: ui_paint_submit_uploads ${cap - 37} ms in the frame before`).test(w)),
      failing(run).join("; "),
    );
    const ok = goodRun();
    ok.boots[b].log = worstIs(ok.boots[b].log, cap);
    assert.ok(verdict(ok).ok, failing(ok).join("; "));
  }
});

test("between frames on a desktop is a stretch; on a phone, waiting is idle and never a hitch", () => {
  const run = goodRun();
  run.boots.desktop.log += "\n" + gapLine(900, 3000, [10, 1, 888, 2], ["ui_run 5 ms", "station 1 ms"], "not started", "continuous");
  assert.ok(failing(run).some((w) => /desktop: before Play, no stretch of a page frame over \d+ ms \(between frames 888 ms; a gap of 900 ms/.test(w)), failing(run).join("; "));
  const idle = PJ.classifyGap(PJ.parseLog(gapLine(2750, 3794, [21, 1, 2726, 2], ["ui_run 12 ms", "station 1 ms"], "not started", "reactive")).gaps[0]);
  assert.strictEqual(idle.kind, "idle", JSON.stringify(idle));
  // A reactive app's slow frame is still slow: the frame's own stretch took the gap, not the wait.
  const slow = PJ.classifyGap(PJ.parseLog(gapLine(900, 3000, [880, 1, 15, 2], ["settings_apply 860 ms", "station 1 ms"], "not started", "reactive")).gaps[0]);
  assert.deepStrictEqual([slow.kind, slow.cause.name, slow.cause.ms], ["slow", "settings_apply", 860]);
});

test("a long-gap line from before the laps (no stretches) is judged by its parts, CRLF or not", () => {
  const old = "[2026-11-21T08:25:46.578Z INFO  humanity_engine::engine::world_prep] [WorldPrep] a page frame 3051 ms after the last (4078 ms after the window opened; the frame before took 17 ms, 1 of them presenting, then 3037 ms between frames, and this one 2 ms to here); the world: not started\r\n";
  const g = PJ.parseLog(old).gaps;
  assert.strictEqual(g.length, 1);
  assert.deepStrictEqual([g[0].gap_ms, g[0].between_ms, g[0].world, g[0].drawing], [3051, 3037, "not started", null]);
  const c = PJ.classifyGap(g[0]);
  assert.deepStrictEqual([c.kind, c.cause.name, c.cause.ms], ["slow", "between frames", 3037]);
  const during = PJ.parseLog(gapLine(192, 8796, [192, 189, 0, 1], ["present 189 ms, ui_run 2 ms, tail 1 ms", "station 0 ms"], "being prepared, step pipelines", "continuous")).gaps[0];
  assert.deepStrictEqual([during.world, PJ.classifyGap(during).cause.name], ["being prepared, step pipelines", "present"]);
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
