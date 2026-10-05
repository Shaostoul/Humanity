// The co-presence judge (scripts/lib/copresence-judge.js), fed made-up
// samples. Pure node, no game, no relay: runs in `just rig-tests`.
//
// A judge that cannot fail proves nothing, so most of this file is samples
// that MUST fail: round one's stop-go drawing, a figure that never appears,
// one that steps back, snaps, drifts off its line, stops short, or walks
// outside the camera's view. Each is built from the same smooth walk with one
// thing broken, so each failure is that thing and nothing else.
//
// The smooth walk is what src/net/sync.rs draws for scripts/second-player.js
// --path line: an approach from the relay's spawn point to the start of the
// line (at least walking speed, at most 4 s, as makeWalk does), then the line
// at walking speed, drawn 150 ms behind (INTERP_DELAY_S), sampled on frames
// whose times vary the way a background game's do.

const { test } = require("node:test");
const assert = require("node:assert");
const { judgeCopresence, LIMITS, figurePixels, isFigureTeal, FIGURE_MIN_PX } = require("../lib/copresence-judge.js");

const SPEED = 1.4; // second-player.js's walking pace, m/s
const SPAWN = [0, 1, 0]; // where the relay puts a new player (handle_game_join)
const A = [27, 1.7, 14]; // the line it walks: 6 m along +X ...
const B = [33, 1.7, 14];
const L = 6;
const CAM = [30, 1.7, 20, 0, -0.05]; // ... 6 m in front of a camera facing -Z (yaw 0)
const WALKER = { id: 2, name: "TestBotCrosser" };
const APPEAR_S = 1.0; // the figure first appears this far into the recording
const DELAY_S = 0.15; // drawn this far behind (sync.rs INTERP_DELAY_S)
const RECORD_S = 16;
const EPOCH0 = 1_790_000_000_000; // the computer's clock when the recording began, ms

/** A seeded random number generator, so every run is identical. */
function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/** Frame times for `seconds`: each 20 to 45 ms, with a 120 ms hitch now and
 *  then, the way a background game's frames really go. */
function frameTimes(seconds, seed = 7) {
  const r = rng(seed);
  const out = [];
  for (let t = 0; t < seconds; ) {
    const dt = r() < 0.03 ? 0.12 : 0.02 + 0.025 * r();
    t += dt;
    out.push(t);
  }
  return out;
}

const lerp = (a, b, x) => a.map((v, i) => v + (b[i] - v) * x);
const GAP = Math.hypot(A[0] - SPAWN[0], A[1] - SPAWN[1], A[2] - SPAWN[2]);
const APPROACH_SPEED = Math.max(SPEED, GAP / 4);
const APPROACH_S = GAP / APPROACH_SPEED;

/** Where the walker really is `tau` seconds after it started: the approach,
 *  then back and forth along the line (second-player.js makeWalk + pathPoint). */
function walkerAt(tau) {
  if (tau <= 0) return SPAWN.slice();
  if (tau < APPROACH_S) return lerp(SPAWN, A, tau / APPROACH_S);
  const s = SPEED * (tau - APPROACH_S);
  const w = s % (2 * L);
  const u = w <= L ? w : 2 * L - w;
  return [A[0] + u, A[1], A[2]];
}

/** Recorder frames, with the walker drawn at `drawnAt(t)` (null = not drawn
 *  that frame). */
function framesFrom(times, drawnAt, { cam = CAM, id = WALKER.id, name = WALKER.name } = {}) {
  return times.map((t) => {
    const pos = drawnAt(t);
    return {
      t,
      epoch_ms: EPOCH0 + t * 1000,
      cam,
      players: pos ? [{ id, name, pos, phase: "Interpolating" }] : [],
    };
  });
}

/** The smooth drawing: the true walk, 150 ms behind. */
const smooth = (t) => (t < APPEAR_S ? null : walkerAt(t - APPEAR_S - DELAY_S));

/** When the walker reached the line (A), by the computer's clock. */
const ON_LINE_EPOCH_MS = EPOCH0 + (APPEAR_S + APPROACH_S) * 1000;

const judge = (frames, extra = {}) =>
  judgeCopresence({ frames, walker: WALKER, line: { start: A, end: B }, speed: SPEED, onLineEpochMs: ON_LINE_EPOCH_MS, ...extra });
const check = (r, id) => r.checks.find((c) => c.id === id) || { ok: undefined, detail: `(no ${id} check)` };
const explain = (r) => r.checks.map((c) => `${c.ok ? "PASS" : "FAIL"} ${c.id}: ${c.detail}`).join("\n");

test("a smooth walk past the camera passes every check", () => {
  const r = judge(framesFrom(frameTimes(RECORD_S), smooth));
  assert.ok(r.pass, explain(r));
  for (const id of ["seen", "only_the_walker", "walked_the_line", "seen_every_frame", "enough_frames", "never_backwards", "steady_speed", "no_jump", "on_the_line", "in_view", "on_time"]) {
    assert.equal(check(r, id).ok, true, `${id}: ${check(r, id).detail}`);
  }
  // Exactly the walker's speed, every frame, however unevenly the frames came.
  assert.ok(Math.abs(r.stats.speed.min - SPEED) < 1e-6 && Math.abs(r.stats.speed.max - SPEED) < 1e-6, explain(r));
  assert.ok(r.stats.pairs >= 80, `pairs judged: ${r.stats.pairs}`);
});

test("a figure whose speed wobbles inside what sync.rs promises (4%) passes", () => {
  // The drawing easing toward a learned clock difference runs a few percent
  // fast or slow for a while (sync.rs STEADY_BAND). Model it as the drawn
  // moment running 4% fast, then 4% slow, alternating every second.
  const warp = (t) => {
    let w = APPEAR_S;
    for (let s = APPEAR_S; s < t; s += 0.001) w += 0.001 * (Math.floor(s) % 2 ? 1.04 : 0.96);
    return w;
  };
  const r = judge(framesFrom(frameTimes(RECORD_S), (t) => (t < APPEAR_S ? null : walkerAt(warp(t) - APPEAR_S - DELAY_S))));
  assert.ok(r.pass, explain(r));
});

test("round one's stop-go (a 50 ms ease to each 15 Hz update, then holding) FAILS on speed", () => {
  // src/net/sync.rs before 2026-10-02: each update that arrived walked the
  // figure to its position over 50 ms on an ease-in-ease-out curve, and the
  // figure then stood still until the next one.
  const updates = [];
  for (let k = 0; k * (1 / 15) < RECORD_S; k++) {
    const sent = APPEAR_S + k / 15;
    updates.push({ arrival: sent + 0.004, pos: walkerAt(k / 15) });
  }
  let next = 0;
  let from = null;
  let to = null;
  let easeStart = 0;
  let drawn = null;
  const times = frameTimes(RECORD_S);
  const frames = framesFrom(times, (t) => {
    while (next < updates.length && updates[next].arrival <= t) {
      from = drawn || updates[next].pos;
      to = updates[next].pos;
      easeStart = t;
      next++;
    }
    if (!to) return null;
    const x = Math.min(1, (t - easeStart) / 0.05);
    drawn = lerp(from, to, x * x * (3 - 2 * x));
    return drawn;
  });
  const r = judge(frames);
  assert.equal(r.pass, false, "the stop-go pattern must fail");
  // It fails on SPEED, on the line it really walked, not on a geometry miss.
  assert.equal(check(r, "seen").ok, true, explain(r));
  assert.equal(check(r, "walked_the_line").ok, true, explain(r));
  assert.equal(check(r, "steady_speed").ok, false, explain(r));
  assert.ok(r.stats.speed.min < 0.5 * SPEED, `its holding frames read near 0 m/s: min ${r.stats.speed.min}`);
});

test("a figure that never appears FAILS", () => {
  const r = judge(framesFrom(frameTimes(RECORD_S), () => null));
  assert.equal(r.pass, false);
  assert.equal(check(r, "seen").ok, false, explain(r));
  assert.match(check(r, "seen").detail, /never drawn/);
});

test("somebody else walking the same line is not the walker: FAILS", () => {
  const r = judge(framesFrom(frameTimes(RECORD_S), smooth, { id: 5, name: "Player 5" }));
  assert.equal(r.pass, false);
  assert.equal(check(r, "seen").ok, false, explain(r));
  assert.match(check(r, "seen").detail, /Player 5 \(id 5\)/);
});

test("one frame drawn back along the line FAILS", () => {
  const times = frameTimes(RECORD_S);
  const mid = APPEAR_S + DELAY_S + APPROACH_S + 2; // two seconds into the line
  const k = times.findIndex((t) => t > mid);
  const r = judge(framesFrom(times, (t) => {
    const p = smooth(t);
    return p && t === times[k] ? [p[0] - 0.12, p[1], p[2]] : p;
  }));
  assert.equal(r.pass, false);
  assert.equal(check(r, "never_backwards").ok, false, explain(r));
});

test("a snap forward in one frame FAILS as a jump", () => {
  const mid = APPEAR_S + DELAY_S + APPROACH_S + 1.5;
  const r = judge(framesFrom(frameTimes(RECORD_S), (t) => {
    const p = smooth(t);
    return p && t > mid ? [p[0] + 0.8, p[1], p[2]] : p;
  }));
  assert.equal(r.pass, false);
  assert.equal(check(r, "no_jump").ok, false, explain(r));
});

test("drifting off the line it walked FAILS, even at the right speed", () => {
  // 0.3 m/s sideways on top of the walk: 2% faster overall, inside the speed
  // band, so only the line check can catch it.
  const start = APPEAR_S + DELAY_S + APPROACH_S;
  const r = judge(framesFrom(frameTimes(RECORD_S), (t) => {
    const p = smooth(t);
    return p && t > start ? [p[0], p[1], p[2] + 0.3 * (t - start)] : p;
  }));
  assert.equal(r.pass, false);
  assert.equal(check(r, "steady_speed").ok, true, explain(r));
  assert.equal(check(r, "on_the_line").ok, false, explain(r));
});

test("stopping halfway along the line FAILS: the pass never ends", () => {
  const r = judge(framesFrom(frameTimes(RECORD_S), (t) => {
    const p = smooth(t);
    return p && p[0] > A[0] + 3 ? [A[0] + 3, A[1], A[2]] : p;
  }));
  assert.equal(r.pass, false);
  assert.equal(check(r, "walked_the_line").ok, false, explain(r));
  assert.match(check(r, "walked_the_line").detail, /never got within/);
});

test("a walk the camera is not facing FAILS the view check", () => {
  const away = [CAM[0], CAM[1], CAM[2], Math.PI, CAM[4]]; // yaw pi looks +Z, away from the line
  const r = judge(framesFrom(frameTimes(RECORD_S), smooth, { cam: away }));
  assert.equal(r.pass, false);
  assert.equal(check(r, "in_view").ok, false, explain(r));
});

test("speeds come from the RECORDED frame times, not an assumed 1/60", () => {
  // The same smooth samples, relabelled as if every frame were 1/60 s: the
  // distances no longer match the times, so a judge that used the recorded
  // times passes the first and fails the second. (A judge that assumed 1/60
  // would do the opposite on a real, uneven run.)
  const frames = framesFrom(frameTimes(RECORD_S), smooth);
  assert.ok(judge(frames).pass);
  const relabelled = frames.map((fr, i) => ({ ...fr, t: (i + 1) / 60 }));
  const r = judge(relabelled);
  assert.equal(check(r, "steady_speed").ok, false, explain(r));
});

// ── Visible in the picture, not just drawn ───────────────────────────────────
// A grey room with the teal body the game draws (lib.rs: [0.15, 0.75, 0.85],
// measured lit at (80, 193, 204)), 40 x 150 px, under a nameplate at (200, 90).
function picture({ cover = false, figureAt = 180 } = {}) {
  const width = 400;
  const height = 400;
  const rgba = new Uint8Array(width * height * 4);
  const paint = (x0, y0, x1, y1, c) => {
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) rgba.set([...c, 255], (y * width + x) * 4);
  };
  paint(0, 0, width, height, [181, 186, 192]); // floor
  paint(0, 0, width, 80, [61, 72, 87]); // wall
  paint(figureAt, 100, figureAt + 40, 250, [80, 193, 204]); // the body
  if (cover) paint(120, 60, 300, 300, [12, 12, 14]); // a window drawn over it
  return { width, height, rgba };
}

test("a teal figure under its nameplate counts as visible", () => {
  const f = figurePixels(picture(), [200, 90]);
  assert.equal(f.count, 40 * 150);
  assert.ok(f.count >= FIGURE_MIN_PX);
  assert.ok(Math.abs(f.centroid[0] - 199.5) < 1e-9, `centre x ${f.centroid[0]}`);
});

test("the same figure with a window drawn over it FAILS: nothing teal shows", () => {
  const f = figurePixels(picture({ cover: true }), [200, 90]);
  assert.equal(f.count, 0);
  assert.ok(f.count < FIGURE_MIN_PX);
});

test("teal far from where the nameplate is does not count for it", () => {
  const f = figurePixels(picture({ figureAt: 360 }), [100, 90]);
  assert.equal(f.count, 0, "the box is 150 px either side of the nameplate");
  assert.equal(figurePixels(picture({ figureAt: 360 })).count, 40 * 150, "without a nameplate the whole picture is counted");
});

// The capture can land a second or more after the nameplate was asked for (a busy
// machine, 2026-10-04: asked at x 1505, the figure captured at x 1286, so "0
// teal px under its nameplate" with the figure in plain view). Asked again right
// after the capture, the box spans both: the figure walked from one to the other.
test("the figure between the nameplate before and after the capture counts; a window over it still fails", () => {
  // The name asked at x 380, the body captured at 180..220, the name after the capture at 160.
  assert.equal(figurePixels(picture(), [380, 90]).count, 0, "asked once, 160 px behind the figure: missed");
  assert.equal(figurePixels(picture(), [380, 90], [160, 90]).count, 40 * 150, "asked before and after: the span holds it");
  assert.equal(figurePixels(picture({ cover: true }), [380, 90], [160, 90]).count, 0, "a window over it still reads nothing");
});

