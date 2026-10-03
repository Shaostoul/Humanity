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