test("the room itself is not teal", () => {
  assert.equal(isFigureTeal(181, 186, 192), false, "floor");
  assert.equal(isFigureTeal(61, 72, 87), false, "wall");
  assert.equal(isFigureTeal(80, 193, 204), true, "the body, lit");
  assert.equal(isFigureTeal(40, 100, 108), true, "the body in a dimmer light");
});

test("the limits say what the rig was promised", () => {
  // The band is the one documented in the judge; a change to it should be a
  // deliberate edit here too.
  assert.equal(LIMITS.SPEED_BAND, 0.1);
  assert.ok(LIMITS.LINE_TOL_M < LIMITS.WINDOW_TOL_M);
  assert.ok(LIMITS.START_MARGIN_M + LIMITS.END_MARGIN_M < L);
});

// ── The critic's four (2026-10-03): each check below had never been seen to
// fail, or did not exist. Each test breaks one thing and expects exactly that
// check to fail.

test("a second, frozen figure (a ghost, or the game drawing itself) fails only_the_walker", () => {
  const frames = framesFrom(frameTimes(RECORD_S), smooth).map((fr) => ({
    ...fr,
    players: [...fr.players, { id: 1, name: "Wanderer", pos: SPAWN.slice(), phase: "Holding" }],
  }));
  const r = judge(frames);
  assert.equal(check(r, "only_the_walker").ok, false, explain(r));
  assert.match(check(r, "only_the_walker").detail, /Wanderer/);
  assert.deepEqual(r.checks.filter((c) => !c.ok).map((c) => c.id), ["only_the_walker"], explain(r));
});

test("a figure that blinks out for three frames mid-pass fails seen_every_frame", () => {
  const times = frameTimes(RECORD_S);
  const mid = times.findIndex((t) => t > APPEAR_S + APPROACH_S + 2.5);
  const blank = new Set([mid, mid + 1, mid + 2]);
  const frames = framesFrom(times, smooth).map((fr, i) => (blank.has(i) ? { ...fr, players: [] } : fr));
  const r = judge(frames);
  assert.equal(check(r, "seen_every_frame").ok, false, explain(r));
  assert.match(check(r, "seen_every_frame").detail, /missing on 3 frame/);
});

test("too few frames in the pass fails enough_frames", () => {
  // Frames 0.25 s apart: the 4.5 m pass holds about 13 pairs, under MIN_PAIRS.
  const times = [];
  for (let t = 0.25; t < RECORD_S; t += 0.25) times.push(t);
  const r = judge(framesFrom(times, smooth));
  assert.equal(check(r, "enough_frames").ok, false, explain(r));
  assert.ok(r.stats.pairs < LIMITS.MIN_PAIRS, `pairs: ${r.stats.pairs}`);
});

test("a frame clock that does not move forward fails enough_frames", () => {
  const times = frameTimes(RECORD_S);
  const mid = times.findIndex((t) => t > APPEAR_S + APPROACH_S + 2.5);
  // One frame stamped with the time of the frame before it.
  const frames = framesFrom(times, smooth).map((fr, i, all) => (i === mid ? { ...fr, t: all[i - 1].t } : fr));
  const r = judge(frames);
  assert.equal(check(r, "enough_frames").ok, false, explain(r));
  assert.match(check(r, "enough_frames").detail, /did not move forward/);
});

test("a figure drawn a second late on the right line at the right speed fails on_time, and only it", () => {
  const late = (t) => (t < APPEAR_S ? null : walkerAt(t - APPEAR_S - 1.0));
  const r = judge(framesFrom(frameTimes(RECORD_S), late));
  assert.equal(check(r, "on_time").ok, false, explain(r));
  assert.deepEqual(r.checks.filter((c) => !c.ok).map((c) => c.id), ["on_time"], explain(r));
  // And the smooth walk's lag is the 150 ms drawing delay, at a walk.
  const ok = judge(framesFrom(frameTimes(RECORD_S), smooth));
  assert.ok(Math.abs(ok.stats.lag_m.min - DELAY_S * SPEED) < 1e-6 && Math.abs(ok.stats.lag_m.max - DELAY_S * SPEED) < 1e-6, explain(ok));
});

test("a game clock running 30% slow fools every frame-clock check; only on_time sees it", () => {
  // The game's frame clock is the code under test: if it ran slow, net::sync
  // would draw the walker slowly AND the recorder would stamp the frames on
  // the same slow clock, so speed measured on that clock still reads exactly
  // walking pace. Only the computer's own clock (epoch_ms, untouched here)
  // shows the figure falling further and further behind the real walker.
  const SLOW = 0.7;
  const frames = frameTimes(RECORD_S).map((t) => {
    const pos = t < APPEAR_S ? null : walkerAt(SLOW * (t - APPEAR_S) - DELAY_S);
    return { t: SLOW * t, epoch_ms: EPOCH0 + t * 1000, cam: CAM, players: pos ? [{ ...WALKER, pos, phase: "Interpolating" }] : [] };
  });
  const r = judge(frames);
  assert.deepEqual(r.checks.filter((c) => !c.ok).map((c) => c.id), ["on_time"], explain(r));
});

test("without the walker's timing, on_time fails rather than passing unchecked", () => {
  const r = judge(framesFrom(frameTimes(RECORD_S), smooth), { onLineEpochMs: undefined });
  assert.equal(check(r, "on_time").ok, false, explain(r));
  const old = framesFrom(frameTimes(RECORD_S), smooth).map(({ epoch_ms, ...fr }) => fr);
  assert.equal(check(judge(old), "on_time").ok, false);
});

// ── Homes on plots (increment 1b, `verify-copresence --plots`) ─────────────
//
// The plot checks judge where things are, not how they move: the two players
// hold different plots by the order they joined, the game's camera is inside
// the plot the game should hold, and every position the game drew for the
// walker is inside the walker's plot. The shapes below are the shipped
// layout (docs/design/ship-homes-and-logistics.md section 2.4).

const { judgePlots, forwardLegStart, readShipPlots, inPlot } = require("../lib/copresence-judge.js");
const PLOTS = [
  { id: "p1", kind: "homestead", origin: [0, 0, 0], size: [55, 3, 89] },
  { id: "p2", kind: "homestead", origin: [0, 0, 99], size: [55, 3, 89] },
];
const P1_SPAWN = [53.5, 1.7, 40.5];
const P2_SPAWN = [53.5, 1.7, 139.5];
/** The walker walking its line in its own plot, from its spawn along +Z. */
const walkInPlot = (spawn) => framesFrom(frameTimes(RECORD_S), (t) => (t < APPEAR_S ? null : [spawn[0], spawn[1], spawn[2] + Math.min(8, SPEED * (t - APPEAR_S))]));
/** The home's own things as the recorder reports them, for a home on the
 *  plot at z offset `dz`: the shipped home's numbers (the hologram hangs half
 *  a metre outside the west wall; three animals by the grain field; plants by
 *  the irrigation and the composter). */
const thingsAt = (dz) => ({
  respawn: [53.5, 1.7, 40.5 + dz],
  hologram: [-0.5, 1, 2.5 + dz],
  showroom: [20, 0, 30 + dz],
  animals: [[9, 0, 78 + dz], [9, 0, 78 + dz], [15, 0, 78 + dz]],
  plants: [[30, 0, 60 + dz], [12, 0, 20 + dz]],
});
const plotsRun = (extra = {}) =>
  judgePlots({ order: "walker-first", plots: PLOTS, gamePlot: "p2", walkerPlot: "p1", camera: P2_SPAWN, homeThings: thingsAt(99), frames: walkInPlot(P1_SPAWN), walker: WALKER, ...extra });

test("plots: each holds its own by the join order, the camera and the walker inside theirs, passes", () => {
  const r = plotsRun();
  assert.ok(r.pass, explain(r));
  assert.deepEqual(r.checks.map((c) => c.id), ["plot_ids_differ", "plots_by_join_order", "camera_in_p2", "home_things_on_its_plot", "walker_drawn_in_its_plot"]);
  // The other order swaps who should hold what.
  const g = judgePlots({ order: "game-first", plots: PLOTS, gamePlot: "p1", walkerPlot: "p2", camera: P1_SPAWN, homeThings: thingsAt(0), frames: walkInPlot(P2_SPAWN), walker: WALKER });
  assert.ok(g.pass, explain(g));
  assert.equal(g.checks[2].id, "camera_in_p1");
});

// The red run the design asks for: a build without 1b hands out no plots, so
// the game's home and camera stay on p1 while it should be on p2.
test("plots: the 1a shape (no plots handed out, the game still at p1) FAILS camera_in_p2", () => {
  const r = plotsRun({ gamePlot: "p1", walkerPlot: null, camera: P1_SPAWN });
  const failed = r.checks.filter((c) => !c.ok).map((c) => c.id);
  assert.ok(failed.includes("camera_in_p2"), explain(r));
  assert.ok(failed.includes("plot_ids_differ") && failed.includes("plots_by_join_order"), explain(r));
  assert.match(r.checks.find((c) => c.id === "camera_in_p2").detail, /OUTSIDE p2 \(x 0\.\.55, y 0\.\.3, z 99\.\.188\)/);
});

// The shape the first 1b build left (the critic's findings 1 and 3): the home
// and the camera moved to p2, but the Respawn point, the farm animals, the
// plants, the hologram and the showroom stage stayed on p1, somebody else's.
// Seen red 2026-10-03: with the check made to pass anything, this test failed
// "home_things_on_its_plot should fail".
test("plots: the home's own things left on p1 while the home moved to p2 FAIL home_things_on_its_plot", () => {
  const r = plotsRun({ homeThings: thingsAt(0) });
  const c = r.checks.find((x) => x.id === "home_things_on_its_plot");
  assert.equal(c.ok, false, "home_things_on_its_plot should fail");
  assert.match(c.detail, /^8 of 8 of the home's things are not on p2: the Respawn point at \(53\.5, 1\.7, 40\.5\) by p1/);
  // Only the Respawn point left behind is enough to fail.
  const one = thingsAt(99);
  one.respawn = P1_SPAWN;
  assert.equal(plotsRun({ homeThings: one }).checks.find((x) => x.id === "home_things_on_its_plot").ok, false);
  // A build that reports none, or reports no animals and no plants, checks nothing: no pass.
  assert.equal(plotsRun({ homeThings: null }).pass, false);
  assert.equal(plotsRun({ homeThings: { ...thingsAt(99), animals: [], plants: [] } }).pass, false);
});

test("plots: two players handed the same plot FAIL plot_ids_differ", () => {
  const r = plotsRun({ gamePlot: "p1", walkerPlot: "p1" });
  assert.equal(r.checks.find((c) => c.id === "plot_ids_differ").ok, false, explain(r));
});

test("plots: the walker drawn outside its plot, even for one frame, FAILS", () => {
  const frames = walkInPlot(P1_SPAWN);
  frames[Math.floor(frames.length / 2)].players = [{ ...WALKER, pos: [53.5, 1.7, 95], phase: "Extrapolating" }];
  const r = plotsRun({ frames });
  const c = r.checks.find((x) => x.id === "walker_drawn_in_its_plot");
  assert.equal(c.ok, false, explain(r));
  assert.match(c.detail, /^1 of \d+ drawn positions OUTSIDE p1/);
  // ...and a walker never drawn at all is no pass either.
  assert.equal(plotsRun({ frames: walkInPlot(P1_SPAWN).map((fr) => ({ ...fr, players: [] })) }).pass, false);
});

test("plots: unknown plots fail rather than pass unchecked", () => {
  assert.equal(plotsRun({ plots: [] }).pass, false);
});

test("plots: the ship file's plots read the way the game has them", () => {
  const fs = require("fs");
  const path = require("path");
  const text = fs.readFileSync(path.join(__dirname, "..", "..", "data", "blueprints", "ship_structure.ron"), "utf8");
  const read = readShipPlots(text);
  assert.deepEqual(read.slice(0, 2), PLOTS, "p1 and p2 where they always were");
  // The twelve plots along First Street (2026-10-04): p3 to p12 down its west side, 99 m apart.
  assert.deepEqual(read.map((p) => p.id), Array.from({ length: 12 }, (_, k) => `p${k + 1}`));
  for (const [k, p] of read.entries()) assert.deepEqual([p.kind, p.origin, p.size], ["homestead", [0, 0, 99 * k], [55, 3, 89]], p.id);
  assert.ok(inPlot(P2_SPAWN, PLOTS[1]) && !inPlot(P2_SPAWN, PLOTS[0]));
});

// A walk that goes back and forth: the judged pass is one FORWARD leg,
// picked by the clock. Without `fromEpochMs` the end of a leg walking back is
// taken for the pass and fails never_backwards; with it, the next forward leg
// passes. (The --plots rig records a walker that has been walking since
// before the game joined.)
test("a back-and-forth walk is judged on one forward leg, picked by the clock", () => {
  const r = L / 2; // second-player.js --radius: the line is 2r long
  // It reached the line 6 s before the recording began, so the recording opens
  // on it walking back (a cycle is 4r / speed = 8.57 s, the forward half 4.29 s).
  const onLine = EPOCH0 - 6000;
  const at = (tau) => {
    const s = SPEED * tau;
    const w = ((s % (2 * L)) + 2 * L) % (2 * L);
    const u = w <= L ? w : 2 * L - w;
    return [A[0] + u, A[1], A[2]];
  };
  // Drawn 150 ms behind the real walker, from the first frame.
  const frames = framesFrom(frameTimes(RECORD_S), (t) => at((EPOCH0 + t * 1000 - onLine) / 1000 - DELAY_S));
  const leg = forwardLegStart(onLine, SPEED, r, EPOCH0 + 500);
  assert.ok(leg > EPOCH0, "the next forward leg starts inside the recording");
  assert.equal(Math.round(leg - onLine), Math.round(((4 * r) / SPEED) * 1000), "one full cycle after it reached the line");
  const blind = judge(frames, { onLineEpochMs: onLine });
  assert.equal(check(blind, "never_backwards").ok, false, explain(blind));
  const picked = judge(frames, { onLineEpochMs: leg, fromEpochMs: leg });
  assert.ok(picked.pass, explain(picked));
});

// Increment 2 review, finding 11: with the view asked for and no frame carrying a
// camera (a recorder change that drops or renames `cam`), in_view simply was not
// added, and the run passed with one check fewer. It must FAIL instead. Seen red
// 2026-10-04 on c98c5465b: "a recording with no camera judged the view: (no
// in_view check)".
test("a recording with no camera FAILS in_view when the view is asked for", () => {
  const r = judge(framesFrom(frameTimes(RECORD_S), smooth, { cam: undefined }).map(({ cam, ...f }) => f));
  const c = check(r, "in_view");
  assert.equal(c.ok, false, `a recording with no camera judged the view: ${c.detail}`);
  assert.match(c.detail, /carries no camera/);
});

test("checkView false drops the in-view check and nothing else", () => {
  const away = [30, 1.7, 20, Math.PI, -0.05]; // facing away from the walk
  const r = judge(framesFrom(frameTimes(RECORD_S), smooth, { cam: away }), { checkView: false });
  assert.ok(r.pass, explain(r));
  assert.equal(r.checks.find((c) => c.id === "in_view"), undefined);
  assert.equal(check(judge(framesFrom(frameTimes(RECORD_S), smooth, { cam: away })), "in_view").ok, false);
});

// Since increment 1b the walker joins on its own plot, p2 when the game holds
// p1, and walks from there to the line in front of the camera. Its straight
// approach must never pass for the walk: the rig checks the real path.
test("an approach that crosses to the line is clear; one running back along it is not", () => {
  const { approachClear } = require("../lib/copresence-judge.js");
  const line = { start: A, end: B };
  assert.equal(approachClear([53.5, 1.7, 139.5], line), true, "from p2 spawn, across to the start");
  assert.equal(approachClear(SPAWN, line), true, "from behind the start");
  assert.equal(approachClear([40, 1.7, 14], line), false, "from beyond the end, back along the line");
  assert.equal(approachClear([30, 1.7, 14.2], line), false, "from the middle of the line, 0.2 m off it");
});

// The second review of 1b, finding 3: the pieces the player built aboard and
// their parked vehicles go with the home too. A fresh rig sandbox has none, so
// they are not required, but every one reported must be on the plot. Seen red
// 2026-10-03 with judgePlots ignoring them (the 65b3e2c0c judge): a chest left
// on p1 while the home moved to p2 passed, "a chest left on p1 should fail".
test("plots: a built piece or a vehicle left on p1 while the home moved to p2 FAILS home_things_on_its_plot", () => {
  const chest = { ...thingsAt(99), structures: [[20, 0, 30]], vehicles: [] };
  const c = plotsRun({ homeThings: chest }).checks.find((x) => x.id === "home_things_on_its_plot");
  assert.equal(c.ok, false, "a chest left on p1 should fail");
  assert.match(c.detail, /built piece 1 at \(20\.0, 0\.0, 30\.0\) by p1/);
  const truck = { ...thingsAt(99), structures: [], vehicles: [[40, 0, 70]] };
  assert.equal(plotsRun({ homeThings: truck }).checks.find((x) => x.id === "home_things_on_its_plot").ok, false, "a truck left on p1 fails too");
  const moved = { ...thingsAt(99), structures: [[20, 0, 129]], vehicles: [[40, 0, 169]] };
  const ok = plotsRun({ homeThings: moved }).checks.find((x) => x.id === "home_things_on_its_plot");
  assert.ok(ok.ok, ok.detail);
  assert.match(ok.detail, /1 built pieces and 1 vehicles are all on p2/);
});

// Stepping out of the shared world and back (the second review's finding 1):
// the relay spawns the returning game afresh at its door, wherever the game
// stands; the game must stand there, and its next move must reach the others.
const { judgeRejoin } = require("../lib/copresence-judge.js");
const REJOIN_OK = {
  far: [70, 1.7, 190], // the end of street-1, 150 m from p1's door
  relaySpawn: P1_SPAWN,
  camera: [53.5, 1.7, 40.5],
  nudged: [53.5, 1.7, 41.5],
  seen: [[53.5, 1.7, 41.5]],
};

test("rejoin: standing where the relay holds it, with the next move relayed, passes", () => {
  const r = judgeRejoin(REJOIN_OK);
  assert.ok(r.pass, explain(r));
  assert.deepEqual(r.checks.map((c) => c.id), ["rejoin_far_from_spawn", "rejoin_stands_where_held", "rejoin_moves_reach_others"]);
});

// The 65b3e2c0c shape: the game kept standing where it was (150 m away) and
// every update it sent was refused. Seen red 2026-10-03 with judgeRejoin
// passing anything: "a game left 150 m from where the relay holds it should fail".
test("rejoin: a game left where it stood, its moves refused, FAILS", () => {
  const r = judgeRejoin({ ...REJOIN_OK, camera: [70, 1.7, 190], nudged: [70, 1.7, 191], seen: [] });
  const failed = r.checks.filter((c) => !c.ok).map((c) => c.id);
  assert.deepEqual(failed, ["rejoin_stands_where_held", "rejoin_moves_reach_others"], "a game left 150 m from where the relay holds it should fail");
  assert.match(r.checks[1].detail, /150\.\d+ m from where the relay holds it/);
});

test("rejoin: an experiment that never left the 100 m rule's reach proves nothing, and FAILS", () => {
  const r = judgeRejoin({ ...REJOIN_OK, far: [53.5, 1.7, 100] });
  assert.equal(r.checks[0].ok, false, explain(r));
  assert.equal(judgeRejoin({ ...REJOIN_OK, relaySpawn: null }).pass, false, "never seen joining again: unknown, not a pass");
});

// The third review's finding 16: the tolerances and the "it is the nudge" rule
// were never pinned, so a judge loosened to 5 m, or one taking any relayed
// update as the nudge, passed all 37 tests. Seen red 2026-10-03, each against
// its own loosened judge (scratch copies of copresence-judge.js):
//  - REJOIN_STAND_TOL_M 0.5 -> 5: "a camera 1 m from where the relay holds it
//    passed rejoin_stands_where_held";
//  - REJOIN_NUDGE_TOL_M 0.3 -> 5, and (separately) `hit = seen[0]`: "an update
//    at [[53.5,1.7,40.5]], short of or past the nudge to [53.5,1.7,41.5], passed
//    rejoin_moves_reach_others".
test("rejoin: a camera a few metres off where the relay holds it FAILS", () => {
  for (const off of [1, 2, 4]) {
    const r = judgeRejoin({ ...REJOIN_OK, camera: [53.5 + off, 1.7, 40.5] });
    assert.equal(r.checks.find((c) => c.id === "rejoin_stands_where_held").ok, false, `a camera ${off} m from where the relay holds it passed rejoin_stands_where_held`);
  }
});

test("rejoin: only an update at the nudge itself counts, not one at the standing point before it", () => {
  for (const seen of [[[53.5, 1.7, 40.5]], [[53.5, 1.7, 40.5], [53.5, 1.7, 40.9]], [[53.5, 1.7, 42.2]]]) {
    const r = judgeRejoin({ ...REJOIN_OK, seen });
    assert.equal(
      r.checks.find((c) => c.id === "rejoin_moves_reach_others").ok,
      false,
      `an update at ${JSON.stringify(seen)}, short of or past the nudge to ${JSON.stringify(REJOIN_OK.nudged)}, passed rejoin_moves_reach_others`,
    );
  }
  // The nudge among other updates still counts.
  assert.ok(judgeRejoin({ ...REJOIN_OK, seen: [[53.5, 1.7, 40.5], [53.5, 1.7, 41.5]] }).pass);
});

// The Respawn leg (the third review's finding 10): the same three checks under
// their own ids. Seen red 2026-10-03 on a judge without the prefix option
// (every id read "rejoin_..."): "the respawn leg's checks are its own".
test("respawn: judged like a rejoin, under its own check ids", () => {
  const r = judgeRejoin({ ...REJOIN_OK, far: [70, 1.7, 194] }, { prefix: "respawn", when: "it pressed Respawn" });
  assert.ok(r.pass, explain(r));
  assert.deepEqual(r.checks.map((c) => c.id), ["respawn_far_from_spawn", "respawn_stands_where_held", "respawn_moves_reach_others"], "the respawn leg's checks are its own");
  assert.match(r.checks[0].detail, /when it pressed Respawn/);
  // The pre-fix game: Respawn put the camera at its door, but the relay still held it at
  // the far end of street-1, so nothing new joined and every update was refused.
  const frozen = judgeRejoin({ far: [70, 1.7, 194], relaySpawn: null, camera: P1_SPAWN, nudged: [53.5, 1.7, 41.5], seen: [] }, { prefix: "respawn" });
  assert.equal(frozen.pass, false);
});

// ── Routes through the ship's doors (increment 2, "Meet in the Commons") ─────
//
// The rig walks the game and the walker on the door points the game reports
// (src/ship/door_points.rs). These tests route on a copy of that report for the
// shipped ship, seen from p1 (fixtures/door-points-p1.json); the Rust test
// door_points::the_rigs_fixture_is_what_the_game_reports keeps the copy true.
const { doorRoute, routeClear, routeWalls, farPlaces, farthestFrom, placeAt, judgeMeet, judgeReboot, inViewM, FAR_VIEW_MARGIN_M, farPlacesAlong, farPlaceInView } = require("../lib/copresence-judge.js");
const DOORS = require("./fixtures/door-points-p1.json");
const P2_DOOR = [53.5, 1.7, 139.5];
const MEET_CAM = [76, 1.7, 64];
const MEET_LINE = { start: [72, 1.7, 70], end: [80, 1.7, 70] };
/** On the ship's floor: inside a place, or inside a door's tube. */
const onFloor = (p) =>
  !!placeAt(DOORS, p) || DOORS.doors.some((d) => p[0] >= d.tube[0][0] - 1e-6 && p[0] <= d.tube[1][0] + 1e-6 && p[2] >= d.tube[0][2] - 1e-6 && p[2] <= d.tube[1][2] + 1e-6);
/** Every point of the straight walk between two points, a centimetre apart. */
const along = (a, b) => {
  const n = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[2] - a[2]) * 100));
  return Array.from({ length: n + 1 }, (_, k) => a.map((v, j) => v + ((b[j] - v) * k) / n));
};

// Seen red 2026-10-04 with doorRoute walking straight to its target
// (`waypoints = [to]`, the shape of the walk before door points): "from p1's
// door the walk leaves the ship's floor at (55.06, 1.70, 42.12)".
test("routes: from each door into the Commons, through the doors, every step on the floor", () => {
  for (const [door, doors] of [
    [P1_SPAWN, ["plot:p1->zone:commons"]],
    [P2_DOOR, ["plot:p2->zone:street-1", "zone:commons->zone:street-1"]],
  ]) {
    const r = doorRoute(DOORS, door, MEET_CAM, 40);
    assert.equal(r.error, null);
    assert.deepEqual(r.doors, doors, `from ${door} the walk goes through ${doors.join(", ")}`);
    assert.deepEqual(r.points[r.points.length - 1], MEET_CAM, "it ends where it was asked to");
    let at = door;
    for (const p of r.points) {
      assert.ok(Math.hypot(p[0] - at[0], p[2] - at[2]) <= 40 + 1e-9, `a step from ${at} to ${p} is longer than 40 m`);
      const off = along(at, p).find((q) => !onFloor(q));
      assert.equal(off, undefined, `from ${door[2] > 90 ? "p2" : "p1"}'s door the walk leaves the ship's floor at (${off && off.map((v) => v.toFixed(2)).join(", ")})`);
      at = p;
    }
  }
  // p2's way: out of its home at its door's step, along First Street, into the Commons.
  assert.deepEqual(doorRoute(DOORS, P2_DOOR, MEET_CAM).waypoints, [[54, 1.7, 139], [66, 1.7, 139], [70, 1.7, 86], [70, 1.7, 74], MEET_CAM]);
  // A point in no place (in a corridor) starts from the place nearest it.
  assert.equal(doorRoute(DOORS, [60, 1.7, 40], MEET_CAM).error, null);
  assert.match(doorRoute({ places: DOORS.places, doors: [] }, P2_DOOR, MEET_CAM).error, /no doors lead from plot:p2 to zone:commons/);
});

// The walker's route into the meeting must never pass for the walk: no leg of
// it may run along the line. Both doors' routes to the meeting line are clear;
// a route along the line is not.
test("routes: the walker's way to the meeting line never runs along it", () => {
  for (const door of [P1_SPAWN, P2_DOOR]) {
    const r = doorRoute(DOORS, door, MEET_LINE.start);
    assert.ok(routeClear([door, ...r.waypoints], MEET_LINE), `the route from ${door} is clear of the line`);
  }
  assert.equal(routeClear([[70, 1.7, 70], [75, 1.7, 70], MEET_LINE.start], MEET_LINE), false, "a leg back along the line is not clear");
});

// The far places, from the report: each shared zone's corners a metre in. Since
// the twelve plots (2026-10-04) First Street runs to z 1185, so its far corner
// is more than a kilometre from either door, past every walker's view.
test("routes: the far places are the shared zones' corners", () => {
  const far = farPlaces(DOORS);
  assert.equal(far.length, 8, "four corners of the Commons and of First Street");
  assert.deepEqual(far[0], [66, 1.7, 21]);
  assert.deepEqual(farthestFrom(far, P1_SPAWN), [74, 1.7, 1184]);
  assert.deepEqual(farthestFrom(far, P2_DOOR), [74, 1.7, 1184]);
});

// THE FAR PLACE THE WALKERS SEE (the twelve plots along First Street, 2026-10-04).
// The rig's far legs judge what a walker logged the relay passing on, and the relay
// sends a move only to the players within its view (increment 4, data/ship/
// shared_world.ron in_view_m). The far place is the farthest point of the shared
// zones' floors (corners, and every 5 m along their sides) still within the view,
// less a margin, of both ends of the watching walker's line; from either door it is
// well past the 100 m the rejoin, Respawn and editor legs need, and far down First
// Street. Seen red 2026-10-04 with farPlaceInView returning the farthest corner
// whatever the watchers (`farthestFrom(farPlaces(report), from)`, the rig before):
// "the far place [74,1.7,1184] is 1114.0 m from the walker at [72,1.7,70], past the
// 225 m it may be".
test("far places: the farthest shared place every watching walker still has in view", () => {
  const view = inViewM(require("fs").readFileSync(require("path").join(__dirname, "..", "..", "data", "ship", "shared_world.ron"), "utf8"));
  assert.equal(view, 250, "the shipped view");
  const within = view - FAR_VIEW_MARGIN_M;
  const line = [MEET_LINE.start, MEET_LINE.end];
  for (const door of [P1_SPAWN, P2_DOOR]) {
    const far = farPlaceInView(DOORS, door, line, within);
    assert.ok(Array.isArray(far), `a far place from ${door}`);
    for (const w of line) {
      const d = Math.hypot(far[0] - w[0], far[2] - w[2]);
      assert.ok(d <= within + 1e-9, `the far place ${JSON.stringify(far)} is ${d.toFixed(1)} m from the walker at ${JSON.stringify(w)}, past the ${within} m it may be`);
    }
    const fromDoor = Math.hypot(far[0] - door[0], far[2] - door[2]);
    assert.ok(fromDoor > 100, `the far place is ${fromDoor.toFixed(1)} m from ${door}: past the 100 m the rejoin legs need`);
    assert.ok(placeAt(DOORS, far) && placeAt(DOORS, far).id === "zone:street-1", `it stands on First Street: ${JSON.stringify(far)}`);
    assert.ok(far[2] > 250, `well down First Street: ${JSON.stringify(far)}`);
  }
  // The guest's: from the Commons arrival, seen by the walker at home on p1.
  const home = [P1_SPAWN, [P1_SPAWN[0], P1_SPAWN[1], P1_SPAWN[2] + 8]];
  const g = farPlaceInView(DOORS, [87.5, 1.7, 67.5], home, within);
  assert.ok(g && home.every((w) => Math.hypot(g[0] - w[0], g[2] - w[2]) <= within + 1e-9), `the guest's far place ${JSON.stringify(g)} is in view of p1's door`);
  assert.ok(Math.hypot(g[0] - 87.5, g[2] - 67.5) > 100, "and more than 100 m from where a guest arrives");
  // Along a zone's sides, not only its corners; nobody in view of anything: none.
  assert.ok(farPlacesAlong(DOORS).length > 400, "points along First Street's 1.1 km");
  assert.equal(farPlaceInView(DOORS, P1_SPAWN, [[5000, 1.7, 5000]], within), null);
  assert.equal(farPlaceInView(DOORS, P1_SPAWN, line, NaN), null, "no view read: none");
});

// The meeting itself, beyond the walk (judgeCopresence and the pictures judge
// that). A run that met: the game moved in short steps from p1's door into the
// Commons, held there by the relay; the walker drawn out of p2's corridor, along
// First Street and into the Commons; the walker saw the game where it stands.
const COMMONS_BOX = DOORS.places[0];
const P2_TUBE = { min: DOORS.doors[2].tube[0], max: DOORS.doors[2].tube[1] };
const MEET_OK = {
  commons: COMMONS_BOX,
  gameFrom: P1_SPAWN,
  // Round the Commons' room block by its west side (doorRoute's detour), as the rig plans it.
  gameSteps: [[54, 1.7, 40], [66, 1.7, 40], [66, 1.7, 64], MEET_CAM, MEET_CAM],
  // What the walker at home logged the relay passing on, in order (the door first: the game
  // stood there when the walk began).
  gameRelayed: [P1_SPAWN, [54, 1.7, 40], [66, 1.7, 40], [66, 1.7, 64], [76, 1.7, 64.02]],
  gameCamera: MEET_CAM,
  gameHeld: [76, 1.7, 64.02],
  line: MEET_LINE,
  walls: DOORS.walls,
  walkerDoor: P2_DOOR,
  walker: { start: P2_DOOR },
  walker_route: [P2_DOOR, ...doorRoute(DOORS, P2_DOOR, MEET_LINE.start).waypoints],
  walkerTube: P2_TUBE,
  walkerDrawn: [P2_DOOR, [54, 1.7, 139], [60, 1.7, 139], [66, 1.7, 139], [70, 1.7, 100], [70, 1.7, 80], [70, 1.7, 74], [72, 1.7, 70]],
  walkerSawGame: [MEET_CAM],
};
test("meet: a meeting in the Commons passes", () => {
  const r = judgeMeet(MEET_OK);
  assert.ok(r.pass, explain(r));
  assert.deepEqual(r.checks.map((c) => c.id), [
    "meet_game_steps_planned_short",
    "meet_game_steps_relayed",
    "meet_game_in_commons",
    "meet_relay_holds_game_there",
    "meet_routes_clear_of_walls",
    "meet_walker_from_its_door",
    "meet_walker_through_its_corridor",
    "meet_walker_into_commons",
    "meet_walker_sees_game",
  ]);
});

// What must fail, one broken thing at a time. Seen red 2026-10-04 with judgeMeet
// passing anything: "one 130 m jump from the end of First Street should fail
// meet_game_steps_planned_short; failed: nothing". The increment 2 review's cases
// (findings 12 to 14) were seen red 2026-10-04 on c98c5465b's judge, which had
// none of their checks: "the relay passed on none of the middle steps should
// fail meet_game_steps_relayed; failed: nothing", and the same for
// meet_walker_from_its_door and meet_routes_clear_of_walls.
test("meet: each broken meeting FAILS its own check", () => {
  const across = (p, q) => [[(p[0] + q[0]) / 2, (p[2] + q[2]) / 2 - 1, (p[0] + q[0]) / 2, (p[2] + q[2]) / 2 + 1]];
  for (const [what, bad, id] of [
    ["one 130 m jump from the end of First Street", { gameSteps: [MEET_CAM], gameFrom: [70, 1.7, 194] }, "meet_game_steps_planned_short"],
    ["the relay passed on none of the middle steps", { gameRelayed: [P1_SPAWN, [76, 1.7, 64.02]] }, "meet_game_steps_relayed"],
    ["the relay passed on nothing (an empty walker log)", { gameRelayed: [] }, "meet_game_steps_relayed"],
    ["the relay passed the steps on out of order", { gameRelayed: [P1_SPAWN, [66, 1.7, 40], [54, 1.7, 40], [66, 1.7, 64], MEET_CAM] }, "meet_game_steps_relayed"],
    ["no record of what the relay passed on", { gameRelayed: undefined }, "meet_game_steps_relayed"],
    ["the relay spawned the walker in the middle of its plot", { walker: { start: [27.5, 1.7, 143.5] } }, "meet_walker_from_its_door"],
    ["a wall moved across the game's way in", { walls: [...DOORS.walls, ...across([54, 1.7, 40], [66, 1.7, 40])] }, "meet_routes_clear_of_walls"],
    ["a wall moved across the walker's way out", { walls: [...DOORS.walls, [60, 135, 60, 143]] }, "meet_routes_clear_of_walls"],
    ["a wall between the camera and the line", { walls: [...DOORS.walls, [70, 67, 82, 67]] }, "meet_routes_clear_of_walls"],
    ["no record of the walls", { walls: undefined }, "meet_routes_clear_of_walls"],
    ["the game left in First Street", { gameCamera: [70, 1.7, 120], gameHeld: [70, 1.7, 120], walkerSawGame: [[70, 1.7, 120]] }, "meet_game_in_commons"],
    ["the relay refused the last move", { gameHeld: [66, 1.7, 40] }, "meet_relay_holds_game_there"],
    ["the walker drawn walking through the wall, never in its corridor", { walkerDrawn: [P2_DOOR, [60, 1.7, 120], [70, 1.7, 74], [72, 1.7, 70]] }, "meet_walker_through_its_corridor"],
    ["the walker never drawn in the Commons", { walkerDrawn: MEET_OK.walkerDrawn.slice(0, 5) }, "meet_walker_into_commons"],
    ["the walker never saw the game", { walkerSawGame: [] }, "meet_walker_sees_game"],
    ["the walker saw the game somewhere else", { walkerSawGame: [P1_SPAWN] }, "meet_walker_sees_game"],
  ]) {
    const r = judgeMeet({ ...MEET_OK, ...bad });
    const failed = r.checks.filter((c) => !c.ok).map((c) => c.id);
    assert.ok(failed.includes(id), `${what} should fail ${id}; failed: ${failed.join(", ") || "nothing"}`);
  }
  assert.equal(judgeMeet(null).pass, false, "a meeting nobody recorded fails");
});

// Increment 2 review, finding 14: inside a zone a route is a straight line, and
// nothing checked it against the zone's walls (the walker from p1 passes 1.2 m from
// the room block's corner). The door points now carry every wall a person walks
// against (everyone's, a neighbour's way home included), and on the shipped ship
// the meeting's routes from both doors, and the camera's view of the line, cross
// none of them. A wall a Dev edit moved across one is found. (It found one at
// once: the game's way in from p1 went straight through the room block, in every
// green run of the branch, since the `cam` verb stands the camera anywhere; doorRoute
// now goes round by one corner.) Seen red 2026-10-04
// with routeWalls finding nothing (the c98c5465b judge had no wall check): "the
// room block's wall moved across p1's way in was not found".
test("routes: the shipped meeting's routes and view cross no wall; a wall moved across one is found", () => {
  assert.ok(Array.isArray(DOORS.walls) && DOORS.walls.length > 50, "the door points carry the ship's walls");
  for (const door of [P1_SPAWN, P2_DOOR]) {
    const game = doorRoute(DOORS, door, MEET_CAM, 40);
    assert.equal(routeWalls([door, ...game.points], DOORS.walls), null, `the game's way in from ${door} crosses a wall`);
    const walker = doorRoute(DOORS, door, MEET_LINE.start);
    assert.equal(routeWalls([door, ...walker.waypoints], DOORS.walls), null, `the walker's way out from ${door} crosses a wall`);
  }
  const mid = MEET_LINE.start.map((v, k) => (v + MEET_LINE.end[k]) / 2);
  for (const p of [MEET_LINE.start, mid, MEET_LINE.end]) assert.equal(routeWalls([MEET_CAM, p], DOORS.walls), null, `a wall between the camera and ${p}`);
  // The room block's south wall stretched west to x 65.5 by a Dev edit, across p1's way in as it
  // was planned before the edit: found.
  const moved = [...DOORS.walls, [65.5, 49, 72, 49]];
  const hit = routeWalls([P1_SPAWN, ...doorRoute(DOORS, P1_SPAWN, MEET_CAM, 40).points], moved);
  assert.ok(hit, "the room block's wall moved across p1's way in was not found");
  assert.deepEqual(hit.wall, [65.5, 49, 72, 49]);
  // Planned on the edited walls, there is no way round by one corner (the block on one side,
  // the stretched wall on the other): since the review of increment 4 the planner finds one on a
  // grid that keeps clear of every wall (clearPath), round the block, where the route used to stay
  // straight into the wall (and a walk there stopped at it).
  const round = doorRoute({ ...DOORS, walls: moved }, P1_SPAWN, MEET_CAM, 40);
  assert.equal(routeWalls([P1_SPAWN, ...round.points], moved), null, `a way in round the stretched wall crosses one: ${JSON.stringify(round.waypoints)}`);
  assert.ok(round.waypoints.length > 4, `more than one corner: ${JSON.stringify(round.waypoints)}`);
  // On the shipped walls the way in goes round the room block by its west side, while the
  // straight way the rig planned before doorRoute knew the walls goes through it.
  assert.deepEqual(doorRoute(DOORS, P1_SPAWN, MEET_CAM).waypoints, [[54, 1.7, 40], [66, 1.7, 40], [66, 1.7, 64], MEET_CAM]);
  assert.ok(routeWalls([P1_SPAWN, [54, 1.7, 40], [66, 1.7, 40], MEET_CAM], DOORS.walls), "the straight way through the room block was not found");
  // Touching a wall's end, or running along one, is no crossing.
  assert.equal(routeWalls([[0, 0, 0], [10, 0, 0]], [[10, 0, 10, 5], [2, 0, 8, 0]]), null);
});

// THE REVIEW OF INCREMENT 4, R4: a walk with no clear corner. Once the rig checked that every walk
// arrives, the Respawn walk from p2 to the Commons' far corner stopped at the room block's wall:
// neither single corner was clear, so the planner went straight at it (a low frame rate had let the
// game tunnel through before). Red check run 2026-10-04: with clearPath answering null (the old
// straight fallback), FAILED "a way round the wall crosses it: [[9,0,1]]".
test("routes: with no clear corner, a grid path round the walls, aboard, clear of them", () => {
  const { clearPath } = require("../lib/copresence-judge.js");
  // A wall across the way, wide enough that neither single corner gets round it.
  const walls = [[2, 5, 10, 5]];
  const report = { places: [{ id: "zone:a", kind: "zone", min: [0, 0, 0], max: [12, 3, 12] }], doors: [], walls };
  const from = [5, 0, 9];
  const to = [9, 0, 1];
  const path = doorRoute(report, from, to).waypoints;
  assert.equal(routeWalls([from, ...path], walls), null, `a way round the wall crosses it: ${JSON.stringify(path)}`);
  assert.deepEqual(path[path.length - 1], to);
  for (let i = 1; i < path.length - 1; i++) assert.ok(walls.every((w) => wallDistXZ(path[i], w) >= 0.75 - 1e-9), `a corner of the path is too near a wall: ${path[i]}`);
  // Never out of the places: with the only way round outside the zone's box, no path.
  const boxed = { places: [{ id: "zone:a", kind: "zone", min: [0, 0, 0], max: [12, 3, 12] }], doors: [], walls: [[0, 5, 12, 5]] };
  assert.equal(clearPath([5, 0, 9], [5, 0, 1], boxed.walls, { inside: (q) => q[0] >= 0 && q[0] <= 12 && q[2] >= 0 && q[2] <= 12 }), null, "a wall from side to side: no way round aboard");
});

// The second boot against the same relay (the remembered plot): the home built
// on the held plot before joining, the welcome a Stay. Seen red 2026-10-04 with
// judgeReboot passing anything: the 1b game's case failed nothing (expected
// reboot_built_on_its_plot and reboot_welcome_confirms).
test("reboot: a home built on its remembered plot, confirmed by a Stay, passes; the default plot FAILS", () => {
  const P2 = { id: "p2", origin: [0, 0, 99], size: [55, 3, 89] };
  const ok = { heldPlot: "p2", defaultPlot: "p1", bootPlot: "p2", lastWelcome: "stay", camera: P2_DOOR, plot: P2 };
  const r = judgeReboot(ok);
  assert.ok(r.pass, explain(r));
  // The 1b game: built on the default plot, moved by the welcome.
  const old = judgeReboot({ ...ok, bootPlot: "p1", lastWelcome: "move" });
  assert.deepEqual(old.checks.filter((c) => !c.ok).map((c) => c.id), ["reboot_built_on_its_plot", "reboot_welcome_confirms"]);
  // Holding the default plot, the run cannot tell, and says so.
  assert.match(judgeReboot({ ...ok, heldPlot: "p1", bootPlot: "p1", plot: { ...P2, id: "p1", origin: [0, 0, 0] }, camera: P1_SPAWN }).checks[0].detail, /cannot tell a remembered plot from the default/);
  assert.equal(judgeReboot(null).pass, false, "a reboot nobody recorded fails");
});
// How the game came into the world (round 4 of the 1b review, finding 1): a
// returning player's game identifies on the main menu and only then is Enter
// World pressed, so the join gate runs before the world loads. A menu-entry
// run proves something only when that race really ran. Seen red 2026-10-03
// with judgeEntry passing every menu entry (`raced = true`): "the socket had
// not identified: expected false".
const { judgeEntry } = require("../lib/copresence-judge.js");
test("entry: a menu entry passes only when the socket identified before the world loaded", () => {
  const RACED = { kind: "menu", identified: true, world_loaded: false, page_after_click: "None", world_loaded_after_click: false };
  const ok = judgeEntry(RACED);
  assert.ok(ok.pass, explain(ok));
  assert.deepEqual(ok.checks.map((c) => c.id), ["entered_from_menu_after_identify"]);
  for (const [what, bad] of [
    ["the socket had not identified", { ...RACED, identified: false }],
    ["a press after the world had loaded did not race", { ...RACED, world_loaded: true }],
    ["a press that left the menu up did not enter", { ...RACED, page_after_click: "MainMenu" }],
    ["the world loaded on the press's own frame", { ...RACED, world_loaded_after_click: true }],
  ]) {
    assert.equal(judgeEntry(bad).pass, false, `${what}: expected false`);
  }
  assert.ok(judgeEntry({ kind: "autopilot" }).pass, "the autopilot path is recorded, not failed");
  assert.equal(judgeEntry(null).pass, false, "an entry nobody recorded fails");
});

// Shutting the build editor far from the build spot (round 5 of the 1b review,
// finding 1): the relay holds the game at the far end of street-1, its build
// spot is at its door, 154 m away. Shutting the editor must leave it where the
// relay holds it, and its next move must reach the others.
const { judgeEditorClose } = require("../lib/copresence-judge.js");
const EDITOR_OK = {
  buildSpot: P1_SPAWN,
  held: [70, 1.7, 194],
  camera: [70, 1.7, 194],
  nudged: [70, 1.7, 195],
  seen: [[70, 1.7, 195]],
};

test("editor: shut far from the build spot, standing where the relay holds it, with the next move relayed, passes", () => {
  const r = judgeEditorClose(EDITOR_OK);
  assert.ok(r.pass, explain(r));
  assert.deepEqual(r.checks.map((c) => c.id), ["editor_far_from_build_spot", "editor_stands_where_held", "editor_moves_reach_others"]);
});

// The c8b3a8d54 game: shutting the editor put it at its build spot, 154 m from
// where the relay holds it, and every update it sent was refused. Seen red
// 2026-10-04 with judgeEditorClose passing anything: "a game put at its build
// spot 154 m from where the relay holds it should fail" (no check failed), and
// "a build spot 74 m away proves nothing".
test("editor: a game put back at its build spot, its moves refused, FAILS", () => {
  const r = judgeEditorClose({ ...EDITOR_OK, camera: P1_SPAWN, nudged: [53.5, 1.7, 41.5], seen: [] });
  const failed = r.checks.filter((c) => !c.ok).map((c) => c.id);
  assert.deepEqual(failed, ["editor_stands_where_held", "editor_moves_reach_others"], "a game put at its build spot 154 m from where the relay holds it should fail");
  assert.match(r.checks[1].detail, /154\.\d+ m from where the relay holds it/);
});

test("editor: a build spot inside the 100 m rule's reach proves nothing, and a check without its evidence FAILS", () => {
  assert.equal(judgeEditorClose({ ...EDITOR_OK, buildSpot: [60, 1.7, 120] }).checks[0].ok, false, "a build spot 74 m away proves nothing");
  assert.equal(judgeEditorClose({ ...EDITOR_OK, held: null }).pass, false, "never seen held: unknown, not a pass");
  assert.equal(judgeEditorClose({ ...EDITOR_OK, buildSpot: null }).pass, false, "no build spot measured: unknown, not a pass");
  assert.equal(judgeEditorClose({ ...EDITOR_OK, camera: [71.5, 1.7, 194] }).checks[1].ok, false, "a camera 1.5 m off where the relay holds it");
  assert.equal(judgeEditorClose({ ...EDITOR_OK, seen: [[70, 1.7, 194]] }).checks[2].ok, false, "only an update at the nudge itself counts");
});

// ── The guest (the increment 2 review, finding 2) ────────────────────────────
//
// With both plots of the shipped ship held by two scripted players, the game
// comes in third, a guest. A run that went right: the home put away with none of
// its things on a plot, the guest in the Commons and refused the build editor,
// Respawn standing it in the Commons, stepping out bringing the home back on the
// default plot with all it holds, and a dropped connection, with the home back on
// p1 and the camera walked into it, coming back inside the grace to stand the
// guest off the plot where the relay holds it.
const { judgeGuest, onPlotGround, GUEST_NO_EDITOR_START, GUEST_ARRIVAL_START } = require("../lib/copresence-judge.js");
const PLOTS2 = [
  { id: "p1", origin: [0, 0, 0], size: [55, 3, 89] },
  { id: "p2", origin: [0, 0, 99], size: [55, 3, 89] },
];
const GUEST_ARRIVAL = [82, 1.7, 47.5];
const AWAY = [-1000, -200, 0];
const THINGS_AWAY = { respawn: GUEST_ARRIVAL, hologram: [-990, -199, 10], showroom: [-980, -199, 20], animals: [[-970, -200, 30]], plants: [[-960, -199, 40]], structures: [], vehicles: [[-950, -200, 50]] };
// The hologram hangs half a metre outside the home's west wall, as the game reports it.
const THINGS_BACK = { respawn: P1_SPAWN, hologram: [-0.5, 1, 2.5], showroom: [20, 1, 20], animals: [[30, 0, 30]], plants: [[40, 1, 40]], structures: [], vehicles: [[50, 0, 50]] };
const GUEST_OK = {
  plots: PLOTS2,
  doors: DOORS.doors,
  commons: COMMONS_BOX,
  defaultPlot: "p1",
  walkers: [{ name: "TestBotPlots", id: 1, plot: "p1" }, { name: "TestBotPlotsTwo", id: 2, plot: "p2" }],
  arrived: { lastWelcome: "guest", homePlot: null, homeAway: true, camera: GUEST_ARRIVAL, homeThings: THINGS_AWAY, notices: [`${GUEST_ARRIVAL_START}, so you are a guest on its ship.`] },
  editor: { open: false, notices: [`${GUEST_NO_EDITOR_START}, so your home is not aboard to build on.`] },
  respawn: { far: [74, 1.7, 194], relaySpawn: GUEST_ARRIVAL, camera: GUEST_ARRIVAL, nudged: [82, 1.7, 48.5], seen: [[82, 1.7, 48.5]] },
  back: { homeAway: false, homePlot: { id: "p1" }, homeThings: THINGS_BACK },
  again: { lastWelcome: "guest", homeAway: true, camera: GUEST_ARRIVAL },
  reconnect: {
    held: [83, 1.7, 47.5],
    during: { joined: false, homeAway: false, homePlot: "p1" },
    before: P1_SPAWN,
    after: { lastWelcome: "guest", rejoin: true, homeAway: true, camera: [83, 1.7, 47.5] },
    nudged: [84, 1.7, 47.5],
    seen: [[84, 1.7, 47.5]],
  },
};
void AWAY;

test("guest: a run that went right passes, every check its own", () => {
  const r = judgeGuest(GUEST_OK);
  assert.ok(r.pass, explain(r));
  assert.deepEqual(r.checks.map((c) => c.id), [
    "guest_plots_taken",
    "guest_welcome",
    "guest_told",
    "guest_in_commons",
    "guest_respawn_point",
    "guest_nothing_on_plots",
    "guest_no_editor",
    "guest_respawn_far_from_spawn",
    "guest_respawn_stands_where_held",
    "guest_respawn_moves_reach_others",
    "guest_respawn_in_commons",
    "guest_home_back",
    "guest_away_again",
    "guest_reconnect_setup",
    "guest_reconnect_off_plot",
    "guest_reconnect_moves_reach_others",
  ]);
});

// Plot ground: a plot's box, or a plot's door corridor (from the door points);
// the Commons and a ship corridor are not. Seen red 2026-10-04 with onPlotGround
// reading the boxes only: "p1's door corridor is plot ground: false".
test("guest: plot ground is a plot's box or its door corridor", () => {
  assert.equal(onPlotGround(P1_SPAWN, PLOTS2, DOORS.doors), true);
  assert.equal(onPlotGround([60, 1.7, 40], PLOTS2, DOORS.doors), true, "p1's door corridor is plot ground: false");
  assert.equal(onPlotGround(GUEST_ARRIVAL, PLOTS2, DOORS.doors), false, "the Commons");
  assert.equal(onPlotGround([70, 1.7, 80], PLOTS2, DOORS.doors), false, "the Commons-to-street corridor");
  assert.equal(onPlotGround(null, PLOTS2, DOORS.doors), false);
});

// What must fail, one broken thing at a time, each its own check. Seen red
// 2026-10-04 with judgeGuest passing everything: "the camera left in p1 after
// the reconnect (finding 1) should fail guest_reconnect_off_plot; failed:
// nothing".
test("guest: each broken guest run FAILS its own check", () => {
  const w = (o) => ({ ...GUEST_OK, ...o });
  for (const [what, bad, id] of [
    ["the camera left in p1 after the reconnect (finding 1)", w({ reconnect: { ...GUEST_OK.reconnect, after: { ...GUEST_OK.reconnect.after, camera: P1_SPAWN } } }), "guest_reconnect_off_plot"],
    ["the camera left in p1's corridor after the reconnect", w({ reconnect: { ...GUEST_OK.reconnect, after: { ...GUEST_OK.reconnect.after, camera: [60, 1.7, 40] } } }), "guest_reconnect_off_plot"],
    ["the reconnect's home left on p1", w({ reconnect: { ...GUEST_OK.reconnect, after: { ...GUEST_OK.reconnect.after, homeAway: false } } }), "guest_reconnect_off_plot"],
    // guest_reconnect_off_plot asks two things, and each case below breaks ONE of them (the
    // two above break both, so deleting either half of the check left this test green). Seen
    // red 2026-10-04 with `!onPlot(af.camera)` deleted from the check: "the relay holding the
    // guest ON p1, the camera standing there should fail guest_reconnect_off_plot; failed:
    // nothing"; with `gap <= REJOIN_STAND_TOL_M` deleted: "the camera off every plot but 1 m
    // from where the relay holds the guest should fail guest_reconnect_off_plot; failed: nothing".
    ["the relay holding the guest ON p1, the camera standing there", w({ reconnect: { ...GUEST_OK.reconnect, held: P1_SPAWN, after: { ...GUEST_OK.reconnect.after, camera: P1_SPAWN } } }), "guest_reconnect_off_plot"],
    ["the camera off every plot but 1 m from where the relay holds the guest", w({ reconnect: { ...GUEST_OK.reconnect, after: { ...GUEST_OK.reconnect.after, camera: GUEST_ARRIVAL } } }), "guest_reconnect_off_plot"],
    ["a reconnect after the grace ran out (a fresh spawn)", w({ reconnect: { ...GUEST_OK.reconnect, after: { ...GUEST_OK.reconnect.after, rejoin: false } } }), "guest_reconnect_setup"],
    ["the home never came back when the connection dropped", w({ reconnect: { ...GUEST_OK.reconnect, during: { ...GUEST_OK.reconnect.during, homeAway: true } } }), "guest_reconnect_setup"],
    ["the camera never walked into the home", w({ reconnect: { ...GUEST_OK.reconnect, before: GUEST_ARRIVAL } }), "guest_reconnect_setup"],
    ["the reconnected guest's move refused", w({ reconnect: { ...GUEST_OK.reconnect, seen: [] } }), "guest_reconnect_moves_reach_others"],
    ["a plot left free (the game held p2)", w({ walkers: [GUEST_OK.walkers[0]] }), "guest_plots_taken"],
    ["the welcome moved the home onto a plot", w({ arrived: { ...GUEST_OK.arrived, lastWelcome: "move", homePlot: { id: "p2" }, homeAway: false } }), "guest_welcome"],
    // Told on arriving (engine/home_plot.rs GUEST_ARRIVAL, the first-hour audit's Blocker 1).
    // Seen red 2026-10-05 before judgeGuest had the check: "a guest told nothing on arriving
    // should fail guest_told; failed: nothing".
    ["a guest told nothing on arriving", w({ arrived: { ...GUEST_OK.arrived, notices: [] } }), "guest_told"],
    ["only the build editor's refusal on screen", w({ arrived: { ...GUEST_OK.arrived, notices: [`${GUEST_NO_EDITOR_START}.`] } }), "guest_told"],
    ["the arrival's notices never recorded", w({ arrived: { ...GUEST_OK.arrived, notices: undefined } }), "guest_told"],
    ["the guest left at its old door", w({ arrived: { ...GUEST_OK.arrived, camera: P1_SPAWN } }), "guest_in_commons"],
    ["Respawn still the old door", w({ arrived: { ...GUEST_OK.arrived, homeThings: { ...THINGS_AWAY, respawn: P1_SPAWN } } }), "guest_respawn_point"],
    ["an animal left on p1", w({ arrived: { ...GUEST_OK.arrived, homeThings: { ...THINGS_AWAY, animals: [[30, 0, 30]] } } }), "guest_nothing_on_plots"],
    ["no record of the home's things", w({ arrived: { ...GUEST_OK.arrived, homeThings: null } }), "guest_nothing_on_plots"],
    ["the build editor opened for a guest", w({ editor: { open: true, notices: [] } }), "guest_no_editor"],
    ["the editor shut with nothing said", w({ editor: { open: false, notices: [] } }), "guest_no_editor"],
    ["Respawn stood the guest at its old door", w({ respawn: { ...GUEST_OK.respawn, relaySpawn: P1_SPAWN, camera: P1_SPAWN } }), "guest_respawn_in_commons"],
    ["stepping out left the home away", w({ back: { ...GUEST_OK.back, homeAway: true, homePlot: null } }), "guest_home_back"],
    ["the home came back on p2, not the default", w({ back: { ...GUEST_OK.back, homePlot: { id: "p2" } } }), "guest_home_back"],
    ["a vehicle left where the home was kept", w({ back: { ...GUEST_OK.back, homeThings: { ...THINGS_BACK, vehicles: [[-950, -200, 50]] } } }), "guest_home_back"],
    ["back in, the home not put away again", w({ again: { ...GUEST_OK.again, homeAway: false } }), "guest_away_again"],
  ]) {
    const r = judgeGuest(bad);
    const failed = r.checks.filter((c) => !c.ok).map((c) => c.id);
    assert.ok(failed.includes(id), `${what} should fail ${id}; failed: ${failed.join(", ") || "nothing"}`);
  }
  assert.equal(judgeGuest(null).pass, false, "a guest run nobody recorded fails");
});

// ── The crew (ship homes increment 3) ──────────────────────────────────────
// The relay's crew work the Commons and its mess hall; the recorder lists each crew figure the
// game drew, every frame. Made-up recordings of the shipped crew where they work, and one
// broken thing at a time.

const { judgeCrew, isFigureAmber, crewPixels, CREW_MIN_PX, CREW_WALL_CLEAR_M, wallDistXZ } = require("../lib/copresence-judge.js");

const CREW_PLOTS = [
  { id: "p1", origin: [0, 0, 0], size: [55, 3, 89] },
  { id: "p2", origin: [0, 0, 99], size: [55, 3, 89] },
];
const CREW_COMMONS = { min: [65, 0, 20], max: [99, 8, 75] };
// The walls of the Commons' room block (ship x 69..85, z 29..49), as the door points carry
// walls: [ax, az, bx, bz] across the floor.
const CREW_WALLS = [
  [69, 29, 85, 29],
  [85, 29, 85, 49],
  [69, 49, 85, 49],
  [69, 29, 69, 49],
];
// The shipped crew at their first sites (data/npc/chores.ron), standing 1 m up.
const CREW_AT = [
  [1, "Helm Officer Vex", [90, 1, 33]],
  [2, "Dr. Kel", [90, 1, 43]],
  [3, "Chief Tan", [90, 1, 61]],
  [4, "CB-7", [68, 1, 24]],
  [5, "Botanist Yara", [88, 1, 45]],
  [6, "Crewmate Nia", [92, 1, 56]],
];
const LOOK_CAM = [91, 1.7, 58, 0, 0]; // the rig's crew look: up the east aisle, north
function crewFrames(n, at = CREW_AT) {
  const out = [];
  for (let k = 0; k < n; k++) out.push({ t: k / 30, crew: at.map(([id, name, p]) => ({ id, name, pos: [p[0], p[1], p[2] - k * 0.01] })) });
  return out;
}
const CREW_OK = {
  recordings: [
    { name: "samples.json", frames: crewFrames(60) },
    { name: "meet_samples.json", frames: crewFrames(60) },
    { name: "crew_samples.json", frames: crewFrames(30) },
    { name: "rejoin_crew_samples.json", frames: crewFrames(20), after: "the step back in" },
    { name: "respawn_crew_samples.json", frames: crewFrames(20), after: "Respawn" },
  ],
  plots: CREW_PLOTS,
  commons: CREW_COMMONS,
  walls: CREW_WALLS,
  expectCrew: 6,
  look: {
    cam: LOOK_CAM,
    frames: crewFrames(30),
    seen: [
      { name: "Dr. Kel", found: true, pos_px: [1280, 600], amber: 900 },
      { name: "Chief Tan", found: false, pos_px: null, amber: null },
    ],
  },
};

test("crew: a run that went right passes, every check its own", () => {
  const r = judgeCrew(CREW_OK);
  assert.ok(r.pass, explain(r));
  assert.deepEqual(r.checks.map((c) => c.id), ["crew_recorded", "crew_drawn", "crew_never_on_a_plot", "crew_clear_of_walls", "crew_back_after_rejoin", "crew_in_the_commons", "crew_seen"]);
});

// One broken thing at a time, each its own check and no other. Seen red 2026-10-04 with
// judgeCrew passing everything: "the Pioneer's crew drawn inside the home on p1 should fail
// crew_never_on_a_plot (only); failed: nothing".
test("crew: each broken crew run FAILS its own check and no other", () => {
  const w = (o) => ({ ...CREW_OK, ...o });
  const recs = (frames) => [{ ...CREW_OK.recordings[0], frames }, ...CREW_OK.recordings.slice(1)];
  // The Pioneer's crew, as the previous relay put them: the bridge and the engine room, which
  // lie inside the home on p1 (one row of each frame of the walk at home).
  const pioneer = CREW_AT.map(([id, name, p], i) => [id, name, i < 2 ? [3, 5, 2.5] : p]);
  for (const [what, bad, id] of [
    ["the Pioneer's crew drawn inside the home on p1", w({ recordings: recs(crewFrames(60, pioneer)) }), "crew_never_on_a_plot"],
    ["one crew figure drawn on p2 for one frame", w({ recordings: recs([...crewFrames(59), { t: 2, crew: [{ id: 1, name: "Helm Officer Vex", pos: [20, 1, 120] }] }]) }), "crew_never_on_a_plot"],
    ["a recorder from before increment 3 (no crew list)", w({ recordings: [{ name: "samples.json", frames: crewFrames(10).map(({ t }) => ({ t })) }, ...CREW_OK.recordings.slice(1)] }), "crew_recorded"],
    // Never drawn at all is never drawn after the step back in either: the two checks go together.
    ["one crew member never drawn", w({ expectCrew: 7 }), ["crew_drawn", "crew_back_after_rejoin"]],
    ["a crew figure on First Street during the look", w({ look: { ...CREW_OK.look, frames: crewFrames(30, CREW_AT.map(([id, n, p], i) => [id, n, i === 3 ? [70, 1, 120] : p])) } }), "crew_in_the_commons"],
    ["no nameplate on screen during the look", w({ look: { ...CREW_OK.look, seen: CREW_OK.look.seen.map((s) => ({ ...s, found: false })) } }), "crew_seen"],
    ["the named crew member's figure not in the picture", w({ look: { ...CREW_OK.look, seen: [{ ...CREW_OK.look.seen[0], amber: CREW_MIN_PX - 1 }] } }), "crew_seen"],
    ["the camera facing away from the crew", w({ look: { ...CREW_OK.look, cam: [91, 1.7, 58, Math.PI, 0] } }), "crew_seen"],
    // The review of increment 3, findings 13 to 16: guards no case above could fail.
    // A crew member named and in front, with a crew figure under the name, but drawn NORTH of the
    // Commons (outside its box): not seen in the Commons.
    ["the named crew member drawn outside the Commons, in front of the camera", w({ look: { ...CREW_OK.look, seen: [{ ...CREW_OK.look.seen[0], pos: [91, 1, 15] }] } }), "crew_seen"],
    // The ship's plots unknown: nothing can be judged against them.
    ["the ship's plots unknown", w({ plots: [] }), "crew_never_on_a_plot"],
    // One crew figure 1 m outside the Commons (x 100, its box ends at 99) for ONE frame in the
    // middle of the look: judged over every frame, with a centimetre of slack, not the last frame.
    [
      "a crew figure 1 m outside the Commons for one middle frame of the look",
      w({ look: { ...CREW_OK.look, frames: crewFrames(30).map((fr, k) => (k === 15 ? { ...fr, crew: fr.crew.map((c, i) => (i === 2 ? { ...c, pos: [100, 1, 50] } : c)) } : fr)) } }),
      "crew_in_the_commons",
    ],
    // As few crew-amber pixels under the name as the red run's whole picture held off the crew
    // (18 stray pixels): not a figure. Pinned as a number, not as CREW_MIN_PX - 1, so lowering
    // the constant fails here.
    ["20 crew-amber pixels under the name (stray pixels, no figure)", w({ look: { ...CREW_OK.look, seen: [{ ...CREW_OK.look.seen[0], amber: 20 }] } }), "crew_seen"],
    // A crew figure drawn 0.1 m from the room block's east wall, in one frame of the walk at home:
    // in the Commons, on no plot, and only this check sees it (finding 15).
    [
      "a crew figure drawn in the room block's wall",
      w({ recordings: recs(crewFrames(60).map((fr, k) => (k === 30 ? { ...fr, crew: fr.crew.map((c, i) => (i === 4 ? { ...c, pos: [85.1, 1, 40] } : c)) } : fr))) }),
      "crew_clear_of_walls",
    ],
    ["the ship's walls unknown", w({ walls: null }), "crew_clear_of_walls"],
    // After the step back in, net::sync stood only five of the six crew again (finding 16).
    [
      "one crew member never drawn again after the step back in",
      w({ recordings: CREW_OK.recordings.map((r) => (r.after === "the step back in" ? { ...r, frames: crewFrames(20, CREW_AT.slice(0, 5)) } : r)) }),
      "crew_back_after_rejoin",
    ],
    ["nothing recorded after the step back in or Respawn", w({ recordings: CREW_OK.recordings.filter((r) => !r.after) }), "crew_back_after_rejoin"],
  ]) {
    const r = judgeCrew(bad);
    const failed = r.checks.filter((c) => !c.ok).map((c) => c.id);
    const ids = Array.isArray(id) ? id : [id];
    assert.deepEqual(failed, ids, `${what} should fail ${ids.join(" and ")} (only); failed: ${failed.join(", ") || "nothing"}`);
  }
  assert.equal(judgeCrew({ recordings: [], plots: CREW_PLOTS, commons: CREW_COMMONS, expectCrew: 6, look: null }).pass, false, "a run that recorded nothing fails");
});

// The review of increment 3, finding 13: guards of judgeCrew that survived every case above (a
// scratch copy stayed green with each removed). Seen red 2026-10-04 by putting each mutation back
// into a copy of the judge and running this file against it; every one now fails a case here or
// in the table above, e.g. (a) crew_seen without its in-the-Commons term: "the named crew member
// drawn outside the Commons, in front of the camera should fail crew_seen (only); failed:
// nothing"; (e) CREW_MIN_PX = 1: "20 crew-amber pixels under the name (stray pixels, no figure)
// should fail crew_seen (only); failed: nothing"; (f) no rows passing: "with no crew drawn,
// crew_never_on_a_plot passed". The two new checks went red the same way with their guards out
// ("a crew figure drawn in the room block's wall should fail crew_clear_of_walls (only); failed:
// nothing", "nothing recorded after the step back in or Respawn should fail
// crew_back_after_rejoin (only); failed: nothing").
test("crew: no crew drawn at all still fails crew_never_on_a_plot on its own", () => {
  const empty = CREW_OK.recordings.map((r) => ({ ...r, frames: r.frames.map((fr) => ({ ...fr, crew: [] })) }));
  const r = judgeCrew({ ...CREW_OK, recordings: empty });
  const failed = r.checks.filter((c) => !c.ok).map((c) => c.id);
  assert.ok(failed.includes("crew_never_on_a_plot"), `with no crew drawn, crew_never_on_a_plot passed: failed ${failed.join(", ")}`);
  assert.ok(failed.includes("crew_clear_of_walls"), `with no crew drawn, crew_clear_of_walls passed: failed ${failed.join(", ")}`);
});

test("crew: the picture's amber threshold is pinned (20 stray pixels fail, 150 pass)", () => {
  const at = (amber) => judgeCrew({ ...CREW_OK, look: { ...CREW_OK.look, seen: [{ ...CREW_OK.look.seen[0], amber }] } }).checks.find((c) => c.id === "crew_seen").ok;
  assert.equal(at(20), false, "20 crew-amber pixels counted as a crew figure seen");
  assert.equal(at(150), true, "150 crew-amber pixels (a figure 35 m away is about 400) not counted as a crew figure seen");
  assert.equal(CREW_MIN_PX, 150);
});

test("crew: a wall's distance is across the floor, to the nearest point of the segment", () => {
  assert.equal(CREW_WALL_CLEAR_M, 0.3);
  assert.ok(Math.abs(wallDistXZ([85.1, 1, 40], [85, 29, 85, 49]) - 0.1) < 1e-9);
  assert.ok(Math.abs(wallDistXZ([85, 1, 52], [85, 29, 85, 49]) - 3) < 1e-9, "past the wall's end, the distance is to its end");
  assert.ok(Math.abs(wallDistXZ([3, 7, 4], [0, 0, 0, 0]) - 5) < 1e-9, "a wall of no length is a point");
});

// The crew's amber body AS THE GAME DRAWS IT in the Commons counts; the HUD's orange activity
// line, the players' teal, oak partitions, steel and the dark do not. The colours are measured
// off the first crew.png (2026-10-04, run 20261004-155649-plots-game-first).
// Seen red 2026-10-04: with the first test, written from the material colour (green 0.45 to
// 0.85 of red, blue under 0.38), "the crew's body as the game draws it in the Commons is not
// counted" (it had passed that picture's run by counting the orange activity text instead);
// with the green floor at 0.72 of red (the first calibration), "oak counted as a crew member's
// body".
test("crew: amber is the crew's body as drawn, not their activity text, oak, teal or steel", () => {
  const px = (c) => c.map((v) => Math.round(v * 255));
  assert.ok(isFigureAmber(176, 151, 71), "the crew's body as the game draws it in the Commons is not counted");
  assert.ok(isFigureAmber(168, 142, 64), "the crew's body in its own shade is not counted");
  assert.ok(isFigureAmber(120, 100, 46), "the crew's body in deep shade is not counted");
  assert.ok(!isFigureAmber(223, 131, 33), "the orange activity line under a crew member's name counted as their body");
  assert.ok(!isFigureAmber(68, 57, 40), "oak as the Commons draws it counted as a crew member's body");
  assert.ok(!isFigureAmber(...px([0.55, 0.40, 0.24])), "oak counted as a crew member's body");
  assert.ok(!isFigureAmber(...px([0.8, 0.67, 0.45])), "pine counted as a crew member's body");
  assert.ok(!isFigureAmber(...px([0.2, 0.75, 0.75])), "teal counted as a crew member's body");
  assert.ok(!isFigureAmber(...px([0.55, 0.57, 0.62])), "steel counted as a crew member's body");
  assert.ok(!isFigureAmber(20, 12, 4), "the dark counted as a crew member's body");
  // Counted in the box under the name only.
  const img = { width: 400, height: 600, rgba: new Uint8Array(400 * 600 * 4) };
  for (let y = 100; y < 300; y++) for (let x = 180; x < 220; x++) img.rgba.set([176, 151, 71, 255], (y * 400 + x) * 4);
  assert.equal(crewPixels(img, [200, 90]).count, 200 * 40);
  assert.equal(crewPixels(img, [380, 450]).count, 0, "nothing under a name drawn elsewhere");
});

// ── Increment 4: the relay's speed check ────────────────────────────────────────────────────

const { judgeJump, judgeHonestMoves, bankedAllowanceM } = require("../lib/copresence-judge.js");

// The relay's allowance, read from its own rules file: 25 m/s x 1.25 x 1.5 s + 1 m.
test("jump: the allowance is read from the relay's rules file", () => {
  const ron = require("fs").readFileSync(require("path").join(__dirname, "..", "..", "data", "ship", "shared_world.ron"), "utf8");
  assert.ok(Math.abs(bankedAllowanceM(ron) - (25 * 1.25 * 1.5 + 1)) < 1e-9, `the shipped rules allow ${bankedAllowanceM(ron)} m`);
  assert.equal(bankedAllowanceM("( moving: ( on_foot_mps: 25.0 ) )"), null, "a number missing reads as unknown");
});

// What a good run of the jump leg records: the game stood at the crew look, jumped 138 m to the
// far end of First Street, was corrected back, and its nudge reached the walker.
const goodJump = () => ({
  from: [91, 1.7, 58],
  held: [91, 1.7, 58],
  target: [66, 1.7, 194],
  allowance_m: 47.875,
  before: { count: 0, applied: 0, last: null, walking: false },
  after: { count: 1, applied: 1, last: { seq: 1, at: [91, 1.7, 58], from: [66, 1.7, 194], reason: "too_fast" }, walking: false },
  camera: [91, 1.7, 58],
  relayedAfterJump: [[91, 1.7, 58]],
  nudged: [90, 1.7, 58],
  seen: [[90.4, 1.7, 58], [90, 1.7, 58]],
  notices: ["The server put you back where it last saw you: that move was faster than anyone can go aboard."],
});
const jumpCheck = (j, id) => judgeJump(j).checks.find((c) => c.id === id);

test("jump: a good run passes every check", () => {
  const r = judgeJump(goodJump());
  assert.ok(r.pass, r.checks.filter((c) => !c.ok).map((c) => `${c.id}: ${c.detail}`).join("\n"));
  assert.deepEqual(r.checks.map((c) => c.id), ["jump_far_enough", "jump_corrected", "jump_corrected_once", "jump_said_once", "jump_stands_where_held", "jump_never_relayed", "jump_moves_reach_others"]);
});

// Each way a broken build shows, failing its own check and no other where the record allows.
test("jump: the old relay's silent refusal FAILS: no correction, and frozen", () => {
  const j = goodJump();
  // The 100 m rule refused the 138 m jump without a word: no correction, the game left standing
  // at the jump's target, and every update after it refused.
  j.after = { count: 0, applied: 0, last: null, walking: false };
  j.camera = j.target.slice();
  j.seen = [];
  const r = judgeJump(j);
  assert.equal(jumpCheck(j, "jump_corrected").ok, false, jumpCheck(j, "jump_corrected").detail);
  assert.equal(jumpCheck(j, "jump_stands_where_held").ok, false);
  assert.equal(jumpCheck(j, "jump_moves_reach_others").ok, false);
  assert.equal(jumpCheck(j, "jump_never_relayed").ok, true, "nothing leaked: the relay refused it");
  assert.equal(r.pass, false);
});

test("jump: a game that ignores the correction FAILS where it stands and frozen", () => {
  const j = goodJump();
  j.after = { count: 0, applied: 0, last: null, walking: false };
  j.camera = [66, 1.7, 194];
  j.seen = [];
  assert.equal(jumpCheck(j, "jump_corrected").ok, false);
  assert.equal(jumpCheck(j, "jump_stands_where_held").ok, false);
  assert.equal(jumpCheck(j, "jump_moves_reach_others").ok, false);
});

test("jump: a relay that passes the jump on FAILS never_relayed", () => {
  const j = goodJump();
  j.relayedAfterJump = [[91, 1.7, 58], [66.2, 1.7, 193.8]];
  assert.equal(jumpCheck(j, "jump_never_relayed").ok, false, jumpCheck(j, "jump_never_relayed").detail);
  assert.equal(judgeJump(j).checks.filter((c) => !c.ok).length, 1, "and nothing else");
});

test("jump: a correction to the wrong place, or for another reason, FAILS corrected", () => {
  const wrong = goodJump();
  wrong.after.last.at = [70, 1.7, 100];
  assert.equal(jumpCheck(wrong, "jump_corrected").ok, false);
  const reason = goodJump();
  reason.after.last.reason = "link_unknown";
  assert.equal(jumpCheck(reason, "jump_corrected").ok, false);
});

test("jump: a jump the relay would have taken anyway means nothing: FAILS far_enough", () => {
  const j = goodJump();
  j.after.last.from = [91, 1.7, 98]; // the game's own record: 40 m, inside the 47.9 m a player can bank
  assert.equal(jumpCheck(j, "jump_far_enough").ok, false, jumpCheck(j, "jump_far_enough").detail);
  const unknown = goodJump();
  unknown.allowance_m = null;
  assert.equal(jumpCheck(unknown, "jump_far_enough").ok, false, "an allowance not recorded fails");
});

test("jump: nothing recorded fails", () => {
  const r = judgeJump({});
  assert.equal(r.pass, false);
  assert.ok(r.checks.every((c) => c.ok === false), r.checks.map((c) => `${c.id} ${c.ok}`).join(", "));
});

test("honest moves: only the jump's correction, and no walker's, passes", () => {
  assert.ok(judgeHonestMoves({ gameTotal: 1, fromJump: 1, relaySent: 1, walkerLines: [] }).pass);
  const extra = judgeHonestMoves({ gameTotal: 2, fromJump: 1, relaySent: 2, walkerLines: [] });
  assert.equal(extra.pass, false, "a correction of an honest move of the game");
  assert.match(extra.checks[0].detail, /1 time\(s\) for moves that were not the jump/);
  const walker = judgeHonestMoves({ gameTotal: 1, fromJump: 1, relaySent: 1, walkerLines: ["[TestBotPlots] second-player: corrected to (70.00, 1.70, 60.00) (too_fast, correction 1)"] });
  assert.equal(walker.checks.find((c) => c.id === "moves_walkers_never_corrected").ok, false, "a walker corrected");
  assert.equal(judgeHonestMoves({}).pass, false, "nothing recorded fails");
  assert.ok(judgeHonestMoves({ gameTotal: 0, fromJump: 0, relaySent: 0, walkerLines: [] }).pass, "the guest order: no jump, no correction");
});

// THE REVIEW OF INCREMENT 4, R7: the jump and the run's honest moves are judged on what the game
// itself recorded and on what the relay sent, not on where the rig aimed or on the game's word
// alone. Red checks run 2026-10-04 on the judge, each restored afterwards:
//  - the jump's correction taken whatever the count did: FAILED "a correction left over from
//    before the jump" (the check passed on the stale one, "correction 1 (too_fast) stood the game at
//    (91.00, 1.70, 58.00)");
//  - corrected_once always true: FAILED "two corrections for one jump", "the jump drew 2
//    correction(s)";
//  - said_once always true: FAILED "no sentence on screen, or two", "0 correction notice(s) on
//    screen after it (exactly one sentence)";
//  - far_enough and the leak measured from the rig's target: FAILED far_enough's test and
//    "measured where the game says it landed" ("... 138.3 m across the floor ...");
//  - the relay's count never compared: FAILED "the relay sent the game 2 correction(s) but the
//    game applied 1: a correction was dropped";
//  - a resend counted as a new correction: FAILED "three new corrections; the one sent again is
//    not one".
test("jump: a correction left over from before the jump FAILS corrected", () => {
  const j = goodJump();
  j.before = { count: 1, applied: 1, last: j.after.last, walking: false };
  j.after = { count: 1, applied: 1, last: { seq: 1, at: [91, 1.7, 58], from: [66, 1.7, 194], reason: "too_fast" }, walking: false };
  assert.equal(jumpCheck(j, "jump_corrected").ok, false, jumpCheck(j, "jump_corrected").detail);
  assert.equal(jumpCheck(j, "jump_corrected_once").ok, false, "and no correction this time");
});

test("jump: two corrections for one jump FAIL corrected_once", () => {
  const j = goodJump();
  j.after.count = 2;
  j.after.last.seq = 2;
  assert.equal(jumpCheck(j, "jump_corrected_once").ok, false, jumpCheck(j, "jump_corrected_once").detail);
  assert.equal(jumpCheck(j, "jump_corrected").ok, true);
});

test("jump: no sentence on screen, or two, FAIL said_once", () => {
  const none = goodJump();
  none.notices = [];
  assert.equal(jumpCheck(none, "jump_said_once").ok, false, jumpCheck(none, "jump_said_once").detail);
  const two = goodJump();
  two.notices = [two.notices[0], two.notices[0]];
  assert.equal(jumpCheck(two, "jump_said_once").ok, false);
  const other = goodJump();
  other.notices = ["Saved your home.", other.notices[0]];
  assert.equal(jumpCheck(other, "jump_said_once").ok, true, "other notices do not count");
  const unknown = goodJump();
  delete unknown.notices;
  assert.equal(jumpCheck(unknown, "jump_said_once").ok, false, "not recorded fails");
});

test("jump: measured where the game says it landed, not where the rig aimed", () => {
  // The rig aimed at the far end of First Street, but the game only got 30 m (its own record).
  const short = goodJump();
  short.after.last.from = [91, 1.7, 88];
  assert.equal(jumpCheck(short, "jump_far_enough").ok, false, jumpCheck(short, "jump_far_enough").detail);
  // The jump reached the walker where the game really stood, far from where the rig aimed.
  const leak = goodJump();
  leak.target = [10, 1.7, 10];
  leak.relayedAfterJump = [[66.1, 1.7, 193.9]];
  assert.equal(jumpCheck(leak, "jump_never_relayed").ok, false, jumpCheck(leak, "jump_never_relayed").detail);
});

test("honest moves: a correction the relay sent and the game dropped FAILS", () => {
  const dropped = judgeHonestMoves({ gameTotal: 1, fromJump: 1, relaySent: 2, walkerLines: [] });
  const c = dropped.checks.find((x) => x.id === "moves_relay_sent_what_the_game_took");
  assert.equal(c.ok, false, c.detail);
  assert.match(c.detail, /dropped/);
  const unknown = judgeHonestMoves({ gameTotal: 1, fromJump: 1, walkerLines: [] });
  assert.equal(unknown.checks.find((x) => x.id === "moves_relay_sent_what_the_game_took").ok, false, "not recorded fails");
});

test("the relay's corrections are counted from its log, new ones only, walkers apart", () => {
  const { relayCorrections } = require("../lib/copresence-judge.js");
  const log = [
    "\u001b[2m2026-10-04T23:33:09Z\u001b[0m WARN msg_handlers: Game: corrected ce376ca5469f7fa6.. (correction 1, too_fast): [91.0,1.7,58.0]",
    "WARN msg_handlers: Game: corrected ce376ca5469f7fa6.. (correction 1 sent again, too_fast): [91.0,1.7,58.0]",
    "WARN msg_handlers: Game: corrected b56a11f1cc7ee551.. (correction 1, too_fast): [70.0,1.7,60.0]",
    "WARN msg_handlers: Game: corrected ce376ca5469f7fa6.. (correction 2, editor_off_plot): [91.0,1.7,58.0]",
    "INFO something else",
  ].join("\n");
  assert.equal(relayCorrections(log).total, 3, "three new corrections; the one sent again is not one");
  const game = relayCorrections(log, ["b56a11f1cc7ee551aaaa"]);
  assert.equal(game.total, 2, "the walker's left out by its key");
  assert.deepEqual(game.byKey, { ce376ca5469f7fa6: 2 });
});

// ── The review of increment 4: R1 and R4 ──────────────────────────────────────────────────────
//
// R1: the rig now walks the game onto its home's own teleporter and shuts the build editor away
// from the build spot, each a jump only its declaration explains; R4: every walk of the rig must
// arrive, and a turn in place is never sent from anywhere else. Red checks run 2026-10-04 on the
// judge, each restored afterwards:
//  - a walk that never arrived passing: FAILED "1 of 3 walks never arrived; the first, crew, to
//    (91.00, 1.70, 58.00), stopped at (81.30, 1.70, 48.90)";
//  - a turn from anywhere passing: FAILED "4.5 m off is a teleport, not a turn";
//  - the editor jump's correction count, or the close's place, never compared: FAILED "the relay
//    corrected the game 1 time(s) between the walk and the shut", and at_build_spot "true == false";
//  - the teleporter's arrival, its correction count, or what the walker saw never compared: FAILED
//    each ("the relay corrected the game 1 time(s) from the step onto the pad ...");
//  - the editor target ignoring the allowance: FAILED "no point past a 500 m allowance aboard:
//    says why".

const { turnInPlace, judgeWalks, padApproach, editorJumpTarget, judgeEditorJump, judgeTeleporter } = require("../lib/copresence-judge.js");

test("walks: a turn in place is only one where the camera already stands", () => {
  assert.ok(turnInPlace([76, 1.7, 64.05], "76,1.7,64,3.14159,-0.05").ok, "5 cm off: a turn");
  const far = turnInPlace([74, 1.7, 60], "76,1.7,64,3.14159,-0.05");
  assert.equal(far.ok, false, "4.5 m off is a teleport, not a turn");
  assert.ok(Math.abs(far.off - Math.hypot(2, 4)) < 1e-9);
  assert.equal(turnInPlace(null, "76,1.7,64,0,0").ok, false, "no camera: no turn");
});

test("walks: every walk arrived and every turn was in place, or FAIL", () => {
  const walks = [{ label: "meet", to: [66, 1.7, 40], at: [66, 1.7, 40], ok: true }, { label: "crew", to: [91, 1.7, 58], at: [91, 1.7, 58], ok: true }];
  const turns = [{ pose: "76,1.7,64,3.14,-0.05", at: [76, 1.7, 64], off: 0, ok: true }];
  assert.ok(judgeWalks(walks, turns).pass);
  const short = judgeWalks([...walks, { label: "crew", to: [91, 1.7, 58], at: [81.3, 1.7, 48.9], ok: false }], turns);
  assert.equal(short.checks.find((c) => c.id === "walks_all_arrived").ok, false, short.checks[0].detail);
  assert.match(short.checks[0].detail, /stopped at \(81\.30, 1\.70, 48\.90\)/);
  const refused = judgeWalks(walks, [...turns, { pose: "91,1.7,58,0,-0.05", at: [81.3, 1.7, 48.9], off: 13.7, ok: false }]);
  assert.equal(refused.checks.find((c) => c.id === "walks_turns_in_place").ok, false);
  assert.equal(judgeWalks(null, null).pass, false, "not recorded fails");
  assert.equal(judgeWalks([], []).checks[0].ok, false, "no walk recorded fails");
});

// BUG-165: the game walks a route the way a person does, facing each leg, and turns to the facing
// the rig asked for only at the route's end. FAILED on main (every point of a route was asked the
// end's facing, so at every door the camera turned to it and back again).
const { walkYaw, routeFacings } = require("../lib/copresence-judge.js");

test("walks: a route faces along its legs and turns to the asked facing only at its end", () => {
  assert.deepEqual(routeFacings(3, 3.14159, -0.05), [{ yaw: null, pitch: 0 }, { yaw: null, pitch: 0 }, { yaw: 3.14159, pitch: -0.05 }], "the doors on the way are walked to facing the leg, level");
  assert.deepEqual(routeFacings(1, 0.5, 0.1), [{ yaw: 0.5, pitch: 0.1 }], "a one-point route is its end");
  assert.deepEqual(routeFacings(0, 0.5, 0.1), []);
});

test("walks: the way a leg looks is the camera's yaw along it", () => {
  // The camera looks along (sin yaw, 0, -cos yaw) (renderer/camera.rs forward).
  assert.equal(walkYaw([0, 1.7, 0], [0, 1.7, -5]), 0, "north (-z) is yaw 0");
  assert.ok(Math.abs(walkYaw([0, 1.7, 0], [5, 1.7, 0]) - Math.PI / 2) < 1e-12, "east (+x) a quarter turn");
  assert.ok(Math.abs(Math.abs(walkYaw([0, 1.7, 0], [0, 1.7, 5])) - Math.PI) < 1e-12, "south (+z) a half turn");
  assert.ok(Math.abs(walkYaw([1, 0, 1], [-4, 9, 1]) + Math.PI / 2) < 1e-12, "west, whatever the height");
  assert.equal(walkYaw([3, 1.7, 3], [3.01, 1.7, 3.02], 0.7), 0.7, "a leg of 2 cm has no way to look: as the camera was");
});

const shipReport = () => ({
  places: [
    { id: "zone:commons", kind: "zone", purpose: "commons", min: [65, 0, 20], max: [99, 8, 75], door: null, own: false },
    { id: "zone:street-1", kind: "zone", purpose: "street", min: [65, 0, 85], max: [75, 4, 195], door: null, own: false },
    { id: "plot:p1", kind: "plot", purpose: "home", min: [0, 0, 0], max: [55, 3, 89], door: [53.5, 1.7, 40.5], own: true },
  ],
  doors: [
    { from: "zone:commons", to: "zone:street-1", axis: "z", lat: 70, mouths: [[70, 0, 75], [70, 0, 85]], steps: [[70, 1.7, 74], [70, 1.7, 86]], tube: [[69, 0, 75], [71, 3, 85]] },
    { from: "plot:p1", to: "zone:commons", axis: "x", lat: 40, mouths: [[55, 0, 40], [65, 0, 40]], steps: [[54, 1.7, 40], [66, 1.7, 40]], tube: [[55, 0, 39], [65, 3, 41]] },
  ],
  walls: [[65, 20, 99, 20], [99, 20, 99, 75], [65, 75, 69, 75], [71, 75, 99, 75], [65, 20, 65, 39], [65, 41, 65, 75], [65, 85, 65, 195], [75, 85, 75, 195], [65, 195, 75, 195], [65, 85, 69, 85], [71, 85, 75, 85]],
});

test("editor jump: the walk goes past one update's allowance, inside the 90 m, through the doors", () => {
  const r = shipReport();
  const t = editorJumpTarget(r, [53.5, 1.7, 40.5], 47.875);
  assert.ok(!t.error, t.error);
  assert.ok(t.dist > 47.875 + 5 && t.dist < 80, `${t.dist.toFixed(1)} m from the build spot`);
  assert.ok(Math.abs(t.dist - 60) < 2, "about 60 m");
  assert.ok(r.walls.every((w) => wallDistXZ(t.at, w) >= 1), "clear of every wall");
  assert.deepEqual(t.route[t.route.length - 1], t.at, "the route ends there");
  assert.ok(editorJumpTarget(r, [53.5, 1.7, 40.5], 500).error, "no point past a 500 m allowance aboard: says why");
});

test("teleporter: the approach is beside the pad, out of its footprint, on its clearest side", () => {
  const a = padApproach([22.5, 0, 20], [[0, 22, 9, 22], [10.6, 22, 25, 22], [20, 0, 20, 10], [20, 11.6, 20, 22]]);
  assert.deepEqual(a.p, [22.5, 1.7, 18.5], "south of the pad, away from the wall 2 m north");
  assert.ok(a.clear >= 1.5);
});

const goodEditorJump = () => ({
  buildSpot: [53.5, 1.7, 40.5],
  walkedTo: [72.5, 1.7, 97.5],
  toggled: true,
  camera: [53.5, 1.7, 40.5],
  seen: [[53.5, 1.7, 40.5]],
  before: { count: 1, applied: 1 },
  after: { count: 1, applied: 1 },
  allowance_m: 47.875,
});
const ejCheck = (e, id) => judgeEditorJump(e).checks.find((c) => c.id === id);

test("editor jump: a good run passes every check", () => {
  const r = judgeEditorJump(goodEditorJump());
  assert.ok(r.pass, r.checks.filter((c) => !c.ok).map((c) => `${c.id}: ${c.detail}`).join("\n"));
  assert.deepEqual(r.checks.map((c) => c.id), ["editorjump_far_enough", "editorjump_toggled", "editorjump_at_build_spot", "editorjump_relayed", "editorjump_never_corrected"]);
});

test("editor jump: a relay that corrects the declared close FAILS, and so does every way it shows", () => {
  // The declaration lost: the relay corrects the 60 m close; the game stands back where it walked.
  const lost = goodEditorJump();
  lost.after = { count: 2, applied: 2 };
  lost.camera = lost.walkedTo.slice();
  lost.seen = [];
  assert.equal(ejCheck(lost, "editorjump_never_corrected").ok, false, ejCheck(lost, "editorjump_never_corrected").detail);
  assert.equal(ejCheck(lost, "editorjump_at_build_spot").ok, false);
  assert.equal(ejCheck(lost, "editorjump_relayed").ok, false);
  const near = goodEditorJump();
  near.walkedTo = [70, 1.7, 60];
  assert.equal(ejCheck(near, "editorjump_far_enough").ok, false, "a close the allowance would have taken means nothing");
  const shut = goodEditorJump();
  shut.toggled = false;
  assert.equal(ejCheck(shut, "editorjump_toggled").ok, false);
  assert.equal(judgeEditorJump({}).pass, false, "nothing recorded fails");
});

const goodTele = () => ({
  link: { zone: "home", from: "teleporter-1", to: "teleporter-2", from_at: [22.5, 0, 20], to_at: [31, 0, 80], reach_m: 0.79 },
  camera: [31, 1.7, 80],
  seen: [[31, 1.7, 80]],
  before: { count: 1, applied: 1 },
  after: { count: 1, applied: 1 },
  allowance_m: 47.875,
});
const teleCheck = (t, id) => judgeTeleporter(t).checks.find((c) => c.id === id);

test("teleporter: a good run passes every check", () => {
  const r = judgeTeleporter(goodTele());
  assert.ok(r.pass, r.checks.filter((c) => !c.ok).map((c) => `${c.id}: ${c.detail}`).join("\n"));
  assert.deepEqual(r.checks.map((c) => c.id), ["tele_far_enough", "tele_arrived", "tele_relayed", "tele_never_corrected"]);
});

test("teleporter: a relay that corrects the declared jump FAILS, and so does every way it shows", () => {
  // The declaration lost: the relay corrects the 60.6 m jump and stands the game back at the west pad.
  const lost = goodTele();
  lost.after = { count: 2, applied: 2 };
  lost.camera = [22.5, 1.7, 19.8];
  lost.seen = [];
  assert.equal(teleCheck(lost, "tele_never_corrected").ok, false, teleCheck(lost, "tele_never_corrected").detail);
  assert.equal(teleCheck(lost, "tele_arrived").ok, false);
  assert.equal(teleCheck(lost, "tele_relayed").ok, false);
  const never = goodTele();
  never.camera = [22.5, 1.7, 18.5];
  assert.equal(teleCheck(never, "tele_arrived").ok, false, "a pad that never jumped");
  const near = goodTele();
  near.link = { ...near.link, to_at: [22.5, 0, 50] };
  near.camera = [22.5, 1.7, 50];
  near.seen = [[22.5, 1.7, 50]];
  assert.equal(teleCheck(near, "tele_far_enough").ok, false, "a jump the allowance would have taken means nothing");
  assert.equal(judgeTeleporter({}).pass, false, "nothing recorded fails");
});
